use super::commands;
use super::table;
use super::{ShortcutCenterWindowUi, ShortcutInspectorUi, ShortcutsPaneUi};
use crate::domain::errors::AppError;
use objc2::rc::Retained;
use objc2::{MainThreadMarker, MainThreadOnly};
use objc2_app_kit::{
    NSBackingStoreType, NSButton, NSEventModifierFlags, NSPopUpButton, NSSplitView, NSSplitViewAutosaveName,
    NSSplitViewDividerStyle, NSTextField, NSView, NSWindow, NSWindowButton, NSWindowFrameAutosaveName,
    NSWindowStyleMask,
};
use objc2_foundation::{NSPoint, NSRect, NSSize, NSString};

const WINDOW_WIDTH: f64 = 1100.0;
const WINDOW_HEIGHT: f64 = 720.0;

pub(super) fn build_shortcut_center_window(
    mtm: MainThreadMarker,
    bridge: Retained<table::ShortcutCenterBridge>,
) -> Result<ShortcutCenterWindowUi, AppError> {
    let style = NSWindowStyleMask::Closable
        | NSWindowStyleMask::Titled
        | NSWindowStyleMask::Resizable
        | NSWindowStyleMask::Miniaturizable;
    let rect = NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(WINDOW_WIDTH, WINDOW_HEIGHT));
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
    window.center();
    window.setTitle(&NSString::from_str("Key Finder Shortcuts"));
    window.setFrameAutosaveName(&NSWindowFrameAutosaveName::from_str(
        "com.zelenya.keyfinder.shortcut-center.window",
    ));

    let toolbar = table::build_toolbar(mtm, &bridge);
    window.setToolbar(Some(&toolbar));

    if let Some(close_button) = window.standardWindowButton(NSWindowButton::CloseButton) {
        close_button.setTag(commands::TAG_WINDOW_CLOSED as _);
        // SAFETY: the bridge outlives the window controls and already exposes `handleAction:`
        // as the shared target-action entry point for shortcut-center widgets.
        unsafe {
            close_button.setTarget(Some(bridge.as_ref()));
            close_button.setAction(Some(objc2::sel!(handleAction:)));
        }
    }

    let content = window
        .contentView()
        .ok_or_else(|| AppError::UiOperation("missing window content view".to_string()))?;

    let split_view = NSSplitView::initWithFrame(
        NSSplitView::alloc(mtm),
        NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(WINDOW_WIDTH, WINDOW_HEIGHT)),
    );
    split_view.setVertical(true);
    split_view.setDividerStyle(NSSplitViewDividerStyle::Thin);
    split_view.setAutosaveName(Some(&NSSplitViewAutosaveName::from_str(
        "com.zelenya.keyfinder.shortcut-center.split",
    )));

    let shortcuts_pane = build_shortcuts_pane(mtm, &bridge);
    let (inspector_pane, inspector) = build_inspector_pane(mtm, &bridge);

    split_view.addSubview(&shortcuts_pane.pane);
    split_view.addSubview(&inspector_pane);
    split_view.adjustSubviews();
    content.addSubview(&split_view);

    Ok(ShortcutCenterWindowUi {
        window,
        toolbar,
        bridge,
        shortcuts_pane,
        inspector,
    })
}

