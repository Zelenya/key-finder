use crate::storage::{ManagedShortcut, ShortcutId};

#[derive(Clone, Debug, Default)]
/// Some actions can apply to multiple selected rows.
/// However, the detailed panel and specific actions only make sense for one focused shortcut.
/// We can carry ids and map them to full shortcuts when needed.
pub(super) struct ShortcutCenterSelectionState {
    pub selected_ids: Vec<ShortcutId>,
    pub focused_id: Option<ShortcutId>,
}

impl ShortcutCenterSelectionState {
    /// Returns the selected shortcuts from the given list of shortcuts
    pub(super) fn selected_shortcuts<'a>(
        &self,
        shortcuts: &'a [ManagedShortcut],
    ) -> Vec<&'a ManagedShortcut> {
        shortcuts.iter().filter(|shortcut| self.selected_ids.contains(&shortcut.id)).collect()
    }

    /// Returns the focused shortcut from the given list of shortcuts
    pub(super) fn focused_shortcut<'a>(
        &self,
        shortcuts: &'a [ManagedShortcut],
    ) -> Option<&'a ManagedShortcut> {
        self.focused_id.and_then(|id| shortcuts.iter().find(|shortcut| shortcut.id == id)).or_else(|| {
            self.selected_ids.last().and_then(|id| shortcuts.iter().find(|shortcut| shortcut.id == *id))
        })
    }
}

/// Preserves (or discards) the selection state by filtering out any shortcuts that are no longer available.
pub(super) fn preserve_selection(
    mut selection: ShortcutCenterSelectionState,
    shortcuts: &[ManagedShortcut],
) -> ShortcutCenterSelectionState {
    selection.selected_ids.retain(|id| shortcuts.iter().any(|shortcut| shortcut.id == *id));
    if selection.focused_id.is_some_and(|id| !selection.selected_ids.contains(&id)) {
        selection.focused_id = None;
    }
    if selection.focused_id.is_none() {
        selection.focused_id = selection.selected_ids.last().copied();
    }
    selection
}
