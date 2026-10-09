//! Optional multi-format branch backend. The stable branch does not contain this module.
use anyhow::{bail, Context, Result};
use std::path::{Path, PathBuf};
use std::process::Command;

pub const INPUT_EXTENSIONS: &[&str] = &[
    "wav", "wave", "irs", "flac", "aif", "aiff", "au", "snd", "caf", "w64", "rf64", "json", "npz",
];

pub fn is_sample_json(path: &Path) -> Result<bool> {
    if std::fs::metadata(path)?.len() > 128 * 1024 * 1024 {
        bail!("JSON 文件过大，支线上限为 128 MiB");
    }
    let value: serde_json::Value =
        serde_json::from_slice(&std::fs::read(path)?).context("JSON 格式无效")?;
    let schema = value.get("schema").and_then(|value| value.as_str());
    if schema.is_some_and(|value| value.starts_with("ir.samples.") && value != "ir.samples.v1") {
        bail!("不支持的 IR 采样 JSON schema");
    }
    Ok(schema == Some("ir.samples.v1"))
}

pub fn is_audio_input(path: &Path) -> Result<bool> {
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    if extension == "json" {
        return is_sample_json(path);
    }
    Ok(INPUT_EXTENSIONS.contains(&extension.as_str()))
}

fn backend_home() -> Result<PathBuf> {
    let executable = std::env::current_exe()?;
    for parent in executable
        .parent()
        .context("无法定位程序目录")?
        .ancestors()
        .take(4)
    {
        for candidate in [parent.to_path_buf(), parent.join("dist")] {
            if candidate.join("runtime/python.exe").is_file()
                && candidate.join("converter/bridge.py").is_file()
            {
                return Ok(candidate);
            }
        }
    }
    bail!("缺少多格式支线的独立转换运行时，请完整复制 dist 目录（包含 runtime 和 converter），不能只复制 EXE")
}

pub fn convert_to_wav(source: &Path, output: &Path, float32: bool) -> Result<serde_json::Value> {
    if !is_audio_input(source)? {
        bail!("这是参数 JSON，不是 ir.samples.v1 采样数据，不能直接转换为 WAV");
    }
    let home = backend_home()?;
    let mut command = Command::new(home.join("runtime/python.exe"));
    command
        .arg("-I")
        .arg("-B")
        .arg("-X")
        .arg("utf8")
        .arg(home.join("converter/bridge.py"))
        .arg(source)
        .arg(output);
    if float32 {
        command.args(["--subtype", "FLOAT"]);
    }
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    let result = command.output().context("启动支线转换后端失败")?;
    if !result.status.success() {
        bail!(
            "多格式转换失败: {}",
            String::from_utf8_lossy(&result.stderr).trim()
        );
    }
    serde_json::from_slice(&result.stdout).context("转换后端返回了无效结果")
}

pub fn read_as_wav(path: &Path) -> Result<crate::wav::WavData> {
    struct Scratch(PathBuf);
    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(self.0.join("normalized.wav"));
            let _ = std::fs::remove_dir(&self.0);
        }
    }
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let directory =
        std::env::temp_dir().join(format!("ncae-multiformat-{}-{nonce}", std::process::id()));
    std::fs::create_dir(&directory)?;
    let scratch = Scratch(directory);
    let output = scratch.0.join("normalized.wav");
    convert_to_wav(path, &output, true)?;
    crate::wav::parse_wav(&std::fs::read(output)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixtures() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
    }
    #[test]
    fn all_reference_formats_decode_for_one_two_and_four_channels() {
        let root = fixtures();
        let expected: serde_json::Value =
            serde_json::from_slice(&std::fs::read(root.join("expected.json")).unwrap()).unwrap();
        for entry in expected.as_array().unwrap() {
            let input = root.join(entry["path"].as_str().unwrap());
            let wav = crate::read_impulse_file(&input)
                .unwrap_or_else(|error| panic!("{}: {error:#}", input.display()));
            assert_eq!(
                wav.sample_rate,
                entry["sample_rate"].as_u64().unwrap() as u32
            );
            assert_eq!(wav.channels, entry["channels"].as_u64().unwrap() as u16);
            let samples: Vec<f32> = entry["samples"]
                .as_array()
                .unwrap()
                .iter()
                .map(|value| value.as_f64().unwrap() as f32)
                .collect();
            assert_eq!(wav.samples, samples, "{}", input.display());
        }
    }
    #[test]
    fn sample_json_and_npz_generate_audio_ncae_not_parameter_payloads() {
        let template = crate::NcaeFile {
            key: vec![1, 2, 3, 4],
            key0: vec![1, 2, 3, 4, 0],
            reserved: [0, 0, 0, 0, 1, 0, 2, 0],
            encrypted: Vec::new(),
            plain: crate::wav::write_wav_float32(&[0.1, -0.1], 48000, 1),
            kind: crate::PayloadKind::Wav,
        };
        for name in ["input_2.json", "input_2.npz", "input_2.flac"] {
            let (data, _) = crate::generate_from_template(
                &template,
                &fixtures().join(name),
                crate::wav::LengthMode::Full,
                crate::wav::ChannelMode::Keep,
                false,
            )
            .unwrap();
            let (_, _, header, _) = crate::parse_header(&data).unwrap();
            assert_eq!(header[6], crate::TYPE_WAV);
            let audio = crate::wav::parse_wav(&crate::decrypt_ncae_bytes(&data).unwrap()).unwrap();
            assert_eq!(audio.channels, 2);
            assert_eq!(audio.frames, 3);
        }
    }
    #[test]
    fn arbitrary_json_is_not_an_audio_input() {
        let path =
            std::env::temp_dir().join(format!("ncae-format-preset-{}.json", std::process::id()));
        std::fs::write(&path, b"{\"eq\":{\"gain\":2}}").unwrap();
        assert!(!is_audio_input(&path).unwrap());
        let _ = std::fs::remove_file(path);
    }
}

/// Export with a friendly, non-overwriting filename while preserving source encoding.
pub fn export_source_wav(source: &Path, directory: &Path) -> Result<(PathBuf, serde_json::Value)> {
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let temp = std::env::temp_dir().join(format!("ncae-export-{}-{nonce}", std::process::id()));
    std::fs::create_dir(&temp)?;
    struct Scratch(PathBuf);
    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(self.0.join("output.wav"));
            let _ = std::fs::remove_dir(&self.0);
        }
    }
    let scratch = Scratch(temp);
    let file = scratch.0.join("output.wav");
    let mut metadata = convert_to_wav(source, &file, false)?;
    let output = crate::export_source_bytes(directory, source, "wav", &std::fs::read(file)?)?;
    metadata["path"] = serde_json::Value::String(output.display().to_string());
    Ok((output, metadata))
}
