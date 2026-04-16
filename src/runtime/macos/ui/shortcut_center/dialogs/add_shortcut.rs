use crate::domain::errors::AppError;
use crate::runtime::macos::ui::dialogs::close_sheet;
use crate::runtime::macos::ui::modal::{add_modal_action_button, show_modal_error};
use objc2::rc::Retained;
use objc2::sel;
use objc2::MainThreadMarker;
use objc2::MainThreadOnly;
use objc2_app_kit::{
    NSApplication, NSBackingStoreType, NSTextField, NSView, NSWindow, NSWindowButton, NSWindowStyleMask,
};
use objc2_foundation::{NSPoint, NSRect, NSSize, NSString};

struct NewShortcutDialogUi {
    window: Retained<NSWindow>,
    shortcut_field: Retained<NSTextField>,
    description_field: Retained<NSTextField>,
}

pub(crate) fn prompt_new_shortcut(
    app_name: &str,
    parent_window: &NSWindow,
) -> Result<Option<(String, String)>, AppError> {
    let Some(mtm) = MainThreadMarker::new() else {
        return Err(AppError::MainThreadRequired);
    };

    let app = NSApplication::sharedApplication(mtm);
    app.activate();
    let ui = build_new_shortcut_window(mtm, &app, app_name)?;
    ui.window.makeFirstResponder(Some(&ui.shortcut_field));
    parent_window.beginSheet_completionHandler(&ui.window, None);

    loop {
        ui.window.makeKeyAndOrderFront(None);
        let response = app.runModalForWindow(&ui.window);
        if !ui.window.isVisible() {
            close_sheet(parent_window, &ui.window, objc2_app_kit::NSModalResponseCancel);
            return Ok(None);
        }
        if response == objc2_app_kit::NSModalResponseStop {
            let shortcut = ui.shortcut_field.stringValue().to_string();
            let description = ui.description_field.stringValue().to_string();
            if shortcut.trim().is_empty() || description.trim().is_empty() {
                show_modal_error(
                    &app,
                    "Both fields are required",
                    "Enter shortcut keys and a description before saving.",
                )?;
                continue;
            }

            close_sheet(parent_window, &ui.window, objc2_app_kit::NSModalResponseOK);
            return Ok(Some((shortcut, description)));
        }

        close_sheet(parent_window, &ui.window, objc2_app_kit::NSModalResponseCancel);
        return Ok(None);
    }
}

fn build_new_shortcut_window(
    mtm: MainThreadMarker,
    app: &NSApplication,
    app_name: &str,
) -> Result<NewShortcutDialogUi, AppError> {
    let style = NSWindowStyleMask::Closable | NSWindowStyleMask::Titled;
    let rect = NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(700.0, 220.0));
    // SAFETY: `mtm` proves we are on the AppKit main thread, and we initialize
    // a fresh `NSWindow` with valid geometry and style values.
    let window = unsafe {
        NSWindow::initWithContentRect_styleMask_backing_defer(
            NSWindow::alloc(mtm),
            rect,
            style,
            NSBackingStoreType::Buffered,
            false,
        )
    };
    window.setTitle(&NSString::from_str(&format!("New Shortcut for {app_name}")));
    window.center();
    if let Some(close_button) = window.standardWindowButton(NSWindowButton::CloseButton) {
        // SAFETY: the close button belongs to this live dialog, `app` is the
        // shared `NSApplication`, and `abortModal` cleanly cancels the dialog.
        unsafe {
            close_button.setTarget(Some(app));
            close_button.setAction(Some(sel!(abortModal)));
        }
    }

    let content = window
        .contentView()
        .ok_or_else(|| AppError::UiOperation("missing new shortcut dialog content view".to_string()))?;

    let intro = NSTextField::labelWithString(&NSString::from_str(new_shortcut_intro_text()), mtm);
    intro.setFrame(NSRect::new(NSPoint::new(20.0, 182.0), NSSize::new(660.0, 20.0)));
    content.addSubview(&intro);

    let shortcut_field = add_labeled_text_field(
        &content,
        mtm,
        "Shortcut keys",
        116.0,
        shortcut_field_placeholder(),
    );
    let description_field = add_labeled_text_field(
        &content,
        mtm,
        "Description",
        56.0,
        "What should this shortcut do?",
    );

    let cancel_button = add_modal_action_button(
        &content,
        mtm,
        app,
        "Cancel",
        NSRect::new(NSPoint::new(500.0, 18.0), NSSize::new(90.0, 30.0)),
        sel!(abortModal),
    );
    cancel_button.setKeyEquivalent(&NSString::from_str("\u{1b}"));

    let save_button = add_modal_action_button(
        &content,
        mtm,
        app,
        "Save",
        NSRect::new(NSPoint::new(600.0, 18.0), NSSize::new(90.0, 30.0)),
        sel!(stopModal),
    );
    save_button.setKeyEquivalent(&NSString::from_str("\r"));

    Ok(NewShortcutDialogUi {
        window,
        shortcut_field,
        description_field,
    })
}

fn add_labeled_text_field(
    content: &NSView,
    mtm: MainThreadMarker,
    label: &str,
    y: f64,
    placeholder: &str,
) -> Retained<NSTextField> {
    let label_view = NSTextField::labelWithString(&NSString::from_str(label), mtm);
    label_view.setFrame(NSRect::new(
        NSPoint::new(20.0, y + 28.0),
        NSSize::new(660.0, 20.0),
    ));
    content.addSubview(&label_view);

    let field = NSTextField::initWithFrame(
        NSTextField::alloc(mtm),
        NSRect::new(NSPoint::new(20.0, y), NSSize::new(660.0, 26.0)),
    );
    field.setPlaceholderString(Some(&NSString::from_str(placeholder)));
    field.setEditable(true);
    field.setSelectable(true);
    content.addSubview(&field);
    field
}

fn new_shortcut_intro_text() -> &'static str {
    "Enter the shortcut and what it does in one step. For multi-step shortcuts, separate chords with spaces."
}

fn shortcut_field_placeholder() -> &'static str {
    "Examples: cmd+,  ctrl+`  cmd+k ->"
}

#[cfg(test)]
mod tests {
    use super::{new_shortcut_intro_text, shortcut_field_placeholder};

    #[test]
    fn add_shortcut_copy_prefers_space_separated_chords() {
        assert!(new_shortcut_intro_text().contains("separate chords with spaces"));
        assert!(shortcut_field_placeholder().contains("cmd+,"));
        assert!(shortcut_field_placeholder().contains("ctrl+`"));
        assert!(shortcut_field_placeholder().contains("cmd+k ->"));
    }
}
