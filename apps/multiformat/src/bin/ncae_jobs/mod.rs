//! Blocking file/codec work lives here, never in an egui frame callback.
use anyhow::{bail, Context, Result};
use ncae_tool::{
    generate_from_template, list_effects, read_ncae,
    wav::{ChannelMode, LengthMode},
    EffectInfo,
};
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::Instant;

pub(super) struct GenerateOptions {
    pub effect: EffectInfo,
    pub input: PathBuf,
    pub output_dir: PathBuf,
    pub records_path: PathBuf,
    pub mode: LengthMode,
    pub channel_mode: ChannelMode,
    pub peak_match: bool,
    pub replace_after: bool,
    pub backup_checked: bool,
}

pub(super) enum Job {
    Refresh(Option<PathBuf>),
    PickFile,
    Inspect(PathBuf),
    Generate(GenerateOptions),
    Decrypt {
        path: PathBuf,
        output_dir: PathBuf,
    },
    ConvertToWav {
        source: PathBuf,
        output_dir: PathBuf,
    },
    Restore {
        path: PathBuf,
        records: PathBuf,
    },
    OpenDirectory(PathBuf),
    #[cfg(test)]
    TestGate(Receiver<()>),
    #[cfg(test)]
    TestPanic,
}

impl Job {
    pub fn title(&self) -> &'static str {
        match self {
            Self::Refresh(_) => "扫描音效库",
            Self::PickFile => "选择输入文件",
            Self::Inspect(_) => "检查输入文件",
            Self::Generate(_) => "生成音效",
            Self::Decrypt { .. } => "解密导出",
            Self::Restore { .. } => "恢复原音效",
            Self::ConvertToWav { .. } => "转为 WAV",
            Self::OpenDirectory(_) => "打开目录",
            #[cfg(test)]
            Self::TestGate(_) | Self::TestPanic => "测试后台任务",
        }
    }
}

pub(super) enum JobResult {
    Effects {
        effects: Vec<EffectInfo>,
        location: super::ncae_locations::Location,
    },
    Input {
        path: PathBuf,
        validation: Result<String, String>,
        audio: bool,
    },
    Done(String),
    Notice {
        message: String,
        effects: Option<Vec<EffectInfo>>,
    },
}

pub(super) enum Event {
    Progress(String),
    Log(String),
    Output(PathBuf),
    Finished(Result<JobResult, String>),
}

pub(super) struct ActiveJob {
    pub title: &'static str,
    pub progress: String,
    pub started: Instant,
    pub receiver: Receiver<Event>,
}

struct Reporter {
    sender: Sender<Event>,
    ctx: egui::Context,
}

impl Reporter {
    fn send(&self, event: Event) {
        let _ = self.sender.send(event);
        self.ctx.request_repaint();
    }
    fn stage(&self, text: &str) {
        self.send(Event::Progress(text.into()));
    }
    fn log(&self, text: String) {
        self.send(Event::Log(text));
    }
}

pub(super) fn spawn(job: Job, ctx: egui::Context) -> std::io::Result<ActiveJob> {
    let title = job.title();
    let (sender, receiver) = mpsc::channel();
    let task = ActiveJob {
        title,
        progress: "准备任务…".into(),
        started: Instant::now(),
        receiver,
    };
    std::thread::Builder::new()
        .name("ncae-background".into())
        .spawn(move || {
            let report = Reporter { sender, ctx };
            let result =
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| execute(job, &report)))
                    .map_err(|_| {
                        "后台任务发生异常，已解除忙碌状态。请检查输入文件并重试。".to_owned()
                    })
                    .and_then(|result| result.map_err(|error| format!("{:#}", error)));
            report.send(Event::Finished(result));
        })?;
    Ok(task)
}

