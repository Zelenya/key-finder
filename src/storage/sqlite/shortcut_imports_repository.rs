use crate::domain::errors::AppError;
use crate::domain::shortcut_norm::{normalize_shortcut, render_canonical_shortcut};
use crate::storage::models::{ImportMergeSummary, ImportShortcut};
use crate::storage::sqlite::sqlite_db::now_unix;
use crate::storage::sqlite::SqliteDb;
use crate::storage::AppId;
use rusqlite::params;
use std::collections::HashMap;

#[derive(Clone, Debug)]
pub(crate) struct SqliteShortcutImportsRepository {
    db: SqliteDb,
}

impl SqliteShortcutImportsRepository {
    pub(crate) fn new(db: SqliteDb) -> Self {
        Self { db }
    }

    pub(crate) fn import_shortcuts(
        &self,
        app_id: AppId,
        incoming: Vec<ImportShortcut>,
    ) -> Result<ImportMergeSummary, AppError> {
        self.db.with_transaction("import shortcuts", |tx| {
            let started_at = now_unix();
            tx.execute(
                "INSERT INTO imports(app_id, started_at, status) VALUES (?1, ?2, 'running')",
                params![app_id, started_at],
            )
            .map_err(|source| AppError::Database {
                operation: "insert import run row".to_string(),
                source,
            })?;
            let import_run_id = tx.last_insert_rowid();

            let mut existing = HashMap::new();
            {
                let mut stmt = tx
                    .prepare(
                        "SELECT id, shortcut_norm, description
                         FROM shortcuts
                         WHERE app_id = ?1",
                    )
                    .map_err(|source| AppError::Database {
                        operation: "prepare existing shortcut query".to_string(),
                        source,
                    })?;
                let rows = stmt
                    .query_map(params![app_id], |row| {
                        Ok((
                            row.get::<_, i64>(0)?,
                            row.get::<_, String>(1)?,
                            row.get::<_, String>(2)?,
                        ))
                    })
                    .map_err(|source| AppError::Database {
                        operation: "run existing shortcut query".to_string(),
                        source,
                    })?;

                for row in rows {
                    let row = row.map_err(|source| AppError::Database {
                        operation: "read existing shortcut row".to_string(),
                        source,
                    })?;
                    existing.insert(make_shortcut_identity_key(&row.1, &row.2), row);
                }
            }

            let mut incoming_by_identity: HashMap<String, ImportShortcut> = HashMap::new();
            let mut summary = ImportMergeSummary::default();
            for mut item in incoming {
                let norm = normalize_shortcut(&item.shortcut_display);
                let description = item.description.trim().to_string();
                if norm.is_empty() || description.is_empty() {
                    summary.skipped += 1;
                    continue;
                }
                item.shortcut_display = item.shortcut_display.trim().to_string();
                item.description = description.clone();
                let identity_key = make_shortcut_identity_key(&norm, &description);
                if incoming_by_identity.insert(identity_key, item).is_some() {
                    summary.deduped += 1;
                }
            }

            for (identity_key, item) in &incoming_by_identity {
                let norm = normalize_shortcut(&item.shortcut_display);
                let description = item.description.trim().to_string();

                if existing.contains_key(identity_key) {
                    summary.unchanged += 1;
                } else {
                    let shortcut_display = render_canonical_shortcut(&norm);
                    // TODO: Make imported default visibility configurable, at least for Custom CSV imports.
                    tx.execute(
                        "INSERT INTO shortcuts(
                            app_id,
                            shortcut_display,
                            shortcut_norm,
                            description,
                            state,
                            created_at,
                            updated_at
                        )
                        VALUES (?1, ?2, ?3, ?4, 'dismissed', ?5, ?5)",
                        params![app_id, shortcut_display, norm, description, now_unix()],
                    )
                    .map_err(|source| AppError::Database {
                        operation: "insert new imported shortcut".to_string(),
                        source,
                    })?;
                    summary.added += 1;
                }
            }

            tx.execute(
                "UPDATE imports
                 SET finished_at = ?1,
                     status = 'ok'
                 WHERE id = ?2",
                params![now_unix(), import_run_id],
            )
            .map_err(|source| AppError::Database {
                operation: "finalize import row".to_string(),
                source,
            })?;

            Ok(summary)
        })
    }
}

