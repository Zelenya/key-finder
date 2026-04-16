use crate::application::notifications::WorkerCommand;
use crate::application::runtime_settings::{self, parse_notify_interval};
use crate::domain::errors::AppError;
use crate::domain::models::AppConfig;
use crate::storage::{AppSettings, SqliteDb, SqliteSettingsRepository};
use humantime::format_duration;
use objc2::rc::Retained;
use objc2::sel;
use objc2::MainThreadMarker;
use objc2::MainThreadOnly;
use objc2_app_kit::{
    NSAlert, NSApplication, NSBackingStoreType, NSButton, NSTextField, NSView, NSWindow, NSWindowButton,
    NSWindowStyleMask,
};
use objc2_foundation::{NSPoint, NSRect, NSSize, NSString};
use std::sync::mpsc;
use std::time::Duration;

pub(crate) enum SettingsDialogResult {
    Saved,
    Canceled,
}

struct SettingsWindowUi {
    window: Retained<NSWindow>,
    notify_interval_field: Retained<NSTextField>,
    terminal_notifier_field: Retained<NSTextField>,
}

pub(crate) fn open_settings(
    config: &AppConfig,
    worker_tx: &mpsc::Sender<WorkerCommand>,
) -> Result<(), AppError> {
    let db = SqliteDb::open(&config.database_path)?;
    let repo = db.settings_repository();
    open_settings_dialog(config, &repo, worker_tx)?;
    Ok(())
}

fn open_settings_dialog(
    config: &AppConfig,
    repo: &SqliteSettingsRepository,
    worker_tx: &mpsc::Sender<WorkerCommand>,
) -> Result<SettingsDialogResult, AppError> {
    let current = AppSettings {
        notify_interval: Some(format_duration(config.interval).to_string()),
        terminal_notifier_path: config.terminal_notifier_path.clone(),
    };

    let Some(mtm) = MainThreadMarker::new() else {
        return Err(AppError::MainThreadRequired);
    };

    let app = NSApplication::sharedApplication(mtm);
    app.activate();

    let ui = build_settings_window(mtm, &app, &current)?;

    loop {
        ui.window.makeKeyAndOrderFront(None);

        let response = app.runModalForWindow(&ui.window);

        if !ui.window.isVisible() {
            ui.window.orderOut(None);
            return Ok(SettingsDialogResult::Canceled);
        }
        if response == objc2_app_kit::NSModalResponseStop {
            let edited = AppSettings {
                notify_interval: read_optional_text_field(&ui.notify_interval_field),
                terminal_notifier_path: read_optional_text_field(&ui.terminal_notifier_field),
            };

            if let Err(error) = save_settings(repo, worker_tx, edited) {
                show_settings_error(&app, "Save failed", &error.to_string())?;
                continue;
            }

            ui.window.orderOut(None);
            return Ok(SettingsDialogResult::Saved);
        }

        ui.window.orderOut(None);
        return Ok(SettingsDialogResult::Canceled);
    }
}

fn save_settings(
    repo: &SqliteSettingsRepository,
    worker_tx: &mpsc::Sender<WorkerCommand>,
    new_settings: AppSettings,
) -> Result<(), AppError> {
    validate_runtime_settings(&new_settings)?;
    repo.save_app_settings(&new_settings)?;
    let saved = repo.load_app_settings()?;
    apply_runtime_worker_overrides(worker_tx, &saved)?;
    Ok(())
}

fn validate_runtime_settings(settings: &AppSettings) -> Result<(), AppError> {
    if let Some(interval) = settings.notify_interval.as_deref() {
        parse_notify_interval(interval)?;
    }
    Ok(())
}

fn apply_runtime_worker_overrides(
    worker_tx: &mpsc::Sender<WorkerCommand>,
    settings: &AppSettings,
) -> Result<(), AppError> {
    let env_notify_interval = std::env::var("NOTIFY_INTERVAL").ok();
    worker_tx
        .send(WorkerCommand::SetInterval(resolve_runtime_notify_interval(
            settings,
            env_notify_interval.as_deref(),
        )?))
        .map_err(|e| AppError::StorageOperation(format!("failed to send interval update: {e}")))?;
    Ok(())
}

