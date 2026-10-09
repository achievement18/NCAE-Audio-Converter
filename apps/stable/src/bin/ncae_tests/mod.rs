use super::*;
use std::sync::mpsc;

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        static SERIAL: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let serial = SERIAL.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "ncae-ui-test-{}-{}-{serial}",
            std::process::id(),
            stamp
        ));
        std::fs::create_dir_all(&path).unwrap();
        Self(path)
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn wait(app: &mut NcaeGuiApp) {
    let deadline = Instant::now() + Duration::from_secs(15);
    while app.busy() && Instant::now() < deadline {
        app.poll_job();
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(!app.busy(), "background job did not complete");
}

#[test]
fn ui_keeps_rendering_while_worker_is_blocked_and_rejects_duplicate_jobs() {
    let ctx = egui::Context::default();
    ncae_ui::setup_style(&ctx);
    let mut app = NcaeGuiApp::empty("unused".into(), "unused".into());
    let (send, receive) = mpsc::channel();
    app.start_job(Job::TestGate(receive), &ctx);
    let (duplicate_send, duplicate_receive) = mpsc::channel();
    app.start_job(Job::TestGate(duplicate_receive), &ctx);
    assert!(
        duplicate_send.send(()).is_err(),
        "second job must not be queued or started"
    );
    for size in [[1180.0, 860.0], [900.0, 640.0], [1600.0, 1000.0]] {
        for _ in 0..3 {
            app.poll_job();
            assert!(app.busy());
            let frame = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(size[0], size[1]),
                    )),
                    ..Default::default()
                },
                |ui| app.render_workspace(ui),
            );
            assert!(!frame.shapes.is_empty());
        }
    }
    send.send(()).unwrap();
    wait(&mut app);
    assert!(app.log.last().unwrap().contains("测试任务完成"));
}

#[test]
fn worker_panic_releases_busy_state() {
    let mut app = NcaeGuiApp::empty("unused".into(), "unused".into());
    app.start_job(Job::TestPanic, &egui::Context::default());
    wait(&mut app);
    assert!(app.notice.as_ref().unwrap().contains("异常"));
}

#[test]
fn missing_and_invalid_input_are_reported_without_blocking_or_retry_loop() {
    let fixture = Fixture::new();
    let ctx = egui::Context::default();
    let mut app = NcaeGuiApp::empty(fixture.0.clone(), fixture.0.clone());
    let missing = fixture.0.join("missing.WAV");
    app.start_job(Job::Inspect(missing), &ctx);
    wait(&mut app);
    assert!(app.input_readiness().unwrap().contains("无法访问"));
    app.schedule_input_check(&ctx);
    assert!(!app.busy());
    let invalid = fixture.0.join("bad.json");
    std::fs::write(&invalid, b"not json").unwrap();
    app.start_job(Job::Inspect(invalid), &ctx);
    wait(&mut app);
    assert!(app.input_readiness().unwrap().contains("JSON 格式无效"));
}

#[test]
fn valid_wav_is_inspected_and_generated_without_changing_target() {
    let fixture = Fixture::new();
    let ctx = egui::Context::default();
    let target = fixture.0.join("template.ncae");
    let wav = ncae_tool::wav::write_wav_float32(&[0.1, -0.2, 0.3, 0.0], 44100, 1);
    ncae_tool::write_ncae(
        &target,
        &[1, 2, 3, 4],
        &[1, 2, 3, 4, 0],
        &[0, 0, 0, 0, 1, 0, 2, 0],
        &wav,
    )
    .unwrap();
    let before = std::fs::read(&target).unwrap();
    let input = fixture.0.join("input.WAV");
    std::fs::write(&input, &wav).unwrap();
    let mut app = NcaeGuiApp::empty(fixture.0.clone(), fixture.0.clone());
    app.effects = ncae_tool::list_effects(&fixture.0).unwrap();
    app.selected = Some(0);
    app.start_job(Job::Inspect(input), &ctx);
    wait(&mut app);
    assert!(app.input_readiness().is_none());
    app.replace_after = false;
    app.request_generate(&ctx);
    wait(&mut app);
    let output = app.last_output.clone().expect("generated output");
    assert_eq!(
        ncae_tool::read_ncae(&output).unwrap().kind,
        ncae_tool::PayloadKind::Wav
    );
    assert_eq!(std::fs::read(&target).unwrap(), before);
    assert!(!fixture.0.join("backup").exists());
    assert!(!app.records_path.exists());
}

