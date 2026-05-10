use crate::application::notification_types::ChosenApp;
use crate::application::shortcut_focus::ShortcutFocusSelector;
use crate::constants::APP_NAME;
use crate::domain::errors::AppError;
use crate::domain::models::AppConfig;
use crate::notifications::notification_payload;
use crate::notifications::notifier::{Notifier, TerminalNotifier};
use crate::storage::NotificationSnapshot;
use std::thread;

pub(crate) fn run(config: AppConfig, initial_snapshot: NotificationSnapshot) -> Result<(), AppError> {
    println!("Starting {} in terminal mode. Press Ctrl+C to quit.", APP_NAME);

    let notifier = TerminalNotifier::new(config.terminal_notifier_path.clone());
    let mut shortcut_focus = ShortcutFocusSelector::new(config.shortcut_focus_count);

    loop {
        let content = notification_payload(&initial_snapshot, ChosenApp::RandomShortcut, &mut shortcut_focus);
        if let Err(err) = notifier.notify(&content) {
            eprintln!("{err}");
        }

        thread::sleep(config.cooldown);
    }
}
