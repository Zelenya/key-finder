use crate::constants::APP_NAME;
use crate::domain::models::NotificationContent;
use crate::notifications::notifier::{NativeNotifier, Notifier};

pub(crate) fn notify_runtime_error(title: &str, message: &str) {
    let notifier = NativeNotifier::new();
    let content = NotificationContent {
        title: title.to_string(),
        subtitle: Some(APP_NAME.to_string()),
        message: message.to_string(),
    };

    if let Err(err) = notifier.notify(&content) {
        eprintln!("failed to send native runtime notification: {err}");
    }
}
