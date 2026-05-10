use std::sync::mpsc;

use objc2::sel;
use objc2::{rc::Retained, MainThreadMarker, MainThreadOnly};
use objc2_app_kit::{
    NSApplication, NSBackingStoreType, NSPopUpButton, NSTextField, NSWindow, NSWindowButton,
    NSWindowStyleMask,
};
use objc2_foundation::{NSPoint, NSRect, NSSize, NSString};

use crate::application::notification_types::{AppFocusState, SchedulerCommand};
use crate::storage::{AppSummary, SqliteShortcutCatalogRepository};
use crate::{
    application::notifications::WorkerCommand,
    domain::{errors::AppError, models::AppConfig},
    runtime::macos::ui::modal::{add_modal_action_button, populate_popup, show_modal_error},
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
            let focus_state = selected_focus_state(&ui.app_popup, &all_apps)?;
            if let Err(error) = focus_on_app(worker_tx, focus_state) {
                show_modal_error(&app, "Save failed", &error.to_string())?;
                continue;
            }

            ui.window.orderOut(None);
            return Ok(FocusAppDialogResult::Saved);
        }

        ui.window.orderOut(None);
        return Ok(FocusAppDialogResult::Canceled);
    }
}

fn focus_on_app(worker_tx: &mpsc::Sender<WorkerCommand>, focus_state: AppFocusState) -> Result<(), AppError> {
    worker_tx
        .send(WorkerCommand::Scheduler(SchedulerCommand::Focus(focus_state)))
        .map_err(|e| AppError::UiOperation(format!("failed to send app focus update: {e}")))
}

const WINDOW_WIDTH: f64 = 500.0;
const WINDOW_HEIGHT: f64 = 220.0;
const CONTENT_LEFT: f64 = 20.0;
const CONTENT_WIDTH: f64 = WINDOW_WIDTH - (CONTENT_LEFT * 2.0);
const ACTION_BUTTON_WIDTH: f64 = 90.0;
const ACTION_BUTTON_HEIGHT: f64 = 30.0;
const ACTION_BUTTON_GAP: f64 = 10.0;
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
        .ok_or_else(|| AppError::UiOperation("missing app focus window content view".to_string()))?;

    // Label
    let label_view = NSTextField::labelWithString(
        &NSString::from_str("Choose the app to focus notifications on, or keep following the current app"),
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

    let default_option = "Follow current app".to_string();
    let items = std::iter::once(default_option).chain(app_names).collect::<Vec<_>>();
    populate_popup(&app_popup, &items, 0);

    let save_button_x = WINDOW_WIDTH - CONTENT_LEFT - ACTION_BUTTON_WIDTH;
    let cancel_button_x = save_button_x - ACTION_BUTTON_GAP - ACTION_BUTTON_WIDTH;

    add_modal_action_button(
        &content,
        mtm,
        app,
        "Cancel",
        NSRect::new(
            NSPoint::new(cancel_button_x, 20.0),
            NSSize::new(ACTION_BUTTON_WIDTH, ACTION_BUTTON_HEIGHT),
        ),
        sel!(abortModal),
    );
    add_modal_action_button(
        &content,
        mtm,
        app,
        "Save",
        NSRect::new(
            NSPoint::new(save_button_x, 20.0),
            NSSize::new(ACTION_BUTTON_WIDTH, ACTION_BUTTON_HEIGHT),
        ),
        sel!(stopModal),
    );

    Ok(FocusAppWindowUi { window, app_popup })
}

fn selected_focus_state(
    app_popup: &NSPopUpButton,
    all_apps: &[AppSummary],
) -> Result<AppFocusState, AppError> {
    match app_popup.indexOfSelectedItem() {
        0 => Ok(AppFocusState::FollowCurrentApp),
        _ => {
            let app_name = app_popup
                .titleOfSelectedItem()
                .map(|value| value.to_string())
                .ok_or_else(|| AppError::UiOperation("focused app selection is missing".to_string()))?;

            let app_id =
                all_apps.iter().find(|app| app.name == app_name).map(|app| app.app_id).ok_or_else(|| {
                    AppError::StorageOperation("selected focused app is no longer available".to_string())
                })?;

            Ok(AppFocusState::FocusOn(app_id))
        }
    }
}
