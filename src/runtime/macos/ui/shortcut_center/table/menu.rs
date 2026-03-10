use super::super::commands;
use super::bridge::ShortcutCenterBridge;
use objc2::rc::Retained;
use objc2::MainThreadMarker;
use objc2::MainThreadOnly;
use objc2_app_kit::{NSMenu, NSMenuItem};
use objc2_foundation::{NSInteger, NSString};

#[derive(Clone)]
pub(crate) struct TableContextMenu {
    pub visibility_item: Retained<NSMenuItem>,
    pub delete_item: Retained<NSMenuItem>,
}

pub(crate) fn build_context_menu(
    mtm: MainThreadMarker,
    bridge: &ShortcutCenterBridge,
) -> (Retained<NSMenu>, TableContextMenu) {
    let menu = NSMenu::initWithTitle(NSMenu::alloc(mtm), &NSString::from_str("Shortcut Actions"));

    let visibility_item = menu_item(
        mtm,
        bridge,
        "Hide",
        commands::TAG_TOGGLE_VISIBILITY_SELECTED,
        true,
    );
    let delete_item = menu_item(mtm, bridge, "Delete", commands::TAG_DELETE_SELECTED, true);

    menu.addItem(&visibility_item);
    menu.addItem(&delete_item);

    (
        menu,
        TableContextMenu {
            visibility_item,
            delete_item,
        },
    )
}

fn menu_item(
    mtm: MainThreadMarker,
    bridge: &ShortcutCenterBridge,
    title: &str,
    tag: i64,
    selection_dependent: bool,
) -> Retained<NSMenuItem> {
    let item = unsafe {
        NSMenuItem::initWithTitle_action_keyEquivalent(
            NSMenuItem::alloc(mtm),
            &NSString::from_str(title),
            Some(objc2::sel!(handleMenuAction:)),
            &NSString::from_str(""),
        )
    };
    item.setTag(tag as NSInteger);
    unsafe {
        item.setTarget(Some(bridge.as_ref()));
    }
    if selection_dependent {
        item.setEnabled(false);
    }
    item
}
