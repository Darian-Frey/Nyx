//! File watcher using `notify`.
//!
//! Watches a sketch file for modifications and sends reload events
//! through a channel.

use std::path::Path;
use std::sync::mpsc;
use std::time::Duration;

use notify::{Config, EventKind, RecommendedWatcher, RecursiveMode, Watcher};

/// Watch a file for modifications.
///
/// Returns a receiver that emits `()` each time the file is modified.
/// The watcher handle must be kept alive.
pub fn watch_file(
    path: &Path,
) -> Result<(mpsc::Receiver<()>, RecommendedWatcher), notify::Error> {
    let (tx, rx) = mpsc::channel();

    let mut watcher = RecommendedWatcher::new(
        move |res: Result<notify::Event, notify::Error>| {
            if let Ok(event) = res && matches!(event.kind, EventKind::Modify(_) | EventKind::Create(_)) {
                    let _ = tx.send(());
            }
        },
        Config::default().with_poll_interval(Duration::from_millis(200)),
    )?;

    watcher.watch(path, RecursiveMode::NonRecursive)?;

    Ok((rx, watcher))
}
