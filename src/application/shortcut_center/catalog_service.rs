use crate::domain::app_norm::normalize_app_name;
use crate::domain::errors::AppError;
use crate::storage::{
    AppId, AppSummary, ManagedShortcut, SqliteShortcutCatalogRepository, SqliteShortcutsRepository,
};

#[derive(Clone, Debug)]
pub(crate) struct ShortcutCenterCatalogService {
    catalog_repo: SqliteShortcutCatalogRepository,
    shortcuts_repo: SqliteShortcutsRepository,
}

#[derive(Clone, Debug)]
pub(crate) struct ShortcutCenterAppView {
    pub aliases: Vec<String>,
    pub shortcuts: Vec<ManagedShortcut>,
}

impl ShortcutCenterCatalogService {
    pub(crate) fn new(
        catalog_repo: SqliteShortcutCatalogRepository,
        shortcuts_repo: SqliteShortcutsRepository,
    ) -> Self {
        Self {
            catalog_repo,
            shortcuts_repo,
        }
    }

    pub(crate) fn load_apps(&self) -> Result<Vec<AppSummary>, AppError> {
        self.catalog_repo.list_apps()
    }

    pub(crate) fn load_app_view(
        &self,
        app_id: AppId,
        include_dismissed: bool,
    ) -> Result<ShortcutCenterAppView, AppError> {
        let aliases = self.catalog_repo.list_aliases_for_app(app_id)?;
        let shortcuts = self.shortcuts_repo.list_shortcuts(app_id, include_dismissed)?;
        Ok(ShortcutCenterAppView { aliases, shortcuts })
    }

    pub(crate) fn resolve_preferred_app(
        &self,
        apps: &[AppSummary],
        frontmost_app_name: Option<&str>,
    ) -> Result<Option<AppId>, AppError> {
        // No app, no match
        let Some(name) = frontmost_app_name else {
            return Ok(None);
        };

        // Case insansitive name match
        if let Some(app_id) = apps
            .iter()
            .find(|candidate| candidate.name.eq_ignore_ascii_case(name))
            .map(|candidate| candidate.app_id)
        {
            return Ok(Some(app_id));
        }

        let frontmost_app_name = normalize_app_name(name);

        // Normalized name match
        if let Some(app_id) =
            apps.iter().find(|app| normalize_app_name(&app.name) == frontmost_app_name).map(|app| app.app_id)
        {
            return Ok(Some(app_id));
        }

        // Normalized alias match
        for app in apps {
            let aliases = self.catalog_repo.list_aliases_for_app(app.app_id)?;
            if aliases.iter().any(|alias| normalize_app_name(alias) == frontmost_app_name) {
                return Ok(Some(app.app_id));
            }
        }

        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::ShortcutCenterCatalogService;
    use crate::storage::{
        sqlite::{
            SqliteAppsRepository, SqliteDb, SqliteShortcutCatalogRepository, SqliteShortcutsRepository,
        },
        AppId, AppSummary,
    };
    use tempfile::{tempdir, TempDir};

    #[test]
    fn load_app_view_returns_aliases_and_shortcuts() {
        let (_dir, apps, service) = init_with_apps();

        let app_id = apps.create_custom_app("Cool Studio", &["coolcode".to_string()]).expect("create app");
        service.shortcuts_repo.add_shortcut(app_id, "⌘ K", "Do thing").expect("add shortcut");

        let view = service.load_app_view(app_id, true).expect("load app view");
        assert_eq!(view.aliases, vec!["coolcode".to_string()]);
        assert_eq!(view.shortcuts.len(), 1);
        assert_eq!(view.shortcuts[0].description, "Do thing");
    }

    #[test]
    fn resolves_preferred_name_to_none() {
        let (_dir, service) = init_service();

        let app = AppSummary {
            app_id: AppId::from(123),
            name: "Foo".to_string(),
            importer: None,
            total_count: 0,
            active_count: 0,
        };
        let result = service.resolve_preferred_app(&[app], None).expect("resolve preferred app");
        assert!(result.is_none());
    }

    #[test]
    fn resolves_preferred_name_by_name() {
        let (_dir, service) = init_service();

        let app = AppSummary {
            app_id: AppId::from(123),
            name: "Foo".to_string(),
            importer: None,
            total_count: 0,
            active_count: 0,
        };
        let frontmost_app_name = Some("foo");

        let result =
            service.resolve_preferred_app(&[app], frontmost_app_name).expect("resolve preferred app");

        assert_eq!(result.unwrap(), AppId::from(123));
    }

    #[test]
    fn resolves_preferred_name_by_normalized_name() {
        let (_dir, service) = init_service();

        let app = AppSummary {
            app_id: AppId::from(123),
            name: "foo-app".to_string(),
            importer: None,
            total_count: 0,
            active_count: 0,
        };
        let frontmost_app_name = Some("FOO App");

        let result =
            service.resolve_preferred_app(&[app], frontmost_app_name).expect("resolve preferred app");

        assert_eq!(result.unwrap(), AppId::from(123));
    }

    #[test]
    fn resolves_preferred_name_by_alias() {
        let (_dir, service) = init_service();
        let app = service
            .load_apps()
            .expect("list apps")
            .into_iter()
            .find(|app| app.name == "Visual Studio Code")
            .expect("vscode app");
        let frontmost_app_name = Some("Code");

        let result =
            service.resolve_preferred_app(&[app], frontmost_app_name).expect("resolve preferred app");

        assert_eq!(result.unwrap(), AppId::from(2));
    }

    fn init_service() -> (TempDir, ShortcutCenterCatalogService) {
        let dir = tempdir().expect("temp dir");
        let db = SqliteDb::open(dir.path().join("library.db")).expect("db");
        let catalog = SqliteShortcutCatalogRepository::new(db.clone());
        let shortcuts = SqliteShortcutsRepository::new(db.clone());
        (dir, ShortcutCenterCatalogService::new(catalog, shortcuts.clone()))
    }

    fn init_with_apps() -> (TempDir, SqliteAppsRepository, ShortcutCenterCatalogService) {
        let dir = tempdir().expect("temp dir");
        let db = SqliteDb::open(dir.path().join("library.db")).expect("db");
        let apps = SqliteAppsRepository::new(db.clone());
        let catalog = SqliteShortcutCatalogRepository::new(db.clone());
        let shortcuts = SqliteShortcutsRepository::new(db.clone());
        let service = ShortcutCenterCatalogService::new(catalog.clone(), shortcuts);
        (dir, apps, service)
    }
}
