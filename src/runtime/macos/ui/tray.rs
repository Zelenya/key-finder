use crate::application::notification_types::SchedulerCommand;
use crate::application::notifications::{start_notification_service, WorkerCommand};
use crate::application::shortcut_center::ShortcutCache;
use crate::constants::APP_NAME;
use crate::domain::errors::AppError;
use crate::domain::models::AppConfig;
use crate::notifications::notifier::NativeNotifier;
use crate::runtime::macos::notifications::notify_runtime_error;
use crate::runtime::macos::platform::frontmost;
use crate::runtime::macos::ui::shortcut_center;
use crate::runtime::macos::ui::{app_focus, settings};
use crate::storage::NotificationSnapshot;
use objc2::rc::Retained;
use objc2::MainThreadMarker;
use objc2_app_kit::{NSApplication, NSApplicationActivationPolicy};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tray_icon::menu::{CheckMenuItem, Menu, MenuEvent, MenuId, MenuItem, PredefinedMenuItem};
use tray_icon::{Icon, TrayIcon, TrayIconBuilder};

struct TrayMenuIds {
    open_shortcuts: MenuId,
    open_settings: MenuId,
    open_app_focus: MenuId,
    toggle_pause: MenuId,
    quit: MenuId,
}

struct TrayRuntime {
    ui_config: AppConfig,
    shortcuts: ShortcutCache,
    command_tx: std::sync::mpsc::Sender<WorkerCommand>,
    paused: Arc<AtomicBool>,
}

pub(crate) fn run(config: AppConfig, initial_snapshot: NotificationSnapshot) -> Result<(), AppError> {
    let app = ensure_appkit_initialized()?;
    println!("Starting Key Finder in tray mode.");

    let shortcuts = ShortcutCache::new(initial_snapshot);
    let (command_tx, worker_service) = start_notification_service(
        config.cooldown,
        config.app_switch_bounce,
        shortcuts.clone(),
        Arc::new(NativeNotifier::new()),
        Arc::new(frontmost::frontmost_app_name),
    );
    let shutdown_tx = command_tx.clone();
    let (_tray, menu_ids) = build_tray()?;
    let runtime = TrayRuntime {
        ui_config: config.clone(),
        shortcuts: shortcuts.clone(),
        command_tx: command_tx.clone(),
        paused: Arc::new(AtomicBool::new(false)),
    };

    MenuEvent::set_event_handler(Some(move |event: MenuEvent| {
        handle_menu_event(&event, &menu_ids, &runtime);
    }));

    app.run();
    let _ = shutdown_tx.send(WorkerCommand::Stop);
    MenuEvent::set_event_handler::<fn(MenuEvent)>(None);
    worker_service.join()?;
    Ok(())
}

fn ensure_appkit_initialized() -> Result<Retained<NSApplication>, AppError> {
    let Some(mtm) = MainThreadMarker::new() else {
        return Err(AppError::MainThreadRequired);
    };

    let app = NSApplication::sharedApplication(mtm);
    let _ = app.setActivationPolicy(NSApplicationActivationPolicy::Accessory);
    app.finishLaunching();
    Ok(app)
}

fn handle_menu_event(event: &MenuEvent, menu_ids: &TrayMenuIds, runtime: &TrayRuntime) {
    if event.id == menu_ids.open_shortcuts {
        if let Err(err) = shortcut_center::open_shortcut_center(&runtime.ui_config, runtime.shortcuts.clone())
        {
            eprintln!("{err}");
            notify_runtime_error("Shortcuts failed", &err.to_string());
        }
        return;
    }

    if event.id == menu_ids.open_settings {
        if let Err(err) = settings::open_settings(&runtime.ui_config, &runtime.command_tx) {
            eprintln!("{err}");
            notify_runtime_error("Settings failed", &err.to_string());
        }
        return;
    }

    if event.id == menu_ids.open_app_focus {
        if let Err(err) = app_focus::open_focus_app(&runtime.ui_config, &runtime.command_tx) {
            eprintln!("{err}");
            notify_runtime_error("App Focus failed", &err.to_string());
        }
        return;
    }

    if event.id == menu_ids.toggle_pause {
        let was_paused = runtime.paused.fetch_xor(true, Ordering::SeqCst);
        let now_paused = !was_paused;
        let _ = runtime.command_tx.send(WorkerCommand::Update(SchedulerCommand::Pause(now_paused)));
        if now_paused {
            eprintln!("notifications paused");
        } else {
            eprintln!("notifications resumed");
        }
        return;
    }

    if event.id == menu_ids.quit {
        let _ = runtime.command_tx.send(WorkerCommand::Stop);
        if let Some(mtm) = MainThreadMarker::new() {
            let app = NSApplication::sharedApplication(mtm);
            app.terminate(None);
        }
    }
}