fn make_shortcut_identity_key(shortcut_norm: &str, description: &str) -> String {
    format!("{shortcut_norm}\u{1f}{description}")
}

#[cfg(test)]
mod tests {
    use super::SqliteShortcutImportsRepository;
    use crate::domain::errors::AppError;
    use crate::storage::models::{ImportShortcut, ShortcutState};
    use crate::storage::sqlite::{SqliteDb, SqliteShortcutCatalogRepository, SqliteShortcutsRepository};
    use crate::storage::AppId;
    use rusqlite::params;
    use tempfile::{tempdir, TempDir};

    #[test]
    fn import_preserves_user_edits_and_state_as_separate_rows() {
        let (_dir, queries, imports, shortcuts) = init_queries_imports_shortcuts();

        imports
            .import_shortcuts(
                app_id(&queries, "Zed"),
                vec![ImportShortcut {
                    shortcut_display: "⌘ B".to_string(),
                    description: "workspace::ToggleLeftDock".to_string(),
                }],
            )
            .expect("initial import");

        let list = shortcuts
            .list_shortcuts(app_id(&queries, "Zed"), true)
            .expect("list shortcuts after first import");
        let id = list[0].id;

        shortcuts
            .update_shortcut_description(id, Some("My custom description"))
            .expect("set user description");
        shortcuts.set_shortcut_states(&[id], ShortcutState::Dismissed).expect("dismiss shortcut");

        imports
            .import_shortcuts(
                app_id(&queries, "Zed"),
                vec![ImportShortcut {
                    shortcut_display: "⌘ B".to_string(),
                    description: "workspace::ToggleLeftDock".to_string(),
                }],
            )
            .expect("second import");

        let list = shortcuts
            .list_shortcuts(app_id(&queries, "Zed"), true)
            .expect("list shortcuts after second import");
        assert_eq!(list.len(), 2);
        assert!(list.iter().any(|shortcut| {
            shortcut.description == "My custom description" && shortcut.state == ShortcutState::Dismissed
        }));
        assert!(list.iter().any(|shortcut| {
            shortcut.description == "workspace::ToggleLeftDock" && shortcut.state == ShortcutState::Dismissed
        }));
    }

    #[test]
    fn import_keeps_distinct_actions_for_same_shortcut() {
        let (_dir, queries, imports, shortcuts) = init_queries_imports_shortcuts();

        imports
            .import_shortcuts(
                app_id(&queries, "IntelliJ IDEA"),
                vec![
                    ImportShortcut {
                        shortcut_display: "BACK_SPACE".to_string(),
                        description: "$Delete".to_string(),
                    },
                    ImportShortcut {
                        shortcut_display: "BACK_SPACE".to_string(),
                        description: "EditorBackSpace".to_string(),
                    },
                ],
            )
            .expect("import two actions on same shortcut");

        let list = shortcuts
            .list_shortcuts(app_id(&queries, "IntelliJ IDEA"), true)
            .expect("list imported shortcuts");
        assert_eq!(list.len(), 2);
        assert!(list.iter().all(|shortcut| shortcut.state == ShortcutState::Dismissed));
    }

