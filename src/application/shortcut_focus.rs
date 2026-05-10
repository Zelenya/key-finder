use crate::application::notification_types::ChosenApp;
use crate::storage::{AppId, NotificationShortcut, NotificationSnapshot, ShortcutId};
use chrono::{Local, NaiveDate};
use rand::prelude::*;
use std::collections::HashMap;

#[derive(Clone, Debug)]
struct DailyFocusSet {
    day: NaiveDate,
    shortcut_ids: Vec<ShortcutId>,
}

trait ShortcutFocusAlgorithm {
    fn choose_app(&mut self, app_ids: &[AppId]) -> Option<AppId>;

    fn choose_focus_set(&mut self, candidates: &[&NotificationShortcut], count: usize) -> Vec<ShortcutId>;

    fn choose_shortcut<'a>(
        &mut self,
        shortcuts: &[&'a NotificationShortcut],
    ) -> Option<&'a NotificationShortcut>;
}

#[derive(Default)]
struct RandomShortcutFocusAlgorithm;

impl ShortcutFocusAlgorithm for RandomShortcutFocusAlgorithm {
    fn choose_app(&mut self, app_ids: &[AppId]) -> Option<AppId> {
        let mut rng = rand::rng();
        app_ids.iter().choose(&mut rng).copied()
    }

    fn choose_focus_set(&mut self, candidates: &[&NotificationShortcut], count: usize) -> Vec<ShortcutId> {
        let mut rng = rand::rng();
        let mut shortcut_ids = candidates.iter().map(|shortcut| shortcut.id).collect::<Vec<_>>();
        shortcut_ids.shuffle(&mut rng);
        shortcut_ids.truncate(count);
        shortcut_ids
    }

    fn choose_shortcut<'a>(
        &mut self,
        shortcuts: &[&'a NotificationShortcut],
    ) -> Option<&'a NotificationShortcut> {
        let mut rng = rand::rng();
        shortcuts.iter().choose(&mut rng).copied()
    }
}

pub(crate) struct ShortcutFocusSelector {
    focus_count: usize,
    algorithm: Box<dyn ShortcutFocusAlgorithm + Send>,
    // The selector stores only shortcut ids, not shortcuts.
    // Daily state should survive snapshot reloads.
    focus_sets: HashMap<AppId, DailyFocusSet>,
}

impl ShortcutFocusSelector {
    pub(crate) fn new(focus_count: usize) -> Self {
        Self {
            focus_count,
            algorithm: Box::<RandomShortcutFocusAlgorithm>::default(),
            focus_sets: HashMap::new(),
        }
    }

    #[cfg(test)]
    fn with_algorithm(focus_count: usize, algorithm: Box<dyn ShortcutFocusAlgorithm + Send>) -> Self {
        Self {
            focus_count,
            algorithm,
            focus_sets: HashMap::new(),
        }
    }

    // Gated update, so unrelated runtime settings changes do not invalidate focus sets
    pub(crate) fn update_focus_count(&mut self, focus_count: usize) {
        if self.focus_count != focus_count {
            self.focus_count = focus_count;
            self.focus_sets.clear();
        }
    }

