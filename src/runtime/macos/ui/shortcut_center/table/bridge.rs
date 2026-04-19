#![allow(non_snake_case)]

use super::super::commands;
use crate::storage::{ManagedShortcut, ShortcutState};
use objc2::rc::Retained;
use objc2::runtime::{AnyObject, Bool};
use objc2::{define_class, msg_send, DefinedClass, MainThreadMarker, MainThreadOnly};
use objc2_app_kit::{
    NSAlert, NSAlertFirstButtonReturn, NSAlertSecondButtonReturn, NSApplication,
    NSControlTextEditingDelegate, NSFont, NSFontAttributeName, NSImage, NSStringDrawing, NSTableColumn,
    NSTableView, NSTableViewDataSource, NSTableViewDelegate, NSTextField, NSToolbar, NSToolbarDelegate,
    NSToolbarItem,
};
use objc2_foundation::{
    NSArray, NSAttributedStringKey, NSDictionary, NSInteger, NSNotification, NSObject, NSObjectProtocol,
    NSPoint, NSRect, NSSize, NSString,
};
use std::cell::{Cell, OnceCell, RefCell};

const COLUMN_SHORTCUT: &str = "shortcut";
const COLUMN_DESCRIPTION: &str = "description";
const COLUMN_STATUS: &str = "status";

const TOOLBAR_NEW_APP: &str = "shortcut-center.new-app";
const TOOLBAR_IMPORT: &str = "shortcut-center.import";
const TOOLBAR_DELETE_APP: &str = "shortcut-center.delete-app";
const TOOLBAR_ADD: &str = "shortcut-center.add";

#[derive(Clone, Debug)]
struct TableRowViewModel {
    shortcut: String,
    description: String,
    status: String,
}

#[derive(Debug, Default)]
pub(crate) struct ShortcutCenterBridgeIvars {
    last_tag: Cell<i64>,
    /// Right-click context menu
    context_row_override: Cell<NSInteger>,
    rows: RefCell<Vec<TableRowViewModel>>,
    /// To handle clicked row / selection
    table_view: OnceCell<Retained<NSTableView>>,
    /// To detected unsaved description changes
    inspector_description_field: OnceCell<Retained<NSTextField>>,
    current_description: RefCell<Option<String>>,
}

