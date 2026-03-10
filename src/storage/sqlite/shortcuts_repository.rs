use crate::domain::errors::AppError;
use crate::domain::shortcut_norm::{normalize_shortcut, render_canonical_shortcut};
use crate::storage::models::{ManagedShortcut, ShortcutId, ShortcutState};
use crate::storage::sqlite::sqlite_db::now_unix;
use crate::storage::sqlite::SqliteDb;
use crate::storage::AppId;
use rusqlite::params;

#[derive(Clone, Debug)]
pub(crate) struct SqliteShortcutsRepository {
    pub(super) db: SqliteDb,
}

impl SqliteShortcutsRepository {
    pub(crate) fn new(db: SqliteDb) -> Self {
        Self { db }
    }
}

impl SqliteShortcutsRepository {
    pub(crate) fn add_shortcut(
        &self,
        app_id: AppId,
        shortcut: &str,
        description: &str,
    ) -> Result<ShortcutId, AppError> {
        let shortcut = normalize_shortcut(shortcut);
        let shortcut_display = render_canonical_shortcut(&shortcut);
        let description = description.trim().to_string();
        if shortcut.is_empty() {
            return Err(AppError::StorageOperation("shortcut can't be empty".to_string()));
        }
        if description.is_empty() {
            return Err(AppError::StorageOperation(
                "description can't be empty".to_string(),
            ));
        }

        self.db.with_connection("add shortcut", |conn| {
            let now = now_unix();

            conn.query_row(
                "insert into shortcuts(
                    app_id,
                    shortcut_display,
                    shortcut_norm,
                    description,
                    state,
                    created_at,
                    updated_at
                ) values (?1, ?2, ?3, ?4, 'active', ?5, ?5)
                on conflict(app_id, shortcut_norm, description)
                do update set
                    shortcut_display = excluded.shortcut_display,
                    state = 'active',
                    updated_at = excluded.updated_at
                returning id",
                params![app_id, shortcut_display, shortcut, description, now],
                |row| row.get(0),
            )
            .map_err(|source| AppError::Database {
                operation: "upsert shortcut".to_string(),
                source,
            })
        })
    }

    pub(crate) fn list_shortcuts(
        &self,
        app_id: AppId,
        include_dismissed: bool,
    ) -> Result<Vec<ManagedShortcut>, AppError> {
        self.db.with_connection("list shortcuts", |conn| {
            let mut stmt = conn
                .prepare(
                    "select s.id,
                            s.shortcut_norm,
                            s.description,
                            s.state
                    from shortcuts s
                    where s.app_id = ?1
                      and (?2 or s.state = 'active')
                    order by s.state, s.created_at, s.id
                    ",
                )
                .map_err(|source| AppError::Database {
                    operation: "prepare shortcut list query".to_string(),
                    source,
                })?;
            let rows = stmt
                .query_map(params![app_id, include_dismissed], |row| {
                    let shortcut_norm: String = row.get(1)?;
                    let state: String = row.get(3)?;
                    Ok(ManagedShortcut {
                        id: row.get(0)?,
                        shortcut_display: render_canonical_shortcut(&shortcut_norm),
                        description: row.get(2)?,
                        state: ShortcutState::from_db(&state),
                    })
                })
                .map_err(|source| AppError::Database {
                    operation: "run shortcut list query".to_string(),
                    source,
                })?;

            rows.collect::<Result<Vec<_>, _>>().map_err(|source| AppError::Database {
                operation: "collect shortcut rows".to_string(),
                source,
            })
        })
    }

    pub(crate) fn update_shortcut_description(
        &self,
        shortcut_id: ShortcutId,
        description: Option<&str>,
    ) -> Result<(), AppError> {
        self.db.with_connection("update shortcut description", |conn| {
            let value = description.map(str::trim).filter(|s| !s.is_empty()).map(str::to_string);
            let Some(value) = value else {
                return Err(AppError::StorageOperation(
                    "description cannot be empty".to_string(),
                ));
            };

            conn.execute(
                "update shortcuts set description = ?1, updated_at = ?2 where id = ?3",
                params![value, now_unix(), shortcut_id],
            )
            .map_err(|source| AppError::Database {
                operation: "update description".to_string(),
                source,
            })?;
            Ok(())
        })
    }

    pub(crate) fn set_shortcut_states(
        &self,
        shortcut_ids: &[ShortcutId],
        state: ShortcutState,
    ) -> Result<usize, AppError> {
        if shortcut_ids.is_empty() {
            return Ok(0);
        }

        self.db.with_connection("set shortcut states", |conn| {
            let mut updated = 0usize;
            let mut stmt = conn
                .prepare("UPDATE shortcuts SET state = ?1, updated_at = ?2 WHERE id = ?3")
                .map_err(|source| AppError::Database {
                    operation: "prepare batch shortcut state update".to_string(),
                    source,
                })?;
            let now = now_unix();

            for shortcut_id in shortcut_ids {
                updated += stmt.execute(params![state.as_str(), now, shortcut_id]).map_err(|source| {
                    AppError::Database {
                        operation: "update batch shortcut state".to_string(),
                        source,
                    }
                })?;
            }

            Ok(updated)
        })
    }

    pub(crate) fn delete_shortcuts(&self, shortcut_ids: &[ShortcutId]) -> Result<usize, AppError> {
        if shortcut_ids.is_empty() {
            return Ok(0);
        }

        self.db.with_connection("delete shortcuts", |conn| {
            let placeholders = std::iter::repeat_n("?", shortcut_ids.len()).collect::<Vec<_>>().join(", ");

            conn.execute(
                &format!("delete from shortcuts where id in ({placeholders})"),
                rusqlite::params_from_iter(shortcut_ids.iter().copied()),
            )
            .map_err(|source| AppError::Database {
                operation: "delete batch shortcut".to_string(),
                source,
            })
        })
    }
}

