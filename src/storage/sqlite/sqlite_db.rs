use crate::domain::errors::AppError;
use crate::storage::sqlite::apps_repository::SqliteAppsRepository;
use crate::storage::sqlite::notification_snapshot_repository::SqliteNotificationSnapshotRepository;
use crate::storage::sqlite::settings_repository::SqliteSettingsRepository;
use crate::storage::sqlite::shortcut_catalog_repository::SqliteShortcutCatalogRepository;
use crate::storage::sqlite::shortcut_imports_repository::SqliteShortcutImportsRepository;
use crate::storage::sqlite::shortcuts_repository::SqliteShortcutsRepository;
use rusqlite::{Connection, Transaction};
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

const SCHEMA_SQL: &str = include_str!("../migrations/0001_initial.sql");
const SEED_SQL: &str = include_str!("../migrations/0002_seed_data.sql");
const APP_IMPORTERS_SCHEMA_SQL: &str = include_str!("../migrations/0003_app_importers.sql");
const APP_IMPORTERS_SEED_SQL: &str = include_str!("../migrations/0004_seed_app_importers.sql");

#[derive(Clone, Debug)]
pub(crate) struct SqliteDb {
    db_path: PathBuf,
}

impl SqliteDb {
    pub(crate) fn open(db_path: impl Into<PathBuf>) -> Result<Self, AppError> {
        let db = Self {
            db_path: db_path.into(),
        };
        db.init_schema()?;
        Ok(db)
    }

    pub(crate) fn with_connection<T>(
        &self,
        operation: &str,
        f: impl FnOnce(&Connection) -> Result<T, AppError>,
    ) -> Result<T, AppError> {
        let conn = self.connection()?;
        f(&conn).map_err(|error| annotate_error(error, operation))
    }

    pub(crate) fn with_transaction<T>(
        &self,
        operation: &str,
        f: impl FnOnce(&Transaction<'_>) -> Result<T, AppError>,
    ) -> Result<T, AppError> {
        let mut conn = self.connection()?;
        let tx = conn.transaction().map_err(|source| AppError::Database {
            operation: format!("start {operation} transaction"),
            source,
        })?;

        let result = f(&tx).map_err(|error| annotate_error(error, operation))?;

        tx.commit().map_err(|source| AppError::Database {
            operation: format!("commit {operation} transaction"),
            source,
        })?;

        Ok(result)
    }

    pub(crate) fn settings_repository(&self) -> SqliteSettingsRepository {
        SqliteSettingsRepository::new(self.clone())
    }

    pub(crate) fn shortcuts_repository(&self) -> SqliteShortcutsRepository {
        SqliteShortcutsRepository::new(self.clone())
    }

    pub(crate) fn shortcut_catalog_repository(&self) -> SqliteShortcutCatalogRepository {
        SqliteShortcutCatalogRepository::new(self.clone())
    }

    pub(crate) fn notification_snapshot_repository(&self) -> SqliteNotificationSnapshotRepository {
        SqliteNotificationSnapshotRepository::new(self.clone())
    }

    pub(crate) fn shortcut_imports_repository(&self) -> SqliteShortcutImportsRepository {
        SqliteShortcutImportsRepository::new(self.clone())
    }

    pub(crate) fn apps_repository(&self) -> SqliteAppsRepository {
        SqliteAppsRepository::new(self.clone())
    }

    fn init_schema(&self) -> Result<(), AppError> {
        let conn = self.connection()?;
        conn.execute_batch(SCHEMA_SQL).map_err(|source| AppError::Database {
            operation: "initialize sqlite schema".to_string(),
            source,
        })?;
        conn.execute_batch(SEED_SQL).map_err(|source| AppError::Database {
            operation: "initialize sqlite seed data".to_string(),
            source,
        })?;
        conn.execute_batch(APP_IMPORTERS_SCHEMA_SQL).map_err(|source| AppError::Database {
            operation: "initialize sqlite app importers schema".to_string(),
            source,
        })?;
        conn.execute_batch(APP_IMPORTERS_SEED_SQL).map_err(|source| AppError::Database {
            operation: "initialize sqlite app importer seed data".to_string(),
            source,
        })
    }

    fn connection(&self) -> Result<Connection, AppError> {
        if let Some(parent) = self.db_path.parent() {
            std::fs::create_dir_all(parent).map_err(|source| AppError::DatabaseIo {
                operation: "create database parent directory".to_string(),
                path: parent.to_path_buf(),
                source,
            })?;
        }

        Connection::open(&self.db_path).map_err(|source| AppError::Database {
            operation: format!("open database at {}", self.db_path.display()),
            source,
        })
    }
}

pub(crate) fn now_unix() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).expect("system time before UNIX_EPOCH").as_secs() as i64
}

