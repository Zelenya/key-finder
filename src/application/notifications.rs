use crate::application::shortcut_center::ShortcutCache;
use crate::domain::errors::AppError;
use crate::notifications::notification_payload;
use crate::notifications::notifier::Notifier;
use std::sync::mpsc;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

#[derive(Debug)]
pub(crate) enum WorkerCommand {
    Stop,
    SetPaused(bool),
    SetInterval(Duration),
}

pub(crate) struct NotificationService {
    worker: thread::JoinHandle<()>,
}

type CurrentAppProvider = Arc<dyn Fn() -> Option<String> + Send + Sync>;

pub(crate) fn start_notification_service(
    interval: Duration,
    shortcuts: ShortcutCache,
    notifier: Arc<dyn Notifier>,
    current_app_provider: CurrentAppProvider,
) -> (mpsc::Sender<WorkerCommand>, NotificationService) {
    let (command_tx, command_rx) = mpsc::channel::<WorkerCommand>();
    let worker = spawn_notification_worker(interval, command_rx, shortcuts, notifier, current_app_provider);
    (command_tx, NotificationService { worker })
}

impl NotificationService {
    pub(crate) fn join(self) -> Result<(), AppError> {
        self.worker.join().map_err(|_| AppError::WorkerPanic)
    }
}

fn spawn_notification_worker(
    interval: Duration,
    command_rx: mpsc::Receiver<WorkerCommand>,
    shortcuts: ShortcutCache,
    notifier: Arc<dyn Notifier>,
    current_app_provider: CurrentAppProvider,
) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        let mut interval = interval;
        let mut paused = false;

        loop {
            if !paused {
                let current_shortcuts = shortcuts.snapshot();
                let current_app = current_app_provider();
                let content = notification_payload(&current_shortcuts, current_app.as_deref());
                if let Err(err) = notifier.notify(&content) {
                    eprintln!("{err}");
                }
            }

            match command_rx.recv_timeout(interval) {
                Ok(WorkerCommand::Stop) => break,
                Ok(WorkerCommand::SetPaused(value)) => paused = value,
                Ok(WorkerCommand::SetInterval(value)) => interval = value,
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
            }
        }
    })
}