fn inspect(path: PathBuf, report: &Reporter) -> JobResult {
    report.stage("读取并校验输入文件…");
    let mut audio = false;
    let validation = (|| -> Result<String> {
        let metadata = std::fs::metadata(&path).context("无法访问输入文件")?;
        if !metadata.is_file() {
            bail!("输入路径不是文件");
        }
        let extension = path
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        match extension.as_str() {
            "wav" | "wave" | "irs" | "flac" | "aif" | "aiff" | "au" | "snd" | "caf" | "w64"
            | "rf64" | "npz" => {
                audio = true;
                let wav = ncae_tool::read_impulse_file(&path)?;
                Ok(format!(
                    "{} 已就绪: {}ch · {}Hz · {}bit · {}帧 · {:.1}ms",
                    if extension == "irs" {
                        "IRS（RIFF/WAVE）".to_owned()
                    } else if extension == "wav" || extension == "wave" {
                        "WAV".to_owned()
                    } else {
                        format!("{} → WAV（float32）", extension.to_ascii_uppercase())
                    },
                    wav.channels,
                    wav.sample_rate,
                    wav.bits,
                    wav.frames,
                    wav.duration_ms()
                ))
            }
            "json" => {
                if ncae_tool::format_bridge::is_sample_json(&path)? {
                    audio = true;
                    let wav = ncae_tool::read_impulse_file(&path)?;
                    return Ok(format!(
                        "IR 采样 JSON 已就绪: {}ch · {}Hz · {}帧（NCAE 转换使用 float32）",
                        wav.channels, wav.sample_rate, wav.frames
                    ));
                }
                let data = std::fs::read(&path).context("读取 JSON 失败")?;
                let value: serde_json::Value =
                    serde_json::from_slice(&data).context("JSON 格式无效")?;
                Ok(format!(
                    "JSON 已就绪: {} 字节 · {}",
                    data.len(),
                    if value.is_object() {
                        "参数对象"
                    } else {
                        "有效 JSON"
                    }
                ))
            }
            _ => bail!("不支持此输入类型，请选择音频 IR 或 ir.samples.v1 数据"),
        }
    })()
    .map_err(|error| format!("{:#}", error));
    JobResult::Input {
        path,
        validation,
        audio,
    }
}

fn refresh_after_write(target: &std::path::Path, message: &str, report: &Reporter) -> JobResult {
    report.stage("正在刷新音效列表…");
    let effects = target
        .parent()
        .and_then(|directory| match list_effects(directory) {
            Ok(effects) => Some(effects),
            Err(error) => {
                report.log(format!("操作已成功，但列表刷新失败，请手动刷新: {error}"));
                None
            }
        });
    JobResult::Notice {
        message: message.into(),
        effects,
    }
}

