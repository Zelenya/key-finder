use super::commands::{ActionCommand, UiIntent};
use super::dialogs;
use super::state::{preserve_selection, ShortcutCenterSelectionState};
use super::{actions, render, table, window, ActionRuntime, ImportTarget};
use crate::application::shortcut_center::{
    ShortcutCache, ShortcutCenterCatalogService, ShortcutCenterCommandService,
};
use crate::domain::errors::AppError;
use crate::domain::models::AppConfig;
use crate::runtime::macos::platform::frontmost;
use crate::runtime::macos::ui::modal::populate_popup;
use crate::storage::{AppId, AppSummary, ManagedShortcut, SqliteDb};
use objc2::rc::Retained;
use objc2::MainThreadMarker;
use objc2_app_kit::NSApplication;

const FILTER_ACTIVE_ONLY: &str = "Only active shortcuts";
const FILTER_INCLUDE_DISMISSED: &str = "All shortcuts (including hidden)";

struct ShortcutCenterRuntime {
    app: Retained<NSApplication>,
    ui: super::ShortcutCenterWindowUi,
    catalog_service: ShortcutCenterCatalogService,
    command_service: ShortcutCenterCommandService,
}

struct LoadedAppState {
    apps: Vec<AppSummary>,
    selected_app: AppSummary,
    include_dismissed: bool,
    aliases: Vec<String>,
    shortcuts: Vec<ManagedShortcut>,
}

/// Main event loop for the shortcut center
pub(super) fn open_shortcut_center(
    config: &AppConfig,
    shortcut_store: ShortcutCache,
) -> Result<(), AppError> {
    let Some(mtm) = MainThreadMarker::new() else {
        return Err(AppError::MainThreadRequired);
    };

    let runtime = initialize_shortcut_center(config, shortcut_store, mtm)?;

    // We start with foremost app, and then update based on user intent (e.g., to preselect newly created app)
    let mut pending_app_id = initial_pending_app_id(&runtime.catalog_service)?;

    let mut status = "Select shortcuts in the table, then use the toolbar, inspector, or right-click menu for shortcut actions. Settings live in the tray menu.".to_string();
    let mut selection = ShortcutCenterSelectionState::default();

    loop {
        let loaded = load_app_state(&runtime, pending_app_id.take())?;
        selection = render_app_state(&runtime.ui, &loaded, selection, &status);

        let Some(intent) = run_modal_intent(&runtime.app, &runtime.ui) else {
            break;
        };
        if matches!(intent, UiIntent::WindowClosed) {
            break;
        }

        // Refresh the inputs, because ui may have changed
        let (action_app, latest_shortcuts, latest_selection) =
            load_action_inputs(&runtime, &loaded.apps, &loaded.selected_app)?;
        let action_runtime = ActionRuntime {
            command_service: &runtime.command_service,
            app: &action_app,
            shortcuts: &latest_shortcuts,
        };

        // Handle the action intent
        if matches!(intent, UiIntent::TableSelectionChanged | UiIntent::Command(_)) {
            selection = latest_selection;
        }
        status = handle_intent(
            intent,
            &action_runtime,
            &mut selection,
            &mut pending_app_id,
            &runtime.ui,
        )?;
    }

    cleanup_shortcut_center(runtime);
    Ok(())
}

fn initialize_shortcut_center(
    config: &AppConfig,
    shortcut_store: ShortcutCache,
    mtm: MainThreadMarker,
) -> Result<ShortcutCenterRuntime, AppError> {
    let db = SqliteDb::open(&config.database_path)?;
    let catalog_service =
        ShortcutCenterCatalogService::new(db.shortcut_catalog_repository(), db.shortcuts_repository());
    let command_service = ShortcutCenterCommandService::new(
        db.apps_repository(),
        db.shortcuts_repository(),
        db.shortcut_catalog_repository(),
        db.notification_snapshot_repository(),
        db.shortcut_imports_repository(),
        shortcut_store,
    );
    let app = NSApplication::sharedApplication(mtm);
    app.activate();
    let bridge = table::ShortcutCenterBridge::new(mtm);
    let ui = window::build_shortcut_center_window(mtm, bridge)?;
    populate_popup(
        &ui.shortcuts_pane.filter_popup,
        &[
            FILTER_ACTIVE_ONLY.to_string(),
            FILTER_INCLUDE_DISMISSED.to_string(),
        ],
        0,
    );

    Ok(ShortcutCenterRuntime {
        app,
        ui,
        catalog_service,
        command_service,
    })
}

