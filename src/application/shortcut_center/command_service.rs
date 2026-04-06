use super::shortcut_cache::ShortcutCache;
use crate::domain::errors::AppError;
use crate::storage::{
    AppId, ImportMergeSummary, ImportShortcut, ShortcutId, ShortcutState, SqliteAppsRepository,
    SqliteNotificationSnapshotRepository, SqliteShortcutCatalogRepository, SqliteShortcutImportsRepository,
    SqliteShortcutsRepository,
};

#[derive(Clone)]
pub(crate) struct ShortcutCenterCommandService {
    apps_repo: SqliteAppsRepository,
    shortcuts_repo: SqliteShortcutsRepository,
    catalog_repo: SqliteShortcutCatalogRepository,
    notification_snapshot_repo: SqliteNotificationSnapshotRepository,
    shortcut_imports_repo: SqliteShortcutImportsRepository,
    shortcut_cache: ShortcutCache,
}

#[derive(Clone, Debug)]
pub(crate) struct CreateAppInput {
    pub app_name: String,
    pub aliases: Vec<String>,
}

#[derive(Clone, Debug)]
pub(crate) struct CreateAppResult {
    pub app_id: AppId,
    pub app_name: String,
    pub alias_count: usize,
}

#[derive(Clone, Debug)]
pub(crate) struct AddShortcutResult;

#[derive(Clone, Debug)]
pub(crate) struct UpdateDescriptionResult;

#[derive(Clone, Debug)]
pub(crate) struct VisibilityChangeResult {
    pub updated: usize,
    pub target_state: ShortcutState,
}

#[derive(Clone, Debug)]
pub(crate) struct DeleteShortcutsResult {
    pub deleted: usize,
}

#[derive(Clone, Debug)]
pub(crate) struct ImportShortcutsResult {
    pub summary: ImportMergeSummary,
}

impl ShortcutCenterCommandService {
    pub(crate) fn new(
        apps_repo: SqliteAppsRepository,
        shortcuts_repo: SqliteShortcutsRepository,
        catalog_repo: SqliteShortcutCatalogRepository,
        notification_snapshot_repo: SqliteNotificationSnapshotRepository,
        shortcut_imports_repo: SqliteShortcutImportsRepository,
        shortcut_cache: ShortcutCache,
    ) -> Self {
        Self {
            apps_repo,
            shortcuts_repo,
            catalog_repo,
            notification_snapshot_repo,
            shortcut_imports_repo,
            shortcut_cache,
        }
    }

    pub(crate) fn create_app(&self, input: CreateAppInput) -> Result<CreateAppResult, AppError> {
        let app_id = self.apps_repo.create_custom_app(&input.app_name, &input.aliases)?;
        self.refresh_snapshot()?;
        let alias_count = self.catalog_repo.list_aliases_for_app(app_id)?.len();
        let app_name = input.app_name.trim().to_string();
        Ok(CreateAppResult {
            app_id,
            app_name,
            alias_count,
        })
    }

    pub(crate) fn add_shortcut(
        &self,
        app_id: AppId,
        shortcut: &str,
        description: &str,
    ) -> Result<AddShortcutResult, AppError> {
        self.shortcuts_repo.add_shortcut(app_id, shortcut, description)?;
        self.refresh_snapshot()?;
        Ok(AddShortcutResult)
    }

    pub(crate) fn import_shortcuts(
        &self,
        app_id: AppId,
        shortcuts: Vec<ImportShortcut>,
    ) -> Result<ImportShortcutsResult, AppError> {
        let summary = self.shortcut_imports_repo.import_shortcuts(app_id, shortcuts)?;
        self.refresh_snapshot()?;
        Ok(ImportShortcutsResult { summary })
    }

    pub(crate) fn update_description(
        &self,
        shortcut_id: ShortcutId,
        description: &str,
    ) -> Result<UpdateDescriptionResult, AppError> {
        self.shortcuts_repo.update_shortcut_description(shortcut_id, Some(description))?;
        self.refresh_snapshot()?;
        Ok(UpdateDescriptionResult)
    }

    pub(crate) fn set_shortcut_state(
        &self,
        shortcut_ids: &[ShortcutId],
        target_state: ShortcutState,
    ) -> Result<VisibilityChangeResult, AppError> {
        let updated = self.shortcuts_repo.set_shortcut_states(shortcut_ids, target_state)?;
        self.refresh_snapshot()?;
        Ok(VisibilityChangeResult {
            updated,
            target_state,
        })
    }

