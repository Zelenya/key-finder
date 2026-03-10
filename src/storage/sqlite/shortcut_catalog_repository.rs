use crate::domain::errors::AppError;
use crate::storage::models::AppSummary;
use crate::storage::sqlite::SqliteDb;
use crate::storage::AppId;

#[derive(Clone, Debug)]
pub(crate) struct SqliteShortcutCatalogRepository {
    db: SqliteDb,
}

impl SqliteShortcutCatalogRepository {
    pub(crate) fn new(db: SqliteDb) -> Self {
        Self { db }
    }

    pub(crate) fn list_apps(&self) -> Result<Vec<AppSummary>, AppError> {
        self.db.with_connection("list apps", |conn| {
            let mut stmt = conn
                .prepare(
                    "select a.id,
                            a.name,
                            ai.importer_family,
                            coalesce(count(s.id), 0) as total_count,
                            coalesce(sum(case when s.state = 'active' then 1 else 0 end), 0) as active_count
                     from apps a
                     left join app_importers ai on ai.app_id = a.id
                     left join shortcuts s on s.app_id = a.id
                     group by a.id
                     order by a.name collate nocase",
                )
                .map_err(|source| AppError::Database {
                    operation: "prepare app summary query".to_string(),
                    source,
                })?;

            let rows = stmt
                .query_map([], |row| {
                    Ok(AppSummary {
                        app_id: row.get(0)?,
                        name: row.get(1)?,
                        importer: row.get(2)?,
                        total_count: row.get(3)?,
                        active_count: row.get(4)?,
                    })
                })
                .map_err(|source| AppError::Database {
                    operation: "run app summary query".to_string(),
                    source,
                })?;

            rows.collect::<Result<Vec<_>, _>>().map_err(|source| AppError::Database {
                operation: "collect app summary rows".to_string(),
                source,
            })
        })
    }

    pub(crate) fn list_aliases_for_app(&self, app_id: AppId) -> Result<Vec<String>, AppError> {
        self.db.with_connection("list app aliases", |conn| {
            let mut stmt = conn
                .prepare(
                    "select alias
                     from app_aliases
                     where app_id = ?1
                     order by alias collate nocase",
                )
                .map_err(|source| AppError::Database {
                    operation: "prepare app alias query".to_string(),
                    source,
                })?;
            let rows = stmt.query_map([app_id], |row| row.get::<_, String>(0)).map_err(|source| {
                AppError::Database {
                    operation: "run app alias query".to_string(),
                    source,
                }
            })?;

            rows.collect::<Result<Vec<_>, _>>().map_err(|source| AppError::Database {
                operation: "collect app aliases".to_string(),
                source,
            })
        })
    }
}

#[cfg(test)]
mod tests {
    use super::SqliteShortcutCatalogRepository;
    use crate::storage::models::ShortcutState;
    use crate::storage::sqlite::{SqliteAppsRepository, SqliteDb, SqliteShortcutsRepository};
    use tempfile::tempdir;

    #[test]
    fn list_apps_returns_seeded_known_apps() {
        let dir = tempdir().expect("temp dir");
        let db = SqliteDb::open(dir.path().join("library.db")).expect("db");
        let repo = SqliteShortcutCatalogRepository::new(db);

        let apps = repo.list_apps().expect("list apps");
        assert!(apps.iter().any(|app| {
            app.name == "IntelliJ IDEA"
                && app.importer == Some(crate::domain::known_apps::KnownImporterFamily::JetBrains)
        }));
        assert!(apps.iter().any(|app| {
            app.name == "Visual Studio Code"
                && app.importer == Some(crate::domain::known_apps::KnownImporterFamily::VSCode)
        }));
        assert!(apps.iter().any(|app| {
            app.name == "Zed" && app.importer == Some(crate::domain::known_apps::KnownImporterFamily::Zed)
        }));
    }

    #[test]
    fn list_aliases_for_app_returns_custom_aliases() {
        let dir = tempdir().expect("temp dir");
        let db = SqliteDb::open(dir.path().join("library.db")).expect("db");
        let apps = SqliteAppsRepository::new(db.clone());
        let repo = SqliteShortcutCatalogRepository::new(db);

        let app_id = apps
            .create_custom_app("Cool Studio", &["my code".to_string(), "studio".to_string()])
            .expect("create app");

        let aliases = repo.list_aliases_for_app(app_id).expect("list aliases");
        assert_eq!(aliases, vec!["my code".to_string(), "studio".to_string()]);
    }

    #[test]
    fn list_apps_includes_total_and_active_shortcut_counts() {
        let dir = tempdir().expect("temp dir");
        let db = SqliteDb::open(dir.path().join("library.db")).expect("db");
        let apps = SqliteAppsRepository::new(db.clone());
        let shortcuts = SqliteShortcutsRepository::new(db.clone());
        let repo = SqliteShortcutCatalogRepository::new(db);

        let app_id = apps.create_custom_app("Cool Studio", &[]).expect("create app");
        let active_id = shortcuts.add_shortcut(app_id, "⌘ K", "Do thing").expect("add shortcut");
        let hidden_id = shortcuts.add_shortcut(app_id, "⌘ P", "Open command").expect("add shortcut");
        shortcuts.set_shortcut_states(&[hidden_id], ShortcutState::Dismissed).expect("hide shortcut");

        let app = repo
            .list_apps()
            .expect("list apps")
            .into_iter()
            .find(|app| app.app_id == app_id)
            .expect("app summary");

        assert_eq!(app.importer, None);
        assert_eq!(app.total_count, 2);
        assert_eq!(app.active_count, 1);
        assert!(i64::from(active_id) > 0);
    }
}
