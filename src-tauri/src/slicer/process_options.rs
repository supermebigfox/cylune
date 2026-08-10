use std::{
    io,
    process::{Child, Command},
};

#[cfg_attr(not(windows), allow(dead_code))]
pub(crate) const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
#[cfg_attr(not(windows), allow(dead_code))]
pub(crate) const CREATE_NO_WINDOW: u32 = 0x0800_0000;
#[cfg_attr(not(windows), allow(dead_code))]
const CREATE_SUSPENDED: u32 = 0x0000_0004;

#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ProcessPlatform {
    Windows,
    MacOs,
    Other,
}

pub(crate) const fn current_process_platform() -> ProcessPlatform {
    #[cfg(target_os = "windows")]
    {
        ProcessPlatform::Windows
    }
    #[cfg(target_os = "macos")]
    {
        ProcessPlatform::MacOs
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        ProcessPlatform::Other
    }
}

#[cfg_attr(not(windows), allow(dead_code))]
pub(crate) const fn creation_flags(platform: ProcessPlatform) -> u32 {
    match platform {
        ProcessPlatform::Windows => CREATE_NO_WINDOW | CREATE_NEW_PROCESS_GROUP,
        ProcessPlatform::MacOs | ProcessPlatform::Other => 0,
    }
}

#[cfg_attr(not(windows), allow(dead_code))]
pub(crate) const fn gui_creation_flags(platform: ProcessPlatform) -> u32 {
    match platform {
        ProcessPlatform::Windows => CREATE_NEW_PROCESS_GROUP,
        ProcessPlatform::MacOs | ProcessPlatform::Other => 0,
    }
}

#[cfg_attr(not(windows), allow(dead_code))]
const fn job_creation_flags(platform: ProcessPlatform) -> u32 {
    match platform {
        ProcessPlatform::Windows => creation_flags(platform) | CREATE_SUSPENDED,
        ProcessPlatform::MacOs | ProcessPlatform::Other => creation_flags(platform),
    }
}

pub(crate) fn configure_background_command(command: &mut Command, platform: ProcessPlatform) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(creation_flags(platform));
    }
    #[cfg(not(windows))]
    {
        let _ = (command, platform);
    }
}

pub(crate) fn configure_gui_command(command: &mut Command, platform: ProcessPlatform) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(gui_creation_flags(platform));
    }
    #[cfg(not(windows))]
    {
        let _ = (command, platform);
    }
}

pub(crate) struct NativeProcessTerminator {
    #[cfg(windows)]
    job: WindowsJob,
}

impl NativeProcessTerminator {
    pub(crate) fn terminate(&mut self, child: &mut Child, process_id: u32) -> io::Result<()> {
        debug_assert_eq!(child.id(), process_id);
        #[cfg(windows)]
        {
            self.job.close()
        }
        #[cfg(not(windows))]
        {
            child.kill()
        }
    }
}

pub(crate) fn spawn_background_process(
    command: &mut Command,
) -> io::Result<(Child, NativeProcessTerminator)> {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;

        let mut job = WindowsJob::create()?;
        command.creation_flags(job_creation_flags(ProcessPlatform::Windows));
        let mut child = command.spawn()?;
        let startup = job
            .assign(&child)
            .and_then(|()| resume_suspended_process(child.id()));
        if let Err(error) = startup {
            let _ = job.close();
            let _ = child.kill();
            let _ = child.wait();
            return Err(error);
        }
        Ok((child, NativeProcessTerminator { job }))
    }
    #[cfg(not(windows))]
    {
        configure_background_command(command, current_process_platform());
        let child = command.spawn()?;
        Ok((child, NativeProcessTerminator {}))
    }
}

#[cfg(windows)]
struct WindowsJob {
    handle: Option<windows_sys::Win32::Foundation::HANDLE>,
}

#[cfg(windows)]
unsafe impl Send for WindowsJob {}

#[cfg(windows)]
impl WindowsJob {
    fn create() -> io::Result<Self> {
        use std::{mem::size_of, ptr};
        use windows_sys::Win32::System::JobObjects::{
            CreateJobObjectW, JobObjectExtendedLimitInformation, SetInformationJobObject,
            JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
        };

        let handle = unsafe { CreateJobObjectW(ptr::null(), ptr::null()) };
        if handle.is_null() {
            return Err(io::Error::last_os_error());
        }
        let job = Self {
            handle: Some(handle),
        };
        let mut limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
        limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        let configured = unsafe {
            SetInformationJobObject(
                handle,
                JobObjectExtendedLimitInformation,
                (&limits as *const JOBOBJECT_EXTENDED_LIMIT_INFORMATION).cast(),
                size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
            )
        };
        if configured == 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(job)
    }

