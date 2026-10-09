//! Portable, first-original backups beside the target. No new records in the EXE directory.
use anyhow::{bail, Context, Result};
use ncae_tool::ReplaceResult;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

// Stable FNV-1a content identifier, not a cryptographic authentication mechanism.
fn fingerprint(bytes: &[u8]) -> String {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in bytes {
        hash = (hash ^ u64::from(*byte)).wrapping_mul(0x100000001b3);
    }
    format!("{hash:016X}")
}

fn prefix(target: &Path) -> Result<String> {
    let stem = target
        .file_stem()
        .and_then(|name| name.to_str())
        .context("目标音效文件名无效")?;
    Ok(format!("{stem}.original-"))
}

fn parent(target: &Path) -> Result<&Path> {
    target
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .context("目标音效没有有效的父目录")
}

fn verified_bytes(path: &Path) -> Result<Vec<u8>> {
    let bytes = fs::read(path).with_context(|| format!("无法读取备份: {}", path.display()))?;
    ncae_tool::decrypt_ncae_bytes(&bytes).context("备份不是完整可解密的 NCAE 文件，已停止操作")?;
    Ok(bytes)
}

fn adjacent_backup(target: &Path) -> Result<Option<PathBuf>> {
    let prefix = prefix(target)?;
    let mut matches = Vec::new();
    for entry in fs::read_dir(parent(target)?).context("读取音效目录失败")? {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.len() < prefix.len()
            || !name
                .get(..prefix.len())
                .is_some_and(|start| start.eq_ignore_ascii_case(&prefix))
        {
            continue;
        }
        let suffix = &name[prefix.len()..];
        let Some(code) = suffix.strip_suffix(".bak") else {
            continue;
        };
        if code.len() != 16 || !code.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            continue;
        }
        let bytes = verified_bytes(&entry.path())?;
        if !fingerprint(&bytes).eq_ignore_ascii_case(code) {
            bail!(
                "备份特征码不匹配，可能已被修改，已停止操作: {}",
                entry.path().display()
            );
        }
        matches.push(entry.path());
    }
    if matches.len() > 1 {
        bail!("发现多个对应的 .bak 原备份，请确认应使用哪一份后再操作；不会自动覆盖或猜测");
    }
    Ok(matches.pop())
}

/// Legacy metadata is read-only. Preserve the real original instead of backing up an already replaced file.
fn legacy_backup(target: &Path, records_path: &Path) -> Result<Option<PathBuf>> {
    let text = match fs::read_to_string(records_path) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error).context("读取旧版备份记录失败"),
    };
    let records: serde_json::Value =
        serde_json::from_str(&text).context("旧版备份记录损坏，无法安全确定原备份")?;
    if !records.is_object() {
        bail!("旧版备份记录格式错误，已停止操作");
    }
    let key = target.to_string_lossy();
    let record = records.get(key.as_ref()).or_else(|| {
        records
            .as_object()?
            .iter()
            .find(|(path, _)| path.eq_ignore_ascii_case(key.as_ref()))
            .map(|(_, value)| value)
    });
    let Some(path) = record
        .and_then(|value| value.get("backup"))
        .and_then(|value| value.as_str())
    else {
        return Ok(None);
    };
    let stored = PathBuf::from(path);
    let path = if stored.is_absolute() {
        stored
    } else {
        records_path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join(stored)
    };
    verified_bytes(&path)
        .context("旧版原备份无法读取；请先找回备份，避免把已替换的音效误存为原版")?;
    Ok(Some(path))
}

fn create_original(target: &Path, bytes: &[u8]) -> Result<PathBuf> {
    let backup = parent(target)?.join(format!("{}{}.bak", prefix(target)?, fingerprint(bytes)));
    // create_new prevents accidental overwrite, even if another process races with this one.
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&backup)
        .with_context(|| format!("无法新建原备份，未替换目标文件: {}", backup.display()))?;
    let write_result = file.write_all(bytes).and_then(|_| file.sync_all());
    drop(file);
    if let Err(error) = write_result {
        let _ = fs::remove_file(&backup); // Roll back only the file created by this call.
        return Err(error).context("备份写入失败，未替换目标文件");
    }
    if fs::read(&backup)? != bytes {
        bail!("备份写入校验失败，未替换目标文件");
    }
    Ok(backup)
}

