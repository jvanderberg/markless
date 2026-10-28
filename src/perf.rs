//! Lightweight performance instrumentation.

use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{LazyLock, Mutex};
use std::time::Instant;

static ENABLED: AtomicBool = AtomicBool::new(false);
static DEBUG_LOGGER: LazyLock<Mutex<DebugLogger>> =
    LazyLock::new(|| Mutex::new(DebugLogger::new()));

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

/// Set (or clear) the debug log output path.
///
/// # Errors
/// Returns an error if the log file cannot be created or written to.
pub fn set_debug_log_path(path: Option<&Path>) -> std::io::Result<()> {
    let mut logger = DEBUG_LOGGER
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
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
    drop(logger);
    Ok(())
}

/// Check whether debug logging is currently enabled.
pub fn is_debug_log_enabled() -> bool {
    DEBUG_LOGGER
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .enabled
}

/// Write a timestamped event to the debug log.
pub fn log_event(name: &str, detail: impl AsRef<str>) {
    let mut logger = DEBUG_LOGGER
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
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

/// `io::Write` sink for `tracing` output.
///
/// The TUI owns the terminal, so log lines written to stdout or stderr would
/// scroll the screen behind ratatui's back and leave stale text (issue #66).
/// Output goes to the debug log when one is set and is dropped otherwise.
///
/// ```
/// use std::io::Write;
/// markless::perf::DebugLogWriter.write_all(b"dropped without a debug log\n").unwrap();
/// ```
#[derive(Debug)]
pub struct DebugLogWriter;

impl Write for DebugLogWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let mut logger = DEBUG_LOGGER
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        // Errors are ignored: tracing-subscriber reports writer errors with
        // eprintln!, which would reach the terminal again.
        if let Some(writer) = logger.writer.as_mut() {
            let _ = writer.write_all(buf);
            let _ = writer.flush();
        }
        drop(logger);
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    // Tests below share the global DEBUG_LOGGER, so they must not run in parallel.
    static DEBUG_LOG_TEST_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn test_set_enabled_toggles_runtime_flag() {
        set_enabled(true);
        assert!(is_enabled());

        set_enabled(false);
        assert!(!is_enabled());
    }

    #[test]
    fn test_debug_log_path_enables_logging_and_writes() {
        let _guard = DEBUG_LOG_TEST_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let temp_file = NamedTempFile::new().unwrap();
        set_debug_log_path(Some(temp_file.path())).unwrap();
        assert!(is_debug_log_enabled());
        log_event("test.event", "hello world");
        DebugLogWriter
            .write_all(b"usvg::text: Fallback from FreeSans\n")
            .unwrap();
        set_debug_log_path(None).unwrap();

        let content = std::fs::read_to_string(temp_file.path()).unwrap();
        assert!(content.contains("markless render debug log start"));
        assert!(content.contains("test.event: hello world"));
        assert!(content.contains("usvg::text: Fallback from FreeSans"));
    }

    #[test]
    fn test_debug_log_writer_without_log_path_drops_output() {
        let _guard = DEBUG_LOG_TEST_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let temp_file = NamedTempFile::new().unwrap();
        set_debug_log_path(Some(temp_file.path())).unwrap();
        set_debug_log_path(None).unwrap();

        // Tracing output must not reach the terminal while the TUI owns it.
        assert_eq!(DebugLogWriter.write(b"dropped\n").unwrap(), 8);

        let content = std::fs::read_to_string(temp_file.path()).unwrap();
        assert!(!content.contains("dropped"));
    }
}