    fn assign(&self, child: &Child) -> io::Result<()> {
        use std::os::windows::io::AsRawHandle;
        use windows_sys::Win32::{
            Foundation::HANDLE, System::JobObjects::AssignProcessToJobObject,
        };

        let handle = self.handle.ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidInput, "job handle is already closed")
        })?;
        let process_handle = child.as_raw_handle() as HANDLE;
        if unsafe { AssignProcessToJobObject(handle, process_handle) } == 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    }

    fn close(&mut self) -> io::Result<()> {
        use windows_sys::Win32::Foundation::CloseHandle;

        let Some(handle) = self.handle.take() else {
            return Ok(());
        };
        if unsafe { CloseHandle(handle) } == 0 {
            Err(io::Error::last_os_error())
        } else {
            Ok(())
        }
    }
}

#[cfg(windows)]
fn resume_suspended_process(process_id: u32) -> io::Result<()> {
    use std::mem::size_of;
    use windows_sys::Win32::{
        Foundation::{CloseHandle, INVALID_HANDLE_VALUE},
        System::{
            Diagnostics::ToolHelp::{
                CreateToolhelp32Snapshot, Thread32First, Thread32Next, TH32CS_SNAPTHREAD,
                THREADENTRY32,
            },
            Threading::{OpenThread, ResumeThread, THREAD_SUSPEND_RESUME},
        },
    };

    let snapshot = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD, 0) };
    if snapshot == INVALID_HANDLE_VALUE {
        return Err(io::Error::last_os_error());
    }
    let result = (|| {
        let mut entry = THREADENTRY32 {
            dwSize: size_of::<THREADENTRY32>() as u32,
            ..Default::default()
        };
        let mut found = unsafe { Thread32First(snapshot, &mut entry) } != 0;
        while found {
            if entry.th32OwnerProcessID == process_id {
                let thread = unsafe { OpenThread(THREAD_SUSPEND_RESUME, 0, entry.th32ThreadID) };
                if thread.is_null() {
                    return Err(io::Error::last_os_error());
                }
                let previous_suspend_count = unsafe { ResumeThread(thread) };
                let close_result = unsafe { CloseHandle(thread) };
                if previous_suspend_count == u32::MAX {
                    return Err(io::Error::last_os_error());
                }
                if close_result == 0 {
                    return Err(io::Error::last_os_error());
                }
                if previous_suspend_count == 0 {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "new process thread was not suspended",
                    ));
                }
                return Ok(());
            }
            found = unsafe { Thread32Next(snapshot, &mut entry) } != 0;
        }
        Err(io::Error::new(
            io::ErrorKind::NotFound,
            "new process thread was not found",
        ))
    })();
    let close_result = unsafe { CloseHandle(snapshot) };
    if result.is_ok() && close_result == 0 {
        Err(io::Error::last_os_error())
    } else {
        result
    }
}

#[cfg(windows)]
impl Drop for WindowsJob {
    fn drop(&mut self) {
        let _ = self.close();
    }
}

#[cfg(test)]
mod tests {
    use super::{creation_flags, ProcessPlatform, CREATE_NEW_PROCESS_GROUP, CREATE_NO_WINDOW};

    #[test]
    fn windows_cli_uses_no_console_window() {
        assert_eq!(
            creation_flags(ProcessPlatform::Windows),
            CREATE_NO_WINDOW | CREATE_NEW_PROCESS_GROUP
        );
        assert_eq!(creation_flags(ProcessPlatform::Windows), 0x0800_0200);
    }

    #[test]
    fn macos_cli_requires_no_windows_flags() {
        assert_eq!(creation_flags(ProcessPlatform::MacOs), 0);
    }

    #[test]
    fn windows_gui_keeps_a_console_eligible_process_group() {
        assert_eq!(
            super::gui_creation_flags(ProcessPlatform::Windows),
            CREATE_NEW_PROCESS_GROUP
        );
        assert_eq!(super::gui_creation_flags(ProcessPlatform::MacOs), 0);
    }

    #[test]
    fn windows_job_spawn_suspends_without_changing_the_base_policy() {
        assert_eq!(
            super::job_creation_flags(ProcessPlatform::Windows),
            0x0800_0204
        );
        assert_eq!(creation_flags(ProcessPlatform::Windows), 0x0800_0200);
    }

    #[cfg(windows)]
    #[test]
    fn windows_job_spawn_assigns_and_resumes_the_cli_process() {
        use std::process::{Command, Stdio};

        let mut command = Command::new("cmd.exe");
        command
            .args(["/D", "/S", "/C", "exit 0"])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());

        let (mut child, mut terminator) = super::spawn_background_process(&mut command).unwrap();
        let process_id = child.id();
        assert!(child.wait().unwrap().success());
        terminator.terminate(&mut child, process_id).unwrap();
    }
}
