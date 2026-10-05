//! OS integration with separate arguments; never interpolate user data into a shell.
use std::io;

pub fn open(target: impl AsRef<std::ffi::OsStr>) -> io::Result<()> {
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        use windows_sys::Win32::UI::Shell::ShellExecuteW;
        let target: Vec<u16> = target.as_ref().encode_wide().chain(Some(0)).collect();
        // SAFETY: the null-terminated target lives through the call; all optional
        // pointers are null. No shell command or argument string is constructed.
        let result = unsafe {
            ShellExecuteW(
                std::ptr::null_mut(),
                std::ptr::null(),
                target.as_ptr(),
                std::ptr::null(),
                std::ptr::null(),
                1,
            )
        } as isize;
        if result <= 32 {
            return Err(io::Error::other(format!(
                "Windows could not open the target (code {result})"
            )));
        }
        Ok(())
    }
    #[cfg(not(windows))]
    {
        let program = if cfg!(target_os = "macos") {
            "open"
        } else {
            "xdg-open"
        };
        let status = std::process::Command::new(program)
            .arg(target)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()?;
        if status.success() {
            Ok(())
        } else {
            Err(io::Error::other("The system opener failed"))
        }
    }
}
