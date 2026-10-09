//! Decryption research/algorithm: AllenHeartcore/audioeffect-ncm (GNU GPL v3.0).
//! https://github.com/AllenHeartcore/audioeffect-ncm — see docs/ATTRIBUTION.md.
//! Rust port and subsequent extensions; modified 2026-10-09. Not an upstream release.
//! NCAE 反向加密核心模块。
//!
//! NCAE 文件结构：
//! ```text
//! 4 字节  NCAE
//! 4 字节  加密数据长度 ldata，小端序
//! 8 字节  reserved / 版本 / 类型标志
//! 1 字节  key0 长度 = lkey + 1
//! N 字节  key0
//! ldata   加密数据区
//! ```
//!
//! 加密/解密：
//!   plain <-> raw Deflate <-> XOR keystream <-> NCAE container
//!
//! 仅用于个人学习和你自己合法拥有的文件。

use anyhow::{bail, Context, Result};
use flate2::read::DeflateDecoder;
use flate2::write::DeflateEncoder;
use flate2::Compression;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
pub const MAGIC: &[u8; 4] = b"NCAE";
pub const TYPE_JSON: u8 = 1;
pub const TYPE_WAV: u8 = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PayloadKind {
    Json,
    Wav,
    Binary,
}

impl PayloadKind {
    pub fn as_str(self) -> &'static str {
        match self {
            PayloadKind::Json => "json",
            PayloadKind::Wav => "wav",
            PayloadKind::Binary => "binary",
        }
    }
}

#[derive(Debug, Clone)]
pub struct NcaeFile {
    pub key: Vec<u8>,
    pub key0: Vec<u8>,
    pub reserved: [u8; 8],
    pub encrypted: Vec<u8>,
    pub plain: Vec<u8>,
    pub kind: PayloadKind,
}

impl NcaeFile {
    pub fn payload_type(&self) -> u8 {
        self.reserved[6]
    }
}

/// RC4 风格的 KSA：用 key 打乱 0..255 表。
fn make_table(key: &[u8]) -> Result<[u8; 256]> {
    if key.is_empty() {
        bail!("密钥不能为空");
    }

    let mut table = [0u8; 256];
    for (i, item) in table.iter_mut().enumerate() {
        *item = i as u8;
    }

    let mut b: usize = 0;
    for i in 0..256 {
        b = (table[i] as usize + key[i % key.len()] as usize + b) & 0xFF;
        table.swap(i, b);
    }
    Ok(table)
}

/// 逐字节 XOR。加解密使用同一个函数。
fn xor_stream(data: &[u8], table: &[u8; 256]) -> Vec<u8> {
    let mut out = data.to_vec();
    for (j, value) in out.iter_mut().enumerate() {
        let b = table[(j + 1) & 0xFF] as usize;
        let idx = (table[(b + j + 1) & 0xFF] as usize + b) & 0xFF;
        *value ^= table[idx];
    }
    out
}

/// 解析 NCAE 头，返回 (真实 key, key0, reserved, encrypted)。
pub fn parse_header(data: &[u8]) -> Result<(Vec<u8>, Vec<u8>, [u8; 8], Vec<u8>)> {
    if data.len() < 17 || &data[0..4] != MAGIC {
        bail!("不是有效的 NCAE 文件");
    }

    let ldata = u32::from_le_bytes(data[4..8].try_into().unwrap()) as usize;

    let mut reserved = [0u8; 8];
    reserved.copy_from_slice(&data[8..16]);

    let key_len_byte = data[16] as usize;
    if key_len_byte == 0 {
        bail!("无效的密钥长度");
    }
    let lkey = key_len_byte - 1;
    if lkey == 0 || lkey % 4 != 0 {
        bail!("无效的密钥长度: {}", lkey);
    }

    let key0_start = 17;
    let key0_end = key0_start + lkey + 1;
    if key0_end > data.len() {
        bail!("NCAE 密钥数据不完整");
    }
    let key0 = data[key0_start..key0_end].to_vec();

    // key0[4] 是掩码，真实 key = (key0 去掉第 5 字节) XOR key0[4]
    let mask = key0[4];
    let mut key = Vec::with_capacity(lkey);
    for &b in key0[..4].iter().chain(key0[5..].iter()) {
        key.push(b ^ mask);
    }

    let encrypted = data[key0_end..].to_vec();
    if encrypted.len() != ldata {
        bail!(
            "NCAE 数据长度不一致: 头部={} 实际={}",
            ldata,
            encrypted.len()
        );
    }

    Ok((key, key0, reserved, encrypted))
}

