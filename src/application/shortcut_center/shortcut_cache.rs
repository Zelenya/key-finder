use crate::storage::NotificationSnapshot;
use std::sync::{Arc, RwLock};

#[derive(Clone)]
pub(crate) struct ShortcutCache {
    inner: Arc<RwLock<NotificationSnapshot>>,
}

impl ShortcutCache {
    pub(crate) fn new(snapshot: NotificationSnapshot) -> Self {
        Self {
            inner: Arc::new(RwLock::new(snapshot)),
        }
    }

    pub(crate) fn replace(&self, snapshot: NotificationSnapshot) {
        match self.inner.write() {
            Ok(mut guard) => {
                let old = std::mem::replace(&mut *guard, snapshot);
                drop(guard);
                drop(old);
            }
            Err(poisoned) => {
                let mut guard = poisoned.into_inner();
                let old = std::mem::replace(&mut *guard, snapshot);
                drop(guard);
                drop(old);
            }
        }
    }

    pub(crate) fn snapshot(&self) -> NotificationSnapshot {
        match self.inner.read() {
            Ok(guard) => guard.clone(),
            Err(poisoned) => poisoned.into_inner().clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::ShortcutCache;
    use crate::storage::{NotificationApp, NotificationShortcut, NotificationSnapshot};

    #[test]
    fn shorcut_cache_shares_snapshot_state() {
        let cache = ShortcutCache::new(NotificationSnapshot {
            apps: vec![NotificationApp {
                app_id: 1.into(),
                name: "Zed".to_string(),
                aliases: vec!["Zed".to_string()],
            }],
            shortcuts: vec![NotificationShortcut {
                id: 1.into(),
                app_id: 1.into(),
                shortcut: "⌘ B".to_string(),
                description: "Toggle dock".to_string(),
            }],
        });
        let clone = cache.clone();

        clone.replace(NotificationSnapshot {
            apps: vec![NotificationApp {
                app_id: 2.into(),
                name: "Visual Studio Code".to_string(),
                aliases: vec!["Visual Studio Code".to_string()],
            }],
            shortcuts: vec![NotificationShortcut {
                id: 2.into(),
                app_id: 2.into(),
                shortcut: "⌘ P".to_string(),
                description: "Go to file".to_string(),
            }],
        });

        let snapshot = cache.snapshot();
        assert_eq!(snapshot.shortcuts.len(), 1);
        assert_eq!(snapshot.apps[0].name, "Visual Studio Code");
        assert_eq!(snapshot.shortcuts[0].description, "Go to file");
    }
}