define_class!(
    #[unsafe(super = NSObject)]
    #[thread_kind = MainThreadOnly]
    #[ivars = ShortcutCenterBridgeIvars]
    pub(crate) struct ShortcutCenterBridge;

    unsafe impl NSObjectProtocol for ShortcutCenterBridge {}
    unsafe impl NSControlTextEditingDelegate for ShortcutCenterBridge {}

    unsafe impl NSTableViewDataSource for ShortcutCenterBridge {
        #[unsafe(method(numberOfRowsInTableView:))]
        // How many rows are in the table view
        fn numberOfRowsInTableView(&self, _table_view: &NSTableView) -> NSInteger {
            self.ivars().rows.borrow().len() as NSInteger
        }
    }

    unsafe impl NSTableViewDelegate for ShortcutCenterBridge {
        #[unsafe(method(selectionShouldChangeInTableView:))]
        // How to handle unsaved description changes on selection change
        fn selectionShouldChangeInTableView(&self, _table_view: &NSTableView) -> Bool {
            if !self.has_unsaved_description() {
                return Bool::YES;
            }

            let alert = NSAlert::new(self.mtm());
            alert.setMessageText(&NSString::from_str("Save your description changes?"));
            alert.setInformativeText(&NSString::from_str(
                "The current shortcut description has unsaved edits.",
            ));
            alert.addButtonWithTitle(&NSString::from_str("Save"));
            alert.addButtonWithTitle(&NSString::from_str("Discard"));
            alert.addButtonWithTitle(&NSString::from_str("Keep Editing"));

            let response = alert.runModal();
            if response == NSAlertFirstButtonReturn {
                self.ivars().last_tag.set(commands::TAG_SAVE_DESCRIPTION);
                NSApplication::sharedApplication(self.mtm())
                    .stopModalWithCode(objc2_app_kit::NSModalResponseStop);
                Bool::NO
            } else if response == NSAlertSecondButtonReturn {
                Bool::YES
            } else {
                Bool::NO
            }
        }

        #[unsafe(method_id(tableView:viewForTableColumn:row:))]
        // How to render cells
        fn tableView_viewForTableColumn_row(
            &self,
            _table_view: &NSTableView,
            table_column: Option<&NSTableColumn>,
            row: NSInteger,
        ) -> Option<Retained<objc2_app_kit::NSView>> {
            usize::try_from(row).ok().and_then(|row_index| {
                let data = self.ivars().rows.borrow().get(row_index)?.clone();
                let identifier = table_column
                    .map(|column| column.identifier().to_string())
                    .unwrap_or_default();
                let value = match identifier.as_str() {
                    COLUMN_SHORTCUT => data.shortcut,
                    COLUMN_DESCRIPTION => data.description,
                    COLUMN_STATUS => data.status,
                    _ => return None,
                };

                let view = NSTextField::labelWithString(&NSString::from_str(&value), self.mtm());
                view.setFrame(NSRect::new(NSPoint::new(6.0, 3.0), NSSize::new(320.0, 22.0)));
                Some(view.into_super().into_super())
            })
        }

        #[unsafe(method(tableViewSelectionDidChange:))]
        // How to handle selection changes
        fn tableViewSelectionDidChange(&self, _notification: &NSNotification) {
            self.ivars()
                .last_tag
                .set(commands::TAG_TABLE_SELECTION_CHANGED);
            self.ivars().context_row_override.set(-1);
            NSApplication::sharedApplication(self.mtm())
                .stopModalWithCode(objc2_app_kit::NSModalResponseStop);
        }
    }

    unsafe impl NSToolbarDelegate for ShortcutCenterBridge {
        #[unsafe(method_id(toolbar:itemForItemIdentifier:willBeInsertedIntoToolbar:))]
        // How to build toolbar items
        fn toolbar_itemForItemIdentifier_willBeInsertedIntoToolbar(
            &self,
            _toolbar: &NSToolbar,
            item_identifier: &NSString,
            will_insert: bool,
        ) -> Option<Retained<NSToolbarItem>> {
            match item_identifier.to_string().as_str() {
                TOOLBAR_NEW_APP => Some((
                    "New App",
                    commands::TAG_NEW_APP_CREATED,
                    "📱",
                    "Create a custom app in Shortcuts",
                )),
                TOOLBAR_IMPORT => Some((
                    "Import",
                    commands::TAG_IMPORT,
                    "📥",
                    "Import shortcuts for the selected app",
                )),
                TOOLBAR_DELETE_APP => Some((
                    "Delete App",
                    commands::TAG_DELETE_APP,
                    "🗑️",
                    "Delete the selected app and all its shortcuts",
                )),
                TOOLBAR_ADD => Some((
                    "Add",
                    commands::TAG_ADD,
                    "🔑",
                    "Add one shortcut manually",
                )),
                _ => None,
            }
            .map(|(label, tag, emoji, tooltip)| {
                let item = NSToolbarItem::initWithItemIdentifier(
                    NSToolbarItem::alloc(self.mtm()),
                    item_identifier,
                );
                item.setLabel(&NSString::from_str(label));
                item.setPaletteLabel(&NSString::from_str(label));
                item.setToolTip(Some(&NSString::from_str(tooltip)));
                item.setTag(tag as NSInteger);
                let image = emoji_image(emoji);
                image.setAccessibilityDescription(Some(&NSString::from_str(label)));
                item.setImage(Some(&image));
                // SAFETY: toolbar items are owned by this window, and the bridge
                // stays alive as the target-action handler for toolbar commands.
                unsafe {
                    item.setTarget(Some(self.as_ref()));
                    item.setAction(Some(objc2::sel!(handleAction:)));
                }
                let _ = will_insert;
                item
            })
        }

        #[unsafe(method_id(toolbarDefaultItemIdentifiers:))]
        fn toolbarDefaultItemIdentifiers(
            &self,
            _toolbar: &NSToolbar,
        ) -> Retained<NSArray<NSString>> {
            super::toolbar::toolbar_identifiers()
        }

        #[unsafe(method_id(toolbarAllowedItemIdentifiers:))]
        fn toolbarAllowedItemIdentifiers(
            &self,
            _toolbar: &NSToolbar,
        ) -> Retained<NSArray<NSString>> {
            super::toolbar::toolbar_identifiers()
        }
    }

    impl ShortcutCenterBridge {
        #[unsafe(method(handleAction:))]
        // How to handle normal control actions (toolbar, buttons, etc.)
        fn handle_action(&self, sender: Option<&AnyObject>) {
            self.ivars().context_row_override.set(-1);
            if let Some(sender) = sender {
                // SAFETY: AppKit action senders responding to `handleAction:` expose
                // an integer `tag` selector, which is how this controller routes events.
                let tag: i64 = unsafe { msg_send![sender, tag] };
                self.ivars().last_tag.set(tag);
            }
            NSApplication::sharedApplication(self.mtm())
                .stopModalWithCode(objc2_app_kit::NSModalResponseStop);
        }

        #[unsafe(method(handleMenuAction:))]
        // How to handle context-menu actions
        fn handle_menu_action(&self, sender: Option<&AnyObject>) {
            if let Some(sender) = sender {
                // SAFETY: AppKit menu item senders responding to `handleMenuAction:`
                // expose an integer `tag` selector used to identify the action.
                let tag: i64 = unsafe { msg_send![sender, tag] };
                self.ivars().last_tag.set(tag);
            }

            let mut override_row = -1;
            if let Some(table_view) = self.ivars().table_view.get() {
                let clicked_row = table_view.clickedRow();
                if clicked_row >= 0 && !table_view.isRowSelected(clicked_row) {
                    override_row = clicked_row;
                }
            }
            self.ivars().context_row_override.set(override_row);

            NSApplication::sharedApplication(self.mtm())
                .stopModalWithCode(objc2_app_kit::NSModalResponseStop);
        }
    }
);