/// 解密 NCAE，返回 JSON / WAV / 二进制。
pub fn decrypt_ncae_bytes(data: &[u8]) -> Result<Vec<u8>> {
    let (key, _key0, _reserved, encrypted) = parse_header(data)?;
    let table = make_table(&key)?;
    let compressed = xor_stream(&encrypted, &table);

    let mut decoder = DeflateDecoder::new(&compressed[..]);
    let mut plain = Vec::new();
    decoder
        .read_to_end(&mut plain)
        .context("raw Deflate 解压失败")?;
    Ok(plain)
}

/// Bounded decoding for automatic read-only previews.
pub fn decrypt_ncae_bytes_limited(data: &[u8], maximum: usize) -> Result<Vec<u8>> {
    let (key, _key0, _reserved, encrypted) = parse_header(data)?;
    let table = make_table(&key)?;
    let compressed = xor_stream(&encrypted, &table);
    let decoder = DeflateDecoder::new(&compressed[..]);
    let mut plain = Vec::new();
    decoder
        .take(maximum.saturating_add(1) as u64)
        .read_to_end(&mut plain)
        .context("raw Deflate 解压失败")?;
    if plain.len() > maximum {
        bail!("解密内容超过 {} MiB 预览上限", maximum / 1024 / 1024);
    }
    Ok(plain)
}

/// 反向加密：plain -> raw Deflate -> XOR -> NCAE。
pub fn encrypt_ncae_bytes(
    key: &[u8],
    key0: &[u8],
    reserved: &[u8; 8],
    plain: &[u8],
) -> Result<Vec<u8>> {
    let mut encoder = DeflateEncoder::new(Vec::new(), Compression::best());
    encoder.write_all(plain)?;
    let compressed = encoder.finish()?;

    let table = make_table(key)?;
    let encrypted = xor_stream(&compressed, &table);

    let mut out = Vec::with_capacity(4 + 4 + 8 + 1 + key0.len() + encrypted.len());
    out.extend_from_slice(MAGIC);
    out.extend_from_slice(&(encrypted.len() as u32).to_le_bytes());
    out.extend_from_slice(reserved);
    out.push(key0.len() as u8);
    out.extend_from_slice(key0);
    out.extend_from_slice(&encrypted);
    Ok(out)
}

/// 判断解密载荷类型。
pub fn payload_kind(plain: &[u8]) -> PayloadKind {
    let stripped: Vec<u8> = plain
        .iter()
        .skip_while(|b| b.is_ascii_whitespace())
        .copied()
        .collect();

    if stripped.first() == Some(&b'{') {
        PayloadKind::Json
    } else if plain.starts_with(b"RIFF") {
        PayloadKind::Wav
    } else {
        PayloadKind::Binary
    }
}

/// 设置 reserved[6]：1=JSON，2=WAV。
pub fn set_payload_type(reserved: &[u8; 8], kind: PayloadKind) -> [u8; 8] {
    let mut r = *reserved;
    match kind {
        PayloadKind::Json => r[6] = TYPE_JSON,
        PayloadKind::Wav => r[6] = TYPE_WAV,
        PayloadKind::Binary => {}
    }
    r
}

/// 读取并解密 .ncae。
pub fn read_ncae(path: &Path) -> Result<NcaeFile> {
    let data = std::fs::read(path)?;
    let (key, key0, reserved, encrypted) = parse_header(&data)?;
    let plain = decrypt_ncae_bytes(&data)?;
    let kind = payload_kind(&plain);
    Ok(NcaeFile {
        key,
        key0,
        reserved,
        encrypted,
        plain,
        kind,
    })
}

/// 反向加密并写入 .ncae。
pub fn write_ncae(
    path: &Path,
    key: &[u8],
    key0: &[u8],
    reserved: &[u8; 8],
    plain: &[u8],
) -> Result<()> {
    let data = encrypt_ncae_bytes(key, key0, reserved, plain)?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, data)?;
    Ok(())
}

/// 往返测试：读取 -> 重新加密 -> 再解密，验证是否一致。
pub fn roundtrip_test(path: &Path) -> Result<bool> {
    let original = read_ncae(path)?;
    let rebuilt = encrypt_ncae_bytes(
        &original.key,
        &original.key0,
        &original.reserved,
        &original.plain,
    )?;
    Ok(decrypt_ncae_bytes(&rebuilt)? == original.plain)
}