fn build_tray() -> Result<(TrayIcon, TrayMenuIds), AppError> {
    let menu = Menu::new();

    let open_shortcuts_item = MenuItem::new("Open Shortcuts", true, None);
    let open_shortcuts_id = open_shortcuts_item.id().clone();
    menu.append(&open_shortcuts_item).map_err(|e| AppError::TrayMenu {
        message: e.to_string(),
    })?;

    let open_settings_item = MenuItem::new("Settings", true, None);
    let open_settings_id = open_settings_item.id().clone();
    menu.append(&open_settings_item).map_err(|e| AppError::TrayMenu {
        message: e.to_string(),
    })?;

    let open_app_focus_item = MenuItem::new("Focus on one App", true, None);
    let open_app_focus_id = open_app_focus_item.id().clone();
    menu.append(&open_app_focus_item).map_err(|e| AppError::TrayMenu {
        message: e.to_string(),
    })?;

    menu.append(&PredefinedMenuItem::separator()).map_err(|e| AppError::TrayMenu {
        message: e.to_string(),
    })?;

    let toggle_pause_item = CheckMenuItem::new("Pause Notifications", true, false, None);
    let toggle_pause_id = toggle_pause_item.id().clone();
    menu.append(&toggle_pause_item).map_err(|e| AppError::TrayMenu {
        message: e.to_string(),
    })?;

    menu.append(&PredefinedMenuItem::separator()).map_err(|e| AppError::TrayMenu {
        message: e.to_string(),
    })?;

    let quit_item = MenuItem::new("Quit", true, None);
    let quit_id = quit_item.id().clone();
    menu.append(&quit_item).map_err(|e| AppError::TrayMenu {
        message: e.to_string(),
    })?;

    let tray_icon = create_tray_icon()?;
    let tray = TrayIconBuilder::new()
        .with_icon(tray_icon)
        .with_icon_as_template(true)
        .with_tooltip(APP_NAME)
        .with_menu(Box::new(menu))
        .build()
        .map_err(|e| AppError::TrayInit {
            message: e.to_string(),
        })?;

    Ok((
        tray,
        TrayMenuIds {
            open_shortcuts: open_shortcuts_id,
            open_settings: open_settings_id,
            open_app_focus: open_app_focus_id,
            toggle_pause: toggle_pause_id,
            quit: quit_id,
        },
    ))
}

fn create_tray_icon() -> Result<Icon, AppError> {
    let width = 24u32;
    let height = 24u32;
    let mut rgba = vec![0u8; (width * height * 4) as usize];

    let mut put = |x: i32, y: i32, alpha: u8| {
        if x < 0 || y < 0 || x >= width as i32 || y >= height as i32 {
            return;
        }
        let idx = ((y as u32 * width + x as u32) * 4) as usize;
        rgba[idx] = 255;
        rgba[idx + 1] = 255;
        rgba[idx + 2] = 255;
        rgba[idx + 3] = alpha;
    };

    let cx = 8i32;
    let cy = 12i32;
    for y in 0..height as i32 {
        for x in 0..width as i32 {
            let dx = x - cx;
            let dy = y - cy;
            let d2 = dx * dx + dy * dy;
            if (16..=34).contains(&d2) {
                put(x, y, 255);
            }
        }
    }
    for y in 11..=13 {
        for x in 12..=20 {
            put(x, y, 255);
        }
    }
    for y in 13..=16 {
        for x in 18..=20 {
            put(x, y, 255);
        }
    }
    for y in 13..=15 {
        for x in 15..=17 {
            put(x, y, 255);
        }
    }

    Icon::from_rgba(rgba, width, height).map_err(|e| AppError::TrayInit {
        message: format!("failed to build tray icon bitmap: {e}"),
    })
}
