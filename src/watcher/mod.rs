//! File watching for live reload.
//!
//! Uses notify crate for cross-platform file system events.
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver};
use std::time::{Duration, Instant};

use notify::{Event, RecommendedWatcher, RecursiveMode, Watcher};

/// Watches a single file and emits debounced change notifications.
pub struct FileWatcher {
    _watcher: RecommendedWatcher,
    rx: Receiver<notify::Result<Event>>,
    watch_root: PathBuf,
    target_path: PathBuf,
    target_name: Option<OsString>,
    debounce: Duration,
    pending_since: Option<Instant>,
}

impl FileWatcher {
    /// Create a watcher for `path`.
    ///
    /// # Errors
    /// Returns an error if the file watcher cannot be created or the path cannot be watched.
    pub fn new(path: impl AsRef<Path>, debounce: Duration) -> notify::Result<Self> {
        // Canonicalize so event paths from the OS (which are always absolute
        // and canonical) match our stored paths.
        let target_path = path
            .as_ref()
            .canonicalize()
            .unwrap_or_else(|_| path.as_ref().to_path_buf());
        let target_name = target_path.file_name().map(std::ffi::OsStr::to_os_string);
        let watch_root = watch_root_for(&target_path);

        let (tx, rx) = mpsc::channel();
        let mut watcher = notify::recommended_watcher(move |res| {
            let _ = tx.send(res);
        })?;
        watcher.watch(&watch_root, RecursiveMode::NonRecursive)?;

        Ok(Self {
            _watcher: watcher,
            rx,
            watch_root,
            target_path,
            target_name,
            debounce,
            pending_since: None,
        })
    }

    /// The canonical path of the file being watched.
    pub fn target_path(&self) -> &Path {
        &self.target_path
    }

    /// Returns true once a debounced file change is ready.
    pub fn take_change_ready(&mut self) -> bool {
        let mut saw_relevant_event = false;
        let mut total_events = 0u32;
        let mut irrelevant_events = 0u32;
        while let Ok(event) = self.rx.try_recv() {
            total_events += 1;
            match event {
                Ok(ev) if self.is_relevant(&ev) => {
                    saw_relevant_event = true;
                }
                Ok(ev) => {
                    irrelevant_events += 1;
                    crate::perf::log_event(
                        "watcher.irrelevant",
                        format!("kind={:?} paths={:?}", ev.kind, ev.paths,),
                    );
                }
                Err(err) => {
                    crate::perf::log_event("watcher.error", format!("{err}"));
                }
            }
        }

        if total_events > 0 {
            crate::perf::log_event(
                "watcher.poll",
                format!(
                    "total={total_events} relevant={} irrelevant={irrelevant_events} target={} root={}",
                    if saw_relevant_event { "yes" } else { "no" },
                    self.target_path.display(),
                    self.watch_root.display(),
                ),
            );
        }

        if saw_relevant_event {
            self.pending_since = Some(Instant::now());
        }

        let Some(pending_since) = self.pending_since else {
            return false;
        };
        if pending_since.elapsed() >= self.debounce {
            self.pending_since = None;
            return true;
        }
        false
    }

    fn is_relevant(&self, event: &Event) -> bool {
        event.paths.iter().any(|path| {
            path == &self.watch_root
                || path == &self.target_path
                || self
                    .target_name
                    .as_ref()
                    .is_some_and(|name| path.file_name().is_some_and(|f| f == name))
        })
    }
}

fn watch_root_for(path: &Path) -> PathBuf {
    path.parent()
        .filter(|p| !p.as_os_str().is_empty())
        .map_or_else(|| PathBuf::from("."), Path::to_path_buf)
}

#[cfg(test)]
mod tests {
    use super::*;
    use notify::EventKind;
    use tempfile::tempdir;

    #[test]
    fn test_directory_level_event_is_relevant_for_watched_file() {
        let dir = tempdir().expect("tempdir");
        let canonical_dir = dir.path().canonicalize().expect("canonicalize");
        let path = canonical_dir.join("doc.md");
        std::fs::write(&path, "hi").expect("write");
        let watcher = FileWatcher::new(&path, Duration::from_millis(10)).expect("watcher");

        // Event with canonical directory path (as macOS FSEvents would report)
        let event = Event {
            kind: EventKind::Any,
            paths: vec![canonical_dir],
            attrs: notify::event::EventAttributes::new(),
        };

        assert!(
            watcher.is_relevant(&event),
            "directory-level events should count as relevant for many backends"
        );
    }

    #[test]
    fn test_watch_root_for_relative_file_is_dot() {
        let root = watch_root_for(Path::new("TEST-README.md"));
        assert_eq!(root, PathBuf::from("."));
    }

    #[test]
    fn test_real_file_modification_detected() {
        let dir = tempdir().expect("tempdir");
        let canonical_dir = dir.path().canonicalize().expect("canonicalize");
        let path = canonical_dir.join("watched.txt");
        std::fs::write(&path, "original").expect("write");

        let mut watcher = FileWatcher::new(&path, Duration::from_millis(50)).expect("watcher");

        // Give FSEvents time to register the watch
        std::thread::sleep(Duration::from_millis(500));

        // Modify the file
        std::fs::write(&path, "modified").expect("write");

        // Poll until the change is ready or timeout after 5 seconds
        let deadline = Instant::now() + Duration::from_secs(5);
        let mut detected = false;
        while Instant::now() < deadline {
            if watcher.take_change_ready() {
                detected = true;
                break;
            }
            std::thread::sleep(Duration::from_millis(50));
        }

        assert!(
            detected,
            "watcher should detect real file modification within 5 seconds"
        );
    }

    /// Test with the same debounce and poll interval as the real event loop.
    #[test]
    fn test_real_modification_with_app_timing() {
        let dir = tempdir().expect("tempdir");
        let canonical_dir = dir.path().canonicalize().expect("canonicalize");
        let path = canonical_dir.join("watched.txt");
        std::fs::write(&path, "original").expect("write");

        // Same debounce as the real app (200ms)
        let mut watcher = FileWatcher::new(&path, Duration::from_millis(200)).expect("watcher");

        // Give FSEvents time to register
        std::thread::sleep(Duration::from_millis(500));

        // Modify the file (simulates another process saving)
        std::fs::write(&path, "modified by another process").expect("write");

        // Poll at 250ms intervals (same as event loop)
        let deadline = Instant::now() + Duration::from_secs(5);
        let mut detected = false;
        while Instant::now() < deadline {
            if watcher.take_change_ready() {
                detected = true;
                break;
            }
            std::thread::sleep(Duration::from_millis(250));
        }

        assert!(
            detected,
            "watcher should detect modification with real app timing (200ms debounce, 250ms poll)"
        );
    }

    #[test]
    fn test_canonical_event_path_matches_relative_watcher() {
        let dir = tempdir().expect("tempdir");
        let relative_path = dir.path().join("LICENSE");
        std::fs::write(&relative_path, "MIT").expect("write");
        let watcher = FileWatcher::new(&relative_path, Duration::from_millis(10)).expect("watcher");

        // macOS FSEvents reports canonical absolute paths
        let canonical_dir = dir.path().canonicalize().expect("canonicalize");
        let event = Event {
            kind: EventKind::Any,
            paths: vec![canonical_dir],
            attrs: notify::event::EventAttributes::new(),
        };

        assert!(
            watcher.is_relevant(&event),
            "canonical event paths should match even when watcher was created with non-canonical path"
        );
    }
}
