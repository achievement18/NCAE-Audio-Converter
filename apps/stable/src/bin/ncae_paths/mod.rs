use std::path::PathBuf;

pub(super) fn downloads_directory() -> Option<PathBuf> {
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::ffi::OsStringExt;
        use windows_sys::Win32::System::Com::CoTaskMemFree;
        use windows_sys::Win32::UI::Shell::{FOLDERID_Downloads, SHGetKnownFolderPath};
        let mut pointer = std::ptr::null_mut();
        // SAFETY: Windows allocates a NUL-terminated UTF-16 path; free it with CoTaskMemFree.
        let status = unsafe {
            SHGetKnownFolderPath(&FOLDERID_Downloads, 0, std::ptr::null_mut(), &mut pointer)
        };
        if !pointer.is_null() {
            let result = if status >= 0 {
                let mut length = 0;
                while length < 32768 && unsafe { *pointer.add(length) } != 0 {
                    length += 1;
                }
                if length < 32768 {
                    Some(PathBuf::from(std::ffi::OsString::from_wide(unsafe {
                        std::slice::from_raw_parts(pointer, length)
                    })))
                } else {
                    None
                }
            } else {
                None
            };
            unsafe { CoTaskMemFree(pointer.cast()) };
            if result.is_some() {
                return result;
            }
        }
    }
    std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .map(|home| PathBuf::from(home).join("Downloads"))
}

#[cfg(test)]
mod tests {
    #[test]
    fn downloads_path_is_absolute_without_creating_any_files() {
        let path = super::downloads_directory().expect("a user downloads directory");
        assert!(path.is_absolute());
    }
}
