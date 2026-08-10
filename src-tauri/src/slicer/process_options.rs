use std::{
    io,
    process::{Child, Command},
};

#[cfg_attr(not(windows), allow(dead_code))]
pub(crate) const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
#[cfg_attr(not(windows), allow(dead_code))]
pub(crate) const CREATE_NO_WINDOW: u32 = 0x0800_0000;

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
    pub(crate) fn attach(child: &Child) -> io::Result<Self> {
        #[cfg(windows)]
        {
            Ok(Self {
                job: WindowsJob::attach(child)?,
            })
        }
        #[cfg(not(windows))]
        {
            let _ = child;
            Ok(Self {})
        }
    }

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

#[cfg(windows)]
struct WindowsJob {
    handle: Option<windows_sys::Win32::Foundation::HANDLE>,
}

#[cfg(windows)]
unsafe impl Send for WindowsJob {}

#[cfg(windows)]
impl WindowsJob {
    fn attach(child: &Child) -> io::Result<Self> {
        use std::{mem::size_of, os::windows::io::AsRawHandle, ptr};
        use windows_sys::Win32::{
            Foundation::HANDLE,
            System::JobObjects::{
                AssignProcessToJobObject, CreateJobObjectW, JobObjectExtendedLimitInformation,
                SetInformationJobObject, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
                JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
            },
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
        let process_handle = child.as_raw_handle() as HANDLE;
        if unsafe { AssignProcessToJobObject(handle, process_handle) } == 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(job)
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
}
