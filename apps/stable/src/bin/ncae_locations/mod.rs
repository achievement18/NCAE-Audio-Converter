//! Detect installed CloudMusic separately from its per-user audio-effect data.
use anyhow::{bail, Result};
use std::path::{Path, PathBuf};

pub(super) struct Location {
    pub effect_dir: PathBuf,
    pub install_dir: Option<PathBuf>,
    pub note: String,
}

pub(super) fn discover(manual: Option<PathBuf>) -> Result<Location> {
    let install_dir = detect_install_dir();
    if let Some(effect_dir) = manual {
        if !effect_dir.is_dir() {
            bail!("指定的音效目录不存在或无法访问: {}", effect_dir.display());
        }
        return Ok(Location {
            effect_dir,
            install_dir,
            note: "使用手动指定的音效目录".into(),
        });
    }
    let mut candidates = Vec::new();
    // CloudMusic normally keeps downloaded effects in the user's profile, not Program Files.
    for variable in ["LOCALAPPDATA", "APPDATA"] {
        if let Some(base) = std::env::var_os(variable) {
            candidates.push(
                PathBuf::from(base)
                    .join("NetEase")
                    .join("CloudMusic")
                    .join("audioeffect"),
            );
        }
    }
    for data_dir in registered_data_dirs() {
        if data_dir
            .file_name()
            .is_some_and(|name| name.eq_ignore_ascii_case("audioeffect"))
        {
            candidates.push(data_dir);
        } else {
            candidates.push(data_dir.join("audioeffect"));
        }
    }
    if let Some(install) = &install_dir {
        candidates.extend([
            install.join("audioeffect"),
            install.join("data").join("audioeffect"),
            install
                .join("Data")
                .join("NetEase")
                .join("CloudMusic")
                .join("audioeffect"),
        ]);
    }
    candidates.dedup();
    let effect_dir = choose_effect_dir(&candidates)
        .or_else(|| candidates.first().cloned())
        .ok_or_else(|| anyhow::anyhow!("无法确定用户数据目录，请手动指定音效目录"))?;
    let note = if contains_effects(&effect_dir) {
        "已自动定位本地音效目录".to_owned()
    } else if effect_dir.is_dir() {
        "目录已找到，但尚无音效；请在网易云中下载后刷新".to_owned()
    } else if install_dir.is_some() {
        "已识别网易云安装位置；尚未发现音效目录，请下载音效后重新检测".to_owned()
    } else {
        "未识别网易云安装位置；请安装客户端或手动指定音效目录".to_owned()
    };
    Ok(Location {
        effect_dir,
        install_dir,
        note,
    })
}

fn contains_effects(path: &Path) -> bool {
    std::fs::read_dir(path).ok().is_some_and(|entries| {
        entries.flatten().any(|entry| {
            entry
                .path()
                .extension()
                .is_some_and(|extension| extension.eq_ignore_ascii_case("ncae"))
        })
    })
}

fn choose_effect_dir(candidates: &[PathBuf]) -> Option<PathBuf> {
    candidates
        .iter()
        .find(|path| contains_effects(path))
        .or_else(|| candidates.iter().find(|path| path.is_dir()))
        .cloned()
}

fn detect_install_dir() -> Option<PathBuf> {
    let mut candidates = registered_install_dirs();
    for variable in ["ProgramFiles", "ProgramFiles(x86)", "LOCALAPPDATA"] {
        if let Some(base) = std::env::var_os(variable) {
            candidates.push(PathBuf::from(base).join("NetEase").join("CloudMusic"));
        }
    }
    candidates
        .into_iter()
        .find(|directory| directory.join("cloudmusic.exe").is_file())
}

#[cfg(target_os = "windows")]
fn registry_string(
    root: windows_sys::Win32::System::Registry::HKEY,
    key: &str,
    value: &str,
) -> Option<String> {
    use windows_sys::Win32::System::Registry::*;
    let key: Vec<u16> = key.encode_utf16().chain(Some(0)).collect();
    let value: Vec<u16> = value.encode_utf16().chain(Some(0)).collect();
    let mut buffer = vec![0u16; 32768];
    let mut bytes = (buffer.len() * 2) as u32;
    // SAFETY: Bounded writable UTF-16 buffer, valid NUL-terminated key/value names;
    // predefined registry roots are borrowed, and RegGetValueW closes its own subkey.
    let status = unsafe {
        RegGetValueW(
            root,
            key.as_ptr(),
            value.as_ptr(),
            RRF_RT_REG_SZ | RRF_RT_REG_EXPAND_SZ,
            std::ptr::null_mut(),
            buffer.as_mut_ptr().cast(),
            &mut bytes,
        )
    };
    if status != 0 {
        return None;
    }
    let length = buffer
        .iter()
        .position(|&value| value == 0)
        .unwrap_or(buffer.len());
    let text = String::from_utf16_lossy(&buffer[..length]);
    let text = text.trim().trim_matches('"');
    if text.is_empty() {
        None
    } else {
        Some(text.to_owned())
    }
}

#[cfg(target_os = "windows")]
fn registry_paths(values: &[&str]) -> Vec<PathBuf> {
    use windows_sys::Win32::System::Registry::*;
    let mut found = Vec::new();
    for root in [HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE] {
        for key in [
            r"SOFTWARE\NetEase\CloudMusic",
            r"SOFTWARE\WOW6432Node\NetEase\CloudMusic",
        ] {
            for value in values {
                if let Some(path) = registry_string(root, key, value) {
                    found.push(PathBuf::from(path));
                }
            }
        }
    }
    found
}

#[cfg(target_os = "windows")]
fn registered_install_dirs() -> Vec<PathBuf> {
    use windows_sys::Win32::System::Registry::*;
    let mut found = registry_paths(&["install_dir", "InstallPath", "InstallLocation", "Path"]);
    for root in [HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE] {
        for key in [
            r"SOFTWARE\Microsoft\Windows\CurrentVersion\App Paths\cloudmusic.exe",
            r"SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\App Paths\cloudmusic.exe",
        ] {
            if let Some(executable) = registry_string(root, key, "") {
                if let Some(parent) = Path::new(&executable).parent() {
                    found.push(parent.to_path_buf());
                }
            }
        }
    }
    found
}

#[cfg(target_os = "windows")]
fn registered_data_dirs() -> Vec<PathBuf> {
    registry_paths(&["data_dir", "data_path", "userdata_dir", "user_data_dir"])
}
#[cfg(not(target_os = "windows"))]
fn registered_install_dirs() -> Vec<PathBuf> {
    Vec::new()
}
#[cfg(not(target_os = "windows"))]
fn registered_data_dirs() -> Vec<PathBuf> {
    Vec::new()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn populated_effect_directory_beats_an_empty_install_candidate() {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "ncae-location-test-{}-{}",
            std::process::id(),
            unique
        ));
        let empty = root.join("install").join("audioeffect");
        let populated = root.join("profile").join("audioeffect");
        std::fs::create_dir_all(&empty).unwrap();
        std::fs::create_dir_all(&populated).unwrap();
        std::fs::write(populated.join("example.NCAE"), b"fixture").unwrap();
        assert_eq!(
            choose_effect_dir(&[empty, populated.clone()]),
            Some(populated)
        );
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn missing_candidates_do_not_create_directories() {
        let missing = std::env::temp_dir().join("ncae-directory-that-does-not-exist-29612804");
        assert!(choose_effect_dir(&[missing.clone()]).is_none());
        assert!(!missing.exists());
    }
}