    // Picks a shortcut from that app’s focused set.
    pub(crate) fn select_shortcut<'a>(
        &mut self,
        snapshot: &'a NotificationSnapshot,
        current_app: &ChosenApp,
    ) -> Option<&'a NotificationShortcut> {
        self.select_shortcut_for_day(snapshot, current_app, Local::now().date_naive())
    }

    fn select_shortcut_for_day<'a>(
        &mut self,
        snapshot: &'a NotificationSnapshot,
        current_app: &ChosenApp,
        day: NaiveDate,
    ) -> Option<&'a NotificationShortcut> {
        let app_id = self.resolve_app_id(snapshot, current_app)?;
        let focused_shortcuts = self
            .focus_ids_for_app(snapshot, app_id, day)
            .iter()
            .filter_map(|shortcut_id| snapshot.shortcut_by_id(*shortcut_id))
            .collect::<Vec<_>>();
        self.algorithm.choose_shortcut(&focused_shortcuts)
    }

    // TODO: Is this the right place for this? How to untie this?
    fn resolve_app_id(&mut self, snapshot: &NotificationSnapshot, current_app: &ChosenApp) -> Option<AppId> {
        match current_app {
            ChosenApp::FocusedId(app_id) => Some(*app_id),
            ChosenApp::GuestimatedName(name) => snapshot.resolve_guessed_app(name),
            ChosenApp::RandomShortcut => {
                let app_ids = snapshot.app_ids_with_shortcuts();
                self.algorithm.choose_app(&app_ids)
            }
        }
    }

    // Either reuses and reconcile today’s set or generates a new daily set.
    // - If shortcuts disappear or become inactive, removes them and refill from active candidates.
    // - When the day changes, generates a new set and try to avoid yesterday’s ids (when there are enough alternatives).
    fn focus_ids_for_app(
        &mut self,
        snapshot: &NotificationSnapshot,
        app_id: AppId,
        day: NaiveDate,
    ) -> Vec<ShortcutId> {
        let needs_new_set = self.focus_sets.get(&app_id).is_none_or(|focus_set| focus_set.day != day);

        if needs_new_set {
            let previous_ids = self
                .focus_sets
                .get(&app_id)
                .map_or_else(Vec::new, |focus_set| focus_set.shortcut_ids.clone());
            let shortcut_ids = self.generate_focus_set(snapshot, app_id, &previous_ids);
            self.focus_sets.insert(app_id, DailyFocusSet { day, shortcut_ids });
        } else {
            self.reconcile_focus_set(snapshot, app_id);
        }

        self.focus_sets.get(&app_id).map_or_else(Vec::new, |focus_set| focus_set.shortcut_ids.clone())
    }

    // Prefer fresh shortcuts, but if there aren’t enough, reuse old ones to reach the configured focus count.
    fn generate_focus_set(
        &mut self,
        snapshot: &NotificationSnapshot,
        app_id: AppId,
        previous_ids: &[ShortcutId],
    ) -> Vec<ShortcutId> {
        let candidates = snapshot.shortcuts_for_app(app_id).collect::<Vec<_>>();
        if candidates.len() <= self.focus_count {
            return candidates.iter().map(|shortcut| shortcut.id).collect();
        }

        let fresh_candidates = candidates
            .iter()
            .copied()
            .filter(|shortcut| !previous_ids.contains(&shortcut.id))
            .collect::<Vec<_>>();

        if fresh_candidates.len() < self.focus_count {
            let mut shortcut_ids = self.algorithm.choose_focus_set(&fresh_candidates, fresh_candidates.len());
            // Not enough fresh candidates, top up with previous ones.
            self.fill_focus_set(&candidates, &mut shortcut_ids);
            shortcut_ids
        } else {
            self.algorithm.choose_focus_set(&fresh_candidates, self.focus_count)
        }
    }

    // When shortcuts disappear or become inactive, removes them and refill from active candidates
    fn reconcile_focus_set(&mut self, snapshot: &NotificationSnapshot, app_id: AppId) {
        let active_ids = snapshot.shortcuts_for_app(app_id).map(|shortcut| shortcut.id).collect::<Vec<_>>();

        let Some(mut focus_set) = self.focus_sets.remove(&app_id) else {
            return;
        };

        let mut shortcut_ids = focus_set
            .shortcut_ids
            .iter()
            .copied()
            .filter(|shortcut_id| active_ids.contains(shortcut_id))
            .take(self.focus_count)
            .collect::<Vec<_>>();

        if shortcut_ids.len() < self.focus_count {
            let candidates = snapshot.shortcuts_for_app(app_id).collect::<Vec<_>>();
            self.fill_focus_set(&candidates, &mut shortcut_ids);
        }

        focus_set.shortcut_ids = shortcut_ids;
        self.focus_sets.insert(app_id, focus_set);
    }

    fn fill_focus_set(&mut self, candidates: &[&NotificationShortcut], shortcut_ids: &mut Vec<ShortcutId>) {
        let remaining = self.focus_count.saturating_sub(shortcut_ids.len());
        if remaining == 0 {
            return;
        }

        let available = candidates
            .iter()
            .copied()
            .filter(|shortcut| !shortcut_ids.contains(&shortcut.id))
            .collect::<Vec<_>>();
        shortcut_ids.extend(self.algorithm.choose_focus_set(&available, remaining));
    }

    #[cfg(test)]
    fn focused_shortcut_ids_for_app(&self, app_id: AppId) -> Option<&[ShortcutId]> {
        self.focus_sets.get(&app_id).map(|focus_set| focus_set.shortcut_ids.as_slice())
    }
}