    pub(crate) fn delete_shortcuts(
        &self,
        shortcut_ids: &[ShortcutId],
    ) -> Result<DeleteShortcutsResult, AppError> {
        let deleted = self.shortcuts_repo.delete_shortcuts(shortcut_ids)?;
        self.refresh_snapshot()?;
        Ok(DeleteShortcutsResult { deleted })
    }

    fn refresh_snapshot(&self) -> Result<(), AppError> {
        let refreshed = self.notification_snapshot_repo.load_notification_shortcuts()?;
        self.shortcut_cache.replace(refreshed);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{CreateAppInput, ShortcutCenterCommandService};
    use crate::application::shortcut_center::ShortcutCache;
    use crate::storage::models::{ImportShortcut, ShortcutState};
    use crate::storage::sqlite::{
        SqliteAppsRepository, SqliteDb, SqliteNotificationSnapshotRepository,
        SqliteShortcutCatalogRepository, SqliteShortcutImportsRepository, SqliteShortcutsRepository,
    };
    use tempfile::{tempdir, TempDir};

    #[test]
    fn create_app_returns_stored_alias_count() {
        let (_dir, service) = init();

        let result = service
            .create_app(CreateAppInput {
                app_name: "Foo Studio".to_string(),
                aliases: vec!["Foo".to_string(), "foo".to_string()],
            })
            .expect("create app");

        assert_eq!(result.app_name, "Foo Studio");
        assert_eq!(result.alias_count, 1);
    }

    #[test]
    fn add_shortcut_refreshes_snapshot_store() {
        let (_dir, service) = init();

        let app_id = service
            .create_app(CreateAppInput {
                app_name: "Foo".to_string(),
                aliases: Vec::new(),
            })
            .expect("create app")
            .app_id;
        service.add_shortcut(app_id, "⌘ P", "Go to file").expect("add shortcut");

        let snapshot = service.shortcut_cache.snapshot();
        assert_eq!(snapshot.len(), 1);
        assert_eq!(snapshot[0].description, "Go to file");
    }

    #[test]
    fn import_shortcuts_refreshes_snapshot_and_returns_summary() {
        let (_dir, service) = init();

        let app_id = service
            .create_app(CreateAppInput {
                app_name: "Foo".to_string(),
                aliases: Vec::new(),
            })
            .expect("create app")
            .app_id;

        let result = service
            .import_shortcuts(
                app_id,
                vec![ImportShortcut {
                    shortcut_display: "⌘ B".to_string(),
                    description: "workspace::ToggleLeftDock".to_string(),
                }],
            )
            .expect("import shortcuts");

        assert_eq!(result.summary.added, 1);
        assert!(service.shortcut_cache.snapshot().is_empty());
    }

    #[test]
    fn set_visibility_returns_updated_count() {
        let (_dir, service) = init();

        let app_id = service
            .create_app(CreateAppInput {
                app_name: "Foo".to_string(),
                aliases: Vec::new(),
            })
            .expect("create app")
            .app_id;
        let shortcut_id =
            service.shortcuts_repo.add_shortcut(app_id, "⌘ K", "Do thing").expect("add shortcut");

        let result =
            service.set_shortcut_state(&[shortcut_id], ShortcutState::Dismissed).expect("hide shortcut");

        assert_eq!(result.updated, 1);
        assert_eq!(result.target_state, ShortcutState::Dismissed);
    }

    fn init() -> (TempDir, ShortcutCenterCommandService) {
        let dir = tempdir().expect("temp dir");
        let db = SqliteDb::open(dir.path().join("library.db")).expect("db");
        let catalog = SqliteShortcutCatalogRepository::new(db.clone());
        let snapshot_queries = SqliteNotificationSnapshotRepository::new(db.clone());
        let apps = SqliteAppsRepository::new(db.clone());
        let shortcuts = SqliteShortcutsRepository::new(db.clone());
        let imports = SqliteShortcutImportsRepository::new(db);
        let cache = ShortcutCache::new(Vec::new());
        let service =
            ShortcutCenterCommandService::new(apps, shortcuts, catalog, snapshot_queries, imports, cache);
        (dir, service)
    }
}