fn annotate_error(error: AppError, operation: &str) -> AppError {
    match error {
        AppError::Database {
            operation: inner_operation,
            source,
        } => AppError::Database {
            operation: format!("{operation}: {inner_operation}"),
            source,
        },
        AppError::DatabaseIo {
            operation: inner_operation,
            path,
            source,
        } => AppError::DatabaseIo {
            operation: format!("{operation}: {inner_operation}"),
            path,
            source,
        },
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::SqliteDb;
    use rusqlite::Connection;
    use tempfile::tempdir;

    #[test]
    fn open_initializes_database_with_current_shortcuts_schema() {
        let dir = tempdir().expect("temp dir");
        let db_path = dir.path().join("library.db");
        SqliteDb::open(&db_path).expect("open db");
        let conn = Connection::open(&db_path).expect("reopen db");
        let mut stmt = conn.prepare("PRAGMA table_info(shortcuts)").expect("prepare table_info");
        let columns = stmt
            .query_map([], |row| row.get::<_, String>(1))
            .expect("run table_info")
            .collect::<Result<Vec<_>, _>>()
            .expect("collect columns");
        assert!(columns.iter().any(|column| column == "description"));
        assert!(!columns.iter().any(|column| column == "command"));
        assert!(!columns.iter().any(|column| column == "description_user"));
        assert!(!columns.iter().any(|column| column == "sort_index"));

        let mut alias_stmt =
            conn.prepare("PRAGMA table_info(app_aliases)").expect("prepare app_aliases table_info");
        let alias_columns = alias_stmt
            .query_map([], |row| row.get::<_, String>(1))
            .expect("run app_aliases table_info")
            .collect::<Result<Vec<_>, _>>()
            .expect("collect app_aliases columns");
        assert!(alias_columns.iter().any(|column| column == "canonical_alias"));

        let mut apps_stmt = conn.prepare("PRAGMA table_info(apps)").expect("prepare apps table_info");
        let app_columns = apps_stmt
            .query_map([], |row| row.get::<_, String>(1))
            .expect("run apps table_info")
            .collect::<Result<Vec<_>, _>>()
            .expect("collect apps columns");
        assert!(!app_columns.iter().any(|column| column == "importer_kind"));
        assert!(!app_columns.iter().any(|column| column == "last_import_at"));
        assert!(!app_columns.iter().any(|column| column == "import_status"));
        assert!(!app_columns.iter().any(|column| column == "notes"));

        let mut app_importers_stmt =
            conn.prepare("PRAGMA table_info(app_importers)").expect("prepare app_importers table_info");
        let app_importer_columns = app_importers_stmt
            .query_map([], |row| row.get::<_, String>(1))
            .expect("run app_importers table_info")
            .collect::<Result<Vec<_>, _>>()
            .expect("collect app_importers columns");
        assert_eq!(
            app_importer_columns,
            vec!["app_id".to_string(), "importer_family".to_string()]
        );

        let mut imports_stmt =
            conn.prepare("PRAGMA table_info(imports)").expect("prepare imports table_info");
        let import_columns = imports_stmt
            .query_map([], |row| row.get::<_, String>(1))
            .expect("run imports table_info")
            .collect::<Result<Vec<_>, _>>()
            .expect("collect imports columns");
        assert!(!import_columns.iter().any(|column| column == "summary_json"));
    }
}