pub mod wav;

use crate::wav::{build_variant, parse_wav, ChannelMode, LengthMode, WavData};

#[derive(Debug, Clone)]
pub struct EffectInfo {
    pub path: PathBuf,
    pub name: String,
    pub size: u64,
    pub kind: PayloadKind,
    pub detail: String,
}

/// Friendly filename matching the effect-library name; strip only NCAE download IDs.
pub fn export_stem(name: &str) -> String {
    let file = Path::new(name);
    let stem = file
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("音效");
    let stem = if file
        .extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case("ncae"))
    {
        match stem.rsplit_once('-') {
            Some((title, id))
                if !title.is_empty()
                    && id.len() >= 10
                    && id.bytes().all(|byte| byte.is_ascii_digit()) =>
            {
                title
            }
            _ => stem,
        }
    } else {
        stem
    };
    if stem.is_empty() {
        "音效".into()
    } else {
        stem.into()
    }
}

/// Exclusive, numbered output names: never overwrite a previous export or the input.
pub fn export_named_bytes(
    out_dir: &Path,
    name: &str,
    extension: &str,
    bytes: &[u8],
) -> Result<PathBuf> {
    write_export_bytes(out_dir, &export_stem(name), extension, bytes)
}

/// Source conversion keeps the complete source stem, including dots and numeric suffixes.
pub fn export_source_bytes(
    out_dir: &Path,
    source: &Path,
    extension: &str,
    bytes: &[u8],
) -> Result<PathBuf> {
    let stem = source
        .file_stem()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .unwrap_or("音频");
    write_export_bytes(out_dir, stem, extension, bytes)
}

fn write_export_bytes(
    out_dir: &Path,
    stem: &str,
    extension: &str,
    bytes: &[u8],
) -> Result<PathBuf> {
    if extension.is_empty() || !extension.bytes().all(|byte| byte.is_ascii_alphanumeric()) {
        bail!("输出扩展名无效");
    }
    std::fs::create_dir_all(out_dir)?;
    for index in 0..10000 {
        let filename = if index == 0 {
            format!("{stem}.{extension}")
        } else {
            format!("{stem} ({index}).{extension}")
        };
        let output = out_dir.join(filename);
        let mut file = match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&output)
        {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error).context("创建导出文件失败"),
        };
        let result = file.write_all(bytes).and_then(|_| file.sync_all());
        drop(file);
        if let Err(error) = result {
            let _ = std::fs::remove_file(&output);
            return Err(error).context("导出写入失败");
        }
        return Ok(output);
    }
    bail!("同名导出文件过多，请整理输出目录")
}

/// 导出解密后的载荷到 out_dir。
pub fn export_decrypted(info: &NcaeFile, out_dir: &Path) -> Result<PathBuf> {
    std::fs::create_dir_all(out_dir)?;
    let ext = match info.kind {
        PayloadKind::Json => "json",
        PayloadKind::Wav => "wav",
        PayloadKind::Binary => "bin",
    };
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let out = out_dir.join(format!("decrypted-{}.{}", ts, ext));
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&out)?;
    let result = file.write_all(&info.plain).and_then(|_| file.sync_all());
    drop(file);
    if let Err(error) = result {
        let _ = std::fs::remove_file(&out);
        return Err(error).context("写入解密输出失败");
    }
    Ok(out)
}

/// 备份 target 并用 generated 覆盖。
pub fn backup_and_replace(target: &Path, generated: &[u8], backup_root: &Path) -> Result<PathBuf> {
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let backup_dir = backup_root.join(ts.to_string());
    std::fs::create_dir_all(&backup_dir)?;
    let backup_path = backup_dir.join(target.file_name().unwrap_or_default());
    std::fs::copy(target, &backup_path)?;
    std::fs::write(target, generated)?;
    Ok(backup_path)
}

