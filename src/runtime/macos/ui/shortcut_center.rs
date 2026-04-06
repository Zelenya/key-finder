mod actions;
mod commands;
mod controller;
mod dialogs;
mod render;
mod state;
mod table;
mod window;

use crate::application::shortcut_center::{ShortcutCache, ShortcutCenterCommandService};
use crate::domain::known_apps::KnownImporterFamily;
use crate::domain::models::AppConfig;
use crate::storage::{AppId, AppSummary, ManagedShortcut};
use objc2::rc::Retained;
use objc2_app_kit::{NSButton, NSPopUpButton, NSScrollView, NSTableView, NSTextField, NSToolbar, NSWindow};
use table::{ShortcutCenterBridge, TableContextMenu};

struct ShortcutCenterWindowUi {
    window: Retained<NSWindow>,
    toolbar: Retained<NSToolbar>,
    bridge: Retained<ShortcutCenterBridge>,
    shortcuts_pane: ShortcutsPaneUi,
    inspector: ShortcutInspectorUi,
}

struct ShortcutsPaneUi {
    pane: Retained<objc2_app_kit::NSView>,
    app_popup: Retained<NSPopUpButton>,
    filter_popup: Retained<NSPopUpButton>,
    status_label: Retained<NSTextField>,
    table_scroll: Retained<NSScrollView>,
    table_view: Retained<NSTableView>,
    table_context_menu: TableContextMenu,
}

struct ShortcutInspectorUi {
    title_label: Retained<NSTextField>,
    summary_label: Retained<NSTextField>,
    alias_label: Retained<NSTextField>,
    selection_label: Retained<NSTextField>,
    shortcut_label: Retained<NSTextField>,
    shortcut_value: Retained<NSTextField>,
    status_label: Retained<NSTextField>,
    status_value: Retained<NSTextField>,
    description_label: Retained<NSTextField>,
    description_field: Retained<NSTextField>,
    save_button: Retained<NSButton>,
    hint_label: Retained<NSTextField>,
    visibility_button: Retained<NSButton>,
    delete_button: Retained<NSButton>,
}

struct ActionRuntime<'a> {
    command_service: &'a ShortcutCenterCommandService,
    app: &'a AppSummary,
    shortcuts: &'a [ManagedShortcut],
}

#[derive(Clone, Debug)]
struct ImportTarget {
    app_id: AppId,
    app_name: String,
    importer: Option<KnownImporterFamily>,
}

impl ImportTarget {
    fn from_app(app: &AppSummary) -> Self {
        Self {
            app_id: app.app_id,
            app_name: app.name.clone(),
            importer: app.importer,
        }
    }
}

pub(super) fn open_shortcut_center(
    config: &AppConfig,
    shortcuts_store: ShortcutCache,
) -> Result<(), crate::domain::errors::AppError> {
    controller::open_shortcut_center(config, shortcuts_store)
}

#[cfg(test)]
mod tests {
    use super::ImportTarget;
    use crate::domain::known_apps::KnownImporterFamily;
    use crate::storage::{AppId, AppSummary};

    #[test]
    fn import_target_uses_stored_importer_family() {
        let app = AppSummary {
            app_id: AppId::from(2),
            name: "Visual Studio Code".to_string(),
            importer: Some(KnownImporterFamily::VSCode),
            total_count: 0,
            active_count: 0,
        };

        let target = ImportTarget::from_app(&app);
        assert_eq!(target.app_id, app.app_id);
        assert_eq!(target.app_name, app.name);
        assert_eq!(target.importer, app.importer);
    }
}