/// Glues the native table view and the shortcut center event flow
impl ShortcutCenterBridge {
    pub(crate) fn new(mtm: MainThreadMarker) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(ShortcutCenterBridgeIvars {
            last_tag: Cell::new(commands::TAG_NONE),
            context_row_override: Cell::new(-1),
            rows: RefCell::new(Vec::new()),
            table_view: OnceCell::new(),
            inspector_description_field: OnceCell::new(),
            current_description: RefCell::new(None),
        });
        // SAFETY: `this` is a freshly allocated Objective-C object whose superclass
        // initializer is required before the instance is used.
        unsafe { msg_send![super(this), init] }
    }

    pub(crate) fn take_last_tag(&self) -> Option<i64> {
        let tag = self.ivars().last_tag.replace(commands::TAG_NONE);
        if tag == commands::TAG_NONE {
            None
        } else {
            Some(tag)
        }
    }

    pub(crate) fn take_context_row_override(&self) -> Option<usize> {
        let row = self.ivars().context_row_override.replace(-1);
        usize::try_from(row).ok()
    }

    pub(crate) fn set_rows(&self, shortcuts: &[ManagedShortcut]) {
        let rows = shortcuts
            .iter()
            .map(|shortcut| TableRowViewModel {
                shortcut: shortcut.shortcut_display.clone(),
                description: shortcut.description.clone(),
                status: match shortcut.state {
                    ShortcutState::Active => "Active".to_string(),
                    ShortcutState::Dismissed => "Hidden".to_string(),
                },
            })
            .collect::<Vec<_>>();
        *self.ivars().rows.borrow_mut() = rows;
    }

    pub(crate) fn register_table_view(&self, table_view: Retained<NSTableView>) {
        let _ = self.ivars().table_view.set(table_view);
    }

    pub(crate) fn register_description_field(&self, field: Retained<NSTextField>) {
        let _ = self.ivars().inspector_description_field.set(field);
    }

    pub(crate) fn set_description_baseline(&self, value: Option<&str>) {
        *self.ivars().current_description.borrow_mut() = value.map(str::to_string);
    }

    fn has_unsaved_description(&self) -> bool {
        let Some(field) = self.ivars().inspector_description_field.get() else {
            return false;
        };
        let current_description = self.ivars().current_description.borrow();
        let Some(current) = current_description.as_ref() else {
            return false;
        };

        field.stringValue().to_string().trim() != current.trim()
    }
}

fn emoji_image(emoji: &str) -> Retained<NSImage> {
    let size = NSSize::new(22.0, 22.0);
    let emoji = emoji.to_string();

    let handler = block2::RcBlock::new(move |_rect: NSRect| -> Bool {
        let font = NSFont::systemFontOfSize(18.0);
        let string = NSString::from_str(&emoji);
        // SAFETY: NSFontAttributeName is an AppKit-owned static string constant.
        let font_key: &NSAttributedStringKey = unsafe { NSFontAttributeName };
        let font_value: &AnyObject = font.as_ref();
        let attrs: Retained<NSDictionary<NSAttributedStringKey, AnyObject>> =
            NSDictionary::from_slices(&[font_key], &[font_value]);
        // SAFETY: invoked from AppKit's drawing handler with a valid graphics context.
        unsafe { string.drawAtPoint_withAttributes(NSPoint::new(1.0, 1.0), Some(&attrs)) };
        Bool::YES
    });

    NSImage::imageWithSize_flipped_drawingHandler(size, false, &handler)
}
