#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod ncae_backup;
mod ncae_jobs;
mod ncae_locations;
mod ncae_paths;
mod ncae_preview;
#[cfg(test)]
mod ncae_tests;
mod ncae_ui;

use eframe::egui;
use ncae_jobs::{ActiveJob, Event, GenerateOptions, Job, JobResult};
use ncae_tool::{
    wav::{ChannelMode, LengthMode},
    EffectInfo,
};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

fn setup_fonts(ctx: &egui::Context) {
    let candidates = [
        r"C:\Windows\Fonts\msyh.ttc",
        r"C:\Windows\Fonts\msyhbd.ttc",
        r"C:\Windows\Fonts\simhei.ttf",
        r"C:\Windows\Fonts\simsun.ttc",
        r"C:\Windows\Fonts\Deng.ttf",
        r"C:\Windows\Fonts\msyhl.ttc",
    ];
    let mut bytes = None;
    for p in candidates {
        if let Ok(b) = std::fs::read(p) {
            bytes = Some(b);
            break;
        }
    }
    let Some(bytes) = bytes else {
        return;
    };
    let mut fonts = egui::FontDefinitions::default();
    fonts.font_data.insert(
        "chinese".to_owned(),
        std::sync::Arc::new(egui::FontData::from_owned(bytes)),
    );
    fonts
        .families
        .entry(egui::FontFamily::Proportional)
        .or_default()
        .insert(0, "chinese".to_owned());
    fonts
        .families
        .entry(egui::FontFamily::Monospace)
        .or_default()
        .insert(0, "chinese".to_owned());
    ctx.set_fonts(fonts);
}

struct PendingConfirmation {
    title: &'static str,
    message: String,
    job: Job,
}

struct NcaeGuiApp {
    effect_dir: PathBuf,
    install_dir: Option<PathBuf>,
    directory_note: String,
    manual_effect_dir: Option<PathBuf>,
    directory_input: String,
    output_dir: PathBuf,
    generated_dir: PathBuf,
    effects: Vec<EffectInfo>,
    selected: Option<usize>,
    input_path: String,
    input_audio: bool,
    mode: LengthMode,
    channel_mode: ChannelMode,
    peak_match: bool,
    replace_after: bool,
    log: Vec<String>,
    last_output: Option<PathBuf>,
    records_path: PathBuf,
    backup_checked: bool,
    search: String,
    sort_by_type: bool,
    show_log: bool,
    preview: ncae_preview::Controller,
    task: Option<ActiveJob>,
    confirmation: Option<PendingConfirmation>,
    notice: Option<String>,
    observed_input: String,
    input_changed: Instant,
    input_validation: Option<(String, Result<String, String>)>,
}