#[test]
fn backup_replace_and_restore_work_only_on_isolated_fixtures() {
    let fixture = Fixture::new();
    let ctx = egui::Context::default();
    let target = fixture.0.join("template.ncae");
    ncae_tool::write_ncae(
        &target,
        &[1, 2, 3, 4],
        &[1, 2, 3, 4, 0],
        &[0, 0, 0, 0, 1, 0, 1, 0],
        br#"{"gain":1}"#,
    )
    .unwrap();
    let before = std::fs::read(&target).unwrap();
    let input = fixture.0.join("input.json");
    std::fs::write(&input, br#"{"gain":2}"#).unwrap();
    let mut app = NcaeGuiApp::empty(fixture.0.clone(), fixture.0.clone());
    app.effects = ncae_tool::list_effects(&fixture.0).unwrap();
    app.selected = Some(0);
    app.start_job(Job::Inspect(input), &ctx);
    wait(&mut app);
    app.replace_after = true;
    app.backup_checked = false;
    app.request_generate(&ctx);
    assert!(!app.busy());
    assert!(app.confirmation.is_some());
    assert_eq!(std::fs::read(&target).unwrap(), before);
    app.confirmation = None;
    app.backup_checked = true;
    app.request_generate(&ctx);
    wait(&mut app);
    assert_ne!(std::fs::read(&target).unwrap(), before);
    assert!(!app.records_path.exists());
    let backups: Vec<_> = std::fs::read_dir(&fixture.0)
        .unwrap()
        .flatten()
        .filter(|entry| entry.path().extension().is_some_and(|ext| ext == "bak"))
        .collect();
    assert_eq!(backups.len(), 1);
    assert_eq!(std::fs::read(backups[0].path()).unwrap(), before);
    assert!(backups[0]
        .file_name()
        .to_string_lossy()
        .starts_with("template.original-"));
    app.start_job(
        Job::Restore {
            path: target.clone(),
            records: app.records_path.clone(),
        },
        &ctx,
    );
    wait(&mut app);
    assert_eq!(std::fs::read(&target).unwrap(), before);
}

#[test]
fn refresh_preserves_selection_and_uses_explicit_manual_directory() {
    let fixture = Fixture::new();
    let ctx = egui::Context::default();
    let target = fixture.0.join("selected.ncae");
    ncae_tool::write_ncae(
        &target,
        &[1, 2, 3, 4],
        &[1, 2, 3, 4, 0],
        &[0, 0, 0, 0, 1, 0, 1, 0],
        b"{}",
    )
    .unwrap();
    let mut app = NcaeGuiApp::empty(fixture.0.clone(), fixture.0.clone());
    app.start_job(Job::Refresh(Some(fixture.0.clone())), &ctx);
    wait(&mut app);
    app.selected = Some(0);
    app.start_job(Job::Refresh(Some(fixture.0.clone())), &ctx);
    wait(&mut app);
    assert_eq!(app.selected_effect().unwrap().path, target);
    assert_eq!(app.effect_dir, fixture.0);
}

#[test]
fn irs_gui_validation_and_generation_work_end_to_end() {
    let fixture = Fixture::new();
    let ctx = egui::Context::default();
    let target = fixture.0.join("target.ncae");
    let input = fixture.0.join("impulse.irs");
    let wave = ncae_tool::wav::write_wav_float32(&[0.1, -0.1, 0.2, -0.2], 44100, 2);
    ncae_tool::write_ncae(
        &target,
        &[1, 2, 3, 4],
        &[1, 2, 3, 4, 0],
        &[0, 0, 0, 0, 1, 0, 2, 0],
        &wave,
    )
    .unwrap();
    std::fs::write(&input, &wave).unwrap();
    let mut app = NcaeGuiApp::empty(fixture.0.clone(), fixture.0.clone());
    app.effects = ncae_tool::list_effects(&fixture.0).unwrap();
    app.selected = Some(0);
    app.start_job(Job::Inspect(input), &ctx);
    wait(&mut app);
    assert!(app.input_readiness().is_none());
    assert!(app.log.iter().any(|line| line.contains("IRS（RIFF/WAVE）")));
    app.replace_after = false;
    app.request_generate(&ctx);
    wait(&mut app);
    let output = ncae_tool::read_ncae(&app.last_output.unwrap()).unwrap();
    assert_eq!(output.kind, ncae_tool::PayloadKind::Wav);
    assert_eq!(
        ncae_tool::wav::parse_wav(&output.plain).unwrap().channels,
        2
    );
}

#[test]
fn convert_to_wav_works_without_a_selected_template_and_preserves_input() {
    let fixture = Fixture::new();
    let ctx = egui::Context::default();
    let input = fixture.0.join("input.irs");
    let wave = ncae_tool::wav::write_wav_float32(&[0.1, -0.1, 0.2, -0.2], 48000, 2);
    std::fs::write(&input, &wave).unwrap();
    let mut app = NcaeGuiApp::empty(fixture.0.clone(), fixture.0.clone());
    app.start_job(Job::Inspect(input.clone()), &ctx);
    wait(&mut app);
    assert!(app.selected.is_none());
    assert_eq!(app.wav_conversion_source(), Some(input.clone()));
    app.start_job(
        Job::ConvertToWav {
            source: input.clone(),
            output_dir: app.output_dir.clone(),
        },
        &ctx,
    );
    wait(&mut app);
    let output = std::fs::read(app.last_output.unwrap()).unwrap();
    assert_eq!(output, wave);
    assert_eq!(std::fs::read(input).unwrap(), wave);
}

#[test]
fn convert_to_wav_rejects_json_parameter_effects() {
    let fixture = Fixture::new();
    let ctx = egui::Context::default();
    let input = fixture.0.join("params.ncae");
    ncae_tool::write_ncae(
        &input,
        &[1, 2, 3, 4],
        &[1, 2, 3, 4, 0],
        &[0, 0, 0, 0, 1, 0, 1, 0],
        b"{\"eq\":{}}",
    )
    .unwrap();
    let mut app = NcaeGuiApp::empty(fixture.0.clone(), fixture.0.clone());
    app.start_job(
        Job::ConvertToWav {
            source: input,
            output_dir: app.output_dir.clone(),
        },
        &ctx,
    );
    wait(&mut app);
    assert!(app.last_output.is_none());
    assert!(app.notice.unwrap().contains("不能直接转换为 WAV"));
}

#[test]
fn replacement_and_restore_refresh_type_and_preserve_selection() {
    let fixture = Fixture::new();
    let ctx = egui::Context::default();
    let target = fixture.0.join("target.ncae");
    ncae_tool::write_ncae(
        &target,
        &[1, 2, 3, 4],
        &[1, 2, 3, 4, 0],
        &[0, 0, 0, 0, 1, 0, 1, 0],
        b"{\"eq\":{}}",
    )
    .unwrap();
    let input = fixture.0.join("input.wav");
    std::fs::write(
        &input,
        ncae_tool::wav::write_wav_float32(&[0.1, -0.1], 48000, 1),
    )
    .unwrap();
    let mut app = NcaeGuiApp::empty(fixture.0.clone(), fixture.0.clone());
    app.effects = ncae_tool::list_effects(&fixture.0).unwrap();
    app.selected = Some(0);
    app.start_job(Job::Inspect(input), &ctx);
    wait(&mut app);
    app.replace_after = true;
    app.request_generate(&ctx);
    wait(&mut app);
    assert_eq!(app.selected_effect().unwrap().path, target);
    assert_eq!(
        app.selected_effect().unwrap().kind,
        ncae_tool::PayloadKind::Wav
    );
    assert!(app.notice.as_ref().unwrap().contains("替换完成"));
    app.start_job(
        Job::Restore {
            path: target.clone(),
            records: app.records_path.clone(),
        },
        &ctx,
    );
    wait(&mut app);
    assert_eq!(app.selected_effect().unwrap().path, target);
    assert_eq!(
        app.selected_effect().unwrap().kind,
        ncae_tool::PayloadKind::Json
    );
}

#[test]
fn decryption_export_returns_completion_notice_and_unique_outputs() {
    let fixture = Fixture::new();
    let ctx = egui::Context::default();
    let target = fixture.0.join("target.ncae");
    ncae_tool::write_ncae(
        &target,
        &[1, 2, 3, 4],
        &[1, 2, 3, 4, 0],
        &[0, 0, 0, 0, 1, 0, 1, 0],
        b"{\"eq\":{}}",
    )
    .unwrap();
    let mut app = NcaeGuiApp::empty(fixture.0.clone(), fixture.0.clone());
    app.start_job(
        Job::Decrypt {
            path: target.clone(),
            output_dir: app.output_dir.clone(),
        },
        &ctx,
    );
    wait(&mut app);
    let first = app.last_output.clone().unwrap();
    assert!(first.is_file());
    assert!(app.notice.as_ref().unwrap().contains("解密导出完成"));
    assert!(app
        .notice
        .as_ref()
        .unwrap()
        .contains(&first.display().to_string()));
    app.start_job(
        Job::Decrypt {
            path: target,
            output_dir: app.output_dir.clone(),
        },
        &ctx,
    );
    wait(&mut app);
    assert_ne!(app.last_output.unwrap(), first);
    assert!(first.is_file());
}

#[test]
fn default_mode_replaces_with_backup_enabled() {
    let app = NcaeGuiApp::empty("unused".into(), "unused".into());
    assert!(app.replace_after && app.backup_checked);
}
#[test]
fn selected_preview_never_follows_the_imported_source() {
    let fixture = Fixture::new();
    let target = fixture.0.join("selected.ncae");
    ncae_tool::write_ncae(
        &target,
        &[1, 2, 3, 4],
        &[1, 2, 3, 4, 0],
        &[0, 0, 0, 0, 1, 0, 1, 0],
        br#"{"eq":{"on":false}}"#,
    )
    .unwrap();
    let before = std::fs::read(&target).unwrap();
    let ctx = egui::Context::default();
    super::ncae_ui::setup_style(&ctx);
    let mut app = NcaeGuiApp::empty(fixture.0.clone(), fixture.0.clone());
    app.effects = ncae_tool::list_effects(&fixture.0).unwrap();
    app.selected = Some(0);
    for input in ["source.wav", "completely_different.irs"] {
        app.input_path = input.into();
        let _ = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1180.0, 860.0),
                )),
                ..Default::default()
            },
            |ui| app.render_workspace(ui),
        );
        assert_eq!(app.preview.target(), Some(target.as_path()));
    }
    assert_eq!(std::fs::read(target).unwrap(), before);
}
#[test]
fn exports_use_effect_names_and_number_duplicates() {
    let fixture = Fixture::new();
    let first =
        ncae_tool::export_named_bytes(&fixture.0, "HiFi现场-1791477478841.ncae", "wav", b"one")
            .unwrap();
    let second =
        ncae_tool::export_named_bytes(&fixture.0, "HiFi现场-1791477478841.ncae", "wav", b"two")
            .unwrap();
    assert_eq!(first.file_name().unwrap(), "HiFi现场.wav");
    assert_eq!(second.file_name().unwrap(), "HiFi现场 (1).wav");
    assert_eq!(std::fs::read(first).unwrap(), b"one");
}

