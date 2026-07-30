use super::{build_bambu_args, InstallationDiscovery, SliceRequest};
use crate::{
    error::{AppError, Result},
    parser::parse_3mf_project,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    fs::{self, File, OpenOptions},
    io::{self, BufRead, BufReader, Read, Write},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Condvar, Mutex,
    },
    thread,
    time::{Duration, Instant, UNIX_EPOCH},
};
use tauri::{Emitter, Manager};
use uuid::Uuid;

const STDERR_LIMIT: usize = 64 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SlicePhase {
    Preparing,
    Slicing,
    Validating,
    Importing,
    Complete,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SliceTaskState {
    Running,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SliceTask {
    pub task_id: Uuid,
    pub state: SliceTaskState,
    pub phase: SlicePhase,
    pub percent: Option<f64>,
    pub project_id: Option<Uuid>,
    pub error_code: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SliceProgress {
    pub task_id: Uuid,
    pub phase: SlicePhase,
    pub percent: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SliceComplete {
    pub task_id: Uuid,
    pub project_id: Uuid,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SliceErrorEvent {
    pub task_id: Uuid,
    pub code: String,
}

pub(crate) trait SliceEventSink: Send + Sync {
    fn progress(&self, event: SliceProgress);
    fn complete(&self, event: SliceComplete);
    fn error(&self, event: SliceErrorEvent);
}

pub(crate) trait SliceImporter: Send + Sync {
    fn import_project(&self, path: &Path) -> Result<Uuid>;
}

struct RunningSlice {
    child: Mutex<Child>,
    cancel_requested: AtomicBool,
    finished: Mutex<bool>,
    finished_changed: Condvar,
    task_dir: PathBuf,
}

impl RunningSlice {
    fn new(child: Child, task_dir: PathBuf) -> Self {
        Self {
            child: Mutex::new(child),
            cancel_requested: AtomicBool::new(false),
            finished: Mutex::new(false),
            finished_changed: Condvar::new(),
            task_dir,
        }
    }

    fn mark_finished(&self) {
        *self
            .finished
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = true;
        self.finished_changed.notify_all();
    }
}

struct SlicerInner {
    discovery: InstallationDiscovery,
    cache_root: PathBuf,
    importer: Arc<dyn SliceImporter>,
    events: Arc<dyn SliceEventSink>,
    stability_delay: Duration,
    running: Mutex<HashMap<Uuid, Arc<RunningSlice>>>,
    tasks: Mutex<HashMap<Uuid, SliceTask>>,
}

#[derive(Clone)]
pub struct SlicerService {
    inner: Arc<SlicerInner>,
}

impl SlicerService {
    pub(crate) fn for_app(
        app: tauri::AppHandle,
        cache_root: PathBuf,
        explicit_app: Option<PathBuf>,
    ) -> Self {
        Self::with_dependencies(
            InstallationDiscovery::new(explicit_app),
            cache_root,
            Arc::new(TauriSliceImporter { app: app.clone() }),
            Arc::new(TauriSliceEventSink { app }),
            Duration::from_millis(150),
        )
    }

    pub(crate) fn with_dependencies(
        discovery: InstallationDiscovery,
        cache_root: PathBuf,
        importer: Arc<dyn SliceImporter>,
        events: Arc<dyn SliceEventSink>,
        stability_delay: Duration,
    ) -> Self {
        Self {
            inner: Arc::new(SlicerInner {
                discovery,
                cache_root,
                importer,
                events,
                stability_delay,
                running: Mutex::new(HashMap::new()),
                tasks: Mutex::new(HashMap::new()),
            }),
        }
    }

    pub fn start(&self, request: SliceRequest) -> Result<SliceTask> {
        let installation = self.inner.discovery.discover()?;
        let task_id = Uuid::new_v4();
        let task_dir = self
            .inner
            .cache_root
            .join("slices")
            .join(task_id.to_string());
        fs::create_dir_all(&task_dir).map_err(|_| AppError::SlicerFailed)?;
        let temporary_output = task_dir.join("output.gcode.3mf");
        let args = match build_bambu_args(&request, &temporary_output) {
            Ok(args) => args,
            Err(error) => {
                let _ = fs::remove_dir_all(&task_dir);
                return Err(error);
            }
        };

        let mut child = match Command::new(&installation.executable)
            .args(&args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
        {
            Ok(child) => child,
            Err(_) => {
                let _ = fs::remove_dir_all(&task_dir);
                return Err(AppError::SlicerFailed);
            }
        };
        let stdout = child.stdout.take();
        let stderr = child.stderr.take();
        let running = Arc::new(RunningSlice::new(child, task_dir));
        self.inner
            .running
            .lock()
            .map_err(|_| AppError::SlicerFailed)?
            .insert(task_id, running.clone());

        let task = SliceTask {
            task_id,
            state: SliceTaskState::Running,
            phase: SlicePhase::Preparing,
            percent: None,
            project_id: None,
            error_code: None,
        };
        self.inner
            .tasks
            .lock()
            .map_err(|_| AppError::SlicerFailed)?
            .insert(task_id, task.clone());
        emit_progress(&self.inner, task_id, SlicePhase::Preparing);
        set_progress(&self.inner, task_id, SlicePhase::Slicing);

        let stdout_reader = thread::spawn(move || {
            if let Some(stdout) = stdout {
                for line in BufReader::new(stdout).lines() {
                    if line.is_err() {
                        break;
                    }
                }
            }
        });
        let stderr_reader = thread::spawn(move || {
            stderr
                .map(|stderr| read_bounded(stderr, STDERR_LIMIT))
                .transpose()
                .unwrap_or_default()
                .unwrap_or_default()
        });
        let inner = self.inner.clone();
        thread::spawn(move || {
            run_to_completion(
                inner,
                running,
                task_id,
                request,
                temporary_output,
                stdout_reader,
                stderr_reader,
            );
        });

        Ok(task)
    }

    pub fn cancel(&self, task_id: Uuid) -> Result<()> {
        let running = self
            .inner
            .running
            .lock()
            .map_err(|_| AppError::SlicerFailed)?
            .get(&task_id)
            .cloned();
        let Some(running) = running else {
            return self
                .get(task_id)
                .filter(|task| task.state != SliceTaskState::Running)
                .map(|_| ())
                .ok_or(AppError::InvalidJob);
        };
        running.cancel_requested.store(true, Ordering::Release);
        if let Ok(mut child) = running.child.lock() {
            if child.try_wait().ok().flatten().is_none() {
                let _ = child.kill();
            }
        }

        let deadline = Instant::now() + Duration::from_secs(10);
        let mut finished = running
            .finished
            .lock()
            .map_err(|_| AppError::SlicerFailed)?;
        while !*finished {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Err(AppError::SlicerFailed);
            }
            let (next, timeout) = running
                .finished_changed
                .wait_timeout(finished, remaining)
                .map_err(|_| AppError::SlicerFailed)?;
            finished = next;
            if timeout.timed_out() && !*finished {
                return Err(AppError::SlicerFailed);
            }
        }
        Ok(())
    }

    pub fn get(&self, task_id: Uuid) -> Option<SliceTask> {
        self.inner
            .tasks
            .lock()
            .ok()
            .and_then(|tasks| tasks.get(&task_id).cloned())
    }

    pub fn shutdown(&self) {
        let task_ids = self
            .inner
            .running
            .lock()
            .map(|running| running.keys().copied().collect::<Vec<_>>())
            .unwrap_or_default();
        for task_id in task_ids {
            let _ = self.cancel(task_id);
        }
    }
}

struct TauriSliceEventSink {
    app: tauri::AppHandle,
}

impl SliceEventSink for TauriSliceEventSink {
    fn progress(&self, event: SliceProgress) {
        let _ = self.app.emit("slice-progress", event);
    }

    fn complete(&self, event: SliceComplete) {
        let _ = self.app.emit("slice-complete", event);
    }

    fn error(&self, event: SliceErrorEvent) {
        let _ = self.app.emit("slice-error", event);
    }
}

struct TauriSliceImporter {
    app: tauri::AppHandle,
}

impl SliceImporter for TauriSliceImporter {
    fn import_project(&self, path: &Path) -> Result<Uuid> {
        let preview = crate::imports::import_print_project(
            path.to_string_lossy().into_owned(),
            self.app.state::<crate::imports::PrintState>(),
            self.app.state::<crate::pet::runtime::PetRuntime>(),
        )?;
        Ok(preview.project_id)
    }
}

fn run_to_completion(
    inner: Arc<SlicerInner>,
    running: Arc<RunningSlice>,
    task_id: Uuid,
    request: SliceRequest,
    temporary_output: PathBuf,
    stdout_reader: thread::JoinHandle<()>,
    stderr_reader: thread::JoinHandle<Vec<u8>>,
) {
    let status = loop {
        let result = running
            .child
            .lock()
            .map_err(|_| AppError::SlicerFailed)
            .and_then(|mut child| child.try_wait().map_err(|_| AppError::SlicerFailed));
        match result {
            Ok(Some(status)) => break Ok(status),
            Ok(None) => thread::sleep(Duration::from_millis(10)),
            Err(error) => break Err(error),
        }
    };
    let _ = stdout_reader.join();
    let _stderr = stderr_reader.join().unwrap_or_default();

    let outcome = if running.cancel_requested.load(Ordering::Acquire) {
        Err(AppError::SlicerCancelled)
    } else {
        match status {
            Ok(status) if status.success() => finish_success(
                &inner,
                task_id,
                &request,
                &temporary_output,
                inner.stability_delay,
            ),
            _ => Err(AppError::SlicerFailed),
        }
    };
    if let Err(error) = outcome {
        finish_error(&inner, task_id, error);
    }

    let _ = fs::remove_dir_all(&running.task_dir);
    if let Ok(mut active) = inner.running.lock() {
        active.remove(&task_id);
    }
    running.mark_finished();
}

fn finish_success(
    inner: &Arc<SlicerInner>,
    task_id: Uuid,
    request: &SliceRequest,
    temporary_output: &Path,
    stability_delay: Duration,
) -> Result<()> {
    set_progress(inner, task_id, SlicePhase::Validating);
    validate_sliced_output(temporary_output, stability_delay)?;
    let published = publish_output(
        temporary_output,
        &request.destination,
        request.allow_overwrite,
        task_id,
    )?;
    set_progress(inner, task_id, SlicePhase::Importing);
    let project_id = match inner.importer.import_project(&request.destination) {
        Ok(project_id) => project_id,
        Err(error) => {
            published.rollback();
            return Err(error);
        }
    };
    published.commit();
    set_completed(inner, task_id, project_id);
    Ok(())
}

fn validate_sliced_output(path: &Path, delay: Duration) -> Result<()> {
    let first = FileStamp::read(path)?;
    if first.size == 0 {
        return Err(AppError::SlicerFailed);
    }
    if !delay.is_zero() {
        thread::sleep(delay);
    }
    let second = FileStamp::read(path)?;
    if first != second {
        return Err(AppError::FileNotStable);
    }
    if parse_3mf_project(path)?.plates.is_empty() {
        return Err(AppError::SlicerFailed);
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct FileStamp {
    size: u64,
    modified_nanos: u128,
}

impl FileStamp {
    fn read(path: &Path) -> Result<Self> {
        let metadata = fs::symlink_metadata(path)?;
        if metadata.file_type().is_symlink() || !metadata.file_type().is_file() {
            return Err(AppError::SlicerFailed);
        }
        let modified_nanos = metadata
            .modified()?
            .duration_since(UNIX_EPOCH)
            .map_err(|_| AppError::SlicerFailed)?
            .as_nanos();
        Ok(Self {
            size: metadata.len(),
            modified_nanos,
        })
    }
}

struct PublishedOutput {
    destination: PathBuf,
    backup: Option<PathBuf>,
}

impl PublishedOutput {
    fn commit(self) {
        if let Some(backup) = self.backup {
            let _ = fs::remove_file(backup);
        }
    }

    fn rollback(self) {
        let _ = fs::remove_file(&self.destination);
        if let Some(backup) = self.backup {
            let _ = fs::rename(backup, self.destination);
        }
    }
}

fn publish_output(
    source: &Path,
    destination: &Path,
    allow_overwrite: bool,
    task_id: Uuid,
) -> Result<PublishedOutput> {
    let parent = destination
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let stage = parent.join(format!(".cylune-{task_id}.tmp"));
    let backup = parent.join(format!(".cylune-{task_id}.backup"));
    let _ = fs::remove_file(&stage);
    let _ = fs::remove_file(&backup);
    copy_new_file(source, &stage)?;

    let existing = fs::symlink_metadata(destination).ok();
    if existing.is_some() && !allow_overwrite {
        let _ = fs::remove_file(&stage);
        return Err(AppError::OutputExists);
    }
    if existing.is_some() {
        fs::rename(destination, &backup).map_err(|_| AppError::SlicerFailed)?;
        if fs::rename(&stage, destination).is_err() {
            let _ = fs::rename(&backup, destination);
            let _ = fs::remove_file(&stage);
            return Err(AppError::SlicerFailed);
        }
        return Ok(PublishedOutput {
            destination: destination.to_path_buf(),
            backup: Some(backup),
        });
    }

    match fs::hard_link(&stage, destination) {
        Ok(()) => {
            let _ = fs::remove_file(stage);
            Ok(PublishedOutput {
                destination: destination.to_path_buf(),
                backup: None,
            })
        }
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
            let _ = fs::remove_file(stage);
            Err(AppError::OutputExists)
        }
        Err(_) => {
            let _ = fs::remove_file(stage);
            Err(AppError::SlicerFailed)
        }
    }
}

fn copy_new_file(source: &Path, destination: &Path) -> Result<()> {
    let mut input = File::open(source).map_err(|_| AppError::SlicerFailed)?;
    let mut output = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(destination)
        .map_err(|_| AppError::SlicerFailed)?;
    io::copy(&mut input, &mut output).map_err(|_| AppError::SlicerFailed)?;
    output.flush().map_err(|_| AppError::SlicerFailed)?;
    output.sync_all().map_err(|_| AppError::SlicerFailed)
}

fn read_bounded(mut reader: impl Read, limit: usize) -> io::Result<Vec<u8>> {
    let mut bytes = Vec::with_capacity(limit.min(8192));
    let mut chunk = [0_u8; 8192];
    loop {
        let read = reader.read(&mut chunk)?;
        if read == 0 {
            break;
        }
        let retained = limit.saturating_sub(bytes.len()).min(read);
        bytes.extend_from_slice(&chunk[..retained]);
    }
    Ok(bytes)
}

fn set_progress(inner: &Arc<SlicerInner>, task_id: Uuid, phase: SlicePhase) {
    if let Ok(mut tasks) = inner.tasks.lock() {
        if let Some(task) = tasks.get_mut(&task_id) {
            task.phase = phase;
        }
    }
    emit_progress(inner, task_id, phase);
}

fn emit_progress(inner: &Arc<SlicerInner>, task_id: Uuid, phase: SlicePhase) {
    inner.events.progress(SliceProgress {
        task_id,
        phase,
        percent: None,
    });
}

fn set_completed(inner: &Arc<SlicerInner>, task_id: Uuid, project_id: Uuid) {
    if let Ok(mut tasks) = inner.tasks.lock() {
        if let Some(task) = tasks.get_mut(&task_id) {
            task.state = SliceTaskState::Completed;
            task.phase = SlicePhase::Complete;
            task.project_id = Some(project_id);
            task.error_code = None;
        }
    }
    emit_progress(inner, task_id, SlicePhase::Complete);
    inner.events.complete(SliceComplete {
        task_id,
        project_id,
    });
}

fn finish_error(inner: &Arc<SlicerInner>, task_id: Uuid, error: AppError) {
    let (state, code) = if matches!(error, AppError::SlicerCancelled) {
        (SliceTaskState::Cancelled, AppError::SlicerCancelled.code())
    } else if matches!(error, AppError::OutputExists) {
        (SliceTaskState::Failed, AppError::OutputExists.code())
    } else {
        (SliceTaskState::Failed, AppError::SlicerFailed.code())
    };
    if let Ok(mut tasks) = inner.tasks.lock() {
        if let Some(task) = tasks.get_mut(&task_id) {
            task.state = state;
            task.error_code = Some(code.to_owned());
        }
    }
    inner.events.error(SliceErrorEvent {
        task_id,
        code: code.to_owned(),
    });
}

#[cfg(test)]
mod tests {
    use super::{
        SliceComplete, SliceEventSink, SliceImporter, SlicePhase, SliceProgress, SliceTaskState,
        SlicerService,
    };
    use crate::{
        error::Result,
        parser::parse_3mf_project,
        printers::SavedPrinter,
        slicer::{FastOverrides, InstallationDiscovery, PlateSelection, SliceRequest},
    };
    use std::{
        fs::{self, File},
        io::Write,
        path::{Path, PathBuf},
        sync::{Arc, Condvar, Mutex},
        time::{Duration, Instant},
    };
    use uuid::Uuid;
    use zip::write::FileOptions;

    #[derive(Default)]
    struct RecordingEvents {
        names: Mutex<Vec<String>>,
        changed: Condvar,
    }

    impl RecordingEvents {
        fn names(&self) -> Vec<String> {
            self.names.lock().unwrap().clone()
        }

        fn push(&self, name: String) {
            self.names.lock().unwrap().push(name);
            self.changed.notify_all();
        }
    }

    impl SliceEventSink for RecordingEvents {
        fn progress(&self, event: SliceProgress) {
            self.push(format!("progress:{:?}", event.phase));
        }

        fn complete(&self, _event: SliceComplete) {
            self.push("complete".to_owned());
        }

        fn error(&self, event: super::SliceErrorEvent) {
            self.push(format!("error:{}", event.code));
        }
    }

    struct RecordingImporter {
        imported: Mutex<Vec<PathBuf>>,
        project_id: Uuid,
    }

    impl RecordingImporter {
        fn new(project_id: Uuid) -> Self {
            Self {
                imported: Mutex::new(Vec::new()),
                project_id,
            }
        }
    }

    impl SliceImporter for RecordingImporter {
        fn import_project(&self, path: &Path) -> Result<Uuid> {
            assert!(!parse_3mf_project(path)?.plates.is_empty());
            self.imported.lock().unwrap().push(path.to_path_buf());
            Ok(self.project_id)
        }
    }

    struct Fixture {
        root: PathBuf,
        app: PathBuf,
        executable: PathBuf,
        cache: PathBuf,
        input: PathBuf,
        machine: PathBuf,
        process: PathBuf,
        filament: PathBuf,
        destination: PathBuf,
    }

    impl Fixture {
        fn success() -> Self {
            let root = std::env::temp_dir().join(format!("cylune-runtime-{}", Uuid::new_v4()));
            let app = root.join("BambuStudio.app");
            let executable = app.join("Contents/MacOS/BambuStudio");
            let profiles = app.join("Contents/Resources/profiles");
            let cache = root.join("cache");
            let source = executable.parent().unwrap().join("fixture.gcode.3mf");
            fs::create_dir_all(executable.parent().unwrap()).unwrap();
            fs::create_dir_all(&profiles).unwrap();
            fs::create_dir_all(&cache).unwrap();
            write_sliced_fixture(&source);
            fs::write(
                &executable,
                b"#!/bin/sh\nout=''\nprev=''\nfor arg in \"$@\"; do\n  if [ \"$prev\" = '--export-3mf' ]; then out=\"$arg\"; break; fi\n  prev=\"$arg\"\ndone\nprintf 'slicing plate 1\\n'\ncp \"$(dirname \"$0\")/fixture.gcode.3mf\" \"$out\"\n",
            )
            .unwrap();
            make_executable(&executable);

            let input = root.join("input project.3mf");
            let machine = root.join("P2S machine.json");
            let process = root.join("0.20 Standard.json");
            let filament = root.join("PLA Basic.json");
            fs::write(&input, b"unsliced project").unwrap();
            fs::write(&machine, b"machine").unwrap();
            fs::write(&process, b"process").unwrap();
            fs::write(&filament, b"filament").unwrap();

            Self {
                destination: root.join("chosen output.gcode.3mf"),
                root,
                app,
                executable,
                cache,
                input,
                machine,
                process,
                filament,
            }
        }

        fn request(&self) -> SliceRequest {
            SliceRequest {
                printer: SavedPrinter {
                    printer_id: "printer".to_owned(),
                    display_name: "My P2S".to_owned(),
                    model_key: "P2S".to_owned(),
                    nozzle_diameter: 0.4,
                    default_plate: "Supertack Plate".to_owned(),
                    ams_kind: "ams".to_owned(),
                    is_default: true,
                    is_available: true,
                },
                expected_filament_count: 1,
                allow_overwrite: false,
                input: self.input.clone(),
                destination: self.destination.clone(),
                plate_selection: PlateSelection::All,
                machine_settings: self.machine.clone(),
                process_settings: self.process.clone(),
                filament_settings: vec![self.filament.clone()],
                fast_overrides: FastOverrides::default(),
            }
        }

        fn set_script(&self, script: &[u8]) {
            fs::write(&self.executable, script).unwrap();
            make_executable(&self.executable);
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    #[cfg(unix)]
    fn make_executable(path: &Path) {
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = fs::metadata(path).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(path, permissions).unwrap();
    }

    #[cfg(not(unix))]
    fn make_executable(_path: &Path) {}

    fn write_sliced_fixture(path: &Path) {
        let mut archive = zip::ZipWriter::new(File::create(path).unwrap());
        let options = FileOptions::default();
        archive
            .start_file("Metadata/filament_settings.config", options)
            .unwrap();
        archive.write_all(br##"{"filament_settings_id":["Bambu PLA Basic"],"filament_type":["PLA"],"filament_colour":["#FFFFFF"],"filament_diameter":["1.75"],"filament_density":["1.24"]}"##).unwrap();
        archive
            .start_file("Metadata/plate_1.gcode", options)
            .unwrap();
        archive
            .write_all(b"; total layer number: 2\nM83\n; LAYER:0\nG1 E10\n")
            .unwrap();
        archive.finish().unwrap();
    }

    fn wait_for_terminal(service: &SlicerService, task_id: Uuid) -> super::SliceTask {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            let task = service.get(task_id).unwrap();
            if task.state != SliceTaskState::Running {
                return task;
            }
            assert!(Instant::now() < deadline, "slice task timed out");
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    #[test]
    fn validates_publishes_and_imports_before_completing() {
        let fixture = Fixture::success();
        let project_id = Uuid::new_v4();
        let importer = Arc::new(RecordingImporter::new(project_id));
        let events = Arc::new(RecordingEvents::default());
        let service = SlicerService::with_dependencies(
            InstallationDiscovery::new(Some(fixture.app.clone())),
            fixture.cache.clone(),
            importer.clone(),
            events.clone(),
            Duration::ZERO,
        );

        let started = service.start(fixture.request()).unwrap();
        let completed = wait_for_terminal(&service, started.task_id);

        assert_eq!(completed.state, SliceTaskState::Completed);
        assert_eq!(completed.project_id, Some(project_id));
        assert_eq!(
            importer.imported.lock().unwrap().as_slice(),
            [fixture.destination.clone()]
        );
        assert!(fixture.destination.is_file());
        assert_eq!(
            events.names(),
            [
                format!("progress:{:?}", SlicePhase::Preparing),
                format!("progress:{:?}", SlicePhase::Slicing),
                format!("progress:{:?}", SlicePhase::Validating),
                format!("progress:{:?}", SlicePhase::Importing),
                format!("progress:{:?}", SlicePhase::Complete),
                "complete".to_owned(),
            ]
        );
        assert!(!fixture
            .cache
            .join("slices")
            .join(started.task_id.to_string())
            .exists());
    }

    #[test]
    fn nonzero_exit_never_imports_or_publishes() {
        let fixture = Fixture::success();
        fixture.set_script(b"#!/bin/sh\nprintf 'private /tmp/model path' >&2\nexit 7\n");
        let importer = Arc::new(RecordingImporter::new(Uuid::new_v4()));
        let events = Arc::new(RecordingEvents::default());
        let service = SlicerService::with_dependencies(
            InstallationDiscovery::new(Some(fixture.app.clone())),
            fixture.cache.clone(),
            importer.clone(),
            events.clone(),
            Duration::ZERO,
        );

        let started = service.start(fixture.request()).unwrap();
        let failed = wait_for_terminal(&service, started.task_id);

        assert_eq!(failed.state, SliceTaskState::Failed);
        assert_eq!(failed.error_code.as_deref(), Some("slicer_failed"));
        assert!(importer.imported.lock().unwrap().is_empty());
        assert!(!fixture.destination.exists());
        assert_eq!(events.names().last().unwrap(), "error:slicer_failed");
    }

    #[test]
    fn truncated_output_is_rejected_before_import_and_removed() {
        let fixture = Fixture::success();
        fixture.set_script(
            b"#!/bin/sh\nout=''\nprev=''\nfor arg in \"$@\"; do\n  if [ \"$prev\" = '--export-3mf' ]; then out=\"$arg\"; break; fi\n  prev=\"$arg\"\ndone\nprintf 'not a zip' > \"$out\"\n",
        );
        let importer = Arc::new(RecordingImporter::new(Uuid::new_v4()));
        let events = Arc::new(RecordingEvents::default());
        let service = SlicerService::with_dependencies(
            InstallationDiscovery::new(Some(fixture.app.clone())),
            fixture.cache.clone(),
            importer.clone(),
            events,
            Duration::ZERO,
        );

        let started = service.start(fixture.request()).unwrap();
        let failed = wait_for_terminal(&service, started.task_id);

        assert_eq!(failed.state, SliceTaskState::Failed);
        assert!(importer.imported.lock().unwrap().is_empty());
        assert!(!fixture.destination.exists());
        assert!(!fixture
            .cache
            .join("slices")
            .join(started.task_id.to_string())
            .exists());
    }

    #[test]
    fn cancellation_waits_for_exit_emits_cancelled_and_cleans_temporary_output() {
        let fixture = Fixture::success();
        fixture.set_script(b"#!/bin/sh\nsleep 5\n");
        let importer = Arc::new(RecordingImporter::new(Uuid::new_v4()));
        let events = Arc::new(RecordingEvents::default());
        let service = SlicerService::with_dependencies(
            InstallationDiscovery::new(Some(fixture.app.clone())),
            fixture.cache.clone(),
            importer.clone(),
            events.clone(),
            Duration::ZERO,
        );

        let started = service.start(fixture.request()).unwrap();
        service.cancel(started.task_id).unwrap();
        let cancelled = service.get(started.task_id).unwrap();

        assert_eq!(cancelled.state, SliceTaskState::Cancelled);
        assert_eq!(cancelled.error_code.as_deref(), Some("slicer_cancelled"));
        assert!(importer.imported.lock().unwrap().is_empty());
        assert!(!fixture.destination.exists());
        assert!(!fixture
            .cache
            .join("slices")
            .join(started.task_id.to_string())
            .exists());
        assert_eq!(events.names().last().unwrap(), "error:slicer_cancelled");
    }

    #[test]
    fn destination_collision_stops_before_process_or_import() {
        let fixture = Fixture::success();
        fs::write(&fixture.destination, b"keep me").unwrap();
        let importer = Arc::new(RecordingImporter::new(Uuid::new_v4()));
        let events = Arc::new(RecordingEvents::default());
        let service = SlicerService::with_dependencies(
            InstallationDiscovery::new(Some(fixture.app.clone())),
            fixture.cache.clone(),
            importer.clone(),
            events.clone(),
            Duration::ZERO,
        );

        let error = service.start(fixture.request()).unwrap_err();

        assert_eq!(error.code(), "output_exists");
        assert_eq!(fs::read(&fixture.destination).unwrap(), b"keep me");
        assert!(importer.imported.lock().unwrap().is_empty());
        assert!(events.names().is_empty());
    }

    struct FailingImporter;

    impl SliceImporter for FailingImporter {
        fn import_project(&self, _path: &Path) -> Result<Uuid> {
            Err(crate::error::AppError::Database(
                "private /Users/robin/model path".to_owned(),
            ))
        }
    }

    #[test]
    fn import_failure_restores_an_explicitly_overwritten_destination() {
        let fixture = Fixture::success();
        fs::write(&fixture.destination, b"original output").unwrap();
        let events = Arc::new(RecordingEvents::default());
        let service = SlicerService::with_dependencies(
            InstallationDiscovery::new(Some(fixture.app.clone())),
            fixture.cache.clone(),
            Arc::new(FailingImporter),
            events.clone(),
            Duration::ZERO,
        );
        let mut request = fixture.request();
        request.allow_overwrite = true;

        let started = service.start(request).unwrap();
        let failed = wait_for_terminal(&service, started.task_id);

        assert_eq!(failed.state, SliceTaskState::Failed);
        assert_eq!(fs::read(&fixture.destination).unwrap(), b"original output");
        assert_eq!(events.names().last().unwrap(), "error:slicer_failed");
    }

    #[test]
    fn stderr_is_bounded_and_error_events_expose_only_stable_codes() {
        let mut stderr = std::io::Cursor::new(vec![b'x'; 70 * 1024]);
        let bytes = super::read_bounded(&mut stderr, 64 * 1024).unwrap();
        assert_eq!(bytes.len(), 64 * 1024);
        assert_eq!(stderr.position(), 70 * 1024);

        let event = super::SliceErrorEvent {
            task_id: Uuid::nil(),
            code: "slicer_failed".to_owned(),
        };
        assert_eq!(
            serde_json::to_value(event).unwrap(),
            serde_json::json!({
                "task_id": "00000000-0000-0000-0000-000000000000",
                "code": "slicer_failed"
            })
        );
    }
}
