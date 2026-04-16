use std::sync::mpsc;

use objc2::sel;
use objc2::{rc::Retained, MainThreadMarker, MainThreadOnly};
use objc2_app_kit::{
    NSAlert, NSApplication, NSBackingStoreType, NSButton, NSPopUpButton, NSTextField, NSView, NSWindow,
    NSWindowButton, NSWindowStyleMask,
};
use objc2_foundation::{NSArray, NSPoint, NSRect, NSSize, NSString};

use crate::storage::{AppId, AppSummary, SqliteShortcutCatalogRepository};
use crate::{
    application::notifications::WorkerCommand,
    domain::{errors::AppError, models::AppConfig},
    storage::SqliteDb,
};

pub(crate) enum FocusAppDialogResult {
    Saved,
    Canceled,
}

struct FocusAppWindowUi {
    window: Retained<NSWindow>,
    app_popup: Retained<NSPopUpButton>,
}

pub(crate) fn open_focus_app(
    config: &AppConfig,
    worker_tx: &mpsc::Sender<WorkerCommand>,
) -> Result<(), AppError> {
    let db = SqliteDb::open(&config.database_path)?;
    let repo = db.shortcut_catalog_repository();
    open_focus_app_dialog(&repo, worker_tx)?;
    Ok(())
}

fn open_focus_app_dialog(
    repo: &SqliteShortcutCatalogRepository,
    worker_tx: &mpsc::Sender<WorkerCommand>,
) -> Result<FocusAppDialogResult, AppError> {
    let Some(mtm) = MainThreadMarker::new() else {
        return Err(AppError::MainThreadRequired);
    };

    let app = NSApplication::sharedApplication(mtm);
    app.activate();

    let all_apps = repo.list_apps()?;
    let app_names = all_apps.iter().map(|app| app.name.clone()).collect::<Vec<_>>();

    let ui = build_focus_app_window(mtm, &app, app_names)?;

    loop {
        ui.window.makeKeyAndOrderFront(None);

        let response = app.runModalForWindow(&ui.window);

        if !ui.window.isVisible() {
            ui.window.orderOut(None);
            return Ok(FocusAppDialogResult::Canceled);
        }
        if response == objc2_app_kit::NSModalResponseStop {
            let app_id = get_selected_app_id(&ui.app_popup, &all_apps);
            if let Err(error) = focus_on_app(worker_tx, app_id) {
                show_app_focus_error(&app, "Save failed", &error.to_string())?;
                continue;
            }

            ui.window.orderOut(None);
            return Ok(FocusAppDialogResult::Saved);
        }

        ui.window.orderOut(None);
        return Ok(FocusAppDialogResult::Canceled);
    }
}

fn get_selected_app_id(popup: &NSPopUpButton, all_apps: &[AppSummary]) -> Option<AppId> {
    popup
        .titleOfSelectedItem()
        .map(|value| value.to_string())
        .and_then(|selected| all_apps.iter().find(|app| app.name == selected).map(|app| app.app_id))
}

fn focus_on_app(worker_tx: &mpsc::Sender<WorkerCommand>, app_id: Option<AppId>) -> Result<(), AppError> {
    worker_tx
        .send(WorkerCommand::SetFocusApp(app_id))
        .map_err(|e| AppError::StorageOperation(format!("failed to send app focus update: {e}")))
}

const WINDOW_WIDTH: f64 = 680.0;
const WINDOW_HEIGHT: f64 = 220.0;
const CONTENT_LEFT: f64 = 20.0;
const CONTENT_WIDTH: f64 = 640.0;
const ACTION_BUTTON_WIDTH: f64 = 90.0;
const ACTION_BUTTON_HEIGHT: f64 = 30.0;
const LABEL_HEIGHT: f64 = 20.0;
const FIELD_LABEL_OFFSET_Y: f64 = 24.0;

