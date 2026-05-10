use crate::application::notification_types::SchedulerCommand;
use crate::application::notifications::WorkerCommand;
use crate::application::runtime_settings::{self, parse_duration_setting, parse_shortcut_focus_count};
use crate::domain::errors::AppError;
use crate::domain::models::AppConfig;
use crate::runtime::macos::ui::modal::{add_modal_action_button, populate_popup, show_modal_error};
use crate::storage::{AppSettings, SqliteDb, SqliteSettingsRepository};
use humantime::format_duration;
use objc2::rc::Retained;
use objc2::sel;
use objc2::MainThreadMarker;
use objc2::MainThreadOnly;
use objc2_app_kit::{
    NSApplication, NSBackingStoreType, NSPopUpButton, NSTextField, NSView, NSWindow, NSWindowButton,
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
    cooldown_field: Retained<NSTextField>,
    app_switch_bounce_field: Retained<NSTextField>,
    shortcut_focus_count_popup: Retained<NSPopUpButton>,
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
    let current = current_runtime_settings(config);

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
                cooldown: read_optional_text_field(&ui.cooldown_field),
                app_switch_bounce: read_optional_text_field(&ui.app_switch_bounce_field),
                shortcut_focus_count: read_selected_popup_title(&ui.shortcut_focus_count_popup),
                terminal_notifier_path: read_optional_text_field(&ui.terminal_notifier_field),
            };

            if let Err(error) = save_settings(repo, worker_tx, edited) {
                show_modal_error(&app, "Save failed", &error.to_string())?;
                continue;
            }

            ui.window.orderOut(None);
            return Ok(SettingsDialogResult::Saved);
        }

        ui.window.orderOut(None);
        return Ok(SettingsDialogResult::Canceled);
    }
}

