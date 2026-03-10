use super::shared::modifier_rank;

// TODO: Make the rendered shortcut style configurable instead of always using this one form.
pub(crate) fn render_canonical_shortcut(shortcut_norm: &str) -> String {
    shortcut_norm.split(',').filter_map(render_canonical_chord).collect::<Vec<_>>().join(", ")
}

fn render_canonical_chord(chord: &str) -> Option<String> {
    let mut modifiers = Vec::new();
    let mut keys = Vec::new();

    for token in chord.split('+').filter(|token| !token.trim().is_empty()) {
        match token {
            "cmd" | "ctrl" | "alt" | "shift" => modifiers.push(token),
            "enter" | "escape" | "space" => keys.push(render_special_key(token)),
            other => keys.push(other.to_ascii_uppercase()),
        }
    }

    modifiers.sort_by_key(|modifier| modifier_rank(modifier));

    let mut parts = modifiers.into_iter().map(render_modifier).map(str::to_string).collect::<Vec<_>>();

    parts.extend(keys);

    Some(parts.join(" ")).filter(|s| !s.is_empty())
}

fn render_modifier(modifier: &str) -> &'static str {
    match modifier {
        "cmd" => "⌘",
        "ctrl" => "⌃",
        "alt" => "⌥",
        "shift" => "⇧",
        _ => "",
    }
}

fn render_special_key(key: &str) -> String {
    match key {
        "enter" => "↩".to_string(),
        "escape" => "⎋".to_string(),
        "space" => "Space".to_string(),
        _ => key.to_ascii_uppercase(),
    }
}

#[cfg(test)]
mod tests {
    use super::render_canonical_shortcut;
    use crate::domain::shortcut_norm::parse::canonical_shortcut_from_chords;

    #[test]
    fn renders_canonical_shortcuts() {
        assert_eq!(render_canonical_shortcut("cmd+shift+b"), "⌘ ⇧ B");
        assert_eq!(render_canonical_shortcut("cmd+k,cmd+r"), "⌘ K, ⌘ R");
        assert_eq!(render_canonical_shortcut("enter"), "↩");
        assert_eq!(render_canonical_shortcut("unassigned1"), "UNASSIGNED1");
    }

    #[test]
    fn canonical_shortcuts_render_through_shared_renderer() {
        let chords = vec![
            vec!["cmd".to_string(), "k".to_string()],
            vec!["cmd".to_string(), "s".to_string()],
        ];

        let rendered = render_canonical_shortcut(&canonical_shortcut_from_chords(&chords));
        assert_eq!(rendered, "⌘ K, ⌘ S");
    }
}
