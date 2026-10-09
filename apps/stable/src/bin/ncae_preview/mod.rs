pub mod core;
use self::core::{Model, Options};
use anyhow::{ensure, Context};
use std::path::{Path, PathBuf};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    mpsc::{self, Receiver},
    Arc,
};

#[derive(Clone, Debug, PartialEq, Eq)]
struct Key {
    path: PathBuf,
    options: Options,
    revision: u64,
}
struct Running {
    key: Key,
    receiver: Receiver<Result<Model, String>>,
    cancel: Arc<AtomicBool>,
}
pub struct Controller {
    pub expanded: bool,
    pub show_markers: bool,
    pub module: String,
    pub options: Options,
    pub model: Option<Arc<Model>>,
    pub error: Option<String>,
    wanted: Option<Key>,
    running: Option<Running>,
    revision: u64,
    completed: bool,
}
impl Default for Controller {
    fn default() -> Self {
        Self {
            expanded: true,
            show_markers: false,
            module: String::new(),
            options: Options::default(),
            model: None,
            error: None,
            wanted: None,
            running: None,
            revision: 0,
            completed: false,
        }
    }
}
impl Drop for Controller {
    fn drop(&mut self) {
        if let Some(job) = &self.running {
            job.cancel.store(true, Ordering::Relaxed);
        }
    }
}
impl Controller {
    pub fn invalidate(&mut self) {
        self.revision = self.revision.wrapping_add(1);
    }
    #[cfg(test)]
    pub fn loading(&self) -> bool {
        self.wanted.is_some() && !self.completed
    }
    #[cfg(test)]
    pub fn target(&self) -> Option<&Path> {
        self.wanted.as_ref().map(|key| key.path.as_path())
    }
    pub fn update(&mut self, ctx: &egui::Context, path: Option<PathBuf>) {
        let changed_path = self.wanted.as_ref().map(|key| &key.path) != path.as_ref();
        if changed_path {
            self.options.channel = None;
            self.module.clear();
        }
        let wanted = path.map(|path| Key {
            path,
            options: self.options,
            revision: self.revision,
        });
        if wanted != self.wanted {
            if let Some(job) = &self.running {
                job.cancel.store(true, Ordering::Relaxed);
            }
            self.wanted = wanted;
            self.model = None;
            self.error = None;
            self.completed = false;
        }
        self.poll(ctx);
        if self.expanded && !self.completed && self.running.is_none() {
            if let Some(key) = self.wanted.clone() {
                let (sender, receiver) = mpsc::channel();
                let cancel = Arc::new(AtomicBool::new(false));
                let flag = cancel.clone();
                let worker_key = key.clone();
                let context = ctx.clone();
                match std::thread::Builder::new()
                    .name("ncae-selected-preview".into())
                    .spawn(move || {
                        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                            load(&worker_key.path, worker_key.options, &flag)
                        }))
                        .map_err(|_| "预览计算异常，未修改任何文件".to_string())
                        .and_then(|result| result.map_err(|error| format!("{error:#}")));
                        let _ = sender.send(result);
                        context.request_repaint();
                    }) {
                    Ok(_) => {
                        self.running = Some(Running {
                            key,
                            receiver,
                            cancel,
                        })
                    }
                    Err(error) => {
                        self.error = Some(format!("无法启动预览: {error}"));
                        self.completed = true;
                    }
                }
            }
        }
        if self.running.is_some() {
            ctx.request_repaint_after(std::time::Duration::from_millis(60));
        }
    }
    fn poll(&mut self, _ctx: &egui::Context) {
        let result = self
            .running
            .as_ref()
            .and_then(|job| match job.receiver.try_recv() {
                Ok(result) => Some(result),
                Err(mpsc::TryRecvError::Disconnected) => Some(Err("预览线程已结束".into())),
                Err(mpsc::TryRecvError::Empty) => None,
            });
        if let Some(result) = result {
            let job = self.running.take().unwrap();
            if self.wanted.as_ref() == Some(&job.key) {
                self.completed = true;
                match result {
                    Ok(model) => self.model = Some(Arc::new(model)),
                    Err(error) => self.error = Some(error),
                }
            }
        }
    }
}
fn load(path: &Path, options: Options, cancel: &AtomicBool) -> anyhow::Result<Model> {
    ensure!(
        std::fs::metadata(path)?.len() <= 128 * 1024 * 1024,
        "NCAE 文件超过预览上限"
    );
    let bytes = std::fs::read(path).context("读取选中音效失败")?;
    let plain = ncae_tool::decrypt_ncae_bytes_limited(&bytes, 128 * 1024 * 1024)
        .context("解密所选音效失败")?;
    core::analyze(&plain, options, cancel)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn effect(path: &Path, gain: i32) {
        let plain = serde_json::to_vec(
            &serde_json::json!({"eq":{"on":true,"eqs":[gain,0,0,0,0,0,0,0,0,0]}}),
        )
        .unwrap();
        ncae_tool::write_ncae(
            path,
            &[1, 2, 3, 4],
            &[1, 2, 3, 4, 0],
            &[0, 0, 0, 0, 1, 0, 1, 0],
            &plain,
        )
        .unwrap();
    }
    fn wait(controller: &mut Controller, ctx: &egui::Context, path: &Path) {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        loop {
            controller.update(ctx, Some(path.to_path_buf()));
            if !controller.loading() {
                break;
            }
            assert!(std::time::Instant::now() < deadline, "preview timeout");
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
        assert!(controller.error.is_none(), "{:?}", controller.error);
    }
    #[test]
    fn latest_selection_wins_and_invalidation_reloads_same_path() {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("ncae-preview-{}-{nonce}", std::process::id()));
        std::fs::create_dir(&dir).unwrap();
        let a = dir.join("a.ncae");
        let b = dir.join("b.ncae");
        effect(&a, 1);
        effect(&b, 7);
        let before = std::fs::read(&b).unwrap();
        let ctx = egui::Context::default();
        let mut controller = Controller::default();
        controller.update(&ctx, Some(a));
        controller.update(&ctx, Some(b.clone()));
        wait(&mut controller, &ctx, &b);
        assert_eq!(controller.model.as_ref().unwrap().controls[0][1], 7.0);
        assert_eq!(std::fs::read(&b).unwrap(), before);
        effect(&b, 4);
        controller.invalidate();
        wait(&mut controller, &ctx, &b);
        assert_eq!(controller.model.as_ref().unwrap().controls[0][1], 4.0);
        controller.update(&ctx, None);
        assert!(controller.model.is_none());
        drop(controller);
        std::fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn bounded_decryption_rejects_oversized_plaintext() {
        let data =
            ncae_tool::encrypt_ncae_bytes(&[1, 2, 3, 4], &[1, 2, 3, 4, 0], &[0; 8], &vec![0; 8192])
                .unwrap();
        assert!(ncae_tool::decrypt_ncae_bytes_limited(&data, 128).is_err());
    }
}