fn build_shortcuts_pane(mtm: MainThreadMarker, bridge: &table::ShortcutCenterBridge) -> ShortcutsPaneUi {
    let pane = NSView::initWithFrame(
        NSView::alloc(mtm),
        NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(760.0, WINDOW_HEIGHT)),
    );

    let app_label = NSTextField::labelWithString(&NSString::from_str("App"), mtm);
    app_label.setFrame(NSRect::new(NSPoint::new(20.0, 682.0), NSSize::new(120.0, 20.0)));
    pane.addSubview(&app_label);

    let app_popup = NSPopUpButton::initWithFrame_pullsDown(
        NSPopUpButton::alloc(mtm),
        NSRect::new(NSPoint::new(20.0, 650.0), NSSize::new(320.0, 28.0)),
        false,
    );
    app_popup.setTag(commands::TAG_APP_CHANGED as _);
    // SAFETY: the bridge outlives the left-pane controls and exposes `handleAction:` for popup actions.
    unsafe {
        app_popup.setTarget(Some(bridge.as_ref()));
        app_popup.setAction(Some(objc2::sel!(handleAction:)));
    }
    pane.addSubview(&app_popup);

    let filter_label = NSTextField::labelWithString(&NSString::from_str("Filter"), mtm);
    filter_label.setFrame(NSRect::new(NSPoint::new(360.0, 682.0), NSSize::new(120.0, 20.0)));
    pane.addSubview(&filter_label);

    let filter_popup = NSPopUpButton::initWithFrame_pullsDown(
        NSPopUpButton::alloc(mtm),
        NSRect::new(NSPoint::new(360.0, 650.0), NSSize::new(220.0, 28.0)),
        false,
    );
    filter_popup.setTag(commands::TAG_FILTER_CHANGED as _);
    // SAFETY: the bridge outlives the left-pane controls and exposes `handleAction:` for popup actions.
    unsafe {
        filter_popup.setTarget(Some(bridge.as_ref()));
        filter_popup.setAction(Some(objc2::sel!(handleAction:)));
    }
    pane.addSubview(&filter_popup);

    let status_label = NSTextField::wrappingLabelWithString(
        &NSString::from_str(
            "Select shortcuts in the table, then use the toolbar, context menu, or inspector on the right.",
        ),
        mtm,
    );
    status_label.setFrame(NSRect::new(NSPoint::new(20.0, 606.0), NSSize::new(700.0, 34.0)));
    pane.addSubview(&status_label);

    let (table_scroll, table_view, table_context_menu) = table::build_table_view(mtm, bridge);
    table_scroll.setFrame(NSRect::new(NSPoint::new(20.0, 20.0), NSSize::new(700.0, 570.0)));
    pane.addSubview(&table_scroll);

    ShortcutsPaneUi {
        pane,
        app_popup,
        filter_popup,
        status_label,
        table_scroll,
        table_view,
        table_context_menu,
    }
}