fn execute(job: Job, report: &Reporter) -> Result<JobResult> {
    match job {
        Job::Refresh(manual) => {
            report.stage("检测网易云安装位置和用户数据目录…");
            let location = super::ncae_locations::discover(manual)?;
            if let Some(install) = &location.install_dir {
                report.log(format!("网易云安装位置: {}", install.display()));
            }
            report.log(format!("本地音效目录: {}", location.effect_dir.display()));
            report.stage("读取目录并识别音效类型…");
            Ok(JobResult::Effects {
                effects: list_effects(&location.effect_dir).context("扫描音效库失败")?,
                location,
            })
        }
        Job::PickFile => {
            report.stage("等待选择文件；取消可返回工作台…");
            match pick_file()? {
                Some(path) => Ok(inspect(path, report)),
                None => Ok(JobResult::Done("已取消文件选择".into())),
            }
        }
        Job::Inspect(path) => Ok(inspect(path, report)),
        Job::Generate(options) => {
            report.stage("1/4 · 读取目标音效与输入文件…");
            if !options.input.is_file() {
                bail!("输入文件不存在或不可访问，请重新选择");
            }
            let template = read_ncae(&options.effect.path).context("读取目标模板失败")?;
            report.stage("2/4 · 转换音频并加密封装（大文件可能需要较长时间）…");
            let (data, label) = generate_from_template(
                &template,
                &options.input,
                options.mode,
                options.channel_mode,
                options.peak_match,
            )
            .context("转换或加密失败")?;
            if options.replace_after {
                report.stage("3/4 · 备份并替换目标文件，请勿关闭程序…");
                let result = super::ncae_backup::replace(
                    &options.effect.path,
                    &data,
                    &options.records_path,
                    options.backup_checked,
                )
                .context("备份或替换目标失败")?;
                if let Some(backup) = result.backup {
                    report.log(format!(
                        "{}: {}",
                        if result.created_backup {
                            "已在音效目录创建 .bak 原备份"
                        } else {
                            "保留已有原备份"
                        },
                        backup.display()
                    ));
                } else {
                    report.log("本次未创建原备份".into());
                }
                report.log(format!("替换完成，未额外导出副本 | {label}"));
                Ok(refresh_after_write(
                    &options.effect.path,
                    "替换完成。请在网易云中切换到其他音效，再切回该音效体验效果。",
                    report,
                ))
            } else {
                report.stage("3/4 · 保存到程序 generated 目录…");
                let output = ncae_tool::export_named_bytes(
                    &options.output_dir,
                    &options.effect.name,
                    "ncae",
                    &data,
                )
                .context("保存生成文件失败")?;
                report.send(Event::Output(output.clone()));
                report.log(format!("已生成: {} | {}", output.display(), label));
                report.stage("4/4 · 生成完成，原音效未修改");
                Ok(JobResult::Done(format!(
                    "仅生成新文件: {}",
                    output.display()
                )))
            }
        }
        Job::Decrypt { path, output_dir } => {
            report.stage("1/2 · 读取并解密目标音效…");
            let file = read_ncae(&path).context("读取或解密失败")?;
            report.stage("2/2 · 导出解密后的文件…");
            let output = ncae_tool::export_named_bytes(
                &output_dir,
                path.file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or("音效"),
                if file.kind == ncae_tool::PayloadKind::Binary {
                    "bin"
                } else {
                    file.kind.as_str()
                },
                &file.plain,
            )
            .context("导出失败")?;
            report.send(Event::Output(output.clone()));
            Ok(JobResult::Notice {
                message: format!(
                    "解密导出完成。\n文件格式：{}\n保存位置：{}",
                    file.kind.as_str().to_uppercase(),
                    output.display()
                ),
                effects: None,
            })
        }
        Job::ConvertToWav { source, output_dir } => {
            report.stage("1/2 · 读取并解析脉冲响应…");
            let extension = source
                .extension()
                .and_then(|value| value.to_str())
                .unwrap_or("")
                .to_ascii_lowercase();
            if extension != "ncae" {
                report.stage("使用独立多格式后端导出 WAV…");
                std::fs::create_dir_all(&output_dir)?;
                let (output, metadata) =
                    ncae_tool::format_bridge::export_source_wav(&source, &output_dir)?;
                report.send(Event::Output(output.clone()));
                return Ok(JobResult::Notice {
                    message: format!(
                        "WAV 导出完成。\n文件格式：WAV（{}）\n保存位置：{}",
                        metadata["subtype"].as_str().unwrap_or("音频"),
                        output.display()
                    ),
                    effects: None,
                });
            }
            let audio = if extension == "ncae" {
                let file = read_ncae(&source)?;
                if file.kind != ncae_tool::PayloadKind::Wav {
                    bail!("当前 NCAE 是参数或未知类型，不能直接转换为 WAV");
                }
                ncae_tool::wav::parse_wav(&file.plain)?
            } else if ["wav", "wave", "irs"].contains(&extension.as_str()) {
                ncae_tool::read_impulse_file(&source)?
            } else {
                bail!("此版本只支持 WAV、IRS 和 WAV/IR 型 NCAE 转为 WAV；参数 JSON 不能直接转换");
            };
            report.stage("2/2 · 写出 WAV，不裁剪、不调整峰值…");
            let bytes = ncae_tool::wav::write_wav_float32(
                &audio.samples,
                audio.sample_rate,
                audio.channels,
            );
            std::fs::create_dir_all(&output_dir)?;
            let output = ncae_tool::export_source_bytes(&output_dir, &source, "wav", &bytes)?;
            report.send(Event::Output(output.clone()));
            Ok(JobResult::Notice {
                message: format!(
                    "WAV 导出完成。\n文件格式：WAV（32-bit float）\n保存位置：{}",
                    output.display()
                ),
                effects: None,
            })
        }
        Job::Restore { path, records } => {
            report.stage("查找同目录 .bak 原备份（兼容旧版记录）并恢复，请勿关闭程序…");
            let backup = super::ncae_backup::restore(&path, &records).context("恢复失败")?;
            report.log(format!("已恢复原音效，来源: {}", backup.display()));
            Ok(refresh_after_write(
                &path,
                "恢复完成。请在网易云中切换音效，体验恢复后的效果。",
                report,
            ))
        }
        Job::OpenDirectory(path) => {
            report.stage("打开文件资源管理器…");
            std::fs::create_dir_all(&path).context("创建目录失败")?;
            #[cfg(target_os = "windows")]
            std::process::Command::new("explorer")
                .arg(&path)
                .spawn()
                .context("打开目录失败")?;
            #[cfg(not(target_os = "windows"))]
            std::process::Command::new("xdg-open")
                .arg(&path)
                .spawn()
                .context("打开目录失败")?;
            Ok(JobResult::Done(format!("已打开: {}", path.display())))
        }
        #[cfg(test)]
        Job::TestGate(receiver) => {
            report.stage("等待测试信号…");
            receiver.recv()?;
            Ok(JobResult::Done("测试任务完成".into()))
        }
        #[cfg(test)]
        Job::TestPanic => panic!("intentional worker panic for regression test"),
    }
}

