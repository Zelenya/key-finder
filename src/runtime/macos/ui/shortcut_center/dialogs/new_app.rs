use crate::application::shortcut_center::{CreateAppInput, ShortcutCenterCommandService};
use crate::domain::errors::AppError;
use crate::runtime::macos::ui::dialogs::close_sheet;
use crate::storage::AppId;
use objc2::rc::Retained;
use objc2::sel;
use objc2::MainThreadMarker;
use objc2::MainThreadOnly;
use objc2_app_kit::{
    NSAlert, NSApplication, NSBackingStoreType, NSButton, NSTextField, NSView, NSWindow, NSWindowButton,
    NSWindowStyleMask,
};
use objc2_foundation::{NSPoint, NSRect, NSSize, NSString};

pub(crate) enum NewAppDialogResult {
    Created {
        app_id: AppId,
        app_name: String,
        alias_count: usize,
    },
    Canceled,
}

struct NewAppDialogUi {
    window: Retained<NSWindow>,
    app_name_field: Retained<NSTextField>,
    aliases_field: Retained<NSTextField>,
}

pub(crate) fn open_new_app_dialog(
    command_service: &ShortcutCenterCommandService,
    parent_window: &NSWindow,
) -> Result<NewAppDialogResult, AppError> {
    let Some(mtm) = MainThreadMarker::new() else {
        return Err(AppError::MainThreadRequired);
    };
    let app = NSApplication::sharedApplication(mtm);
    app.activate();

    let ui = build_new_app_window(mtm, &app)?;
    ui.window.makeFirstResponder(Some(&ui.app_name_field));
    parent_window.beginSheet_completionHandler(&ui.window, None);

    loop {
        ui.window.makeKeyAndOrderFront(None);
        let response = app.runModalForWindow(&ui.window);
        if !ui.window.isVisible() {
            close_sheet(parent_window, &ui.window, objc2_app_kit::NSModalResponseCancel);
            return Ok(NewAppDialogResult::Canceled);
        }
        if response == objc2_app_kit::NSModalResponseStop {
            let app_name = ui.app_name_field.stringValue().to_string();
            let aliases = parse_aliases(&ui.aliases_field.stringValue().to_string());
            match command_service.create_app(CreateAppInput {
                app_name: app_name.clone(),
                aliases,
            }) {
                Ok(result) => {
                    close_sheet(parent_window, &ui.window, objc2_app_kit::NSModalResponseOK);
                    return Ok(NewAppDialogResult::Created {
                        app_id: result.app_id,
                        app_name: result.app_name,
                        alias_count: result.alias_count,
                    });
                }
                Err(error) => {
                    show_error(&app, "Couldn't create app", &error.to_string())?;
                    continue;
                }
            }
        }

        close_sheet(parent_window, &ui.window, objc2_app_kit::NSModalResponseCancel);
        return Ok(NewAppDialogResult::Canceled);
    }
}

fn build_new_app_window(mtm: MainThreadMarker, app: &NSApplication) -> Result<NewAppDialogUi, AppError> {
    let style = NSWindowStyleMask::Closable | NSWindowStyleMask::Titled;
    let rect = NSRect::new(NSPoint::new(260.0, 220.0), NSSize::new(620.0, 220.0));
    let window = unsafe {
        NSWindow::initWithContentRect_styleMask_backing_defer(
            NSWindow::alloc(mtm),
            rect,
            style,
            NSBackingStoreType::Buffered,
            false,
        )
    };
    window.setTitle(&NSString::from_str("New App"));
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
        .ok_or_else(|| AppError::StorageOperation("missing new app dialog content view".to_string()))?;

    let intro = NSTextField::labelWithString(
        &NSString::from_str("Create a custom app, then import a CSV or add shortcuts manually."),
        mtm,
    );
    intro.setFrame(NSRect::new(NSPoint::new(20.0, 182.0), NSSize::new(580.0, 20.0)));
    content.addSubview(&intro);

    let app_name_field = add_labeled_text_field(
        &content,
        mtm,
        "App name",
        116.0,
        "The name shown in notifications and Shortcuts",
    );
    let aliases_field = add_labeled_text_field(
        &content,
        mtm,
        "Aliases (optional)",
        56.0,
        "Comma-separated alternate names, like Code, VS Code",
    );

    add_action_button(
        &content,
        mtm,
        app,
        "Cancel",
        NSRect::new(NSPoint::new(420.0, 18.0), NSSize::new(90.0, 30.0)),
        sel!(abortModal),
    );
    add_action_button(
        &content,
        mtm,
        app,
        "Create",
        NSRect::new(NSPoint::new(520.0, 18.0), NSSize::new(90.0, 30.0)),
        sel!(stopModal),
    );

    Ok(NewAppDialogUi {
        window,
        app_name_field,
        aliases_field,
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
        NSSize::new(580.0, 20.0),
    ));
    content.addSubview(&label_view);

    let field = NSTextField::initWithFrame(
        NSTextField::alloc(mtm),
        NSRect::new(NSPoint::new(20.0, y), NSSize::new(580.0, 26.0)),
    );
    field.setPlaceholderString(Some(&NSString::from_str(placeholder)));
    field.setEditable(true);
    field.setSelectable(true);
    content.addSubview(&field);
    field
}

fn add_action_button(
    content: &NSView,
    mtm: MainThreadMarker,
    app: &NSApplication,
    title: &str,
    frame: NSRect,
    action: objc2::runtime::Sel,
) {
    // SAFETY: `app` is the shared `NSApplication` on the main thread, and the
    // provided selector is one of AppKit's standard modal-ending actions.
    let button = unsafe {
        NSButton::buttonWithTitle_target_action(&NSString::from_str(title), Some(app), Some(action), mtm)
    };
    button.setFrame(frame);
    content.addSubview(&button);
}

fn parse_aliases(raw: &str) -> Vec<String> {
    raw.split(',').map(str::trim).filter(|value| !value.is_empty()).map(str::to_string).collect()
}

fn show_error(app: &NSApplication, title: &str, message: &str) -> Result<(), AppError> {
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