// To preselect the selected app based on the frontmost app
fn initial_pending_app_id(catalog_service: &ShortcutCenterCatalogService) -> Result<Option<AppId>, AppError> {
    let apps = load_apps_or_error(catalog_service)?;
    let default_app = frontmost::frontmost_app_name();
    let preferred = catalog_service
        .resolve_preferred_app(&apps, default_app.as_deref())?
        .or_else(|| apps.first().map(|app| app.app_id));
    Ok(preferred)
}

/// Gather data
fn load_app_state(
    runtime: &ShortcutCenterRuntime,
    pending_app_id: Option<AppId>,
) -> Result<LoadedAppState, AppError> {
    let apps = load_apps_or_error(&runtime.catalog_service)?;
    let selected_app = select_app(&runtime.ui, &apps, pending_app_id)?;
    let include_dismissed = include_dismissed(&runtime.ui);
    let view = runtime.catalog_service.load_app_view(selected_app.app_id, include_dismissed)?;

    Ok(LoadedAppState {
        apps,
        selected_app,
        include_dismissed,
        aliases: view.aliases,
        shortcuts: view.shortcuts,
    })
}

fn load_apps_or_error(catalog_service: &ShortcutCenterCatalogService) -> Result<Vec<AppSummary>, AppError> {
    let apps = catalog_service.load_apps()?;
    if apps.is_empty() {
        return Err(AppError::StorageOperation(
            "no apps are available in Shortcuts".to_string(),
        ));
    }
    Ok(apps)
}

fn select_app(
    ui: &super::ShortcutCenterWindowUi,
    apps: &[AppSummary],
    pending_app_id: Option<AppId>,
) -> Result<AppSummary, AppError> {
    let app_index = pending_app_id
        .and_then(|app_id| apps.iter().position(|candidate| candidate.app_id == app_id))
        .or_else(|| {
            window::popup_selected_title(&ui.shortcuts_pane.app_popup)
                .as_ref()
                .and_then(|name| apps.iter().position(|candidate| &candidate.name == name))
        })
        .unwrap_or(0)
        .min(apps.len().saturating_sub(1));

    populate_popup(
        &ui.shortcuts_pane.app_popup,
        &apps.iter().map(|app| app.name.clone()).collect::<Vec<_>>(),
        app_index,
    );

    apps.get(app_index)
        .cloned()
        .ok_or_else(|| AppError::StorageOperation("selected app is no longer available".to_string()))
}

fn include_dismissed(ui: &super::ShortcutCenterWindowUi) -> bool {
    window::popup_selected_title(&ui.shortcuts_pane.filter_popup)
        .as_deref()
        .is_some_and(|value| value == FILTER_INCLUDE_DISMISSED)
}

/// Show data
fn render_app_state(
    ui: &super::ShortcutCenterWindowUi,
    loaded: &LoadedAppState,
    selection: ShortcutCenterSelectionState,
    status: &str,
) -> ShortcutCenterSelectionState {
    let selection = preserve_selection(selection, &loaded.shortcuts);
    ui.bridge.set_rows(&loaded.shortcuts);
    ui.shortcuts_pane.table_view.reloadData();
    table::sync_table_selection(
        &ui.shortcuts_pane.table_view,
        &loaded.shortcuts,
        &selection.selected_ids,
    );

    update_ui(
        ui,
        &loaded.selected_app,
        &loaded.aliases,
        loaded.include_dismissed,
        &loaded.shortcuts,
        &selection,
        status,
    );

    selection
}

fn run_modal_intent(app: &NSApplication, ui: &super::ShortcutCenterWindowUi) -> Option<UiIntent> {
    ui.window.makeKeyAndOrderFront(None);
    let response = app.runModalForWindow(&ui.window);
    if !ui.window.isVisible() {
        return None;
    }
    if response != objc2_app_kit::NSModalResponseOK && response != objc2_app_kit::NSModalResponseStop {
        return None;
    }

    let tag = ui.bridge.take_last_tag();
    if response == objc2_app_kit::NSModalResponseStop && tag.is_none() {
        return None;
    }

    Some(super::commands::decode_intent(tag))
}

