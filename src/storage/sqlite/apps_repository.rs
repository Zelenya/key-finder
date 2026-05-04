use std::collections::{HashMap, HashSet};

use crate::domain::app_norm::normalize_app_name;
use crate::domain::errors::AppError;
use crate::storage::sqlite::SqliteDb;
use crate::storage::AppId;
use rusqlite::{params, params_from_iter};

#[derive(Clone, Debug)]
pub(crate) struct SqliteAppsRepository {
    db: SqliteDb,
}

struct NameCandidate {
    display_name: String,
    canonical_name: String,
}

impl SqliteAppsRepository {
    pub(crate) fn new(db: SqliteDb) -> Self {
        Self { db }
    }

    pub(crate) fn create_custom_app(&self, app_name: &str, aliases: &[String]) -> Result<AppId, AppError> {
        let app_name = app_name.trim();
        if app_name.is_empty() {
            return Err(AppError::AppNameEmpty);
        }
        let canonical_name = normalize_app_name(app_name);
        if canonical_name.is_empty() {
            return Err(AppError::AppNameInvalid);
        }

        let alias_rows = normalize_aliases(aliases, &canonical_name);
        let candidates = build_name_candidates(app_name, &canonical_name, &alias_rows);
        self.db.with_transaction("create custom app", |tx| {
            ensure_all_available(tx, &candidates)?;
            tx.execute(
                "insert into apps(name, canonical_name)
                 values (?1, ?2)",
                params![app_name, canonical_name],
            )
            .map_err(|source| AppError::Database {
                operation: "insert custom app".to_string(),
                source,
            })?;
            let app_id = AppId::from(tx.last_insert_rowid());

            for (alias, canonical_alias) in alias_rows {
                tx.execute(
                    "insert into app_aliases(app_id, alias, canonical_alias)
                     values (?1, ?2, ?3)",
                    params![app_id, alias, canonical_alias],
                )
                .map_err(|source| AppError::Database {
                    operation: "insert app alias".to_string(),
                    source,
                })?;
            }

            Ok(app_id)
        })
    }

    /// Delete an app. Aliases, shortcuts, and app_importers cascade via FK;
    /// imports rows have app_id set to null.
    pub(crate) fn delete_app(&self, app_id: AppId) -> Result<(), AppError> {
        self.db.with_connection("delete app", |conn| {
            conn.execute("delete from apps where id = ?1", params![app_id]).map_err(|source| {
                AppError::Database {
                    operation: "delete app".to_string(),
                    source,
                }
            })?;
            Ok(())
        })
    }
}

fn build_name_candidates(
    app_name: &str,
    canonical_name: &str,
    alias_rows: &[(String, String)],
) -> Vec<NameCandidate> {
    std::iter::once(NameCandidate {
        display_name: app_name.to_string(),
        canonical_name: canonical_name.to_string(),
    })
    .chain(alias_rows.iter().map(|(alias, canonical_alias)| NameCandidate {
        display_name: alias.clone(),
        canonical_name: canonical_alias.clone(),
    }))
    .collect()
}

fn normalize_aliases(aliases: &[String], primary_canonical_name: &str) -> Vec<(String, String)> {
    let mut seen = HashSet::from([primary_canonical_name.to_string()]);

    aliases
        .iter()
        .map(|alias| alias.trim())
        .filter(|trimmed| !trimmed.is_empty())
        .filter_map(|trimmed| {
            let canonical = normalize_app_name(trimmed);
            (!canonical.is_empty()).then_some((trimmed, canonical))
        })
        .filter(|(_, canonical)| seen.insert(canonical.clone()))
        .map(|(trimmed, canonical)| (trimmed.to_string(), canonical))
        .collect()
}