fn build_inspector_pane(
    mtm: MainThreadMarker,
    bridge: &table::ShortcutCenterBridge,
) -> (Retained<NSView>, ShortcutInspectorUi) {
    let pane = NSView::initWithFrame(
        NSView::alloc(mtm),
        NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(340.0, WINDOW_HEIGHT)),
    );

    let title_label = NSTextField::labelWithString(&NSString::from_str("Shortcut Inspector"), mtm);
    title_label.setFrame(NSRect::new(NSPoint::new(20.0, 682.0), NSSize::new(280.0, 22.0)));
    pane.addSubview(&title_label);

    let summary_label = NSTextField::wrappingLabelWithString(
        &NSString::from_str("Choose an app to review its shortcuts."),
        mtm,
    );
    summary_label.setFrame(NSRect::new(NSPoint::new(20.0, 612.0), NSSize::new(300.0, 56.0)));
    pane.addSubview(&summary_label);

    let alias_label = NSTextField::wrappingLabelWithString(&NSString::from_str(""), mtm);
    alias_label.setFrame(NSRect::new(NSPoint::new(20.0, 570.0), NSSize::new(300.0, 34.0)));
    pane.addSubview(&alias_label);

    let selection_label = NSTextField::labelWithString(&NSString::from_str("No shortcuts selected"), mtm);
    selection_label.setFrame(NSRect::new(NSPoint::new(20.0, 534.0), NSSize::new(280.0, 20.0)));
    pane.addSubview(&selection_label);

    let shortcut_label = add_field_label(&pane, mtm, "Shortcut", 492.0);
    let shortcut_value = add_value_label(&pane, mtm, 468.0);

    let status_label = add_field_label(&pane, mtm, "Status", 430.0);
    let status_value = add_value_label(&pane, mtm, 406.0);

    let description_label = add_field_label(&pane, mtm, "Description", 360.0);
    let description_field = NSTextField::initWithFrame(
        NSTextField::alloc(mtm),
        NSRect::new(NSPoint::new(20.0, 322.0), NSSize::new(300.0, 28.0)),
    );
    description_field.setEditable(true);
    description_field.setSelectable(true);
    description_field.setPlaceholderString(Some(&NSString::from_str("Describe what this shortcut does")));
    pane.addSubview(&description_field);
    bridge.register_description_field(description_field.clone());

    let save_button = add_action_button(
        &pane,
        mtm,
        bridge,
        "Save Description",
        NSRect::new(NSPoint::new(188.0, 284.0), NSSize::new(132.0, 30.0)),
        commands::TAG_SAVE_DESCRIPTION,
    );
    save_button.setKeyEquivalent(&NSString::from_str("s"));
    save_button.setKeyEquivalentModifierMask(NSEventModifierFlags::Command);
    save_button.setToolTip(Some(&NSString::from_str(
        "Save the edited description (Command-S)",
    )));

    let hint_label = NSTextField::wrappingLabelWithString(
        &NSString::from_str(
            "Select one shortcut to edit its description, or select several to update their status together.",
        ),
        mtm,
    );
    hint_label.setFrame(NSRect::new(NSPoint::new(20.0, 202.0), NSSize::new(300.0, 56.0)));
    pane.addSubview(&hint_label);

    let visibility_button = add_action_button(
        &pane,
        mtm,
        bridge,
        "Hide",
        NSRect::new(NSPoint::new(20.0, 20.0), NSSize::new(184.0, 30.0)),
        commands::TAG_TOGGLE_VISIBILITY_SELECTED,
    );
    visibility_button.setToolTip(Some(&NSString::from_str(
        "Hide selected shortcuts from notifications",
    )));

    let delete_button = add_action_button(
        &pane,
        mtm,
        bridge,
        "Delete",
        NSRect::new(NSPoint::new(214.0, 20.0), NSSize::new(106.0, 30.0)),
        commands::TAG_DELETE_SELECTED,
    );
    delete_button.setToolTip(Some(&NSString::from_str("Delete the selected shortcuts")));

    (
        pane,
        ShortcutInspectorUi {
            title_label,
            summary_label,
            alias_label,
            selection_label,
            shortcut_label,
            shortcut_value,
            status_label,
            status_value,
            description_label,
            description_field,
            save_button,
            hint_label,
            visibility_button,
            delete_button,
        },
    )
}

fn add_field_label(content: &NSView, mtm: MainThreadMarker, title: &str, y: f64) -> Retained<NSTextField> {
    let label = NSTextField::labelWithString(&NSString::from_str(title), mtm);
    label.setFrame(NSRect::new(NSPoint::new(20.0, y), NSSize::new(180.0, 20.0)));
    content.addSubview(&label);
    label
}

fn add_value_label(content: &NSView, mtm: MainThreadMarker, y: f64) -> Retained<NSTextField> {
    let label = NSTextField::wrappingLabelWithString(&NSString::from_str(""), mtm);
    label.setFrame(NSRect::new(NSPoint::new(20.0, y), NSSize::new(300.0, 28.0)));
    content.addSubview(&label);
    label
}

fn add_action_button(
    content: &NSView,
    mtm: MainThreadMarker,
    bridge: &table::ShortcutCenterBridge,
    title: &str,
    frame: NSRect,
    tag: i64,
) -> Retained<NSButton> {
    // SAFETY: the bridge outlives these buttons and exposes `handleAction:` as the shared
    // target-action entry point for shortcut-center controls.
    let button = unsafe {
        NSButton::buttonWithTitle_target_action(
            &NSString::from_str(title),
            Some(bridge.as_ref()),
            Some(objc2::sel!(handleAction:)),
            mtm,
        )
    };
    button.setTag(tag as _);
    button.setFrame(frame);
    content.addSubview(&button);
    button
}

pub(super) fn popup_selected_title(popup: &NSPopUpButton) -> Option<String> {
    popup.titleOfSelectedItem().map(|value| value.to_string())
}

pub(super) fn set_label_text(label: &NSTextField, text: &str) {
    label.setStringValue(&NSString::from_str(text));
}