fn load_action_inputs(
    runtime: &ShortcutCenterRuntime,
    apps: &[AppSummary],
    fallback_app: &AppSummary,
) -> Result<(AppSummary, Vec<ManagedShortcut>, ShortcutCenterSelectionState), AppError> {
    let selected_app_name = window::popup_selected_title(&runtime.ui.shortcuts_pane.app_popup)
        .unwrap_or(fallback_app.name.clone());
    let selected_app = apps
        .iter()
        .find(|app| app.name == selected_app_name)
        .cloned()
        .ok_or_else(|| AppError::StorageOperation("selected app is no longer available".to_string()))?;
    let include_dismissed = include_dismissed(&runtime.ui);
    let latest_shortcuts =
        runtime.catalog_service.load_app_view(selected_app.app_id, include_dismissed)?.shortcuts;

    let mut selection = ShortcutCenterSelectionState {
        selected_ids: table::selected_shortcut_ids(&runtime.ui.shortcuts_pane.table_view, &latest_shortcuts),
        focused_id: table::focused_shortcut_id(&runtime.ui.shortcuts_pane.table_view, &latest_shortcuts),
    };
    if let Some(row) = runtime.ui.bridge.take_context_row_override() {
        if let Some(shortcut) = latest_shortcuts.get(row) {
            selection.selected_ids = vec![shortcut.id];
            selection.focused_id = Some(shortcut.id);
        }
    }

    Ok((selected_app, latest_shortcuts, selection))
}

fn cleanup_shortcut_center(runtime: ShortcutCenterRuntime) {
    runtime.ui.window.orderOut(None);
    let _ = &runtime.ui.toolbar;
    let _ = &runtime.ui.shortcuts_pane.table_scroll;
}

fn handle_intent(
    intent: UiIntent,
    runtime: &ActionRuntime<'_>,
    selection: &mut ShortcutCenterSelectionState,
    pending_app_id: &mut Option<AppId>,
    ui: &super::ShortcutCenterWindowUi,
) -> Result<String, AppError> {
    match intent {
        UiIntent::None => Ok("Ready.".to_string()),
        UiIntent::WindowClosed => Ok("Closing Shortcuts.".to_string()),
        UiIntent::AppChanged => {
            selection.selected_ids.clear();
            selection.focused_id = None;
            *pending_app_id = None;
            Ok("List updated for the selected app.".to_string())
        }
        UiIntent::FilterChanged => {
            selection.selected_ids.clear();
            selection.focused_id = None;
            Ok("List updated for the selected filter.".to_string())
        }
        UiIntent::TableSelectionChanged => Ok(match selection.selected_ids.len() {
            0 => "Selection cleared.".to_string(),
            1 => "1 shortcut selected.".to_string(),
            count => format!("{count} shortcuts selected."),
        }),
        UiIntent::ImportSelectedApp => dialogs::open_import_dialog(
            runtime.command_service,
            ImportTarget::from_app(runtime.app),
            &ui.window,
        ),
        UiIntent::DeleteSelectedApp => {
            let status = actions::delete_app(runtime.command_service, runtime.app)?;
            selection.selected_ids.clear();
            selection.focused_id = None;
            *pending_app_id = None;
            Ok(status)
        }
        UiIntent::NewAppCreated => match dialogs::open_new_app_dialog(runtime.command_service, &ui.window) {
            Ok(dialogs::NewAppDialogResult::Created {
                app_id,
                app_name,
                alias_count,
            }) => {
                selection.selected_ids.clear();
                selection.focused_id = None;
                *pending_app_id = Some(app_id);
                Ok(if alias_count == 0 {
                    format!(
                        "Created {app_name}. Next: use Import Shortcuts for a CSV import, or Add Shortcut to add one by one."
                    )
                } else {
                    format!(
                        "Created {app_name} with {alias_count} alias(es). Next: use Import Shortcuts for a CSV import, or Add Shortcut to add one by one."
                    )
                })
            }
            Ok(dialogs::NewAppDialogResult::Canceled) => Ok("New app canceled.".to_string()),
            Err(error) => Ok(format!("Couldn't create app: {error}")),
        },
        UiIntent::Command(command) => run_command(command, runtime, selection, ui),
    }
}

fn run_command(
    command: ActionCommand,
    runtime: &ActionRuntime<'_>,
    selection: &ShortcutCenterSelectionState,
    ui: &super::ShortcutCenterWindowUi,
) -> Result<String, AppError> {
    let selected_shortcuts = selection.selected_shortcuts(runtime.shortcuts);
    actions::apply_command(
        command,
        runtime.command_service,
        runtime.app.app_id,
        &runtime.app.name,
        &selected_shortcuts,
        Some(&ui.inspector.description_field.stringValue().to_string()),
        &ui.window,
    )
}