fn build_focus_app_window(
    mtm: MainThreadMarker,
    app: &NSApplication,
    app_names: Vec<String>,
) -> Result<FocusAppWindowUi, AppError> {
    let style = NSWindowStyleMask::Closable | NSWindowStyleMask::Titled;
    // Initial position doesn't matter, we (re)center the window after showing it
    let rect = NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(WINDOW_WIDTH, WINDOW_HEIGHT));
    // SAFETY: `mtm` proves we are on the AppKit main thread, and we initialize
    // a fresh `NSWindow` with valid geometry and style values.
    let window = unsafe {
        NSWindow::initWithContentRect_styleMask_backing_defer(
            NSWindow::alloc(mtm),
            rect,
            style,
            NSBackingStoreType::Buffered,
            false,
        )
    };
    window.setTitle(&NSString::from_str("Focus on one App"));
    window.center();
    if let Some(close_button) = window.standardWindowButton(NSWindowButton::CloseButton) {
        // SAFETY: the close button belongs to this live window, `app` is the
        // shared `NSApplication`, and `abortModal` cleanly cancels this dialog.
        unsafe {
            close_button.setTarget(Some(app));
            close_button.setAction(Some(sel!(abortModal)));
        }
    }

    let content = window
        .contentView()
        .ok_or_else(|| AppError::StorageOperation("missing app focus window content view".to_string()))?;

    // Label
    let label_view = NSTextField::labelWithString(
        &NSString::from_str("Only show the shortcuts for the selected app"),
        mtm,
    );
    label_view.setFrame(NSRect::new(
        NSPoint::new(CONTENT_LEFT, 140.0 + FIELD_LABEL_OFFSET_Y),
        NSSize::new(CONTENT_WIDTH, LABEL_HEIGHT),
    ));
    content.addSubview(&label_view);

    // Selected app drop down
    let app_popup = NSPopUpButton::initWithFrame_pullsDown(
        NSPopUpButton::alloc(mtm),
        NSRect::new(
            NSPoint::new(CONTENT_LEFT, 140.0 + FIELD_LABEL_OFFSET_Y - LABEL_HEIGHT - 4.0),
            NSSize::new(320.0, 28.0),
        ),
        false,
    );
    content.addSubview(&app_popup);

    // TODO: First element should be None? Or a separate flag for no focus?

    populate_popup(&app_popup, &app_names, 0);

    add_action_button(
        &content,
        mtm,
        app,
        "Cancel",
        NSRect::new(
            NSPoint::new(490.0, 20.0),
            NSSize::new(ACTION_BUTTON_WIDTH, ACTION_BUTTON_HEIGHT),
        ),
        sel!(abortModal),
    );
    add_action_button(
        &content,
        mtm,
        app,
        "Save",
        NSRect::new(
            NSPoint::new(580.0, 20.0),
            NSSize::new(ACTION_BUTTON_WIDTH, ACTION_BUTTON_HEIGHT),
        ),
        sel!(stopModal),
    );

    Ok(FocusAppWindowUi { window, app_popup })
}

// TODO: Copy-pasted from shortcut_center.rs, dedupe
fn populate_popup(popup: &NSPopUpButton, items: &[String], selected_index: usize) {
    popup.removeAllItems();
    let ns_items = items.iter().map(|item| NSString::from_str(item)).collect::<Vec<_>>();
    let array = NSArray::from_retained_slice(&ns_items);
    popup.addItemsWithTitles(&array);
    if !items.is_empty() {
        popup.selectItemAtIndex(selected_index.min(items.len() - 1) as _);
    }
}

fn add_action_button(
    content: &NSView,
    mtm: MainThreadMarker,
    app: &NSApplication,
    title: &str,
    frame: NSRect,
    action: objc2::runtime::Sel,
) {
    // SAFETY: `app` is the shared `NSApplication` on the main thread, and the
    // provided selector is one of AppKit's standard modal-ending actions.
    let button = unsafe {
        NSButton::buttonWithTitle_target_action(&NSString::from_str(title), Some(app), Some(action), mtm)
    };
    button.setFrame(frame);
    content.addSubview(&button);
}

fn show_app_focus_error(app: &NSApplication, title: &str, message: &str) -> Result<(), AppError> {
    let Some(mtm) = MainThreadMarker::new() else {
        return Err(AppError::MainThreadRequired);
    };
    app.activate();
    let alert = NSAlert::new(mtm);
    alert.setMessageText(&NSString::from_str(title));
    alert.setInformativeText(&NSString::from_str(message));
    alert.addButtonWithTitle(&NSString::from_str("OK"));
    let _ = alert.runModal();
    Ok(())
}
