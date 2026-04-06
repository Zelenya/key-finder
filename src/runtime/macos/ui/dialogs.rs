use objc2_app_kit::{NSApplication, NSControl, NSModalResponse, NSWindow};

pub(crate) fn control_tag_from_current_event(app: &NSApplication, window: &NSWindow) -> Option<i64> {
    let event = app.currentEvent()?;
    let content = window.contentView()?;
    let hit = content.hitTest(event.locationInWindow())?;
    if let Ok(control) = hit.downcast::<NSControl>() {
        return Some(control.tag() as i64);
    }
    None
}

// We use it as fallback when we can't identify the control (get the tag) from the current event
pub(crate) fn control_tag_from_focus(window: &NSWindow) -> Option<i64> {
    let responder = window.firstResponder()?;
    if let Ok(control) = responder.downcast::<NSControl>() {
        return Some(control.tag() as i64);
    }
    None
}

pub(crate) fn close_sheet(parent_window: &NSWindow, sheet_window: &NSWindow, return_code: NSModalResponse) {
    parent_window.endSheet_returnCode(sheet_window, return_code);
    sheet_window.orderOut(None);
}