fn ensure_all_available(conn: &rusqlite::Connection, candidates: &[NameCandidate]) -> Result<(), AppError> {
    if candidates.is_empty() {
        return Ok(());
    }

    let placeholders = std::iter::repeat_n("?", candidates.len()).collect::<Vec<_>>().join(", ");

    let sql = format!(
        "select canonical_name, app_name
         from (
           select canonical_name, name as app_name
           from apps
           union all
           select aliases.canonical_alias as canonical_name, apps.name as app_name
           from app_aliases aliases
           join apps on apps.id = aliases.app_id
         )
         where canonical_name in ({placeholders})"
    );

    let mut stmt = conn.prepare(&sql).map_err(|source| AppError::Database {
        operation: "prepare naming conflict query".to_string(),
        source,
    })?;

    let params = params_from_iter(candidates.iter().map(|candidate| candidate.canonical_name.as_str()));

    let conflicts_by_canonical = stmt
        .query_map(params, |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(|source| AppError::Database {
            operation: "run naming conflict query".to_string(),
            source,
        })?
        .collect::<Result<HashMap<_, _>, _>>()
        .map_err(|source| AppError::Database {
            operation: "collect naming conflict rows".to_string(),
            source,
        })?;

    for candidate in candidates {
        if let Some(app_name) = conflicts_by_canonical.get(&candidate.canonical_name) {
            return Err(AppError::AppNameConflict {
                name: candidate.display_name.clone(),
                existing_app: app_name.clone(),
            });
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{SqliteAppsRepository, SqliteDb};
    use crate::domain::errors::AppError;
    use crate::storage::sqlite::SqliteShortcutCatalogRepository;
    use tempfile::tempdir;

    #[test]
    fn open_seeds_known_apps_and_aliases() {
        let dir = tempdir().expect("temp dir");
        let db = SqliteDb::open(dir.path().join("library.db")).expect("db");
        let queries = SqliteShortcutCatalogRepository::new(db);

        let apps = queries.list_apps().expect("list apps");
        assert!(apps.iter().any(|app| app.name == "IntelliJ IDEA"));
        assert!(apps.iter().any(|app| app.name == "Visual Studio Code"));
        assert!(apps.iter().any(|app| app.name == "Zed"));

        let app_id = apps
            .iter()
            .find(|app| app.name == "Visual Studio Code")
            .map(|app| app.app_id)
            .expect("vscode app id");
        let vscode_aliases = queries.list_aliases_for_app(app_id).expect("list aliases");
        assert_eq!(vscode_aliases, vec!["Code".to_string()]);
    }

    #[test]
    fn create_custom_app_stores_aliases() {
        let dir = tempdir().expect("temp dir");
        let db = SqliteDb::open(dir.path().join("library.db")).expect("db");
        let repo = SqliteAppsRepository::new(db.clone());
        let queries = SqliteShortcutCatalogRepository::new(db);

        repo.create_custom_app("Foo Studio", &["Foo".to_string(), "Foo App".to_string()])
            .expect("create app");

        let apps = queries.list_apps().expect("list apps");
        assert!(apps.iter().any(|app| app.name == "Foo Studio"));
        let app_id = apps
            .iter()
            .find(|app| app.name == "Foo Studio")
            .map(|app| app.app_id)
            .expect("foo studio app id");
        let aliases = queries.list_aliases_for_app(app_id).expect("list aliases");
        assert_eq!(aliases, vec!["Foo".to_string(), "Foo App".to_string()]);
    }

    #[test]
    fn create_custom_app_rejects_conflict_with_existing_alias() {
        let dir = tempdir().expect("temp dir");
        let db = SqliteDb::open(dir.path().join("library.db")).expect("db");
        let repo = SqliteAppsRepository::new(db);

        let error = repo.create_custom_app("Code", &[]).expect_err("expected alias conflict");

        assert!(matches!(
            error,
            AppError::AppNameConflict { name, existing_app }
                if name == "Code" && existing_app == "Visual Studio Code"
        ));
    }

    #[test]
    fn create_custom_app_ignores_empty_and_duplicate_aliases() {
        let dir = tempdir().expect("temp dir");
        let db = SqliteDb::open(dir.path().join("library.db")).expect("db");
        let repo = SqliteAppsRepository::new(db.clone());

        repo.create_custom_app(
            "Foo Studio",
            &[
                "".to_string(),
                "Foo".to_string(),
                "foo".to_string(),
                "Foo Studio".to_string(),
            ],
        )
        .expect("create app");

        let queries = SqliteShortcutCatalogRepository::new(db);
        let app_id = queries
            .list_apps()
            .expect("list apps")
            .into_iter()
            .find(|app| app.name == "Foo Studio")
            .map(|app| app.app_id)
            .expect("foo studio app id");
        let aliases = queries.list_aliases_for_app(app_id).expect("list aliases");
        assert_eq!(aliases, vec!["Foo".to_string()]);
    }
}
