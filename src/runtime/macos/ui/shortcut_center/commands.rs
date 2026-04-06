#[derive(Clone, Copy, Debug)]
pub(super) enum ActionCommand {
    AddShortcut,
    SaveDescription,
    ToggleVisibilitySelected,
    DeleteSelected,
}

#[derive(Clone, Copy, Debug)]
pub(super) enum UiIntent {
    None,
    WindowClosed,
    AppChanged,
    FilterChanged,
    ImportSelectedApp,
    NewAppCreated,
    TableSelectionChanged,
    Command(ActionCommand),
}

pub(super) const TAG_NONE: i64 = 0;
pub(super) const TAG_APP_CHANGED: i64 = 10;
pub(super) const TAG_FILTER_CHANGED: i64 = 11;
pub(super) const TAG_WINDOW_CLOSED: i64 = 12;
pub(super) const TAG_NEW_APP_CREATED: i64 = 13;
pub(super) const TAG_TABLE_SELECTION_CHANGED: i64 = 14;

pub(super) const TAG_IMPORT: i64 = 101;
pub(super) const TAG_ADD: i64 = 200;
pub(super) const TAG_SAVE_DESCRIPTION: i64 = 201;
pub(super) const TAG_TOGGLE_VISIBILITY_SELECTED: i64 = 202;
pub(super) const TAG_DELETE_SELECTED: i64 = 204;

pub(super) fn decode_intent(tag: Option<i64>) -> UiIntent {
    match tag.unwrap_or(TAG_NONE) {
        TAG_WINDOW_CLOSED => UiIntent::WindowClosed,
        TAG_APP_CHANGED => UiIntent::AppChanged,
        TAG_FILTER_CHANGED => UiIntent::FilterChanged,
        TAG_IMPORT => UiIntent::ImportSelectedApp,
        TAG_NEW_APP_CREATED => UiIntent::NewAppCreated,
        TAG_TABLE_SELECTION_CHANGED => UiIntent::TableSelectionChanged,
        TAG_ADD => UiIntent::Command(ActionCommand::AddShortcut),
        TAG_SAVE_DESCRIPTION => UiIntent::Command(ActionCommand::SaveDescription),
        TAG_TOGGLE_VISIBILITY_SELECTED => UiIntent::Command(ActionCommand::ToggleVisibilitySelected),
        TAG_DELETE_SELECTED => UiIntent::Command(ActionCommand::DeleteSelected),
        _ => UiIntent::None,
    }
}