/// Call the native Unicode dialog directly on the worker. No PowerShell startup or UI-thread wait.
#[cfg(target_os = "windows")]
fn pick_file() -> Result<Option<PathBuf>> {
    use std::os::windows::ffi::OsStringExt;
    use windows_sys::Win32::UI::Controls::Dialogs::*;
    let filter: Vec<u16> = "音频与 IR 数据\0*.wav;*.wave;*.irs;*.flac;*.aif;*.aiff;*.au;*.snd;*.caf;*.w64;*.rf64;*.json;*.npz\0所有文件\0*.*\0\0"
        .encode_utf16()
        .collect();
    let title: Vec<u16> = "选择音频或 IR 采样数据\0".encode_utf16().collect();
    let mut filename = vec![0u16; 32768];
    let mut dialog = OPENFILENAMEW {
        lStructSize: std::mem::size_of::<OPENFILENAMEW>() as u32,
        lpstrFilter: filter.as_ptr(),
        nFilterIndex: 1,
        lpstrFile: filename.as_mut_ptr(),
        nMaxFile: filename.len() as u32,
        lpstrTitle: title.as_ptr(),
        Flags: OFN_EXPLORER | OFN_FILEMUSTEXIST | OFN_PATHMUSTEXIST | OFN_NOCHANGEDIR,
        ..Default::default()
    };
    // SAFETY: All UTF-16 buffers outlive this blocking call, are NUL-terminated,
    // and the writable buffer length matches nMaxFile. The struct uses its ABI size.
    if unsafe { GetOpenFileNameW(&mut dialog) } != 0 {
        let length = filename
            .iter()
            .position(|&value| value == 0)
            .unwrap_or(filename.len());
        Ok(Some(PathBuf::from(std::ffi::OsString::from_wide(
            &filename[..length],
        ))))
    } else {
        // Zero means user cancellation; other native errors must be surfaced.
        let error = unsafe { CommDlgExtendedError() };
        if error != 0 {
            bail!("无法打开文件选择器（Windows 错误 {}）", error);
        }
        Ok(None)
    }
}

#[cfg(not(target_os = "windows"))]
fn pick_file() -> Result<Option<PathBuf>> {
    bail!("此平台请拖入文件或粘贴完整路径")
}