/// 读取并分析一个 .ncae。
pub fn inspect_effect(path: &Path) -> Result<EffectInfo> {
    let file = read_ncae(path)?;
    let detail = match file.kind {
        PayloadKind::Json => {
            let keys = serde_json::from_slice::<serde_json::Value>(&file.plain)
                .ok()
                .and_then(|v| {
                    v.as_object().map(|m| {
                        let mut k: Vec<String> = m.keys().cloned().collect();
                        k.sort();
                        k.join(", ")
                    })
                })
                .unwrap_or_else(|| "JSON 解析失败".to_string());
            format!("JSON 参数型: {}", keys)
        }
        PayloadKind::Wav => match parse_wav(&file.plain) {
            Ok(w) => format!(
                "WAV/IR 型: {}ch {}Hz {}bit {}帧 {:.1}ms",
                w.channels,
                w.sample_rate,
                w.bits,
                w.frames,
                w.duration_ms()
            ),
            Err(e) => format!("WAV 解析失败: {}", e),
        },
        PayloadKind::Binary => format!("未知二进制载荷: {} 字节", file.plain.len()),
    };
    Ok(EffectInfo {
        path: path.to_path_buf(),
        name: path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string(),
        size: std::fs::metadata(path)?.len(),
        kind: file.kind,
        detail,
    })
}

/// 列出目录下的 .ncae 文件。
pub fn list_effects(dir: &Path) -> Result<Vec<EffectInfo>> {
    let mut out = Vec::new();
    if !dir.exists() {
        return Ok(out);
    }
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path
            .extension()
            .and_then(|s| s.to_str())
            .map(|s| s.eq_ignore_ascii_case("ncae"))
            == Some(true)
        {
            match inspect_effect(&path) {
                Ok(info) => out.push(info),
                Err(e) => out.push(EffectInfo {
                    path: path.clone(),
                    name: path
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .to_string(),
                    size: std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0),
                    kind: PayloadKind::Binary,
                    detail: format!("读取失败: {}", e),
                }),
            }
        }
    }
    out.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    Ok(out)
}

/// Read a WAV or RIFF/WAVE-based IRS impulse response without renaming the source.
/// Proprietary IRS containers are rejected rather than guessed or encrypted as opaque bytes.
pub fn read_impulse_file(path: &Path) -> Result<WavData> {
    let data =
        std::fs::read(path).with_context(|| format!("读取脉冲响应失败: {}", path.display()))?;
    let is_irs = path
        .extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case("irs"));
    if is_irs && (data.len() < 12 || &data[..4] != b"RIFF" || &data[8..12] != b"WAVE") {
        bail!("此 IRS 不是 RIFF/WAVE 封装，当前无法适配。请提供样本或先导出为 WAV；不会将未知 IRS 直接封装进 NCAE");
    }
    parse_wav(&data).with_context(|| {
        if is_irs {
            "IRS 内部音频解析失败"
        } else {
            "WAV 音频解析失败"
        }
    })
}

/// 用模板的 key/key0/reserved 生成新 NCAE。
pub fn generate_from_template(
    template: &NcaeFile,
    input: &Path,
    mode: LengthMode,
    channel_mode: ChannelMode,
    do_peak_match: bool,
) -> Result<(Vec<u8>, String)> {
    let ext = input
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();

    if ext == "json" {
        let plain = std::fs::read(input)?;
        serde_json::from_slice::<serde_json::Value>(&plain)
            .map_err(|e| anyhow::anyhow!("JSON 文件无效: {}", e))?;
        let reserved = set_payload_type(&template.reserved, PayloadKind::Json);
        let out = encrypt_ncae_bytes(&template.key, &template.key0, &reserved, &plain)?;
        return Ok((out, "JSON 参数型".to_string()));
    }

    if ext == "wav" || ext == "wave" || ext == "irs" {
        let input_wav = read_impulse_file(input)?;
        let template_wav = if template.kind == PayloadKind::Wav {
            parse_wav(&template.plain).ok()
        } else {
            None
        };
        let plain = build_variant(
            &input_wav,
            template_wav.as_ref(),
            mode,
            channel_mode,
            do_peak_match,
        );
        let reserved = set_payload_type(&template.reserved, PayloadKind::Wav);
        let out = encrypt_ncae_bytes(&template.key, &template.key0, &reserved, &plain)?;
        let label = format!(
            "{} ({:?}, {:?}, peak_match={})",
            if ext == "irs" {
                "IRS → WAV/IR 型"
            } else {
                "WAV/IR 型"
            },
            mode,
            channel_mode,
            do_peak_match
        );
        return Ok((out, label));
    }

    let plain = std::fs::read(input)?;
    let kind = payload_kind(&plain);
    let reserved = set_payload_type(&template.reserved, kind);
    let out = encrypt_ncae_bytes(&template.key, &template.key0, &reserved, &plain)?;
    Ok((out, format!("原始载荷: {}", kind.as_str())))
}

// ---------------------------------------------------------------------------
// 替换记录 / 永久原备份 / 恢复
// ---------------------------------------------------------------------------

