use crate::storage::ShortcutMessage;
use std::sync::{Arc, RwLock};

#[derive(Clone)]
pub(crate) struct ShortcutCache {
    inner: Arc<RwLock<Vec<ShortcutMessage>>>,
}

impl ShortcutCache {
    pub(crate) fn new(shortcuts: Vec<ShortcutMessage>) -> Self {
        Self {
            inner: Arc::new(RwLock::new(shortcuts)),
        }
    }

    pub(crate) fn replace(&self, shortcuts: Vec<ShortcutMessage>) {
        match self.inner.write() {
            Ok(mut guard) => {
                let old = std::mem::replace(&mut *guard, shortcuts);
                drop(guard);
                drop(old);
            }
            Err(poisoned) => {
                let mut guard = poisoned.into_inner();
                let old = std::mem::replace(&mut *guard, shortcuts);
                drop(guard);
                drop(old);
            }
        }
    }

    pub(crate) fn snapshot(&self) -> Vec<ShortcutMessage> {
        match self.inner.read() {
            Ok(guard) => guard.clone(),
            Err(poisoned) => poisoned.into_inner().clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::ShortcutCache;
    use crate::storage::ShortcutMessage;

    #[test]
    fn shorcut_cache_shares_snapshot_state() {
        let cache = ShortcutCache::new(vec![ShortcutMessage {
            app_id: 1.into(),
            app: "Zed".to_string(),
            match_names: vec!["Zed".to_string()],
            shortcut: "⌘ B".to_string(),
            description: "Toggle dock".to_string(),
        }]);
        let clone = cache.clone();

        clone.replace(vec![ShortcutMessage {
            app_id: 2.into(),
            app: "Visual Studio Code".to_string(),
            match_names: vec!["Visual Studio Code".to_string()],
            shortcut: "⌘ P".to_string(),
            description: "Go to file".to_string(),
        }]);

        let snapshot = cache.snapshot();
        assert_eq!(snapshot.len(), 1);
        assert_eq!(snapshot[0].app, "Visual Studio Code");
        assert_eq!(snapshot[0].description, "Go to file");
    }
}
