//! On-demand runtime extraction for the single-file Windows release.
//! Cache contents are compared byte-for-byte with the embedded archive before use.
use anyhow::{bail, Context, Result};
use flate2::read::ZlibDecoder;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

static ARCHIVE: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/embedded-backend.bin"));
static HOME: Mutex<Option<PathBuf>> = Mutex::new(None);
const MAX_TOTAL: u64 = 512 * 1024 * 1024;

fn ordinary_path(path: &Path) -> Result<()> {
    for ancestor in path.ancestors() {
        match std::fs::symlink_metadata(ancestor) {
            Ok(metadata) => {
                #[cfg(windows)]
                {
                    use std::os::windows::fs::MetadataExt;
                    if metadata.file_attributes() & 0x400 != 0 {
                        bail!("运行时缓存不能位于链接目录: {}", ancestor.display());
                    }
                }
                if metadata.file_type().is_symlink() {
                    bail!("运行时缓存不能位于链接目录: {}", ancestor.display());
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
    }
    Ok(())
}

fn safe_relative(name: &str) -> Result<PathBuf> {
    if name.is_empty() || name.contains(['\\', ':', '\0']) {
        bail!("无效运行时文件名");
    }
    let parts: Vec<_> = name.split('/').collect();
    if parts
        .iter()
        .any(|part| part.is_empty() || *part == "." || *part == ".." || part.ends_with(['.', ' ']))
    {
        bail!("运行时文件路径超出允许范围");
    }
    if !matches!(parts[0], "runtime" | "converter" | "licenses" | "docs") {
        bail!("无效运行时目录");
    }
    Ok(parts.iter().collect())
}

fn visit_archive(archive: &[u8], mut visit: impl FnMut(&Path, &[u8]) -> Result<()>) -> Result<()> {
    let mut reader = ZlibDecoder::new(archive).take(MAX_TOTAL + 1);
    let mut header = [0u8; 12];
    reader.read_exact(&mut header).context("内嵌运行时损坏")?;
    if &header[..8] != b"NCAERUN1" {
        bail!("内嵌运行时版本无效");
    }
    let count = u32::from_le_bytes(header[8..12].try_into().unwrap());
    if count == 0 || count > 20000 {
        bail!("运行时文件数量无效");
    }
    let mut total = 12u64;
    let mut names = std::collections::HashSet::new();
    for _ in 0..count {
        let mut lengths = [0u8; 12];
        reader.read_exact(&mut lengths)?;
        let name_len = u32::from_le_bytes(lengths[..4].try_into().unwrap()) as usize;
        let size = u64::from_le_bytes(lengths[4..].try_into().unwrap());
        if name_len == 0 || name_len > 4096 || size > 128 * 1024 * 1024 {
            bail!("运行时条目过大");
        }
        total = total
            .checked_add(12 + name_len as u64 + size)
            .context("运行时大小溢出")?;
        if total > MAX_TOTAL {
            bail!("运行时解压超过安全上限");
        }
        let mut name = vec![0; name_len];
        reader.read_exact(&mut name)?;
        let name = std::str::from_utf8(&name)?;
        let relative = safe_relative(name)?;
        if !names.insert(name.to_lowercase()) {
            bail!("运行时包含重复路径");
        }
        let mut data = vec![0; size as usize];
        reader.read_exact(&mut data)?;
        visit(&relative, &data)?;
    }
    let mut extra = [0];
    if reader.read(&mut extra)? != 0 {
        bail!("运行时尾部有未知数据");
    }
    Ok(())
}

fn matches_archive(root: &Path) -> Result<bool> {
    if !root.is_dir() {
        return Ok(false);
    }
    ordinary_path(root)?;
    let mut expected = std::collections::HashSet::new();
    let mut matched = true;
    visit_archive(ARCHIVE, |relative, bytes| {
        let path = root.join(relative);
        ordinary_path(&path)?;
        expected.insert(relative.to_path_buf());
        match std::fs::metadata(&path) {
            Ok(meta) if meta.is_file() && meta.len() == bytes.len() as u64 => {
                if std::fs::read(path)? != bytes {
                    matched = false;
                }
            }
            _ => matched = false,
        }
        Ok(())
    })?;
    // Unexpected Python modules/DLLs must not be loaded from an altered cache.
    fn walk(root: &Path, dir: &Path, files: &mut std::collections::HashSet<PathBuf>) -> Result<()> {
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            ordinary_path(&entry.path())?;
            if entry.file_type()?.is_dir() {
                walk(root, &entry.path(), files)?;
            } else {
                files.insert(entry.path().strip_prefix(root)?.to_path_buf());
            }
        }
        Ok(())
    }
    let mut actual = std::collections::HashSet::new();
    walk(root, root, &mut actual)?;
    Ok(matched && expected == actual)
}

pub fn home() -> Result<Option<PathBuf>> {
    if ARCHIVE.is_empty() {
        return Ok(None);
    }
    let mut cached = HOME
        .lock()
        .map_err(|_| anyhow::anyhow!("运行时初始化锁异常"))?;
    if let Some(path) = cached.as_ref() {
        return Ok(Some(path.clone()));
    }
    let base = std::env::var_os("LOCALAPPDATA").context("无法定位 LOCALAPPDATA 运行时缓存目录")?;
    // Identifier only; trust comes from full byte comparison, not this non-cryptographic hash.
    let id = ARCHIVE.iter().fold(0xcbf29ce484222325u64, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(0x100000001b3)
    });
    let root = PathBuf::from(base)
        .join("NCAEAudioConverter")
        .join("runtime")
        .join(format!("{}-{id:016x}", env!("CARGO_PKG_VERSION")));
    ordinary_path(&root)?;
    std::fs::create_dir_all(&root).context("不能创建运行时缓存，请检查磁盘与写入权限")?;
    // Only complete generations are discoverable. Interrupted staging directories are ignored.
    for entry in std::fs::read_dir(&root)? {
        let entry = entry?;
        if entry.file_name().to_string_lossy().starts_with("ready-")
            && matches_archive(&entry.path())?
        {
            let path = entry.path();
            *cached = Some(path.clone());
            return Ok(Some(path));
        }
    }
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_nanos();
    let suffix = format!("{}-{nonce}", std::process::id());
    let staging = root.join(format!("staging-{suffix}"));
    std::fs::create_dir(&staging)?;
    visit_archive(ARCHIVE, |relative, bytes| {
        let path = staging.join(relative);
        ordinary_path(&path)?;
        std::fs::create_dir_all(path.parent().context("无效运行时路径")?)?;
        use std::io::Write;
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)?;
        file.write_all(bytes)?;
        Ok(())
    })?;
    if !staging.join("runtime/python.exe").is_file()
        || !staging.join("converter/bridge.py").is_file()
    {
        bail!("内嵌运行时缺少必要文件");
    }
    let ready = root.join(format!("ready-{suffix}"));
    std::fs::rename(&staging, &ready)?;
    *cached = Some(ready.clone());
    Ok(Some(ready))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn refuses_unsafe_archive_paths() {
        for name in [
            "../bad",
            "runtime/../bad",
            "/runtime/a",
            "runtime//a",
            "runtime/a:stream",
            "runtime/a\\b",
            "runtime/a.",
            "unknown/file",
            "runtime/./a",
        ] {
            assert!(safe_relative(name).is_err(), "{name}");
        }
        assert_eq!(
            safe_relative("runtime/Lib/os.py").unwrap(),
            PathBuf::from("runtime/Lib/os.py")
        );
    }
    #[test]
    fn rejects_corrupt_archive() {
        assert!(visit_archive(b"not zlib", |_, _| Ok(())).is_err());
    }
    #[test]
    fn bundled_archive_is_bounded_and_complete() {
        if ARCHIVE.is_empty() {
            return;
        }
        let mut python = false;
        let mut bridge = false;
        visit_archive(ARCHIVE, |path, _| {
            python |= path == Path::new("runtime/python.exe");
            bridge |= path == Path::new("converter/bridge.py");
            Ok(())
        })
        .unwrap();
        assert!(python && bridge);
    }
}