fn now_string() -> String {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
        .to_string()
}

fn load_records(records_path: &Path) -> serde_json::Value {
    std::fs::read_to_string(records_path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_else(|| serde_json::json!({}))
}

fn save_records(records_path: &Path, records: &serde_json::Value) -> Result<()> {
    if let Some(parent) = records_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let tmp = records_path.with_extension("json.tmp");
    std::fs::write(&tmp, serde_json::to_vec_pretty(records)?)?;
    std::fs::rename(&tmp, records_path)?;
    Ok(())
}

#[derive(Debug, Clone)]
pub struct ReplaceResult {
    pub backup: Option<PathBuf>,
    pub created_backup: bool,
}

/// 带本地记录的备份并替换。
///
/// 规则：
/// - 首次替换：把原文件复制到 backup/original/<文件名>，永不覆盖。
/// - 之后替换：如果记录里已有原备份，直接覆盖目标，不再重复备份。
/// - allow_no_backup=true：不创建新备份；已有备份仍保留。
pub fn backup_and_replace_recorded(
    target: &Path,
    generated: &[u8],
    backup_root: &Path,
    records_path: &Path,
    source: &Path,
    mode: &str,
    channels: &str,
    peak_match: bool,
    allow_no_backup: bool,
) -> Result<ReplaceResult> {
    let mut records = load_records(records_path);
    if !records.is_object() {
        records = serde_json::json!({});
    }

    let key = target.to_string_lossy().to_string();
    let existing = records.get(&key).cloned();

    let mut backup_path: Option<PathBuf> = existing
        .as_ref()
        .and_then(|v| v.get("backup"))
        .and_then(|v| v.as_str())
        .map(PathBuf::from);

    if let Some(b) = &backup_path {
        if !b.exists() {
            backup_path = None;
        }
    }

    let mut created_backup = false;

    if !allow_no_backup && backup_path.is_none() {
        let original_dir = backup_root.join("original");
        std::fs::create_dir_all(&original_dir)?;
        let candidate = original_dir.join(target.file_name().unwrap_or_default());
        if !candidate.exists() {
            std::fs::copy(target, &candidate)?;
            created_backup = true;
        }
        backup_path = Some(candidate);
    }

    // 真正覆盖目标文件
    std::fs::write(target, generated)?;

    let now = now_string();
    let first = existing
        .as_ref()
        .and_then(|v| v.get("first_replaced_at"))
        .and_then(|v| v.as_str())
        .unwrap_or(&now)
        .to_string();
    let count = existing
        .as_ref()
        .and_then(|v| v.get("replace_count"))
        .and_then(|v| v.as_u64())
        .unwrap_or(0)
        + 1;

    let record = serde_json::json!({
        "target": key,
        "backup": backup_path.as_ref().map(|p| p.to_string_lossy().to_string()),
        "source": source.to_string_lossy().to_string(),
        "mode": mode,
        "channels": channels,
        "peak_match": peak_match,
        "first_replaced_at": first,
        "last_replaced_at": now,
        "replace_count": count,
    });

    if let Some(obj) = records.as_object_mut() {
        obj.insert(key, record);
    }
    save_records(records_path, &records)?;

    Ok(ReplaceResult {
        backup: backup_path,
        created_backup,
    })
}

/// 从永久原备份恢复目标文件。
pub fn restore_recorded(target: &Path, records_path: &Path) -> Result<PathBuf> {
    let key = target.to_string_lossy().to_string();
    let mut records = load_records(records_path);
    let backup = records
        .get(&key)
        .and_then(|v| v.get("backup"))
        .and_then(|v| v.as_str())
        .map(PathBuf::from);

    let Some(backup) = backup else {
        bail!("没有找到原音效备份记录");
    };
    if !backup.exists() {
        bail!("备份文件不存在: {}", backup.display());
    }

    std::fs::copy(&backup, target)?;

    if let Some(obj) = records.get_mut(&key).and_then(|v| v.as_object_mut()) {
        obj.insert("restored_at".to_string(), serde_json::json!(now_string()));
    }
    save_records(records_path, &records)?;
    Ok(backup)
}

/// 读取某个目标的替换记录。
pub fn get_record(target: &Path, records_path: &Path) -> Option<serde_json::Value> {
    let key = target.to_string_lossy().to_string();
    load_records(records_path).get(&key).cloned()
}

#[cfg(test)]
mod irs_tests;