fn resolve_runtime_notify_interval(
    settings: &AppSettings,
    env_notify_interval: Option<&str>,
) -> Result<Duration, AppError> {
    runtime_settings::resolve_notify_interval(None, env_notify_interval, settings.notify_interval.as_deref())
}

const WINDOW_WIDTH: f64 = 680.0;
const WINDOW_HEIGHT: f64 = 220.0;
const CONTENT_LEFT: f64 = 20.0;
const CONTENT_WIDTH: f64 = 640.0;
const ACTION_BUTTON_WIDTH: f64 = 90.0;
const ACTION_BUTTON_HEIGHT: f64 = 30.0;
const LABEL_HEIGHT: f64 = 20.0;
const FIELD_HEIGHT: f64 = 26.0;
const FIELD_LABEL_OFFSET_Y: f64 = 24.0;

fn build_settings_window(
    mtm: MainThreadMarker,
    app: &NSApplication,
    settings: &AppSettings,
) -> Result<SettingsWindowUi, AppError> {
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
    window.setTitle(&NSString::from_str("Key Finder Settings"));
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
        .ok_or_else(|| AppError::StorageOperation("missing settings window content view".to_string()))?;

    let notify_interval_field = add_labeled_text_field(
        &content,
        mtm,
        "Notification interval (applies immediately; examples: 45s, 10m, 1h)",
        140.0,
        settings.notify_interval.as_deref().unwrap_or(""),
    );
    let terminal_notifier_field = add_labeled_text_field(
        &content,
        mtm,
        "terminal-notifier path (optional; used for future terminal or non-bundled runs)",
        80.0,
        settings.terminal_notifier_path.as_deref().unwrap_or(""),
    );

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

    Ok(SettingsWindowUi {
        window,
        notify_interval_field,
        terminal_notifier_field,
    })
}

fn add_labeled_text_field(
    content: &NSView,
    mtm: MainThreadMarker,
    label: &str,
    y: f64,
    value: &str,
) -> Retained<NSTextField> {
    // Label
    let label_view = NSTextField::labelWithString(&NSString::from_str(label), mtm);
    label_view.setFrame(NSRect::new(
        NSPoint::new(CONTENT_LEFT, y + FIELD_LABEL_OFFSET_Y),
        NSSize::new(CONTENT_WIDTH, LABEL_HEIGHT),
    ));
    content.addSubview(&label_view);

    // Input field
    let field = NSTextField::initWithFrame(
        NSTextField::alloc(mtm),
        NSRect::new(
            NSPoint::new(CONTENT_LEFT, y),
            NSSize::new(CONTENT_WIDTH, FIELD_HEIGHT),
        ),
    );
    field.setStringValue(&NSString::from_str(value));
    field.setEditable(true);
    field.setSelectable(true);
    content.addSubview(&field);
    field
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

fn read_optional_text_field(field: &NSTextField) -> Option<String> {
    let value = field.stringValue().to_string();
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn show_settings_error(app: &NSApplication, title: &str, message: &str) -> Result<(), AppError> {
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

#[cfg(test)]
mod tests {
    use super::resolve_runtime_notify_interval;
    use crate::storage::AppSettings;
    use std::time::Duration;

    #[test]
    fn live_interval_uses_env_when_db_setting_is_cleared() {
        let settings = AppSettings {
            notify_interval: None,
            terminal_notifier_path: None,
        };

        assert_eq!(
            resolve_runtime_notify_interval(&settings, Some("45s")).expect("interval"),
            Duration::from_secs(45)
        );
    }

    #[test]
    fn live_interval_matches_startup_precedence_when_env_and_db_are_both_present() {
        let settings = AppSettings {
            notify_interval: Some("10m".to_string()),
            terminal_notifier_path: None,
        };

        assert_eq!(
            resolve_runtime_notify_interval(&settings, Some("45s")).expect("interval"),
            Duration::from_secs(45)
        );
    }

    #[test]
    fn live_interval_uses_db_when_env_is_missing() {
        let settings = AppSettings {
            notify_interval: Some("10m".to_string()),
            terminal_notifier_path: None,
        };

        assert_eq!(
            resolve_runtime_notify_interval(&settings, None).expect("interval"),
            Duration::from_secs(10 * 60)
        );
    }
}