#[test]
fn source_wav_export_keeps_source_name_and_reports_completion() {
    let fixture = Fixture::new();
    let ctx = egui::Context::default();
    let input = fixture.0.join("unrelated-source.irs");
    std::fs::write(
        &input,
        ncae_tool::wav::write_wav_float32(&[0.1, -0.1], 48000, 1),
    )
    .unwrap();
    let mut app = NcaeGuiApp::empty(fixture.0.clone(), fixture.0.clone());
    app.start_job(
        Job::ConvertToWav {
            source: input,
            output_dir: app.output_dir.clone(),
        },
        &ctx,
    );
    wait(&mut app);
    assert!(app.notice.as_ref().unwrap().starts_with("WAV 导出完成"));
    assert_eq!(
        app.last_output.unwrap().file_name().unwrap(),
        "unrelated-source.wav"
    );
}

#[test]
fn replace_export_and_generate_only_have_separate_disk_destinations() {
    let fixture = Fixture::new();
    let effects = fixture.0.join("effects");
    std::fs::create_dir(&effects).unwrap();
    let target = effects.join("target.ncae");
    ncae_tool::write_ncae(
        &target,
        &[1, 2, 3, 4],
        &[1, 2, 3, 4, 0],
        &[0, 0, 0, 0, 1, 0, 2, 0],
        &ncae_tool::wav::write_wav_float32(&[0.1, 0.1], 48000, 1),
    )
    .unwrap();
    let input = fixture.0.join("input.wav");
    std::fs::write(
        &input,
        ncae_tool::wav::write_wav_float32(&[0.3, -0.1], 48000, 1),
    )
    .unwrap();
    let before = std::fs::read(&target).unwrap();
    let ctx = egui::Context::default();
    let mut app = NcaeGuiApp::empty(effects.clone(), fixture.0.join("software"));
    app.output_dir = fixture.0.join("Downloads");
    app.effects = ncae_tool::list_effects(&effects).unwrap();
    app.selected = Some(0);
    app.start_job(Job::Inspect(input), &ctx);
    wait(&mut app);
    app.request_generate(&ctx);
    wait(&mut app);
    assert_ne!(std::fs::read(&target).unwrap(), before);
    assert!(!app.output_dir.exists());
    assert!(!app.generated_dir.exists());
    assert!(app.last_output.is_none());
    app.start_job(
        Job::Decrypt {
            path: target,
            output_dir: app.output_dir.clone(),
        },
        &ctx,
    );
    wait(&mut app);
    assert!(app
        .last_output
        .as_ref()
        .unwrap()
        .starts_with(&app.output_dir));
    let count = std::fs::read_dir(&app.output_dir).unwrap().count();
    app.replace_after = false;
    app.request_generate(&ctx);
    wait(&mut app);
    assert!(app.last_output.unwrap().starts_with(&app.generated_dir));
    assert_eq!(std::fs::read_dir(&app.output_dir).unwrap().count(), count);
}

#[test]
fn source_exports_keep_dots_and_numeric_suffixes_and_never_overwrite() {
    let fixture = Fixture::new();
    let source = fixture.0.join("mix.ncae-1791470979752.irs");
    let first = ncae_tool::export_source_bytes(&fixture.0, &source, "wav", b"first").unwrap();
    let second = ncae_tool::export_source_bytes(&fixture.0, &source, "wav", b"second").unwrap();
    assert_eq!(first.file_name().unwrap(), "mix.ncae-1791470979752.wav");
    assert_eq!(
        second.file_name().unwrap(),
        "mix.ncae-1791470979752 (1).wav"
    );
    assert_eq!(std::fs::read(first).unwrap(), b"first");
}
