use crate::domain::errors::AppError;
use objc2::runtime::Sel;
use objc2::{rc::Retained, MainThreadMarker};
use objc2_app_kit::{NSAlert, NSApplication, NSButton, NSPopUpButton, NSView};
use objc2_foundation::{NSArray, NSRect, NSString};

pub(crate) fn populate_popup(popup: &NSPopUpButton, items: &[String], selected_index: usize) {
    popup.removeAllItems();
    let ns_items = items.iter().map(|item| NSString::from_str(item)).collect::<Vec<_>>();
    let array = NSArray::from_retained_slice(&ns_items);
    popup.addItemsWithTitles(&array);
    if !items.is_empty() {
        popup.selectItemAtIndex(selected_index.min(items.len() - 1) as _);
    }
}

pub(crate) fn add_modal_action_button(
    content: &NSView,
    mtm: MainThreadMarker,
    app: &NSApplication,
    title: &str,
    frame: NSRect,
    action: Sel,
) -> Retained<NSButton> {
    // SAFETY: `app` is the shared `NSApplication` on the main thread, and the
    // provided selector is one of AppKit's standard modal-ending actions.
    let button = unsafe {
        NSButton::buttonWithTitle_target_action(&NSString::from_str(title), Some(app), Some(action), mtm)
    };
    button.setFrame(frame);
    content.addSubview(&button);
    button
}

pub(crate) fn show_modal_error(app: &NSApplication, title: &str, message: &str) -> Result<(), AppError> {
    let Some(mtm) = MainThreadMarker::new() else {
        return Err(AppError::MainThreadRequired);
    };
    app.activate();
    let alert = NSAlert::new(mtm);
    alert.setMessageText(&NSString::from_str(title));
    alert.setInformativeText(&NSString::from_str(message));
    alert.addButtonWithTitle(&NSString::from_str("OK"));
    let _ = alert.runModal();
    Ok(())
}