fn current_runtime_settings(config: &AppConfig) -> AppSettings {
    AppSettings {
        cooldown: Some(format_duration(config.cooldown).to_string()),
        app_switch_bounce: Some(format_duration(config.app_switch_bounce).to_string()),
        shortcut_focus_count: Some(config.shortcut_focus_count.to_string()),
        terminal_notifier_path: config.terminal_notifier_path.clone(),
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
    if let Some(cooldown) = settings.cooldown.as_deref() {
        parse_duration_setting("cooldown", cooldown)?;
    }
    if let Some(app_switch_bounce) = settings.app_switch_bounce.as_deref() {
        parse_duration_setting("app switch bounce", app_switch_bounce)?;
    }
    if let Some(shortcut_focus_count) = settings.shortcut_focus_count.as_deref() {
        parse_shortcut_focus_count(shortcut_focus_count)?;
    }
    Ok(())
}

fn apply_runtime_worker_overrides(
    worker_tx: &mpsc::Sender<WorkerCommand>,
    settings: &AppSettings,
) -> Result<(), AppError> {
    let env_cooldown = std::env::var("COOLDOWN").ok();
    let env_app_switch_bounce = std::env::var("APP_SWITCH_BOUNCE").ok();
    let env_shortcut_focus_count = std::env::var("SHORTCUT_FOCUS_COUNT").ok();
    worker_tx
        .send(WorkerCommand::Scheduler(SchedulerCommand::Cooldown(
            resolve_runtime_cooldown(settings, env_cooldown.as_deref())?,
        )))
        .map_err(|e| AppError::UiOperation(format!("failed to send cooldown update: {e}")))?;
    worker_tx
        .send(WorkerCommand::Scheduler(SchedulerCommand::AppSwitchBounce(
            resolve_runtime_app_switch_bounce(settings, env_app_switch_bounce.as_deref())?,
        )))
        .map_err(|e| AppError::UiOperation(format!("failed to send app switch bounce update: {e}")))?;
    worker_tx
        .send(WorkerCommand::ShortcutFocusCount(
            resolve_runtime_shortcut_focus_count(settings, env_shortcut_focus_count.as_deref())?,
        ))
        .map_err(|e| AppError::UiOperation(format!("failed to send shortcut focus update: {e}")))?;
    Ok(())
}

fn resolve_runtime_cooldown(
    settings: &AppSettings,
    env_cooldown: Option<&str>,
) -> Result<Duration, AppError> {
    runtime_settings::resolve_cooldown(None, env_cooldown, settings.cooldown.as_deref())
}

fn resolve_runtime_app_switch_bounce(
    settings: &AppSettings,
    env_app_switch_bounce: Option<&str>,
) -> Result<Duration, AppError> {
    runtime_settings::resolve_app_switch_bounce(
        None,
        env_app_switch_bounce,
        settings.app_switch_bounce.as_deref(),
    )
}

fn resolve_runtime_shortcut_focus_count(
    settings: &AppSettings,
    env_shortcut_focus_count: Option<&str>,
) -> Result<usize, AppError> {
    runtime_settings::resolve_shortcut_focus_count(
        None,
        env_shortcut_focus_count,
        settings.shortcut_focus_count.as_deref(),
    )
}

const WINDOW_WIDTH: f64 = 680.0;
const WINDOW_HEIGHT: f64 = 360.0;
const CONTENT_LEFT: f64 = 20.0;
const CONTENT_WIDTH: f64 = 640.0;
const ACTION_BUTTON_WIDTH: f64 = 90.0;
const ACTION_BUTTON_HEIGHT: f64 = 30.0;
const LABEL_HEIGHT: f64 = 20.0;
const FIELD_HEIGHT: f64 = 26.0;
const FIELD_LABEL_OFFSET_Y: f64 = 24.0;
const SHORTCUT_FOCUS_COUNT_OPTIONS: [usize; 12] = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 15, 20];

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
        .ok_or_else(|| AppError::UiOperation("missing settings window content view".to_string()))?;

    let cooldown_field = add_labeled_text_field(
        &content,
        mtm,
        "Choose the duration between showing shortcuts (examples: 45s, 10m, 1h)",
        260.0,
        settings.cooldown.as_deref().unwrap_or(""),
    );
    let app_switch_bounce_field = add_labeled_text_field(
        &content,
        mtm,
        "You need to stay in the app for this duration before shortcuts for it can appear (examples: 30s, 1m)",
        200.0,
        settings.app_switch_bounce.as_deref().unwrap_or(""),
    );
    let shortcut_focus_count_popup = add_labeled_popup(
        &content,
        mtm,
        "Number of shortcuts to focus on daily (per app)",
        140.0,
        settings.shortcut_focus_count.as_deref().unwrap_or(""),
    );
    let terminal_notifier_field = add_labeled_text_field(
        &content,
        mtm,
        "terminal-notifier path (optional; used for future terminal or non-bundled runs)",
        80.0,
        settings.terminal_notifier_path.as_deref().unwrap_or(""),
    );

    add_modal_action_button(
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
    add_modal_action_button(
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
        cooldown_field,
        app_switch_bounce_field,
        shortcut_focus_count_popup,
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

fn add_labeled_popup(
    content: &NSView,
    mtm: MainThreadMarker,
    label: &str,
    y: f64,
    value: &str,
) -> Retained<NSPopUpButton> {
    let label_view = NSTextField::labelWithString(&NSString::from_str(label), mtm);
    label_view.setFrame(NSRect::new(
        NSPoint::new(CONTENT_LEFT, y + FIELD_LABEL_OFFSET_Y),
        NSSize::new(CONTENT_WIDTH, LABEL_HEIGHT),
    ));
    content.addSubview(&label_view);

    let popup = NSPopUpButton::initWithFrame_pullsDown(
        NSPopUpButton::alloc(mtm),
        NSRect::new(NSPoint::new(CONTENT_LEFT, y), NSSize::new(160.0, FIELD_HEIGHT)),
        false,
    );
    populate_shortcut_focus_count_popup(&popup, value);
    content.addSubview(&popup);
    popup
}

fn populate_shortcut_focus_count_popup(popup: &NSPopUpButton, value: &str) {
    let current = parse_positive_usize(value);
    let mut values = SHORTCUT_FOCUS_COUNT_OPTIONS.to_vec();
    preserve_current_shortcut_focus_count(&mut values, current);
    values.sort_unstable();

    let selected_value = selected_shortcut_focus_count(current);
    let selected_index = values.iter().position(|value| *value == selected_value).unwrap_or_else(|| {
        values.iter().position(|value| *value == selected_shortcut_focus_count(None)).unwrap_or(0)
    });

    let items = values.into_iter().map(|value| value.to_string()).collect::<Vec<_>>();
    populate_popup(popup, &items, selected_index);
}

fn preserve_current_shortcut_focus_count(values: &mut Vec<usize>, current: Option<usize>) {
    if let Some(current) = current {
        if !values.contains(&current) {
            values.push(current);
        }
    }
}

fn selected_shortcut_focus_count(current: Option<usize>) -> usize {
    current.unwrap_or(runtime_settings::DEFAULT_SHORTCUT_FOCUS_COUNT)
}

fn parse_positive_usize(value: &str) -> Option<usize> {
    value.trim().parse::<usize>().ok().filter(|value| *value > 0)
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

fn read_selected_popup_title(popup: &NSPopUpButton) -> Option<String> {
    popup.titleOfSelectedItem().map(|title| title.to_string()).filter(|value| !value.trim().is_empty())
}

#[cfg(test)]
mod tests {
    use super::{
        parse_positive_usize, preserve_current_shortcut_focus_count, resolve_runtime_app_switch_bounce,
        resolve_runtime_cooldown, resolve_runtime_shortcut_focus_count, selected_shortcut_focus_count,
        SHORTCUT_FOCUS_COUNT_OPTIONS,
    };
    use crate::storage::AppSettings;
    use std::time::Duration;

    fn app_settings(
        cooldown: Option<&str>,
        app_switch_bounce: Option<&str>,
        shortcut_focus_count: Option<&str>,
    ) -> AppSettings {
        AppSettings {
            cooldown: cooldown.map(str::to_string),
            app_switch_bounce: app_switch_bounce.map(str::to_string),
            shortcut_focus_count: shortcut_focus_count.map(str::to_string),
            terminal_notifier_path: None,
        }
    }

    #[test]
    fn live_settings_use_env_when_present() {
        let settings = AppSettings {
            cooldown: Some("10m".to_string()),
            app_switch_bounce: Some("30s".to_string()),
            shortcut_focus_count: Some("4".to_string()),
            terminal_notifier_path: None,
        };

        assert_eq!(
            resolve_runtime_cooldown(&settings, Some("45s")).expect("cooldown"),
            Duration::from_secs(45)
        );
        assert_eq!(
            resolve_runtime_app_switch_bounce(&settings, Some("15s")).expect("bounce"),
            Duration::from_secs(15)
        );
        assert_eq!(
            resolve_runtime_shortcut_focus_count(&settings, Some("8")).expect("focus count"),
            8
        );
    }

    #[test]
    fn live_settings_use_db_when_env_is_missing() {
        let settings = app_settings(Some("10m"), Some("30s"), Some("4"));

        assert_eq!(
            resolve_runtime_cooldown(&settings, None).expect("cooldown"),
            Duration::from_secs(10 * 60)
        );
        assert_eq!(
            resolve_runtime_app_switch_bounce(&settings, None).expect("bounce"),
            Duration::from_secs(30)
        );
        assert_eq!(
            resolve_runtime_shortcut_focus_count(&settings, None).expect("focus count"),
            4
        );
    }

    #[test]
    fn shortcut_focus_count_popup_preserves_valid_current_value() {
        let mut values = SHORTCUT_FOCUS_COUNT_OPTIONS.to_vec();

        preserve_current_shortcut_focus_count(&mut values, parse_positive_usize("13"));

        assert!(values.contains(&13));
        assert_eq!(selected_shortcut_focus_count(parse_positive_usize("13")), 13);
    }

    #[test]
    fn shortcut_focus_count_popup_falls_back_to_default_for_invalid_value() {
        let mut values = SHORTCUT_FOCUS_COUNT_OPTIONS.to_vec();

        preserve_current_shortcut_focus_count(&mut values, parse_positive_usize("0"));

        assert_eq!(selected_shortcut_focus_count(parse_positive_usize("0")), 5);
        assert!(!values.contains(&0));
    }
}