    #[test]
    fn new_imported_shortcuts_start_hidden() {
        let (_dir, queries, imports, shortcuts) = init_queries_imports_shortcuts();

        imports
            .import_shortcuts(
                app_id(&queries, "Zed"),
                vec![ImportShortcut {
                    shortcut_display: "⌘ B".to_string(),
                    description: "workspace::ToggleLeftDock".to_string(),
                }],
            )
            .expect("initial import");

        let list =
            shortcuts.list_shortcuts(app_id(&queries, "Zed"), true).expect("list shortcuts after import");
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].state, ShortcutState::Dismissed);
    }

    #[test]
    fn import_dedupes_duplicate_incoming_rows() {
        let (_dir, queries, imports, shortcuts) = init_queries_imports_shortcuts();

        let summary = imports
            .import_shortcuts(
                app_id(&queries, "Zed"),
                vec![
                    ImportShortcut {
                        shortcut_display: "⌘ B".to_string(),
                        description: "workspace::ToggleLeftDock".to_string(),
                    },
                    ImportShortcut {
                        shortcut_display: "⌘ B".to_string(),
                        description: "workspace::ToggleLeftDock".to_string(),
                    },
                ],
            )
            .expect("import duplicate rows");

        assert_eq!(summary.added, 1);
        assert_eq!(summary.deduped, 1);

        let list =
            shortcuts.list_shortcuts(app_id(&queries, "Zed"), true).expect("list shortcuts after import");
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].state, ShortcutState::Dismissed);
    }

    #[test]
    fn import_shortcuts_fails_for_missing_app_id() {
        let (_dir, _, imports, _) = init_queries_imports_shortcuts();

        let error = imports
            .import_shortcuts(
                AppId::from(999_999),
                vec![ImportShortcut {
                    shortcut_display: "⌘ B".to_string(),
                    description: "workspace::ToggleLeftDock".to_string(),
                }],
            )
            .expect_err("missing app should fail");

        assert!(matches!(error, AppError::Database { .. }));
    }

    #[test]
    fn import_shortcuts_records_finished_import_run() {
        let dir = tempdir().expect("temp dir");
        let db = SqliteDb::open(dir.path().join("library.db")).expect("db");
        let queries = SqliteShortcutCatalogRepository::new(db.clone());
        let imports = SqliteShortcutImportsRepository::new(db.clone());

        imports
            .import_shortcuts(
                app_id(&queries, "Zed"),
                vec![ImportShortcut {
                    shortcut_display: "⌘ B".to_string(),
                    description: "workspace::ToggleLeftDock".to_string(),
                }],
            )
            .expect("import shortcuts");

        let (status, finished_at, error_text): (String, Option<i64>, Option<String>) = db
            .with_connection("read import run", |conn| {
                conn.query_row(
                    "SELECT status, finished_at, error_text
                     FROM imports
                     WHERE app_id = ?1
                     ORDER BY id DESC
                     LIMIT 1",
                    params![app_id(&queries, "Zed")],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
                )
                .map_err(|source| AppError::Database {
                    operation: "read import run row".to_string(),
                    source,
                })
            })
            .expect("read import run");

        assert_eq!(status, "ok");
        assert!(finished_at.is_some());
        assert_eq!(error_text, None);
    }

    #[test]
    fn import_shortcuts_skips_invalid_rows_without_inserting_shortcuts() {
        let (_dir, queries, imports, shortcuts) = init_queries_imports_shortcuts();

        let app_id = app_id(&queries, "Zed");
        let summary = imports
            .import_shortcuts(
                app_id,
                vec![
                    ImportShortcut {
                        shortcut_display: "".to_string(),
                        description: "workspace::ToggleLeftDock".to_string(),
                    },
                    ImportShortcut {
                        shortcut_display: "⌘ B".to_string(),
                        description: "".to_string(),
                    },
                    ImportShortcut {
                        shortcut_display: "   ".to_string(),
                        description: "   ".to_string(),
                    },
                ],
            )
            .expect("import invalid rows");

        assert_eq!(summary.added, 0);
        assert_eq!(summary.unchanged, 0);
        assert_eq!(summary.deduped, 0);
        assert_eq!(summary.skipped, 3);
        assert_eq!(
            shortcuts.list_shortcuts(app_id, true).expect("list shortcuts").len(),
            0
        );
    }

    fn init_queries_imports_shortcuts() -> (
        TempDir,
        SqliteShortcutCatalogRepository,
        SqliteShortcutImportsRepository,
        SqliteShortcutsRepository,
    ) {
        let dir = tempdir().expect("temp dir");
        let db = SqliteDb::open(dir.path().join("library.db")).expect("db");
        let queries = SqliteShortcutCatalogRepository::new(db.clone());
        let imports = SqliteShortcutImportsRepository::new(db.clone());
        let shortcuts = SqliteShortcutsRepository::new(db);
        (dir, queries, imports, shortcuts)
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