impl NcaeGuiApp {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        setup_fonts(&cc.egui_ctx);
        ncae_ui::setup_style(&cc.egui_ctx);
        let effect_dir = std::env::var("LOCALAPPDATA")
            .map(|p| {
                PathBuf::from(p)
                    .join("NetEase")
                    .join("CloudMusic")
                    .join("audioeffect")
            })
            .unwrap_or_else(|_| PathBuf::from("."));
        let base = std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(Path::to_path_buf))
            .unwrap_or_else(|| PathBuf::from("."));
        let mut app = Self::empty(effect_dir, base);
        if let Some(downloads) = ncae_paths::downloads_directory() {
            app.output_dir = downloads;
        } else {
            app.log("无法定位下载目录，暂时使用程序输出目录。");
        }
        app.log(format!("音效目录: {}", app.effect_dir.display()));
        app.log(format!("导出目录: {}", app.output_dir.display()));
        app.start_job(Job::Refresh(None), &cc.egui_ctx);
        app
    }

    fn empty(effect_dir: PathBuf, base: PathBuf) -> Self {
        Self {
            install_dir: None,
            directory_note: "正在自动检测网易云安装位置与音效目录…".into(),
            manual_effect_dir: None,
            directory_input: effect_dir.display().to_string(),
            effect_dir,
            output_dir: base.join("generated"),
            generated_dir: base.join("generated"),
            records_path: base.join("replacement_records.json"),
            effects: Vec::new(),
            selected: None,
            input_path: String::new(),
            input_audio: false,
            mode: LengthMode::Full,
            channel_mode: ChannelMode::Keep,
            peak_match: true,
            replace_after: true,
            log: Vec::new(),
            last_output: None,
            backup_checked: true,
            search: String::new(),
            sort_by_type: true,
            show_log: false,
            preview: ncae_preview::Controller::default(),
            task: None,
            confirmation: None,
            notice: None,
            observed_input: String::new(),
            input_changed: Instant::now(),
            input_validation: None,
        }
    }

    fn log(&mut self, message: impl Into<String>) {
        self.log.push(message.into());
        if self.log.len() > 500 {
            self.log.remove(0);
        }
    }

    fn selected_effect(&self) -> Option<EffectInfo> {
        self.selected
            .and_then(|index| self.effects.get(index).cloned())
    }

    fn wav_conversion_source(&self) -> Option<PathBuf> {
        if !self.input_path.trim().is_empty() {
            return (self.input_readiness().is_none() && self.input_audio)
                .then(|| PathBuf::from(self.input_path.trim()));
        }
        self.selected_effect()
            .filter(|effect| effect.kind == ncae_tool::PayloadKind::Wav)
            .map(|effect| effect.path)
    }

    fn backup_directory(&self) -> PathBuf {
        self.selected_effect()
            .and_then(|effect| effect.path.parent().map(Path::to_path_buf))
            .unwrap_or_else(|| self.effect_dir.clone())
    }

    fn busy(&self) -> bool {
        self.task.is_some()
    }

    fn start_job(&mut self, job: Job, ctx: &egui::Context) {
        // Single-flight: never let two writes, dialogs, or stale results race.
        if self.busy() {
            return;
        }
        self.notice = None;
        let title = job.title();
        match ncae_jobs::spawn(job, ctx.clone()) {
            Ok(task) => {
                self.log(format!("{}…", title));
                self.task = Some(task);
            }
            Err(error) => self.log(format!("启动任务失败: {}", error)),
        }
        ctx.request_repaint();
    }

    fn request_generate(&mut self, ctx: &egui::Context) {
        if self.busy() || self.input_readiness().is_some() {
            return;
        }
        let Some(effect) = self.selected_effect() else {
            return;
        };
        let job = Job::Generate(GenerateOptions {
            effect,
            input: PathBuf::from(self.input_path.trim()),
            output_dir: self.generated_dir.clone(),
            records_path: self.records_path.clone(),
            mode: self.mode,
            channel_mode: self.channel_mode,
            peak_match: self.peak_match,
            replace_after: self.replace_after,
            backup_checked: self.backup_checked,
        });
        if self.replace_after && !self.backup_checked {
            self.confirmation = Some(PendingConfirmation {
                title: "未开启原音效备份",
                message: "继续将覆盖目标音效，且不会创建原文件备份。修改后可能无法恢复。确定仍要生成并替换吗？".into(), job,
            });
        } else {
            self.start_job(job, ctx);
        }
    }

    fn request_restore(&mut self) {
        if self.busy() {
            return;
        }
        if let Some(effect) = self.selected_effect() {
            self.confirmation = Some(PendingConfirmation {
                title: "恢复原音效",
                message: format!(
                    "将使用备份覆盖「{}」当前的音效文件。是否继续？",
                    effect.name
                ),
                job: Job::Restore {
                    path: effect.path,
                    records: self.records_path.clone(),
                },
            });
        }
    }

    fn input_readiness(&self) -> Option<String> {
        if let Some(problem) = ncae_ui::input_problem(&self.input_path) {
            return Some(problem.into());
        }
        match &self.input_validation {
            Some((path, Ok(_))) if path == self.input_path.trim() => None,
            Some((path, Err(error))) if path == self.input_path.trim() => Some(error.clone()),
            _ => Some("正在检查输入文件，请稍候…".into()),
        }
    }

    fn schedule_input_check(&mut self, ctx: &egui::Context) {
        let value = self.input_path.trim().to_owned();
        if value != self.observed_input {
            self.observed_input = value.clone();
            self.input_changed = Instant::now();
            self.input_validation = None;
            self.input_audio = false;
        }
        if ncae_ui::input_problem(&value).is_some() || self.input_validation.is_some() {
            return;
        }
        if !self.busy()
            && self.confirmation.is_none()
            && self.input_changed.elapsed() >= Duration::from_millis(350)
        {
            self.start_job(Job::Inspect(PathBuf::from(value)), ctx);
        } else {
            ctx.request_repaint_after(Duration::from_millis(80));
        }
    }

    fn poll_job(&mut self) {
        let mut events = Vec::new();
        let mut disconnected = false;
        if let Some(task) = &self.task {
            loop {
                match task.receiver.try_recv() {
                    Ok(event) => events.push(event),
                    Err(std::sync::mpsc::TryRecvError::Empty) => break,
                    Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                        disconnected = true;
                        break;
                    }
                }
            }
        }
        for event in events {
            match event {
                Event::Progress(message) => {
                    if let Some(task) = &mut self.task {
                        task.progress.clone_from(&message);
                    }
                    self.log(message);
                }
                Event::Log(message) => self.log(message),
                Event::Output(path) => self.last_output = Some(path),
                Event::Finished(result) => {
                    let elapsed = self
                        .task
                        .take()
                        .map(|task| task.started.elapsed().as_secs_f32())
                        .unwrap_or_default();
                    match result {
                        Ok(JobResult::Effects { effects, location }) => {
                            self.preview.invalidate();
                            self.effect_dir = location.effect_dir;
                            self.install_dir = location.install_dir;
                            self.directory_note = location.note;
                            self.directory_input = self.effect_dir.display().to_string();
                            self.log(self.directory_note.clone());
                            let selected = self.selected_effect().map(|effect| effect.path);
                            self.effects = effects;
                            self.selected = selected
                                .and_then(|path| self.effects.iter().position(|e| e.path == path));
                            self.log(format!(
                                "已加载 {} 个音效 · {:.1} 秒",
                                self.effects.len(),
                                elapsed
                            ));
                        }
                        Ok(JobResult::Input {
                            path,
                            validation,
                            audio,
                        }) => {
                            self.input_audio = audio && validation.is_ok();
                            // Input widgets are disabled while a job is running; still correlate results.
                            self.input_path = path.display().to_string();
                            self.observed_input = self.input_path.trim().to_owned();
                            match &validation {
                                Ok(info) => self.log(info),
                                Err(error) => self.log(format!("输入检查失败: {}", error)),
                            }
                            self.input_validation = Some((self.observed_input.clone(), validation));
                        }
                        Ok(JobResult::Done(message)) => {
                            self.log(format!("{} · {:.1} 秒", message, elapsed))
                        }
                        Ok(JobResult::Notice { message, effects }) => {
                            if let Some(effects) = effects {
                                self.preview.invalidate();
                                let selected = self.selected_effect().map(|effect| effect.path);
                                self.effects = effects;
                                self.selected = selected.and_then(|path| {
                                    self.effects.iter().position(|effect| effect.path == path)
                                });
                            }
                            self.log(format!("{} · {:.1} 秒", message, elapsed));
                            self.notice = Some(message);
                        }
                        Err(error) => {
                            self.log(format!("操作失败: {} · {:.1} 秒", error, elapsed));
                            self.notice = Some(format!("操作失败：{}", error));
                        }
                    }
                }
            }
        }
        if disconnected && self.task.take().is_some() {
            self.log("后台任务异常结束，请重试；详细信息见日志。");
        }
    }
}