fn update_ui(
    ui: &super::ShortcutCenterWindowUi,
    app: &AppSummary,
    aliases: &[String],
    include_dismissed: bool,
    shortcuts: &[crate::storage::ManagedShortcut],
    selection: &ShortcutCenterSelectionState,
    status: &str,
) {
    window::set_label_text(&ui.shortcuts_pane.status_label, status);
    let (summary, alias_text) = render::summarize_app(app, aliases, include_dismissed, shortcuts);
    let selected_shortcuts = selection.selected_shortcuts(shortcuts);
    let focused_shortcut = selection.focused_shortcut(shortcuts);
    let availability = render::shortcut_action_availability(&selected_shortcuts);

    window::set_label_text(&ui.inspector.title_label, &app.name);
    window::set_label_text(&ui.inspector.summary_label, &summary);
    window::set_label_text(&ui.inspector.alias_label, &alias_text);
    window::set_label_text(
        &ui.inspector.selection_label,
        &render::selection_title(&selected_shortcuts),
    );
    window::set_label_text(
        &ui.inspector.hint_label,
        &if selected_shortcuts.is_empty() {
            render::app_hint(&ImportTarget::from_app(app))
        } else {
            render::selection_hint(&selected_shortcuts)
        },
    );

    let show_single = selected_shortcuts.len() == 1;
    set_single_selection_visible(ui, show_single);
    ui.inspector.save_button.setEnabled(availability.can_save_description);
    let can_change_visibility = availability.visibility_action.is_some();
    ui.inspector.visibility_button.setEnabled(can_change_visibility);
    ui.inspector.delete_button.setEnabled(availability.can_delete);
    ui.shortcuts_pane.table_context_menu.visibility_item.setEnabled(can_change_visibility);
    ui.shortcuts_pane.table_context_menu.delete_item.setEnabled(availability.can_delete);

    let (visibility_label, visibility_tooltip) = match availability.visibility_action {
        Some(render::VisibilityAction::Hide) => ("Hide", "Hide selected shortcuts from notifications"),
        Some(render::VisibilityAction::ShowAgain) => {
            ("Show Again", "Show selected shortcuts in notifications again")
        }
        None => (
            "Change Visibility",
            "Select shortcuts that are all visible or all hidden",
        ),
    };
    ui.inspector.visibility_button.setTitle(&objc2_foundation::NSString::from_str(visibility_label));
    ui.inspector
        .visibility_button
        .setToolTip(Some(&objc2_foundation::NSString::from_str(visibility_tooltip)));
    ui.shortcuts_pane
        .table_context_menu
        .visibility_item
        .setTitle(&objc2_foundation::NSString::from_str(visibility_label));

    let delete_tooltip = if selected_shortcuts.len() == 1 {
        "Delete this shortcut"
    } else {
        "Delete the selected shortcuts"
    };
    ui.inspector.delete_button.setToolTip(Some(&objc2_foundation::NSString::from_str(delete_tooltip)));

    if let Some(shortcut) = focused_shortcut.filter(|_| show_single) {
        window::set_label_text(&ui.inspector.shortcut_value, &shortcut.shortcut_display);
        window::set_label_text(
            &ui.inspector.status_value,
            match shortcut.state {
                crate::storage::ShortcutState::Active => "Active",
                crate::storage::ShortcutState::Dismissed => "Hidden",
            },
        );
        ui.inspector
            .description_field
            .setStringValue(&objc2_foundation::NSString::from_str(&shortcut.description));
        ui.bridge.set_description_baseline(Some(&shortcut.description));
    } else {
        window::set_label_text(&ui.inspector.shortcut_value, "");
        window::set_label_text(&ui.inspector.status_value, "");
        ui.inspector.description_field.setStringValue(&objc2_foundation::NSString::from_str(""));
        ui.bridge.set_description_baseline(None);
    }
}

fn set_single_selection_visible(ui: &super::ShortcutCenterWindowUi, visible: bool) {
    ui.inspector.shortcut_label.setHidden(!visible);
    ui.inspector.shortcut_value.setHidden(!visible);
    ui.inspector.status_label.setHidden(!visible);
    ui.inspector.status_value.setHidden(!visible);
    ui.inspector.description_label.setHidden(!visible);
    ui.inspector.description_field.setHidden(!visible);
    ui.inspector.save_button.setHidden(!visible);
}
