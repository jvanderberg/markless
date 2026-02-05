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
    pub fn new(path: impl AsRef<Path>, debounce: Duration) -> notify::Result<Self> {
        let target_path = path.as_ref().to_path_buf();
        let target_name = target_path.file_name().map(|s| s.to_os_string());
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

    /// Returns true once a debounced file change is ready.
    pub fn take_change_ready(&mut self) -> bool {
        let mut saw_relevant_event = false;
        while let Ok(event) = self.rx.try_recv() {
            match event {
                Ok(ev) if self.is_relevant(&ev) => {
                    saw_relevant_event = true;
                }
                Ok(_) | Err(_) => {}
            }
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
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."))
}

#[cfg(test)]
mod tests {
    use super::*;
    use notify::EventKind;
    use tempfile::tempdir;

    #[test]
    fn test_directory_level_event_is_relevant_for_watched_file() {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("doc.md");
        std::fs::write(&path, "hi").expect("write");
        let watcher = FileWatcher::new(&path, Duration::from_millis(10)).expect("watcher");

        let event = Event {
            kind: EventKind::Any,
            paths: vec![dir.path().to_path_buf()],
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
}