impl eframe::App for NcaeGuiApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.poll_job();
        let dropped: Vec<PathBuf> = ui.ctx().input(|input| {
            input
                .raw
                .dropped_files
                .iter()
                .map(|file| file.path().to_path_buf())
                .collect()
        });
        if let Some(path) = dropped.first() {
            if self.busy() || self.confirmation.is_some() {
                self.log("当前任务尚未结束，请完成后再拖入新文件。");
            } else {
                self.input_path = path.display().to_string();
                self.observed_input = self.input_path.trim().to_owned();
                self.input_validation = None;
                self.input_audio = false;
                self.start_job(Job::Inspect(path.clone()), ui.ctx());
            }
        }
        if self.busy() {
            ui.ctx().request_repaint_after(Duration::from_millis(60));
            if ui.ctx().input(|input| input.viewport().close_requested()) {
                ui.ctx()
                    .send_viewport_cmd(egui::ViewportCommand::CancelClose);
                self.notice = Some("后台任务尚未结束。为避免中断文件写入，请完成任务后关闭；若正在选文件，请先关闭文件选择对话框。".into());
            }
        }
        self.render_workspace(ui);
        self.render_dialogs(ui.ctx());
        self.schedule_input_check(ui.ctx());
    }
}

fn main() -> eframe::Result<()> {
    #[cfg(target_os = "windows")]
    {
        let app_id: Vec<u16> = "NCAE.AudioEffect.Workbench.MultiFormat"
            .encode_utf16()
            .chain(Some(0))
            .collect();
        // SAFETY: The NUL-terminated UTF-16 string is valid throughout this synchronous call.
        let _ = unsafe {
            windows_sys::Win32::UI::Shell::SetCurrentProcessExplicitAppUserModelID(app_id.as_ptr())
        };
    }
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_icon(egui::IconData {
                rgba: include_bytes!("../../assets/app-icon.rgba").to_vec(),
                width: 64,
                height: 64,
            })
            .with_inner_size([1180.0, 860.0])
            .with_min_inner_size([900.0, 640.0])
            .with_drag_and_drop(true),
        ..Default::default()
    };
    eframe::run_native(
        "NCAE 多格式支线 · 独立版本",
        options,
        Box::new(|cc| Ok(Box::new(NcaeGuiApp::new(cc)))),
    )
}
