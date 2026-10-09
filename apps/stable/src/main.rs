use anyhow::{bail, Result};
use ncae_tool::{
    backup_and_replace_recorded, decrypt_ncae_bytes, list_effects, parse_header, payload_kind,
    read_ncae, restore_recorded, set_payload_type, write_ncae, PayloadKind,
};
use serde_json::json;
use std::path::{Path, PathBuf};

fn usage() {
    eprintln!(
        "用法:\n  ncae-tool info <file.ncae>\n  ncae-tool inspect <file.ncae>\n  ncae-tool list\n  ncae-tool decrypt <file.ncae> <out>\n  ncae-tool encrypt <template.ncae> <plain> <out> [--kind json|wav|binary]\n  ncae-tool replace <target.ncae> <generated.ncae> [--backup-dir DIR] [--no-backup]\n  ncae-tool restore <target.ncae> [--backup-dir DIR]\n  ncae-tool roundtrip <file.ncae>"
    );
}

fn kind_from_str(s: &str) -> Result<PayloadKind> {
    match s.to_ascii_lowercase().as_str() {
        "json" => Ok(PayloadKind::Json),
        "wav" => Ok(PayloadKind::Wav),
        "binary" | "bin" => Ok(PayloadKind::Binary),
        other => bail!("未知类型: {}", other),
    }
}

fn default_effect_dir() -> PathBuf {
    std::env::var("LOCALAPPDATA")
        .map(|p| {
            PathBuf::from(p)
                .join("NetEase")
                .join("CloudMusic")
                .join("audioeffect")
        })
        .unwrap_or_else(|_| PathBuf::from("."))
}

fn cmd_info(path: &Path) -> Result<()> {
    let data = std::fs::read(path)?;
    let (key, key0, reserved, encrypted) = parse_header(&data)?;
    let plain = decrypt_ncae_bytes(&data)?;
    let kind = payload_kind(&plain);

    let mut obj = json!({
        "path": path.display().to_string(),
        "size": data.len(),
        "kind": kind.as_str(),
        "key_len": key.len(),
        "key0_len": key0.len(),
        "reserved": reserved.iter().map(|b| format!("{:02x}", b)).collect::<Vec<_>>().join(""),
        "payload_type": reserved[6],
        "plain_size": plain.len(),
        "encrypted_size": encrypted.len(),
    });

    if kind == PayloadKind::Json {
        if let Ok(v) = serde_json::from_slice::<serde_json::Value>(&plain) {
            if let Some(map) = v.as_object() {
                let mut keys: Vec<&String> = map.keys().collect();
                keys.sort();
                obj["json_keys"] = json!(keys);
            }
        }
    }

    println!("{}", serde_json::to_string_pretty(&obj)?);
    Ok(())
}

fn cmd_list() -> Result<()> {
    let dir = default_effect_dir();
    let list = list_effects(&dir)?;
    for e in list {
        println!("{}\t{}\t{}", e.name, e.kind.as_str(), e.detail);
    }
    Ok(())
}

fn cmd_decrypt(path: &Path, out: &Path) -> Result<()> {
    let f = read_ncae(path)?;
    let ext = match f.kind {
        PayloadKind::Json => "json",
        PayloadKind::Wav => "wav",
        PayloadKind::Binary => "bin",
    };
    let out = if out.extension().and_then(|s| s.to_str()) == Some(ext) {
        out.to_path_buf()
    } else {
        out.with_extension(ext)
    };
    if let Some(parent) = out.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&out, &f.plain)?;
    println!("{}", out.display());
    Ok(())
}

fn cmd_encrypt(
    template: &Path,
    plain_path: &Path,
    out: &Path,
    kind_arg: Option<&str>,
) -> Result<()> {
    let template_file = read_ncae(template)?;
    let plain = if plain_path
        .extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case("irs"))
    {
        let impulse = ncae_tool::read_impulse_file(plain_path)?;
        ncae_tool::wav::write_wav_float32(&impulse.samples, impulse.sample_rate, impulse.channels)
    } else {
        std::fs::read(plain_path)?
    };
    let kind = match kind_arg {
        Some(s) => kind_from_str(s)?,
        None => payload_kind(&plain),
    };
    let reserved = set_payload_type(&template_file.reserved, kind);
    write_ncae(
        out,
        &template_file.key,
        &template_file.key0,
        &reserved,
        &plain,
    )?;
    println!("{}", out.display());
    Ok(())
}

fn cmd_replace(
    target: &Path,
    generated_path: &Path,
    backup_dir: &Path,
    no_backup: bool,
) -> Result<()> {
    let generated = std::fs::read(generated_path)?;
    let records_path = backup_dir
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("replacement_records.json");
    let result = backup_and_replace_recorded(
        target,
        &generated,
        backup_dir,
        &records_path,
        generated_path,
        "cli",
        "keep",
        true,
        no_backup,
    )?;
    match result.backup {
        Some(p) => println!("{}", p.display()),
        None => println!("NO_BACKUP"),
    }
    Ok(())
}

fn cmd_restore(target: &Path, backup_dir: &Path) -> Result<()> {
    let records_path = backup_dir
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("replacement_records.json");
    let backup = restore_recorded(target, &records_path)?;
    println!("{}", backup.display());
    Ok(())
}

fn cmd_roundtrip(path: &Path) -> Result<()> {
    let ok = ncae_tool::roundtrip_test(path)?;
    println!(
        "{}",
        if ok {
            "ROUNDTRIP_OK"
        } else {
            "ROUNDTRIP_FAILED"
        }
    );
    if ok {
        Ok(())
    } else {
        bail!("往返测试失败")
    }
}

fn parse_backup_dir(args: &[String]) -> PathBuf {
    for i in 0..args.len().saturating_sub(1) {
        if args[i] == "--backup-dir" {
            return PathBuf::from(&args[i + 1]);
        }
    }
    PathBuf::from("backup")
}

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        usage();
        return Ok(());
    }

    match args[1].as_str() {
        "info" | "inspect" if args.len() == 3 => cmd_info(Path::new(&args[2])),
        "list" if args.len() == 2 => cmd_list(),
        "decrypt" if args.len() == 4 => cmd_decrypt(Path::new(&args[2]), Path::new(&args[3])),
        "encrypt" if args.len() >= 5 => {
            let kind = if args.len() >= 7 && args[5] == "--kind" {
                Some(args[6].as_str())
            } else {
                None
            };
            cmd_encrypt(
                Path::new(&args[2]),
                Path::new(&args[3]),
                Path::new(&args[4]),
                kind,
            )
        }
        "replace" if args.len() >= 4 => {
            let backup_dir = parse_backup_dir(&args);
            let no_backup = args.iter().any(|a| a == "--no-backup");
            cmd_replace(
                Path::new(&args[2]),
                Path::new(&args[3]),
                &backup_dir,
                no_backup,
            )
        }
        "restore" if args.len() >= 3 => {
            let backup_dir = parse_backup_dir(&args);
            cmd_restore(Path::new(&args[2]), &backup_dir)
        }
        "roundtrip" if args.len() == 3 => cmd_roundtrip(Path::new(&args[2])),
        _ => {
            usage();
            std::process::exit(2);
        }
    }
}
