//! Notification scheduling state machine.
//!
//! Main rules:
//! - no notification more often than every X minutes
//! - no notification for interim apps (app-switch bounce of Z seconds)
//! - TODO: no notification for the same app more often than every Y minutes
//!
//! Main loop:
//! 1. Sleep until the next scheduled check.
//! 2. If user selected a specific app, use that.
//! 3. Otherwise inspect frontmost app.
//! 4. If it’s the same app as before, notify for that app, wake up in X minutes.
//! 5. If it changed, don’t notify yet; schedule a "confirmation wake" in Z seconds.
//! 6. On that "confirmation wake":
//!    - If it’s still the same app, notify for that app, and schedule next wake in X minutes.
//!    - If it changed again, treat it as unstable and keep waiting.
//! 7. If current app is unknown, skip and debug notify (or consider doing fallback to random).

use std::time::{Duration, Instant};

use crate::application::notification_types::{AppFocusState, ChosenApp, SchedulerCommand};
use crate::storage::AppId;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct SchedulerConfig {
    pub(crate) cooldown: Duration,
    pub(crate) app_switch_bounce: Duration,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RunState {
    Paused,
    Running,
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[allow(clippy::enum_variant_names)]
enum SchedulerState {
    // The user explicitly selected an app,
    // and we can show it again when the cooldown is over
    WaitingForFocusedAppCooldown(AppId),

    // We want to follow frontmost app,
    // but we haven't seen any app yet
    WaitingToInspectFrontmost,

    // We saw a new frontmost app,
    // but it needs to remain frontmost for some time before we commit to it
    WaitingToConfirmFrontmost(String),

    // We have a confirmed frontmost app,
    // and we can show it again when the cooldown is over
    WaitingForFrontmostCooldown(String),
}

pub(super) struct Scheduler {
    pub(super) deadline: Instant,
    config: SchedulerConfig,
    run_state: RunState,
    state: SchedulerState,
    last_shown_at: Option<Instant>,
    last_shown_for: ChosenApp,
}

impl Scheduler {
    pub(super) fn new(config: SchedulerConfig) -> Self {
        let now = Instant::now();
        Scheduler {
            deadline: now,
            config,
            run_state: RunState::Running,
            state: SchedulerState::WaitingToInspectFrontmost,
            last_shown_at: None,
            last_shown_for: ChosenApp::RandomShortcut,
        }
    }

    pub(super) fn is_paused(&self) -> bool {
        matches!(self.run_state, RunState::Paused)
    }

    /// On the surface, each command is just a setter,
    /// but every configuration change can reset the deadline
    pub(super) fn on_command(&mut self, command: SchedulerCommand, now: Instant) {
        match command {
            SchedulerCommand::Pause(paused) => {
                self.run_state = if paused {
                    RunState::Paused
                } else {
                    // Reset the state to avoid showing stale notifications right after resuming.
                    self.last_shown_at = None;
                    self.last_shown_for = ChosenApp::RandomShortcut;
                    self.deadline = now;
                    if !matches!(self.state, SchedulerState::WaitingForFocusedAppCooldown(_)) {
                        self.state = SchedulerState::WaitingToInspectFrontmost;
                    }
                    RunState::Running
                };
            }
            SchedulerCommand::Focus(focus_state) => {
                self.state = match focus_state {
                    AppFocusState::FollowCurrentApp => SchedulerState::WaitingToInspectFrontmost,
                    AppFocusState::FocusOn(app_id) => SchedulerState::WaitingForFocusedAppCooldown(app_id),
                };
                self.recompute_deadline(now);
            }
            SchedulerCommand::Cooldown(cooldown) => {
                self.config.cooldown = cooldown;
                self.recompute_deadline(now);
            }
            SchedulerCommand::AppSwitchBounce(app_switch_bounce) => {
                self.config.app_switch_bounce = app_switch_bounce;
                self.recompute_deadline(now);
            }
        }
    }

    /// Main state machine logic, called on wake up.
    /// If we are not ready to show a notification, returns None and schedules the next wake up.
    pub(super) fn on_wake(&mut self, frontmost_app: Option<String>, now: Instant) -> Option<ChosenApp> {
        match self.state.clone() {
            // If now >= ready_at(app_id), show a shortcut for app_id, record timestamps, and stay in this state with a new deadline.
            SchedulerState::WaitingForFocusedAppCooldown(app_id) => {
                self.record_notification(now, ChosenApp::FocusedId(app_id));
                Some(ChosenApp::FocusedId(app_id))
            }

            // We need to read frontmost app.
            // - If unknown, reschedule another inspect.
            // - If known, we need to confirm it remains frontmost for some time before selecting it.
            SchedulerState::WaitingToInspectFrontmost => {
                self.deadline = now + self.config.app_switch_bounce;

                if let Some(frontmost_app) = frontmost_app {
                    self.state = SchedulerState::WaitingToConfirmFrontmost(frontmost_app);
                } else {
                    self.state = SchedulerState::WaitingToInspectFrontmost;
                }
                None
            }

            // We need to read frontmost app (again).
            // - If it's the same as a candidate, we can select it and move to cooldown.
            // - If it's different but known, we restart with new candidate.
            // - If it's different and unknown, we go back to waiting.
            //
            // TODO: If frontmost app the same as prev stable app, we can show it too
            // (we just need to deal with name vs id properly)
            SchedulerState::WaitingToConfirmFrontmost(candidate_app) => {
                if let Some(frontmost_app) = frontmost_app {
                    if frontmost_app == candidate_app {
                        let chosen = ChosenApp::GuestimatedName(candidate_app.clone());
                        self.state = SchedulerState::WaitingForFrontmostCooldown(candidate_app);
                        self.record_notification(now, chosen.clone());
                        Some(chosen)
                    } else {
                        self.deadline = now + self.config.app_switch_bounce;
                        self.state = SchedulerState::WaitingToConfirmFrontmost(frontmost_app);
                        None
                    }
                } else {
                    self.deadline = now + self.config.app_switch_bounce;
                    self.state = SchedulerState::WaitingToInspectFrontmost;
                    None
                }
            }

            SchedulerState::WaitingForFrontmostCooldown(candidate_app) => {
                if frontmost_app.as_deref() == Some(candidate_app.as_str()) {
                    let chosen = ChosenApp::GuestimatedName(candidate_app);
                    self.record_notification(now, chosen.clone());
                    Some(chosen)
                } else {
                    self.deadline = now + self.config.app_switch_bounce;
                    self.state = frontmost_app.map_or(
                        SchedulerState::WaitingToInspectFrontmost,
                        SchedulerState::WaitingToConfirmFrontmost,
                    );
                    None
                }
            }
        }
    }

    fn record_notification(&mut self, now: Instant, chosen_app: ChosenApp) {
        self.last_shown_at = Some(now);
        self.last_shown_for = chosen_app;
        self.deadline = now + self.config.cooldown;
    }

    fn recompute_deadline(&mut self, now: Instant) {
        self.deadline = match &self.state {
            SchedulerState::WaitingForFocusedAppCooldown(_) => self.next_cooldown_deadline(now),
            SchedulerState::WaitingToInspectFrontmost => now,
            SchedulerState::WaitingToConfirmFrontmost(_) => now + self.config.app_switch_bounce,
            SchedulerState::WaitingForFrontmostCooldown(_) => self.next_cooldown_deadline(now),
        };
    }

    // When recomputing a cooldown, we use the previous notification as a reference.
    // We don't want to "lose" already passed time if the cooldown was extended by a command,
    // but we also don't want to set a deadline in the past
    fn next_cooldown_deadline(&self, now: Instant) -> Instant {
        self.last_shown_at.map_or(now, |last_shown_at| {
            (last_shown_at + self.config.cooldown).max(now)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{Scheduler, SchedulerConfig, SchedulerState};
    use crate::application::notification_types::{AppFocusState, ChosenApp, SchedulerCommand};
    use std::time::{Duration, Instant};

    const DEFAULT_CONFIG: SchedulerConfig = SchedulerConfig {
        cooldown: Duration::from_secs(600),
        app_switch_bounce: Duration::from_secs(30),
    };

    fn scheduler(now: Instant) -> Scheduler {
        Scheduler {
            deadline: now,
            config: DEFAULT_CONFIG,
            run_state: super::RunState::Running,
            state: SchedulerState::WaitingToInspectFrontmost,
            last_shown_at: None,
            last_shown_for: ChosenApp::RandomShortcut,
        }
    }

    #[test]
    fn initial_inspect_returns_no_notification() {
        let now = Instant::now();
        let mut scheduler = scheduler(now);

        assert_eq!(scheduler.on_wake(Some("Zed".to_string()), now), None);
        assert_eq!(
            scheduler.state,
            SchedulerState::WaitingToConfirmFrontmost("Zed".to_string())
        );
        assert_eq!(scheduler.deadline, now + DEFAULT_CONFIG.app_switch_bounce);
    }

    #[test]
    fn bounced_switch_returns_no_notification() {
        let now = Instant::now();
        let mut scheduler = scheduler(now);
        scheduler.on_wake(Some("Zed".to_string()), now);

        let switched = now + DEFAULT_CONFIG.app_switch_bounce;
        assert_eq!(scheduler.on_wake(Some("Code".to_string()), switched), None);
        assert_eq!(
            scheduler.state,
            SchedulerState::WaitingToConfirmFrontmost("Code".to_string())
        );
        assert_eq!(scheduler.deadline, switched + DEFAULT_CONFIG.app_switch_bounce);
    }

    #[test]
    fn confirmation_works_and_takes_wake_time_for_cooldown() {
        let now = Instant::now();
        let mut scheduler = scheduler(now);
        scheduler.on_wake(Some("Zed".to_string()), now);

        let confirmed_at = now + DEFAULT_CONFIG.app_switch_bounce;
        let chosen = scheduler.on_wake(Some("Zed".to_string()), confirmed_at);

        assert_eq!(chosen, Some(ChosenApp::GuestimatedName("Zed".to_string())));
        assert_eq!(scheduler.deadline, confirmed_at + DEFAULT_CONFIG.cooldown);
        assert_eq!(
            scheduler.state,
            SchedulerState::WaitingForFrontmostCooldown("Zed".to_string())
        );
    }

    #[test]
    fn set_focus_to_current_app_reschedules_it() {
        let now = Instant::now();
        let mut scheduler = scheduler(now);
        scheduler.deadline = now + Duration::from_secs(500);

        scheduler.on_command(SchedulerCommand::Focus(AppFocusState::FollowCurrentApp), now);

        assert_eq!(scheduler.state, SchedulerState::WaitingToInspectFrontmost);
        assert_eq!(scheduler.deadline, now);
    }

    #[test]
    fn set_focus_to_specific_app_keeps_existing_cooldown() {
        let now = Instant::now();
        let mut scheduler = scheduler(now);
        scheduler.last_shown_at = Some(now - Duration::from_secs(60));

        let app_id = 7.into();
        scheduler.on_command(SchedulerCommand::Focus(AppFocusState::FocusOn(app_id)), now);

        assert_eq!(
            scheduler.state,
            SchedulerState::WaitingForFocusedAppCooldown(app_id)
        );
        assert_eq!(scheduler.deadline, now + Duration::from_secs(540));
    }

    #[test]
    fn set_cooldown_recomputes_deadline() {
        let now = Instant::now();
        let mut scheduler = scheduler(now);
        scheduler.state = SchedulerState::WaitingForFrontmostCooldown("Zed".to_string());
        scheduler.last_shown_at = Some(now - DEFAULT_CONFIG.cooldown);
        scheduler.last_shown_for = ChosenApp::GuestimatedName("Zed".to_string());

        let double = DEFAULT_CONFIG.cooldown * 2;
        scheduler.on_command(SchedulerCommand::Cooldown(double), now);

        assert_eq!(scheduler.deadline, now + DEFAULT_CONFIG.cooldown);
    }

    #[test]
    fn set_app_switch_bounce_recomputes_deadline() {
        let now = Instant::now();
        let mut scheduler = scheduler(now);
        scheduler.state = SchedulerState::WaitingToConfirmFrontmost("Zed".to_string());

        let double = DEFAULT_CONFIG.app_switch_bounce * 2;
        scheduler.on_command(SchedulerCommand::AppSwitchBounce(double), now);

        assert_eq!(scheduler.deadline, now + double);
    }

    #[test]
    fn resume_restarts_frontmost_tracking() {
        let now = Instant::now();
        let mut scheduler = scheduler(now);
        scheduler.state = SchedulerState::WaitingForFrontmostCooldown("Zed".to_string());
        scheduler.on_command(SchedulerCommand::Pause(true), now);

        let resumed_at = now + Duration::from_secs(5);
        scheduler.on_command(SchedulerCommand::Pause(false), resumed_at);

        assert_eq!(scheduler.state, SchedulerState::WaitingToInspectFrontmost);
        assert_eq!(scheduler.deadline, resumed_at);
    }

    #[test]
    fn resume_restarts_focused_app_tracking() {
        let now = Instant::now();
        let mut scheduler = scheduler(now);
        let app_id = 7.into();
        scheduler.state = SchedulerState::WaitingForFocusedAppCooldown(app_id);
        scheduler.last_shown_at = Some(now - DEFAULT_CONFIG.cooldown);
        scheduler.last_shown_for = ChosenApp::FocusedId(app_id);
        scheduler.on_command(SchedulerCommand::Pause(true), now);

        let resumed_at = now + Duration::from_secs(5);
        scheduler.on_command(SchedulerCommand::Pause(false), resumed_at);

        assert_eq!(
            scheduler.state,
            SchedulerState::WaitingForFocusedAppCooldown(app_id)
        );
        assert_eq!(scheduler.deadline, resumed_at);
    }
}