#[cfg(test)]
mod tests {
    use super::SqliteShortcutsRepository;
    use crate::domain::errors::AppError;
    use crate::storage::models::ShortcutState;
    use crate::storage::sqlite::{SqliteAppsRepository, SqliteShortcutCatalogRepository};
    use crate::storage::AppId;
    use crate::storage::SqliteDb;
    use tempfile::{tempdir, TempDir};

    #[test]
    fn add_shortcut_for_seeded_known_app_succeeds() {
        let (_dir, queries, shortcuts) = init_queries_and_shortcuts();
        let app_id = app_id(&queries, "Visual Studio Code");

        shortcuts.add_shortcut(app_id, "⌘ P", "Go to file").expect("add manual shortcut");

        let list = shortcuts.list_shortcuts(app_id, true).expect("list shortcuts");
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].description, "Go to file");
    }

    #[test]
    fn add_shortcut_reactivates_existing_matching_shortcut() {
        let (_dir, apps, shortcuts) = init_apps_and_shortcuts();

        let app_id = apps.create_custom_app("Foo", &[]).expect("create app");

        let id = shortcuts.add_shortcut(app_id, "⌘ K", "Do k command").expect("add shortcut");
        shortcuts.set_shortcut_states(&[id], ShortcutState::Dismissed).expect("dismiss shortcut");

        let reactivated_id =
            shortcuts.add_shortcut(app_id, "⌘ K", "Do k command").expect("reactivate shortcut");

        assert_eq!(reactivated_id, id);

        let list = shortcuts.list_shortcuts(app_id, true).expect("list shortcuts");
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].state, ShortcutState::Active);
    }

    #[test]
    fn list_shortcuts_returns_no_shortcuts_for_new_app() {
        let (_dir, apps, shortcuts) = init_apps_and_shortcuts();

        let foo_id = apps.create_custom_app("Foo", &[]).expect("create app");

        let list = shortcuts.list_shortcuts(foo_id, true).expect("list shortcuts");

        assert_eq!(list.len(), 0);
    }

    #[test]
    fn list_shortcuts_returns_all_shortcuts_for_the_app() {
        let (_dir, apps, shortcuts) = init_apps_and_shortcuts();

        let foo_id = apps.create_custom_app("Foo", &[]).expect("create app");
        let bar_id = apps.create_custom_app("Bar", &[]).expect("create app");

        shortcuts.add_shortcut(foo_id, "⌘ K", "Do k command").expect("add manual shortcut");

        shortcuts.add_shortcut(bar_id, "⌘ J", "Do j command").expect("add manual shortcut");

        let list = shortcuts.list_shortcuts(foo_id, true).expect("list shortcuts");

        assert_eq!(list.len(), 1);
        assert_eq!(list[0].shortcut_display, "⌘ K");
        assert_eq!(list[0].description, "Do k command");
    }

    #[test]
    fn list_shortcuts_excludes_dismissed_by_default() {
        let (_dir, apps, shortcuts) = init_apps_and_shortcuts();

        let app_id = apps.create_custom_app("Foo", &[]).expect("create app");

        let active_id = shortcuts.add_shortcut(app_id, "⌘ K", "Do k command").expect("active shortcut");
        let hidden_id = shortcuts.add_shortcut(app_id, "⌘ J", "Do j command").expect("hidden shortcut");
        shortcuts.set_shortcut_states(&[hidden_id], ShortcutState::Dismissed).expect("dismiss shortcut");

        let list = shortcuts.list_shortcuts(app_id, false).expect("list shortcuts");

        assert_eq!(list.len(), 1);
        assert_eq!(list[0].id, active_id);
        assert_eq!(list[0].state, ShortcutState::Active);
    }

    #[test]
    fn update_shortcut_description_updates_selected_rows() {
        let (_dir, apps, shortcuts) = init_apps_and_shortcuts();

        let app_id = apps.create_custom_app("Foo", &[]).expect("create app");

        let id1 = shortcuts.add_shortcut(app_id, "⌘ K", "Do k command").expect("manual shortcut");

        shortcuts.update_shortcut_description(id1, Some("Updated k command")).expect("batch update");

        let list = shortcuts.list_shortcuts(app_id, true).expect("list shortcuts");

        assert_eq!(list[0].shortcut_display, "⌘ K");
        assert_eq!(list[0].description, "Updated k command");
    }

    #[test]
    fn update_shortcut_description_rejects_empty_value() {
        let (_dir, apps, shortcuts) = init_apps_and_shortcuts();

        let app_id = apps.create_custom_app("Foo", &[]).expect("create app");

        let id = shortcuts.add_shortcut(app_id, "⌘ K", "Do k command").expect("manual shortcut");

        let error = shortcuts
            .update_shortcut_description(id, Some("   "))
            .expect_err("empty description should fail");
        assert!(matches!(
            error,
            AppError::StorageOperation(message) if message == "description cannot be empty"
        ));

        let list = shortcuts.list_shortcuts(app_id, true).expect("list shortcuts");
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].description, "Do k command");
    }

    #[test]
    fn set_shortcut_states_updates_selected_rows() {
        let (_dir, apps, shortcuts) = init_apps_and_shortcuts();

        let app_id = apps.create_custom_app("Foo", &[]).expect("create app");

        let id1 = shortcuts.add_shortcut(app_id, "⌘ K", "Do k command").expect("manual shortcut");
        let id2 = shortcuts.add_shortcut(app_id, "⌘ J", "Do j command").expect("manual shortcut");

        let updated =
            shortcuts.set_shortcut_states(&[id1, id2], ShortcutState::Dismissed).expect("batch update");
        assert_eq!(updated, 2);

        let list = shortcuts.list_shortcuts(app_id, true).expect("list shortcuts");
        assert!(list.iter().all(|shortcut| shortcut.state == ShortcutState::Dismissed));
    }

    #[test]
    fn delete_shortcuts_removes_any_selected_rows() {
        let (_dir, apps, shortcuts) = init_apps_and_shortcuts();

        let app_id = apps.create_custom_app("Foo", &[]).expect("create app");

        let id1 = shortcuts.add_shortcut(app_id, "⌘ K", "Do k command").expect("manual shortcut");

        let id2 = shortcuts.add_shortcut(app_id, "⌘ J", "Do j command").expect("manual shortcut");

        let list = shortcuts.list_shortcuts(app_id, true).expect("list shortcuts");
        assert_eq!(list.len(), 2);

        let deleted = shortcuts.delete_shortcuts(&[id1, id2]).expect("batch delete");
        assert_eq!(deleted, 2);

        let list = shortcuts.list_shortcuts(app_id, true).expect("list shortcuts");
        assert_eq!(list.len(), 0);
    }

    fn init_apps_and_shortcuts() -> (TempDir, SqliteAppsRepository, SqliteShortcutsRepository) {
        let dir = tempdir().expect("temp dir");
        let db = SqliteDb::open(dir.path().join("library.db")).expect("db");
        let apps = SqliteAppsRepository::new(db.clone());
        let shortcuts = SqliteShortcutsRepository::new(db);
        (dir, apps, shortcuts)
    }

    fn init_queries_and_shortcuts() -> (
        TempDir,
        SqliteShortcutCatalogRepository,
        SqliteShortcutsRepository,
    ) {
        let dir = tempdir().expect("temp dir");
        let db = SqliteDb::open(dir.path().join("library.db")).expect("db");
        let queries = SqliteShortcutCatalogRepository::new(db.clone());
        let shortcuts = SqliteShortcutsRepository::new(db);
        (dir, queries, shortcuts)
    }

    fn app_id(queries: &SqliteShortcutCatalogRepository, app_name: &str) -> AppId {
        queries
            .list_apps()
            .expect("list apps")
            .into_iter()
            .find(|app| app.name == app_name)
            .map(|app| app.app_id)
            .expect("app id")
    }
}
