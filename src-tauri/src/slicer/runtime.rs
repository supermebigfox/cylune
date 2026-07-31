use super::{
    build_bambu_args, catalog::load_slice_preset_catalog, inspect_3mf_content,
    progress::BambuProgressParser, project::remap_project_for_machine, resolve_fast_request,
    FastSliceRequest, InstallationDiscovery, SlicePresetCatalog, SliceRequest,
};
use crate::{
    error::{AppError, Result},
    parser::parse_3mf_project,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    fs,
    io::{self, BufRead, BufReader, Read},
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
const RESULT_JSON_LIMIT: u64 = 1024 * 1024;

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
    fn import_project(&self, generated_path: &Path, original_path: &Path) -> Result<Uuid>;
}

struct RunningSlice {
    child: Mutex<Child>,
    cancel_requested: AtomicBool,
    import_started: AtomicBool,
    finished: Mutex<bool>,
    finished_changed: Condvar,
    task_dir: PathBuf,
}

impl RunningSlice {
    fn new(child: Child, task_dir: PathBuf) -> Self {
        Self {
            child: Mutex::new(child),
            cancel_requested: AtomicBool::new(false),
            import_started: AtomicBool::new(false),
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
        cleanup_orphaned_slice_tasks(&cache_root);
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

    pub fn start_fast(
        &self,
        request: FastSliceRequest,
        printer: crate::printers::SavedPrinter,
    ) -> Result<SliceTask> {
        let installation = self.inner.discovery.discover()?;
        let request = resolve_fast_request(&installation.profiles_root, printer, request)?;
        self.start(request)
    }

    pub fn list_presets(
        &self,
        printer: &crate::printers::SavedPrinter,
    ) -> Result<SlicePresetCatalog> {
        let installation = self.inner.discovery.discover()?;
        load_slice_preset_catalog(&installation.profiles_root, printer)
    }

    pub fn open_in_bambu_studio(&self, path: &Path) -> Result<()> {
        inspect_3mf_content(path)?;
        let installation = self.inner.discovery.discover()?;
        let mut child = Command::new(&installation.executable)
            .arg(path)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|_| AppError::SlicerFailed)?;
        thread::spawn(move || {
            let _ = child.wait();
        });
        Ok(())
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
        let command_request = if request.estimate_mode {
            let compatible_input = task_dir.join("input.compat.3mf");
            if let Err(error) = remap_project_for_machine(
                &request.input,
                &compatible_input,
                &request.machine_settings,
            ) {
                let _ = fs::remove_dir_all(&task_dir);
                return Err(error);
            }
            let mut compatible_request = request.clone();
            compatible_request.input = compatible_input;
            compatible_request.estimate_mode = false;
            compatible_request
        } else {
            request.clone()
        };
        let args = match build_bambu_args(&command_request, &temporary_output) {
            Ok(args) => args,
            Err(error) => {
                let _ = fs::remove_dir_all(&task_dir);
                return Err(error);
            }
        };
        #[cfg(test)]
        if std::env::var_os("CYLUNE_TEST_PRINT_SLICER_STDERR").is_some() {
            eprintln!(
                "bambu slicer command: {:?} {:?}",
                installation.executable, args
            );
        }

        let mut child = match Command::new(&installation.executable)
            .args(&args)
            .current_dir(&task_dir)
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
            percent: Some(0.0),
            project_id: None,
            error_code: None,
        };
        self.inner
            .tasks
            .lock()
            .map_err(|_| AppError::SlicerFailed)?
            .insert(task_id, task.clone());
        emit_progress(&self.inner, task_id, SlicePhase::Preparing);
        set_progress(&self.inner, task_id, SlicePhase::Preparing);
        set_progress(&self.inner, task_id, SlicePhase::Slicing);

        let stdout_inner = self.inner.clone();
        let stdout_running = running.clone();
        let stdout_reader = thread::spawn(move || {
            if let Some(stdout) = stdout {
                let mut parser = BambuProgressParser::default();
                for line in BufReader::new(stdout).lines() {
                    let Ok(line) = line else {
                        break;
                    };
                    if let Some(percent) = parser.observe(&line) {
                        set_slicing_progress(&stdout_inner, &stdout_running, task_id, percent);
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
        if !running.import_started.load(Ordering::SeqCst) {
            running.cancel_requested.store(true, Ordering::SeqCst);
            if let Ok(mut child) = running.child.lock() {
                if child.try_wait().ok().flatten().is_none() {
                    let _ = child.kill();
                }
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

fn cleanup_orphaned_slice_tasks(cache_root: &Path) {
    let slices_root = cache_root.join("slices");
    let Ok(root_metadata) = fs::symlink_metadata(&slices_root) else {
        return;
    };
    if root_metadata.file_type().is_symlink() || !root_metadata.file_type().is_dir() {
        return;
    }
    let Ok(entries) = fs::read_dir(&slices_root) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
            continue;
        };
        let Ok(task_id) = Uuid::parse_str(&name) else {
            continue;
        };
        if task_id.to_string() != name {
            continue;
        }
        let Ok(metadata) = fs::symlink_metadata(&path) else {
            continue;
        };
        if metadata.file_type().is_symlink() || !metadata.file_type().is_dir() {
            continue;
        }
        let _ = fs::remove_dir_all(path);
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
    fn import_project(&self, generated_path: &Path, original_path: &Path) -> Result<Uuid> {
        let print_state = self.app.state::<crate::imports::PrintState>();
        let mut service = print_state
            .lock()
            .map_err(|_| AppError::Database("print service lock poisoned".to_owned()))?;
        let preview = service.import_generated_project(generated_path, original_path)?;
        let summary = service.pending_summary()?;
        let job_id = preview
            .plates
            .first()
            .map(|plate| plate.job_id)
            .ok_or(AppError::InvalidJob)?;
        drop(service);
        self.app
            .state::<crate::pet::runtime::PetRuntime>()
            .refresh_pending(
                summary,
                Some(crate::pet::runtime::PetSignal::ImportSucceeded {
                    job_id,
                    pending_count: summary.count,
                }),
            );
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
    #[cfg(test)]
    if std::env::var_os("CYLUNE_TEST_PRINT_SLICER_STDERR").is_some() && !_stderr.is_empty() {
        eprintln!(
            "bambu slicer stderr:\n{}",
            String::from_utf8_lossy(&_stderr)
        );
    }
    #[cfg(test)]
    if std::env::var_os("CYLUNE_TEST_PRINT_SLICER_STDERR").is_some() {
        eprintln!(
            "bambu slicer status: {:?}; output: {:?}",
            status,
            fs::metadata(&temporary_output)
                .ok()
                .map(|metadata| metadata.len())
        );
    }

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
                &running,
            ),
            _ => Err(classify_slicer_failure(&running.task_dir)),
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

fn classify_slicer_failure(task_dir: &Path) -> AppError {
    let result_path = task_dir.join("result.json");
    let Ok(metadata) = fs::symlink_metadata(&result_path) else {
        return AppError::SlicerFailed;
    };
    if metadata.file_type().is_symlink()
        || !metadata.file_type().is_file()
        || metadata.len() > RESULT_JSON_LIMIT
    {
        return AppError::SlicerFailed;
    }
    let Ok(bytes) = fs::read(result_path) else {
        return AppError::SlicerFailed;
    };
    let Ok(result) = serde_json::from_slice::<serde_json::Value>(&bytes) else {
        return AppError::SlicerFailed;
    };
    match result
        .get("return_code")
        .and_then(serde_json::Value::as_i64)
    {
        Some(-101) => return AppError::SlicerPlateConflict,
        Some(-17) => return AppError::SlicerProcessIncompatible,
        _ => {}
    }
    AppError::SlicerFailed
}

fn finish_success(
    inner: &Arc<SlicerInner>,
    task_id: Uuid,
    request: &SliceRequest,
    temporary_output: &Path,
    stability_delay: Duration,
    running: &RunningSlice,
) -> Result<()> {
    set_progress(inner, task_id, SlicePhase::Validating);
    validate_sliced_output(temporary_output, stability_delay)?;
    if running.cancel_requested.load(Ordering::SeqCst) {
        return Err(AppError::SlicerCancelled);
    }
    running.import_started.store(true, Ordering::SeqCst);
    if running.cancel_requested.load(Ordering::SeqCst) {
        return Err(AppError::SlicerCancelled);
    }
    set_progress(inner, task_id, SlicePhase::Importing);
    let project_id = inner
        .importer
        .import_project(temporary_output, &request.input)?;
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
            task.percent = Some(match phase {
                SlicePhase::Preparing => 1.0,
                SlicePhase::Slicing => 3.0,
                SlicePhase::Validating => 98.0,
                SlicePhase::Importing => 99.0,
                SlicePhase::Complete => 100.0,
            });
        }
    }
    emit_progress(inner, task_id, phase);
}

fn emit_progress(inner: &Arc<SlicerInner>, task_id: Uuid, phase: SlicePhase) {
    let percent = inner
        .tasks
        .lock()
        .ok()
        .and_then(|tasks| tasks.get(&task_id).and_then(|task| task.percent));
    inner.events.progress(SliceProgress {
        task_id,
        phase,
        percent,
    });
}

fn set_slicing_progress(
    inner: &Arc<SlicerInner>,
    running: &RunningSlice,
    task_id: Uuid,
    percent: f64,
) {
    if running.cancel_requested.load(Ordering::Acquire) {
        return;
    }
    let updated = if let Ok(mut tasks) = inner.tasks.lock() {
        if let Some(task) = tasks.get_mut(&task_id) {
            if task.state == SliceTaskState::Running
                && percent > task.percent.unwrap_or_default()
                && !running.cancel_requested.load(Ordering::Acquire)
            {
                task.phase = SlicePhase::Slicing;
                task.percent = Some(percent);
                true
            } else {
                false
            }
        } else {
            false
        }
    } else {
        false
    };
    if updated && !running.cancel_requested.load(Ordering::Acquire) {
        emit_progress(inner, task_id, SlicePhase::Slicing);
    }
}

fn set_completed(inner: &Arc<SlicerInner>, task_id: Uuid, project_id: Uuid) {
    if let Ok(mut tasks) = inner.tasks.lock() {
        if let Some(task) = tasks.get_mut(&task_id) {
            task.state = SliceTaskState::Completed;
            task.phase = SlicePhase::Complete;
            task.percent = Some(100.0);
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
    } else if matches!(error, AppError::SlicerPlateConflict) {
        (SliceTaskState::Failed, AppError::SlicerPlateConflict.code())
    } else if matches!(error, AppError::SlicerProcessIncompatible) {
        (
            SliceTaskState::Failed,
            AppError::SlicerProcessIncompatible.code(),
        )
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
        classify_slicer_failure, SliceComplete, SliceEventSink, SliceImporter, SlicePhase,
        SliceProgress, SliceTaskState, SlicerService,
    };
    use crate::{
        db::AppDatabase,
        error::{AppError, Result},
        history::ImportProjectPreview,
        imports::{sha256, PrintService},
        media::MediaStore,
        parser::parse_3mf_project,
        printers::SavedPrinter,
        slicer::{
            inspect_3mf_content, FastSliceRequest, InstallationDiscovery, PlateSelection,
            SliceRequest, ThreeMfKind,
        },
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
        progresses: Mutex<Vec<SliceProgress>>,
        changed: Condvar,
    }

    impl RecordingEvents {
        fn names(&self) -> Vec<String> {
            self.names.lock().unwrap().clone()
        }

        fn progresses(&self) -> Vec<SliceProgress> {
            self.progresses.lock().unwrap().clone()
        }

        fn push(&self, name: String) {
            self.names.lock().unwrap().push(name);
            self.changed.notify_all();
        }
    }

    impl SliceEventSink for RecordingEvents {
        fn progress(&self, event: SliceProgress) {
            self.progresses.lock().unwrap().push(event.clone());
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
        imported: Mutex<Vec<(PathBuf, PathBuf)>>,
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
        fn import_project(&self, generated_path: &Path, original_path: &Path) -> Result<Uuid> {
            assert!(!parse_3mf_project(generated_path)?.plates.is_empty());
            self.imported
                .lock()
                .unwrap()
                .push((generated_path.to_path_buf(), original_path.to_path_buf()));
            Ok(self.project_id)
        }
    }

    struct ProjectImporter {
        service: Mutex<PrintService>,
        preview: Mutex<Option<ImportProjectPreview>>,
    }

    impl ProjectImporter {
        fn new(data_root: &Path) -> Self {
            let database = AppDatabase::open(data_root.join("cylune.sqlite")).unwrap();
            let media_store = MediaStore::new(data_root.to_path_buf()).unwrap();
            Self {
                service: Mutex::new(PrintService::with_media_store_and_stability_delay(
                    database,
                    media_store,
                    Duration::ZERO,
                )),
                preview: Mutex::new(None),
            }
        }
    }

    impl SliceImporter for ProjectImporter {
        fn import_project(&self, generated_path: &Path, original_path: &Path) -> Result<Uuid> {
            let preview = self
                .service
                .lock()
                .unwrap()
                .import_generated_project(generated_path, original_path)?;
            let project_id = preview.project_id;
            *self.preview.lock().unwrap() = Some(preview);
            Ok(project_id)
        }
    }

    struct Fixture {
        root: PathBuf,
        app: PathBuf,
        executable: PathBuf,
        cache: PathBuf,
        input: PathBuf,
        machine: PathBuf,
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
            fs::write(&input, b"unsliced project").unwrap();
            fs::write(&machine, b"machine").unwrap();

            Self {
                root,
                app,
                executable,
                cache,
                input,
                machine,
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
                input: self.input.clone(),
                plate_selection: PlateSelection::All,
                estimate_mode: false,
                machine_settings: self.machine.clone(),
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

    fn write_unsliced_fixture(path: &Path) {
        let mut archive = zip::ZipWriter::new(File::create(path).unwrap());
        let options = FileOptions::default();
        for (name, contents) in [
            ("[Content_Types].xml", b"<Types/>".as_slice()),
            ("3D/3dmodel.model", b"<model/>".as_slice()),
            (
                "Metadata/project_settings.config",
                br##"{"filament_settings_id":["Bambu PLA Basic"],"filament_type":["PLA"],"filament_colour":["#FFFFFF"]}"##.as_slice(),
            ),
            (
                "Metadata/model_settings.config",
                br#"<config><plate><metadata key="plater_id" value="1"/></plate></config>"#.as_slice(),
            ),
        ] {
            archive.start_file(name, options).unwrap();
            archive.write_all(contents).unwrap();
        }
        archive.finish().unwrap();
    }

    fn write_machine_mismatch_fixture(path: &Path) {
        let mut archive = zip::ZipWriter::new(File::create(path).unwrap());
        let options = FileOptions::default();
        archive
            .start_file("Metadata/project_settings.config", options)
            .unwrap();
        archive.write_all(br##"{"printer_model":"Bambu Lab X2D","printer_settings_id":"Bambu Lab X2D 0.4 nozzle","print_settings_id":"0.16mm Optimal @BBL X2D","default_print_profile":"0.20mm Standard @BBL X2D","print_compatible_printers":["Bambu Lab X2D 0.4 nozzle"],"filament_settings_id":["Bambu PLA Basic @BBL X2D 0.4 nozzle"],"default_filament_profile":["Bambu PLA Basic @BBL X2D 0.4 nozzle"],"filament_type":["PLA"],"filament_colour":["#FFFFFF"],"layer_height":"0.12","sparse_infill_density":"37%"}"##).unwrap();
        archive.start_file("3D/3dmodel.model", options).unwrap();
        archive.write_all(b"<model/>").unwrap();
        archive.finish().unwrap();
    }

    fn wait_for_terminal(service: &SlicerService, task_id: Uuid) -> super::SliceTask {
        wait_for_terminal_until(service, task_id, Duration::from_secs(15))
    }

    fn wait_for_terminal_until(
        service: &SlicerService,
        task_id: Uuid,
        timeout: Duration,
    ) -> super::SliceTask {
        let deadline = Instant::now() + timeout;
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
    fn startup_removes_only_orphaned_slice_tasks() {
        let root = std::env::temp_dir().join(format!("cylune-orphan-cleanup-{}", Uuid::new_v4()));
        let cache = root.join("cache");
        let slices = cache.join("slices");
        let orphan = slices.join(Uuid::new_v4().to_string());
        let unrelated_dir = slices.join("keep-me");
        let uuid_file = slices.join(Uuid::new_v4().to_string());
        fs::create_dir_all(&orphan).unwrap();
        fs::create_dir_all(&unrelated_dir).unwrap();
        fs::write(orphan.join("output.gcode.3mf"), b"stale").unwrap();
        fs::write(&uuid_file, b"not a task directory").unwrap();

        let _service = SlicerService::with_dependencies(
            InstallationDiscovery::new(None),
            cache,
            Arc::new(RecordingImporter::new(Uuid::new_v4())),
            Arc::new(RecordingEvents::default()),
            Duration::ZERO,
        );

        assert!(!orphan.exists());
        assert!(unrelated_dir.is_dir());
        assert!(uuid_file.is_file());

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn validates_imports_private_output_and_removes_it() {
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
        let task_root = fixture
            .cache
            .join("slices")
            .join(started.task_id.to_string());
        assert_eq!(
            importer.imported.lock().unwrap().as_slice(),
            [(task_root.join("output.gcode.3mf"), fixture.input.clone())]
        );
        let event_names = events.names();
        let validating = event_names
            .iter()
            .position(|name| name == &format!("progress:{:?}", SlicePhase::Validating))
            .expect("real slicing must enter validation");
        assert_eq!(
            &event_names[..2],
            [
                format!("progress:{:?}", SlicePhase::Preparing),
                format!("progress:{:?}", SlicePhase::Preparing),
            ]
        );
        assert!(event_names[2..validating]
            .iter()
            .all(|name| name == &format!("progress:{:?}", SlicePhase::Slicing)));
        assert_eq!(
            &event_names[validating..],
            [
                format!("progress:{:?}", SlicePhase::Validating),
                format!("progress:{:?}", SlicePhase::Importing),
                format!("progress:{:?}", SlicePhase::Complete),
                "complete".to_owned(),
            ]
        );
        let numeric_progress = events
            .progresses()
            .into_iter()
            .map(|event| event.percent.expect("real progress must be determinate"))
            .collect::<Vec<_>>();
        assert!(numeric_progress.windows(2).all(|pair| pair[0] <= pair[1]));
        assert!(!task_root.exists());
    }

    #[test]
    fn streams_monotonic_bambu_progress_and_publishes_determinate_lifecycle_values() {
        let fixture = Fixture::success();
        fixture.set_script(
            b"#!/bin/sh\nout=''\nprev=''\nfor arg in \"$@\"; do\n  if [ \"$prev\" = '--export-3mf' ]; then out=\"$arg\"; break; fi\n  prev=\"$arg\"\ndone\nprintf '%s\\n' 'Need to slice for plate 0, total plate count 2 partplates!'\nsleep 0.02\nprintf '%s\\n' 'start Print::process for partplate 1'\nprintf '%s\\n' 'default_status_callback: percent=50, warning_step=-1, message=Generating infill, message_type=0'\nsleep 0.02\nprintf '%s\\n' 'start Print::process for partplate 2'\nprintf '%s\\n' 'default_status_callback: percent=50, warning_step=-1, message=Generating infill, message_type=0'\nprintf '%s\\n' 'will export 3mf'\ncp \"$(dirname \"$0\")/fixture.gcode.3mf\" \"$out\"\n",
        );
        let events = Arc::new(RecordingEvents::default());
        let service = SlicerService::with_dependencies(
            InstallationDiscovery::new(Some(fixture.app.clone())),
            fixture.cache.clone(),
            Arc::new(RecordingImporter::new(Uuid::new_v4())),
            events.clone(),
            Duration::ZERO,
        );

        let started = service.start(fixture.request()).unwrap();
        assert_eq!(started.percent, Some(0.0));
        let completed = wait_for_terminal(&service, started.task_id);
        let progress = events.progresses();
        let numeric = progress
            .iter()
            .map(|event| event.percent.expect("every progress event is determinate"))
            .collect::<Vec<_>>();

        assert!(numeric.windows(2).all(|pair| pair[0] <= pair[1]));
        assert!(progress
            .iter()
            .any(|event| { event.phase == SlicePhase::Preparing && event.percent == Some(1.0) }));
        assert!(progress
            .iter()
            .any(|event| { event.phase == SlicePhase::Slicing && event.percent == Some(25.5) }));
        assert!(progress
            .iter()
            .any(|event| { event.phase == SlicePhase::Slicing && event.percent == Some(70.5) }));
        assert!(progress
            .iter()
            .any(|event| { event.phase == SlicePhase::Validating && event.percent == Some(98.0) }));
        assert!(progress
            .iter()
            .any(|event| { event.phase == SlicePhase::Importing && event.percent == Some(99.0) }));
        assert_eq!(progress.last().unwrap().percent, Some(100.0));
        assert_eq!(completed.percent, Some(100.0));
    }

    #[test]
    fn cancellation_prevents_later_stdout_progress_updates() {
        let fixture = Fixture::success();
        fixture.set_script(
            b"#!/bin/sh\nprintf '%s\\n' 'default_status_callback: percent=50, warning_step=-1, message=Generating infill, message_type=0'\nsleep 2\nprintf '%s\\n' 'default_status_callback: percent=90, warning_step=-1, message=Generating infill, message_type=0'\n",
        );
        let events = Arc::new(RecordingEvents::default());
        let service = SlicerService::with_dependencies(
            InstallationDiscovery::new(Some(fixture.app.clone())),
            fixture.cache.clone(),
            Arc::new(RecordingImporter::new(Uuid::new_v4())),
            events.clone(),
            Duration::ZERO,
        );

        let started = service.start(fixture.request()).unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        while !events
            .progresses()
            .iter()
            .any(|event| event.percent == Some(48.0))
        {
            assert!(
                Instant::now() < deadline,
                "first stdout progress was not emitted"
            );
            std::thread::sleep(Duration::from_millis(5));
        }
        service.cancel(started.task_id).unwrap();
        std::thread::sleep(Duration::from_millis(50));

        let progress = events.progresses();
        assert!(progress.iter().any(|event| event.percent == Some(48.0)));
        assert!(progress.iter().all(|event| event.percent != Some(84.0)));
    }

    #[test]
    fn runs_the_cli_inside_the_private_task_directory() {
        let fixture = Fixture::success();
        fixture.set_script(
            b"#!/bin/sh\npwd > \"$(dirname \"$0\")/captured-cwd.txt\"\nprintf 'bambu side effect' > result.json\nout=''\nprev=''\nfor arg in \"$@\"; do\n  if [ \"$prev\" = '--export-3mf' ]; then out=\"$arg\"; break; fi\n  prev=\"$arg\"\ndone\ncp \"$(dirname \"$0\")/fixture.gcode.3mf\" \"$out\"\n",
        );
        let service = SlicerService::with_dependencies(
            InstallationDiscovery::new(Some(fixture.app.clone())),
            fixture.cache.clone(),
            Arc::new(RecordingImporter::new(Uuid::new_v4())),
            Arc::new(RecordingEvents::default()),
            Duration::ZERO,
        );

        let started = service.start(fixture.request()).unwrap();
        let completed = wait_for_terminal(&service, started.task_id);

        assert_eq!(completed.state, SliceTaskState::Completed);
        let task_directory = fs::canonicalize(&fixture.cache)
            .unwrap()
            .join("slices")
            .join(started.task_id.to_string());
        let captured = fs::read_to_string(
            fixture
                .executable
                .parent()
                .unwrap()
                .join("captured-cwd.txt"),
        )
        .unwrap();
        assert_eq!(PathBuf::from(captured.trim()), task_directory);
        assert!(!fixture.root.join("result.json").exists());
        let cleanup_deadline = Instant::now() + Duration::from_secs(5);
        while task_directory.exists() && Instant::now() < cleanup_deadline {
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(!task_directory.exists());
    }

    #[test]
    fn confirmed_machine_conversion_uses_a_private_identity_remap_without_estimate_mode() {
        let fixture = Fixture::success();
        write_machine_mismatch_fixture(&fixture.input);
        fs::write(
            &fixture.machine,
            br#"{"name":"Bambu Lab P2S 0.4 nozzle","printer_model":"Bambu Lab P2S","default_print_profile":"0.20mm Standard @BBL P2S","default_filament_profile":["Bambu PLA Basic @BBL P2S"]}"#,
        )
        .unwrap();
        fixture.set_script(
            b"#!/bin/sh\nout=''\nprev=''\ninput=''\n: > \"$(dirname \"$0\")/captured-args.txt\"\nfor arg in \"$@\"; do\n  printf '%s\\n' \"$arg\" >> \"$(dirname \"$0\")/captured-args.txt\"\n  input=\"$arg\"\n  if [ \"$prev\" = '--export-3mf' ]; then out=\"$arg\"; fi\n  prev=\"$arg\"\ndone\nunzip -p \"$input\" Metadata/project_settings.config > \"$(dirname \"$0\")/captured-project.json\"\ncp \"$(dirname \"$0\")/fixture.gcode.3mf\" \"$out\"\n",
        );
        let source_before = fs::read(&fixture.input).unwrap();
        let service = SlicerService::with_dependencies(
            InstallationDiscovery::new(Some(fixture.app.clone())),
            fixture.cache.clone(),
            Arc::new(RecordingImporter::new(Uuid::new_v4())),
            Arc::new(RecordingEvents::default()),
            Duration::ZERO,
        );
        let mut request = fixture.request();
        request.estimate_mode = true;

        let started = service.start(request).unwrap();
        let completed = wait_for_terminal(&service, started.task_id);

        assert_eq!(completed.state, SliceTaskState::Completed);
        assert_eq!(fs::read(&fixture.input).unwrap(), source_before);
        let executable_dir = fixture.executable.parent().unwrap();
        let captured = fs::read_to_string(executable_dir.join("captured-args.txt")).unwrap();
        assert!(!captured
            .lines()
            .any(|argument| argument == "--estimate-mode"));
        assert!(captured
            .lines()
            .last()
            .unwrap()
            .contains("input.compat.3mf"));
        let project: serde_json::Value = serde_json::from_slice(
            &fs::read(executable_dir.join("captured-project.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(project["printer_model"], "Bambu Lab P2S");
        assert_eq!(project["layer_height"], "0.12");
        assert_eq!(project["sparse_infill_density"], "37%");
    }

    #[test]
    fn opens_a_valid_project_only_when_the_explicit_gui_action_is_called() {
        let fixture = Fixture::success();
        write_unsliced_fixture(&fixture.input);
        fixture.set_script(
            b"#!/bin/sh\nprintf '%s\\n' \"$#\" \"$1\" > \"$(dirname \"$0\")/opened-project.txt\"\n",
        );
        let service = SlicerService::with_dependencies(
            InstallationDiscovery::new(Some(fixture.app.clone())),
            fixture.cache.clone(),
            Arc::new(RecordingImporter::new(Uuid::new_v4())),
            Arc::new(RecordingEvents::default()),
            Duration::ZERO,
        );

        service.open_in_bambu_studio(&fixture.input).unwrap();

        let capture = fixture
            .executable
            .parent()
            .unwrap()
            .join("opened-project.txt");
        // macOS can defer a newly spawned app-bundle executable while other
        // process-heavy tests are running, even though spawn itself succeeds.
        let deadline = Instant::now() + Duration::from_secs(10);
        while !capture.exists() {
            assert!(
                Instant::now() < deadline,
                "open action did not launch the executable"
            );
            std::thread::sleep(Duration::from_millis(10));
        }
        let opened = fs::read_to_string(capture).unwrap();
        assert_eq!(opened, format!("1\n{}\n", fixture.input.display()));
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
        let task_root = fixture
            .cache
            .join("slices")
            .join(started.task_id.to_string());
        let deadline = Instant::now() + Duration::from_secs(5);
        while task_root.exists() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(!task_root.exists());
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
        assert!(!fixture
            .cache
            .join("slices")
            .join(started.task_id.to_string())
            .exists());
        assert_eq!(events.names().last().unwrap(), "error:slicer_cancelled");
    }

    #[test]
    fn cancellation_during_validation_never_imports_the_project() {
        let fixture = Fixture::success();
        let importer = Arc::new(RecordingImporter::new(Uuid::new_v4()));
        let service = SlicerService::with_dependencies(
            InstallationDiscovery::new(Some(fixture.app.clone())),
            fixture.cache.clone(),
            importer.clone(),
            Arc::new(RecordingEvents::default()),
            Duration::from_secs(2),
        );

        let started = service.start(fixture.request()).unwrap();
        let deadline = Instant::now() + Duration::from_secs(15);
        while service.get(started.task_id).unwrap().phase != SlicePhase::Validating {
            assert!(Instant::now() < deadline, "task never entered validation");
            std::thread::sleep(Duration::from_millis(5));
        }
        service.cancel(started.task_id).unwrap();

        let cancelled = service.get(started.task_id).unwrap();
        assert_eq!(cancelled.state, SliceTaskState::Cancelled);
        assert!(importer.imported.lock().unwrap().is_empty());
    }

    struct FailingImporter;

    impl SliceImporter for FailingImporter {
        fn import_project(&self, _generated_path: &Path, _original_path: &Path) -> Result<Uuid> {
            Err(crate::error::AppError::Database(
                "private /Users/robin/model path".to_owned(),
            ))
        }
    }

    #[test]
    fn import_failure_removes_private_slice_artifacts() {
        let fixture = Fixture::success();
        let events = Arc::new(RecordingEvents::default());
        let service = SlicerService::with_dependencies(
            InstallationDiscovery::new(Some(fixture.app.clone())),
            fixture.cache.clone(),
            Arc::new(FailingImporter),
            events.clone(),
            Duration::ZERO,
        );
        let started = service.start(fixture.request()).unwrap();
        let failed = wait_for_terminal(&service, started.task_id);

        assert_eq!(failed.state, SliceTaskState::Failed);
        assert!(!fixture
            .cache
            .join("slices")
            .join(started.task_id.to_string())
            .exists());
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

    #[test]
    fn classifies_bambu_plate_path_conflicts_from_private_result_json() {
        let task_dir =
            std::env::temp_dir().join(format!("cylune-slicer-conflict-result-{}", Uuid::new_v4()));
        fs::create_dir_all(&task_dir).unwrap();
        fs::write(
            task_dir.join("result.json"),
            br#"{"return_code":-101,"plate_index":4,"error_string":"untrusted details"}"#,
        )
        .unwrap();

        assert!(matches!(
            classify_slicer_failure(&task_dir),
            AppError::SlicerPlateConflict
        ));

        fs::remove_dir_all(task_dir).unwrap();
    }

    #[test]
    fn classifies_bambu_process_incompatibility_without_exposing_private_details() {
        let task_dir =
            std::env::temp_dir().join(format!("cylune-slicer-process-result-{}", Uuid::new_v4()));
        fs::create_dir_all(&task_dir).unwrap();
        fs::write(
            task_dir.join("result.json"),
            br#"{"return_code":-17,"error_string":"untrusted details"}"#,
        )
        .unwrap();

        let error = classify_slicer_failure(&task_dir);
        assert!(matches!(error, AppError::SlicerProcessIncompatible));
        assert_eq!(error.code(), "slicer_process_incompatible");

        fs::remove_dir_all(task_dir).unwrap();
    }

    #[test]
    #[ignore = "requires CYLUNE_SLICE_INPUT_3MF and CYLUNE_EXPECTED_PLATE_COUNT with an installed Bambu Studio"]
    fn smoke_real_slice_validates_output_then_imports_one_project() {
        let source = PathBuf::from(
            std::env::var_os("CYLUNE_SLICE_INPUT_3MF").expect("CYLUNE_SLICE_INPUT_3MF is required"),
        );
        let expected_plate_count = std::env::var("CYLUNE_EXPECTED_PLATE_COUNT")
            .expect("CYLUNE_EXPECTED_PLATE_COUNT is required")
            .parse::<usize>()
            .expect("CYLUNE_EXPECTED_PLATE_COUNT must be a positive integer");
        assert!(expected_plate_count > 0);
        let expected_source_hash = std::env::var("CYLUNE_EXPECTED_SOURCE_SHA256").ok();
        let source_hash_before = sha256(&source).unwrap();
        if let Some(expected) = expected_source_hash.as_deref() {
            assert_eq!(source_hash_before, expected);
        }
        let source_metadata_before = fs::metadata(&source).unwrap();

        let root =
            std::env::temp_dir().join(format!("cylune-real-slice-import-{}", Uuid::new_v4()));
        let cache = root.join("cache");
        let data_root = root.join("data");
        fs::create_dir_all(&cache).unwrap();
        fs::create_dir_all(&data_root).unwrap();
        let copied_input = root.join("input.3mf");
        fs::copy(&source, &copied_input).unwrap();
        assert_eq!(sha256(&copied_input).unwrap(), source_hash_before);

        let inspection = inspect_3mf_content(&copied_input).unwrap();
        assert_eq!(inspection.kind, ThreeMfKind::Unsliced);
        assert_eq!(inspection.plate_count as usize, expected_plate_count);
        let model_key = std::env::var("CYLUNE_TARGET_MODEL_KEY")
            .ok()
            .or_else(|| inspection.embedded_model_key.clone())
            .expect("the supplied project or test target must declare a printer model");
        let nozzle_diameter = std::env::var("CYLUNE_TARGET_NOZZLE_DIAMETER")
            .ok()
            .map(|value| {
                value
                    .parse::<f64>()
                    .expect("CYLUNE_TARGET_NOZZLE_DIAMETER must be a number")
            })
            .or(inspection.embedded_nozzle_diameter)
            .expect("the supplied project or test target must declare a nozzle diameter");
        let default_plate = std::env::var("CYLUNE_TARGET_DEFAULT_PLATE")
            .ok()
            .or_else(|| inspection.embedded_plate_key.clone())
            .expect("the supplied project or test target must declare a plate type");
        let printer_mismatch = inspection
            .embedded_model_key
            .as_deref()
            .is_some_and(|embedded| embedded != model_key)
            || inspection
                .embedded_nozzle_diameter
                .is_some_and(|embedded| (embedded - nozzle_diameter).abs() > 0.001);
        let printer = SavedPrinter {
            printer_id: Uuid::new_v4().to_string(),
            display_name: model_key.clone(),
            model_key,
            nozzle_diameter,
            default_plate,
            ams_kind: "none".to_owned(),
            is_default: true,
            is_available: true,
        };
        let request = FastSliceRequest {
            input_path: copied_input.clone(),
            printer_id: printer.printer_id.clone(),
            confirm_printer_mismatch: printer_mismatch,
        };
        let app = std::env::var_os("BAMBU_STUDIO_APP")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("/Applications/BambuStudio.app"));
        let importer = Arc::new(ProjectImporter::new(&data_root));
        let events = Arc::new(RecordingEvents::default());
        let service = SlicerService::with_dependencies(
            InstallationDiscovery::new(Some(app)),
            cache.clone(),
            importer.clone(),
            events.clone(),
            Duration::from_millis(150),
        );

        let started = service.start_fast(request, printer).unwrap();
        let completed =
            wait_for_terminal_until(&service, started.task_id, Duration::from_secs(1800));
        if let Ok(expected_error_code) = std::env::var("CYLUNE_EXPECTED_SLICE_ERROR_CODE") {
            assert_eq!(completed.state, SliceTaskState::Failed);
            assert_eq!(
                completed.error_code.as_deref(),
                Some(expected_error_code.as_str())
            );
            assert!(importer.preview.lock().unwrap().is_none());
            let task_root = cache.join("slices").join(started.task_id.to_string());
            assert!(!task_root.exists());
            fs::remove_dir_all(&root).unwrap();
            let source_metadata_after = fs::metadata(&source).unwrap();
            assert_eq!(source_metadata_after.len(), source_metadata_before.len());
            assert_eq!(
                source_metadata_after.modified().unwrap(),
                source_metadata_before.modified().unwrap()
            );
            assert_eq!(sha256(&source).unwrap(), source_hash_before);
            return;
        }
        println!(
            "real_slice terminal state={:?} error_code={:?}",
            completed.state, completed.error_code
        );
        assert_eq!(completed.state, SliceTaskState::Completed);
        let preview = importer
            .preview
            .lock()
            .unwrap()
            .clone()
            .expect("validated output must be imported before completion");
        assert_eq!(completed.project_id, Some(preview.project_id));
        assert_eq!(preview.plates.len(), expected_plate_count);
        for imported in &preview.plates {
            assert_imported_plate_has_metrics_and_thumbnail(imported, &data_root);
        }
        if let Ok(expected_total_grams) = std::env::var("CYLUNE_EXPECTED_TOTAL_GRAMS") {
            let expected_total_grams = expected_total_grams
                .parse::<f64>()
                .expect("CYLUNE_EXPECTED_TOTAL_GRAMS must be a number");
            let tolerance = std::env::var("CYLUNE_EXPECTED_TOTAL_GRAMS_TOLERANCE")
                .ok()
                .map(|value| {
                    value
                        .parse::<f64>()
                        .expect("CYLUNE_EXPECTED_TOTAL_GRAMS_TOLERANCE must be a number")
                })
                .unwrap_or(0.001);
            let imported_total_grams = preview
                .plates
                .iter()
                .flat_map(|plate| &plate.filaments)
                .map(|filament| filament.total_grams)
                .sum::<f64>();
            assert!(
                (imported_total_grams - expected_total_grams).abs() <= tolerance,
                "expected {expected_total_grams}g ± {tolerance}g, got {imported_total_grams}g"
            );
        }
        let imported_total_seconds = preview
            .plates
            .iter()
            .map(|plate| plate.estimated_seconds.unwrap_or(0))
            .sum::<u32>();
        if let Ok(minimum) = std::env::var("CYLUNE_EXPECTED_TOTAL_SECONDS_MIN") {
            let minimum = minimum
                .parse::<u32>()
                .expect("CYLUNE_EXPECTED_TOTAL_SECONDS_MIN must be an integer");
            assert!(imported_total_seconds >= minimum);
        }
        if let Ok(maximum) = std::env::var("CYLUNE_EXPECTED_TOTAL_SECONDS_MAX") {
            let maximum = maximum
                .parse::<u32>()
                .expect("CYLUNE_EXPECTED_TOTAL_SECONDS_MAX must be an integer");
            assert!(imported_total_seconds <= maximum);
        }

        let print_service = importer.service.lock().unwrap();
        let project_count: u32 = print_service
            .database
            .connection
            .query_row("SELECT COUNT(*) FROM print_projects", [], |row| row.get(0))
            .unwrap();
        let distinct_project_count: u32 = print_service
            .database
            .connection
            .query_row(
                "SELECT COUNT(DISTINCT project_id) FROM print_plates",
                [],
                |row| row.get(0),
            )
            .unwrap();
        let plate_count: u32 = print_service
            .database
            .connection
            .query_row("SELECT COUNT(*) FROM print_plates", [], |row| row.get(0))
            .unwrap();
        let job_count: u32 = print_service
            .database
            .connection
            .query_row("SELECT COUNT(*) FROM print_jobs", [], |row| row.get(0))
            .unwrap();
        assert_eq!(project_count, 1);
        assert_eq!(distinct_project_count, 1);
        assert_eq!(plate_count as usize, expected_plate_count);
        assert_eq!(job_count as usize, expected_plate_count);
        drop(print_service);

        let event_names = events.names();
        let validating = event_names
            .iter()
            .position(|name| name == &format!("progress:{:?}", SlicePhase::Validating))
            .expect("real slicing must enter validation");
        assert_eq!(
            &event_names[..2],
            [
                format!("progress:{:?}", SlicePhase::Preparing),
                format!("progress:{:?}", SlicePhase::Preparing),
            ]
        );
        assert!(event_names[2..validating]
            .iter()
            .all(|name| name == &format!("progress:{:?}", SlicePhase::Slicing)));
        assert_eq!(
            &event_names[validating..],
            [
                format!("progress:{:?}", SlicePhase::Validating),
                format!("progress:{:?}", SlicePhase::Importing),
                format!("progress:{:?}", SlicePhase::Complete),
                "complete".to_owned(),
            ]
        );
        let numeric_progress = events
            .progresses()
            .into_iter()
            .map(|event| event.percent.expect("real progress must be determinate"))
            .collect::<Vec<_>>();
        assert!(numeric_progress.windows(2).all(|pair| pair[0] <= pair[1]));
        println!(
            "real_slice project_id={} plates={} metrics={:?}",
            preview.project_id,
            preview.plates.len(),
            preview
                .plates
                .iter()
                .map(|plate| (
                    plate.plate_index,
                    plate.estimated_seconds,
                    plate.max_layer,
                    plate
                        .filaments
                        .iter()
                        .map(|filament| (
                            filament.tool,
                            filament.profile.color_hex.clone(),
                            filament.total_grams
                        ))
                        .collect::<Vec<_>>()
                ))
                .collect::<Vec<_>>()
        );

        let cleanup_deadline = Instant::now() + Duration::from_secs(5);
        let task_root = cache.join("slices").join(started.task_id.to_string());
        while task_root.exists() && Instant::now() < cleanup_deadline {
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(!task_root.exists());
        fs::remove_dir_all(&root).unwrap();

        let source_metadata_after = fs::metadata(&source).unwrap();
        assert_eq!(source_metadata_after.len(), source_metadata_before.len());
        assert_eq!(
            source_metadata_after.modified().unwrap(),
            source_metadata_before.modified().unwrap()
        );
        assert_eq!(sha256(&source).unwrap(), source_hash_before);
    }

    fn assert_imported_plate_has_metrics_and_thumbnail(
        imported: &crate::history::ImportPlatePreview,
        data_root: &Path,
    ) {
        assert!(imported
            .estimated_seconds
            .is_some_and(|seconds| seconds > 0));
        assert!(imported.max_layer > 0);
        assert!(imported
            .filaments
            .iter()
            .any(|filament| filament.total_grams > 0.0));
        let relative_thumbnail = imported
            .thumbnail_url
            .as_deref()
            .expect("every sliced plate must import a thumbnail");
        let thumbnail = data_root.join(relative_thumbnail);
        assert!(thumbnail.is_file());
        let pixels = image::open(thumbnail).unwrap().to_rgb8();
        assert!(pixels.width() > 1 && pixels.height() > 1);
        let background = pixels.get_pixel(0, 0).0;
        assert!(pixels.pixels().any(|pixel| {
            let [red, green, blue] = pixel.0;
            red.abs_diff(background[0]) > 3
                || green.abs_diff(background[1]) > 3
                || blue.abs_diff(background[2]) > 3
        }));
    }
}
