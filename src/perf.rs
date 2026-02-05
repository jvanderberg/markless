//! Lightweight performance instrumentation.

use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::Path;
use std::sync::{LazyLock, Mutex};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

static ENABLED: AtomicBool = AtomicBool::new(false);
static DEBUG_LOGGER: LazyLock<Mutex<DebugLogger>> = LazyLock::new(|| Mutex::new(DebugLogger::new()));

#[derive(Debug)]
pub struct Scope {
    name: &'static str,
    start: Instant,
}

impl Drop for Scope {
    fn drop(&mut self) {
        if !is_enabled() {
            return;
        }
        let elapsed_ms = self.start.elapsed().as_secs_f64() * 1000.0;
        eprintln!("[perf] {}: {:.2} ms", self.name, elapsed_ms);
    }
}

#[derive(Debug)]
struct DebugLogger {
    enabled: bool,
    start: Instant,
    writer: Option<BufWriter<File>>,
}

impl DebugLogger {
    fn new() -> Self {
        Self {
            enabled: false,
            start: Instant::now(),
            writer: None,
        }
    }
}

pub fn set_enabled(enabled: bool) {
    ENABLED.store(enabled, Ordering::Relaxed);
}

pub fn scope(name: &'static str) -> Scope {
    Scope {
        name,
        start: Instant::now(),
    }
}

pub fn is_enabled() -> bool {
    ENABLED.load(Ordering::Relaxed)
}

pub fn set_debug_log_path(path: Option<&Path>) -> std::io::Result<()> {
    let mut logger = DEBUG_LOGGER.lock().expect("debug logger lock poisoned");
    if let Some(path) = path {
        let file = File::create(path)?;
        logger.enabled = true;
        logger.start = Instant::now();
        logger.writer = Some(BufWriter::new(file));
        if let Some(writer) = logger.writer.as_mut() {
            writeln!(writer, "markless render debug log start")?;
            writer.flush()?;
        }
    } else {
        logger.enabled = false;
        logger.writer = None;
    }
    Ok(())
}

pub fn is_debug_log_enabled() -> bool {
    DEBUG_LOGGER
        .lock()
        .expect("debug logger lock poisoned")
        .enabled
}

pub fn log_event(name: &str, detail: impl AsRef<str>) {
    let mut logger = DEBUG_LOGGER.lock().expect("debug logger lock poisoned");
    if !logger.enabled {
        return;
    }
    let elapsed_ms = logger.start.elapsed().as_secs_f64() * 1000.0;
    if let Some(writer) = logger.writer.as_mut() {
        let _ = writeln!(
            writer,
            "[{elapsed_ms:>10.3} ms] {name}: {}",
            detail.as_ref()
        );
        let _ = writer.flush();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    #[test]
    fn test_set_enabled_toggles_runtime_flag() {
        set_enabled(true);
        assert!(is_enabled());

        set_enabled(false);
        assert!(!is_enabled());
    }

    #[test]
    fn test_debug_log_path_enables_logging_and_writes() {
        let temp_file = NamedTempFile::new().unwrap();
        set_debug_log_path(Some(temp_file.path())).unwrap();
        assert!(is_debug_log_enabled());
        log_event("test.event", "hello world");
        set_debug_log_path(None).unwrap();

        let content = std::fs::read_to_string(temp_file.path()).unwrap();
        assert!(content.contains("markless render debug log start"));
        assert!(content.contains("test.event: hello world"));
    }
}