fn replace_target(target: &Path, bytes: &[u8]) -> Result<()> {
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let temp = parent(target)?.join(format!(".ncae-write-{}-{nonce}.tmp", std::process::id()));
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temp)
        .context("创建临时写入文件失败")?;
    let write_result = file.write_all(bytes).and_then(|_| file.sync_all());
    drop(file);
    let result = write_result.and_then(|_| fs::rename(&temp, target));
    if let Err(error) = result {
        let _ = fs::remove_file(&temp); // Only our unique temporary file is removed.
        return Err(error).context("替换文件失败，原备份仍保留；请检查文件占用或权限");
    }
    Ok(())
}

pub(super) fn replace(
    target: &Path,
    generated: &[u8],
    legacy_records: &Path,
    make_backup: bool,
) -> Result<ReplaceResult> {
    let mut backup = adjacent_backup(target)?;
    let mut created_backup = false;
    if make_backup && backup.is_none() {
        let original = if let Some(legacy) = legacy_backup(target, legacy_records)? {
            verified_bytes(&legacy)?
        } else {
            verified_bytes(target)?
        };
        backup = Some(create_original(target, &original)?);
        created_backup = true;
    }
    replace_target(target, generated)?;
    Ok(ReplaceResult {
        backup,
        created_backup,
    })
}

