use objc2_app_kit::NSWorkspace;

/// Asks macOS for the name of the frontmost application, so we can prioritize relevant shortcuts.
pub(crate) fn frontmost_app_name() -> Option<String> {
    NSWorkspace::sharedWorkspace().frontmostApplication()?.localizedName().and_then(|name| {
        let s = name.to_string();
        let trimmed = s.trim();
        (!trimmed.is_empty()).then(|| trimmed.to_owned())
    })
}
