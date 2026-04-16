use crate::constants::APP_NAME;
use crate::domain::errors::AppError;
use crate::domain::models::AppConfig;
use crate::notifications::notification_payload;
use crate::notifications::notifier::{Notifier, TerminalNotifier};
use crate::notifications::SelectedApp;
use crate::storage::NotificationSnapshot;
use std::thread;

pub(crate) fn run(config: AppConfig, initial_snapshot: NotificationSnapshot) -> Result<(), AppError> {
    println!("Starting {} in terminal mode. Press Ctrl+C to quit.", APP_NAME);

    let notifier = TerminalNotifier::new(config.terminal_notifier_path.clone());

    loop {
        let content = notification_payload(&initial_snapshot, SelectedApp::Unknown);
        if let Err(err) = notifier.notify(&content) {
            eprintln!("{err}");
        }

        thread::sleep(config.interval);
    }
}
