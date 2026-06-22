#![cfg(windows)]

use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;
use std::sync::Mutex;

static LOG_PATH: Mutex<Option<PathBuf>> = Mutex::new(None);

fn default_log_path() -> PathBuf {
    std::env::var_os("TEMP")
        .or_else(|| std::env::var_os("TMP"))
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
        .join("blackhole-screensaver.log")
}

struct FileLogger {
    level: log::LevelFilter,
}

impl log::Log for FileLogger {
    fn enabled(&self, metadata: &log::Metadata) -> bool {
        metadata.level() <= self.level
    }

    fn log(&self, record: &log::Record) {
        if !self.enabled(record.metadata()) {
            return;
        }
        let path = LOG_PATH.lock().ok().and_then(|g| g.clone());
        let Some(path) = path else {
            return;
        };
        if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(&path) {
            let _ = writeln!(
                f,
                "[{} {} {}:{}] {}",
                chrono_like_now(),
                record.level(),
                record.target(),
                record.line().unwrap_or(0),
                record.args()
            );
        }
    }

    fn flush(&self) {}
}

// Avoid pulling in chrono just for a timestamp.
fn chrono_like_now() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let h = (secs / 3600) % 24;
    let m = (secs / 60) % 60;
    let s = secs % 60;
    format!("{:02}:{:02}:{:02}Z", h, m, s)
}

pub fn init() {
    let path = default_log_path();
    if let Ok(mut g) = LOG_PATH.lock() {
        *g = Some(path.clone());
    }

    // Rotate if too big (>1 MiB) so it doesn't grow forever.
    if let Ok(meta) = std::fs::metadata(&path) {
        if meta.len() > 1_048_576 {
            let _ = std::fs::remove_file(&path);
        }
    }

    let logger = Box::leak(Box::new(FileLogger {
        level: log::LevelFilter::Info,
    }));
    let _ = log::set_logger(logger);
    log::set_max_level(log::LevelFilter::Info);

    std::panic::set_hook(Box::new(|info| {
        let payload = if let Some(s) = info.payload().downcast_ref::<&str>() {
            (*s).to_string()
        } else if let Some(s) = info.payload().downcast_ref::<String>() {
            s.clone()
        } else {
            "<non-string panic payload>".to_string()
        };
        let loc = info
            .location()
            .map(|l| format!("{}:{}", l.file(), l.line()))
            .unwrap_or_else(|| "<unknown>".to_string());
        log::error!("PANIC at {loc}: {payload}");
    }));

    log::info!("=== blackhole-screensaver start (log: {}) ===", path.display());
}