pub(super) fn restore(target: &Path, legacy_records: &Path) -> Result<PathBuf> {
    let backup = match adjacent_backup(target)? {
        Some(path) => path,
        None => legacy_backup(target, legacy_records)?
            .context("未找到对应的 .bak 原备份或可用的旧版备份")?,
    };
    let original = verified_bytes(&backup)?;
    replace_target(target, &original)?;
    Ok(backup)
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> Self {
            static SERIAL: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
            let serial = SERIAL.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            let nonce = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let root = std::env::temp_dir().join(format!(
                "ncae-adjacent-test-{}-{nonce}-{serial}",
                std::process::id()
            ));
            fs::create_dir_all(&root).unwrap();
            Self(root)
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    fn data(gain: u8) -> Vec<u8> {
        ncae_tool::encrypt_ncae_bytes(
            &[1, 2, 3, 4],
            &[1, 2, 3, 4, 0],
            &[0, 0, 0, 0, 1, 0, 1, 0],
            format!("{{\"gain\":{gain}}}").as_bytes(),
        )
        .unwrap()
    }
    #[test]
    fn fingerprint_is_stable_and_backups_keep_the_first_original() {
        assert_eq!(fingerprint(b"hello"), "A430D84680AABD0B");
        let root = Fixture::new();
        let target = root.0.join("音乐厅-123.ncae");
        let records = root.0.join("absent.json");
        fs::write(&target, data(1)).unwrap();
        let first = replace(&target, &data(2), &records, true).unwrap();
        let backup = first.backup.unwrap();
        assert!(first.created_backup);
        assert_eq!(backup.parent(), target.parent());
        assert_eq!(backup.extension().unwrap(), "bak");
        let second = replace(&target, &data(3), &records, true).unwrap();
        assert!(!second.created_backup);
        assert_eq!(second.backup.as_ref(), Some(&backup));
        assert_eq!(fs::read(&backup).unwrap(), data(1));
        assert!(!records.exists());
        restore(&target, &records).unwrap();
        assert_eq!(fs::read(&target).unwrap(), data(1));
    }
    #[test]
    fn old_original_is_copied_next_to_target_without_touching_legacy_files() {
        let root = Fixture::new();
        let target = root.0.join("target.ncae");
        let legacy = root.0.join("legacy.ncae");
        let records = root.0.join("records.json");
        fs::write(&legacy, data(1)).unwrap();
        fs::write(&target, data(2)).unwrap();
        let json = serde_json::json!({target.to_string_lossy().to_string(): {"backup": legacy.to_string_lossy()}});
        let bytes = serde_json::to_vec(&json).unwrap();
        fs::write(&records, &bytes).unwrap();
        let result = replace(&target, &data(3), &records, true).unwrap();
        assert_eq!(fs::read(result.backup.unwrap()).unwrap(), data(1));
        assert_eq!(fs::read(&legacy).unwrap(), data(1));
        assert_eq!(fs::read(&records).unwrap(), bytes);
    }
    #[test]
    fn moved_folder_needs_no_software_directory_or_metadata() {
        let old = Fixture::new();
        let new = Fixture::new();
        let target = old.0.join("target.ncae");
        let missing = old.0.join("none.json");
        fs::write(&target, data(1)).unwrap();
        let result = replace(&target, &data(2), &missing, true).unwrap();
        let backup = result.backup.unwrap();
        fs::copy(&backup, new.0.join(backup.file_name().unwrap())).unwrap();
        let moved = new.0.join("target.ncae");
        fs::write(&moved, data(2)).unwrap();
        restore(&moved, &new.0.join("none.json")).unwrap();
        assert_eq!(fs::read(&moved).unwrap(), data(1));
    }
    #[test]
    fn corrupted_backup_blocks_restore_and_new_replacement() {
        let root = Fixture::new();
        let target = root.0.join("target.ncae");
        let records = root.0.join("none.json");
        fs::write(&target, data(1)).unwrap();
        let backup = replace(&target, &data(2), &records, true)
            .unwrap()
            .backup
            .unwrap();
        fs::write(&backup, data(9)).unwrap();
        assert!(restore(&target, &records).is_err());
        assert!(replace(&target, &data(3), &records, true).is_err());
        assert_eq!(fs::read(&target).unwrap(), data(2));
    }
    #[test]
    fn explicit_no_backup_creates_no_backup_or_records() {
        let root = Fixture::new();
        let target = root.0.join("target.ncae");
        let records = root.0.join("none.json");
        fs::write(&target, data(1)).unwrap();
        assert!(replace(&target, &data(2), &records, false)
            .unwrap()
            .backup
            .is_none());
        assert_eq!(fs::read_dir(&root.0).unwrap().count(), 1);
        assert_eq!(fs::read(&target).unwrap(), data(2));
    }
    #[test]
    fn ambiguity_is_not_silently_resolved() {
        let root = Fixture::new();
        let target = root.0.join("target.ncae");
        let records = root.0.join("none.json");
        fs::write(&target, data(1)).unwrap();
        create_original(&target, &data(1)).unwrap();
        create_original(&target, &data(2)).unwrap();
        assert!(restore(&target, &records).is_err());
        assert_eq!(fs::read(&target).unwrap(), data(1));
    }
    #[test]
    fn portable_relative_legacy_backup_paths_are_supported() {
        let fixture = Fixture::new();
        let audio = fixture.0.join("audio");
        let app = fixture.0.join("app");
        fs::create_dir_all(&audio).unwrap();
        fs::create_dir_all(app.join("legacy-backups")).unwrap();
        let target = audio.join("target.ncae");
        fs::write(&target, data(2)).unwrap();
        let backup = app.join("legacy-backups/original.bak");
        fs::write(&backup, data(1)).unwrap();
        let records = app.join("replacement_records.json");
        let record = serde_json::json!({target.to_string_lossy().to_string():{"backup":"legacy-backups/original.bak"}});
        fs::write(&records, serde_json::to_vec(&record).unwrap()).unwrap();
        restore(&target, &records).unwrap();
        assert_eq!(fs::read(target).unwrap(), data(1));
    }
}
