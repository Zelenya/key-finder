use super::bridge::ShortcutCenterBridge;
use objc2::rc::Retained;
use objc2::runtime::ProtocolObject;
use objc2::MainThreadMarker;
use objc2::MainThreadOnly;
use objc2_app_kit::{NSToolbar, NSToolbarDelegate, NSToolbarDisplayMode};
use objc2_foundation::{NSArray, NSString};

const TOOLBAR_NEW_APP: &str = "shortcut-center.new-app";
const TOOLBAR_IMPORT: &str = "shortcut-center.import";
const TOOLBAR_DELETE_APP: &str = "shortcut-center.delete-app";
const TOOLBAR_ADD: &str = "shortcut-center.add";

pub(crate) fn build_toolbar(mtm: MainThreadMarker, bridge: &ShortcutCenterBridge) -> Retained<NSToolbar> {
    let identifier = NSString::from_str("com.zelenya.keyfinder.shortcut-center.toolbar");
    let toolbar = NSToolbar::initWithIdentifier(NSToolbar::alloc(mtm), &identifier);
    toolbar.setAllowsUserCustomization(false);
    toolbar.setVisible(true);
    toolbar.setDisplayMode(NSToolbarDisplayMode::IconAndLabel);
    let delegate: &ProtocolObject<dyn NSToolbarDelegate> = ProtocolObject::from_ref(bridge);
    toolbar.setDelegate(Some(delegate));
    toolbar
}

pub(crate) fn toolbar_identifiers() -> Retained<NSArray<NSString>> {
    NSArray::from_retained_slice(&[
        NSString::from_str(TOOLBAR_NEW_APP),
        NSString::from_str(TOOLBAR_IMPORT),
        NSString::from_str(TOOLBAR_DELETE_APP),
        NSString::from_str(TOOLBAR_ADD),
    ])
}