#[cfg(test)]
mod tests {
    use super::{NaiveDate, ShortcutFocusAlgorithm, ShortcutFocusSelector};
    use crate::application::notification_types::ChosenApp;
    use crate::storage::{AppId, NotificationApp, NotificationShortcut, NotificationSnapshot, ShortcutId};

    #[derive(Default)]
    struct FirstAlgorithm;

    impl ShortcutFocusAlgorithm for FirstAlgorithm {
        fn choose_app(&mut self, app_ids: &[AppId]) -> Option<AppId> {
            app_ids.first().copied()
        }

        fn choose_focus_set(
            &mut self,
            candidates: &[&NotificationShortcut],
            count: usize,
        ) -> Vec<ShortcutId> {
            candidates.iter().take(count).map(|shortcut| shortcut.id).collect()
        }

        fn choose_shortcut<'a>(
            &mut self,
            shortcuts: &[&'a NotificationShortcut],
        ) -> Option<&'a NotificationShortcut> {
            shortcuts.first().copied()
        }
    }

    fn selector(focus_count: usize) -> ShortcutFocusSelector {
        ShortcutFocusSelector::with_algorithm(focus_count, Box::<FirstAlgorithm>::default())
    }

    fn day(year: i32, month: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(year, month, day).expect("valid test date")
    }

    fn snapshot(apps: Vec<NotificationApp>, shortcuts: Vec<NotificationShortcut>) -> NotificationSnapshot {
        NotificationSnapshot { apps, shortcuts }
    }

    fn app(app_id: i64, name: &str) -> NotificationApp {
        NotificationApp {
            app_id: app_id.into(),
            name: name.to_string(),
            aliases: vec![name.to_string()],
        }
    }

    fn shortcut(id: i64, app_id: i64) -> NotificationShortcut {
        NotificationShortcut {
            id: id.into(),
            app_id: app_id.into(),
            shortcut: format!("Shortcut {id}"),
            description: format!("Description {id}"),
        }
    }

    #[test]
    fn same_day_reuses_the_existing_app_focus_set() {
        let mut selector = selector(3);
        let day = day(2026, 4, 10);
        let first_snapshot = snapshot(
            vec![app(1, "Zed")],
            vec![shortcut(1, 1), shortcut(2, 1), shortcut(3, 1), shortcut(4, 1)],
        );
        selector.select_shortcut_for_day(&first_snapshot, &ChosenApp::FocusedId(1.into()), day);

        let reordered_snapshot = snapshot(
            vec![app(1, "Zed")],
            vec![shortcut(4, 1), shortcut(3, 1), shortcut(2, 1), shortcut(1, 1)],
        );
        selector.select_shortcut_for_day(&reordered_snapshot, &ChosenApp::FocusedId(1.into()), day);

        assert_eq!(
            selector.focused_shortcut_ids_for_app(1.into()),
            Some([1.into(), 2.into(), 3.into()].as_slice())
        );
    }

    #[test]
    fn different_apps_keep_independent_focus_sets() {
        let mut selector = selector(2);
        let day = day(2026, 4, 10);
        let snapshot = snapshot(
            vec![app(1, "Zed"), app(2, "Code")],
            vec![
                shortcut(1, 1),
                shortcut(2, 1),
                shortcut(3, 1),
                shortcut(10, 2),
                shortcut(11, 2),
                shortcut(12, 2),
            ],
        );

        selector.select_shortcut_for_day(&snapshot, &ChosenApp::FocusedId(1.into()), day);
        selector.select_shortcut_for_day(&snapshot, &ChosenApp::FocusedId(2.into()), day);

        assert_eq!(
            selector.focused_shortcut_ids_for_app(1.into()),
            Some([1.into(), 2.into()].as_slice())
        );
        assert_eq!(
            selector.focused_shortcut_ids_for_app(2.into()),
            Some([10.into(), 11.into()].as_slice())
        );
    }

    #[test]
    fn next_day_generates_a_new_focus_set_when_enough_alternatives_exist() {
        let mut selector = selector(2);
        let snapshot = snapshot(
            vec![app(1, "Zed")],
            vec![shortcut(1, 1), shortcut(2, 1), shortcut(3, 1), shortcut(4, 1)],
        );

        selector.select_shortcut_for_day(&snapshot, &ChosenApp::FocusedId(1.into()), day(2026, 4, 10));
        selector.select_shortcut_for_day(&snapshot, &ChosenApp::FocusedId(1.into()), day(2026, 4, 11));

        assert_eq!(
            selector.focused_shortcut_ids_for_app(1.into()),
            Some([3.into(), 4.into()].as_slice())
        );
    }

    #[test]
    fn config_change_resets_focus_sets() {
        let mut selector = selector(2);
        let day = day(2026, 4, 10);
        let snapshot = snapshot(
            vec![app(1, "Zed")],
            vec![shortcut(1, 1), shortcut(2, 1), shortcut(3, 1)],
        );

        selector.select_shortcut_for_day(&snapshot, &ChosenApp::FocusedId(1.into()), day);
        selector.update_focus_count(3);
        selector.select_shortcut_for_day(&snapshot, &ChosenApp::FocusedId(1.into()), day);

        assert_eq!(
            selector.focused_shortcut_ids_for_app(1.into()),
            Some([1.into(), 2.into(), 3.into()].as_slice())
        );
    }

    #[test]
    fn snapshot_changes_remove_missing_shortcuts_and_top_up() {
        let mut selector = selector(2);
        let day = day(2026, 4, 10);
        let first_snapshot = snapshot(
            vec![app(1, "Zed")],
            vec![shortcut(1, 1), shortcut(2, 1), shortcut(3, 1)],
        );
        selector.select_shortcut_for_day(&first_snapshot, &ChosenApp::FocusedId(1.into()), day);

        let changed_snapshot = snapshot(
            vec![app(1, "Zed")],
            vec![shortcut(2, 1), shortcut(3, 1), shortcut(4, 1)],
        );
        selector.select_shortcut_for_day(&changed_snapshot, &ChosenApp::FocusedId(1.into()), day);

        assert_eq!(
            selector.focused_shortcut_ids_for_app(1.into()),
            Some([2.into(), 3.into()].as_slice())
        );
    }

    #[test]
    fn random_shortcut_chooses_an_app_then_uses_that_apps_focus_set() {
        let mut selector = selector(2);
        let day = day(2026, 4, 10);
        let snapshot = snapshot(
            vec![app(1, "Zed"), app(2, "Code")],
            vec![shortcut(10, 2), shortcut(1, 1), shortcut(2, 1), shortcut(3, 1)],
        );

        let selected = selector
            .select_shortcut_for_day(&snapshot, &ChosenApp::RandomShortcut, day)
            .expect("selected shortcut");

        assert_eq!(selected.app_id, 2.into());
        assert_eq!(
            selector.focused_shortcut_ids_for_app(2.into()),
            Some([10.into()].as_slice())
        );
    }
}
