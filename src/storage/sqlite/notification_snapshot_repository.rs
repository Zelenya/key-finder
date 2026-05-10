use crate::domain::errors::AppError;
use crate::domain::shortcut_norm::render_canonical_shortcut;
use crate::storage::sqlite::SqliteDb;
use crate::storage::{AppId, NotificationApp, NotificationShortcut, NotificationSnapshot};
use rusqlite::Connection;
use std::collections::HashMap;

#[derive(Clone, Debug)]
pub(crate) struct SqliteNotificationSnapshotRepository {
    db: SqliteDb,
}

impl SqliteNotificationSnapshotRepository {
    pub(crate) fn new(db: SqliteDb) -> Self {
        Self { db }
    }

    pub(crate) fn load_notification_snapshot(&self) -> Result<NotificationSnapshot, AppError> {
        self.db.with_connection("load notification snapshot", |conn| {
            Ok(NotificationSnapshot {
                shortcuts: load_notification_shortcuts(conn)?,
                apps: load_notification_apps(conn)?,
            })
        })
    }
}

fn load_notification_shortcuts(conn: &Connection) -> Result<Vec<NotificationShortcut>, AppError> {
    let mut stmt = conn
        .prepare(
            "select s.id,
                    s.app_id,
                    s.shortcut_norm,
                    s.description
             from shortcuts s
             where s.state = 'active'
             order by s.app_id, s.id",
        )
        .map_err(|source| AppError::Database {
            operation: "prepare notification shortcut query".to_string(),
            source,
        })?;

    let rows = stmt
        .query_map([], |row| {
            let id = row.get(0)?;
            let app_id: AppId = row.get(1)?;
            let shortcut_norm: String = row.get(2)?;
            let description: String = row.get(3)?;
            Ok(NotificationShortcut {
                id,
                app_id,
                shortcut: render_canonical_shortcut(&shortcut_norm),
                description,
            })
        })
        .map_err(|source| AppError::Database {
            operation: "run notification shortcut query".to_string(),
            source,
        })?;

    rows.collect::<Result<Vec<_>, _>>().map_err(|source| AppError::Database {
        operation: "collect notification shortcuts".to_string(),
        source,
    })
}

fn load_notification_apps(conn: &Connection) -> Result<Vec<NotificationApp>, AppError> {
    let mut aliases_by_app = load_app_aliases(conn)?;

    let mut stmt = conn
        .prepare(
            "select id, name
             from apps
             order by name collate nocase",
        )
        .map_err(|source| AppError::Database {
            operation: "prepare notification app query".to_string(),
            source,
        })?;

    let rows = stmt
        .query_map([], |row| {
            let app_id: AppId = row.get(0)?;
            let name: String = row.get(1)?;
            let aliases = aliases_by_app.remove(&app_id).unwrap_or_default();
            Ok(NotificationApp {
                app_id,
                name,
                aliases,
            })
        })
        .map_err(|source| AppError::Database {
            operation: "run notification app query".to_string(),
            source,
        })?;

    rows.collect::<Result<Vec<_>, _>>().map_err(|source| AppError::Database {
        operation: "collect notification apps".to_string(),
        source,
    })
}

fn load_app_aliases(conn: &Connection) -> Result<HashMap<AppId, Vec<String>>, AppError> {
    let mut aliases_by_app = HashMap::<AppId, Vec<String>>::new();
    let mut stmt = conn
        .prepare("select app_id, alias from app_aliases order by alias collate nocase")
        .map_err(|source| AppError::Database {
            operation: "prepare app alias notification query".to_string(),
            source,
        })?;
    let rows = stmt.query_map([], |row| Ok((row.get::<_, AppId>(0)?, row.get::<_, String>(1)?))).map_err(
        |source| AppError::Database {
            operation: "run app alias notification query".to_string(),
            source,
        },
    )?;

    for row in rows {
        let (app_id, alias) = row.map_err(|source| AppError::Database {
            operation: "collect app alias notification rows".to_string(),
            source,
        })?;
        aliases_by_app.entry(app_id).or_default().push(alias);
    }

    Ok(aliases_by_app)
}

#[cfg(test)]
mod tests {
    use super::SqliteNotificationSnapshotRepository;
    use crate::storage::sqlite::{SqliteAppsRepository, SqliteDb, SqliteShortcutsRepository};
    use tempfile::tempdir;

    #[test]
    fn notification_snapshot_includes_app_aliases() {
        let dir = tempdir().expect("temp dir");
        let db = SqliteDb::open(dir.path().join("library.db")).expect("db");
        let apps = SqliteAppsRepository::new(db.clone());
        let shortcuts = SqliteShortcutsRepository::new(db.clone());
        let repo = SqliteNotificationSnapshotRepository::new(db);

        let app_id = apps.create_custom_app("Cool Studio", &["my code".to_string()]).expect("create app");
        shortcuts.add_shortcut(app_id, "⌘ K", "Do the thing").expect("add shortcut");

        let snapshot = repo.load_notification_snapshot().expect("load snapshot");
        assert_eq!(snapshot.shortcuts.len(), 1);
        assert_eq!(
            snapshot.apps.iter().find(|app| app.app_id == app_id).expect("notification app").aliases,
            vec!["my code".to_string()]
        );
        assert_eq!(snapshot.shortcuts[0].app_id, app_id);
    }
}
