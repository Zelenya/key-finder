use std::time::Duration;

use crate::storage::AppId;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum AppFocusState {
    FollowCurrentApp,
    FocusOn(AppId),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ChosenApp {
    FocusedId(AppId),
    // TODO: Maybe we can resolve this to an id at an earlier stage
    GuestimatedName(String),
    RandomShortcut,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SchedulerCommand {
    Pause(bool),
    Focus(AppFocusState),
    Cooldown(Duration),
    AppSwitchBounce(Duration),
}
