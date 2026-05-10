use crate::application::notification_scheduler::Scheduler;
use crate::application::notification_scheduler::SchedulerConfig;
use crate::application::notification_types::SchedulerCommand;
use crate::application::shortcut_center::ShortcutCache;
use crate::application::shortcut_focus::ShortcutFocusSelector;
use crate::domain::errors::AppError;
use crate::notifications::notification_payload;
use crate::notifications::notifier::Notifier;
use std::sync::mpsc;
use std::sync::Arc;
use std::thread;
use std::time::Duration;
use std::time::Instant;

#[derive(Debug)]
pub(crate) enum WorkerCommand {
    Stop,
    Scheduler(SchedulerCommand),
    ShortcutFocusCount(usize),
}

pub(crate) struct NotificationService {
    worker: thread::JoinHandle<()>,
}

type CurrentAppProvider = Arc<dyn Fn() -> Option<String> + Send + Sync>;

pub(crate) fn start_notification_service(
    cooldown: Duration,
    app_switch_bounce: Duration,
    shortcut_focus_count: usize,
    shortcuts: ShortcutCache,
    notifier: Arc<dyn Notifier>,
    current_app_provider: CurrentAppProvider,
) -> (mpsc::Sender<WorkerCommand>, NotificationService) {
    let (command_tx, command_rx) = mpsc::channel::<WorkerCommand>();
    let config = SchedulerConfig {
        cooldown,
        app_switch_bounce,
    };
    let worker = spawn_notification_worker(
        config,
        shortcut_focus_count,
        command_rx,
        shortcuts,
        notifier,
        current_app_provider,
    );
    (command_tx, NotificationService { worker })
}

impl NotificationService {
    pub(crate) fn join(self) -> Result<(), AppError> {
        self.worker.join().map_err(|_| AppError::WorkerPanic)
    }
}

fn spawn_notification_worker(
    config: SchedulerConfig,
    shortcut_focus_count: usize,
    command_rx: mpsc::Receiver<WorkerCommand>,
    shortcuts: ShortcutCache,
    notifier: Arc<dyn Notifier>,
    current_app_provider: CurrentAppProvider,
) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        let mut scheduler = Scheduler::new(config);
        let mut shortcut_focus = ShortcutFocusSelector::new(shortcut_focus_count);

        loop {
            if scheduler.is_paused() {
                match command_rx.recv() {
                    Ok(WorkerCommand::Stop) => break,
                    Ok(WorkerCommand::Scheduler(cmd)) => scheduler.on_command(cmd, Instant::now()),
                    Ok(WorkerCommand::ShortcutFocusCount(count)) => shortcut_focus.update_focus_count(count),
                    Err(_) => break,
                }
            } else {
                let timeout = scheduler.deadline.saturating_duration_since(Instant::now());
                match command_rx.recv_timeout(timeout) {
                    Ok(WorkerCommand::Stop) => break,
                    Ok(WorkerCommand::Scheduler(cmd)) => scheduler.on_command(cmd, Instant::now()),
                    Ok(WorkerCommand::ShortcutFocusCount(count)) => shortcut_focus.update_focus_count(count),
                    Err(mpsc::RecvTimeoutError::Disconnected) => break,
                    Err(mpsc::RecvTimeoutError::Timeout) => {
                        let now = Instant::now();
                        let frontmost_app = current_app_provider();
                        if let Some(app) = scheduler.on_wake(frontmost_app, now) {
                            let current_shortcuts = shortcuts.snapshot();
                            let content = notification_payload(&current_shortcuts, app, &mut shortcut_focus);
                            if let Err(err) = notifier.notify(&content) {
                                eprintln!("{err}");
                            }
                        }
                    }
                }
            }
        }
    })
}
