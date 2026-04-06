use super::bridge::ShortcutCenterBridge;
use super::menu::{build_context_menu, TableContextMenu};
use crate::storage::{ManagedShortcut, ShortcutId};
use objc2::rc::Retained;
use objc2::runtime::ProtocolObject;
use objc2::MainThreadMarker;
use objc2::MainThreadOnly;
use objc2_app_kit::{
    NSScrollView, NSTableColumn, NSTableColumnResizingOptions, NSTableView, NSTableViewAutosaveName,
    NSTableViewDataSource, NSTableViewDelegate, NSTableViewGridLineStyle, NSTableViewSelectionHighlightStyle,
    NSUserInterfaceItemIdentifier,
};
use objc2_foundation::{NSMutableIndexSet, NSPoint, NSRect, NSSize, NSUInteger};

const COLUMN_SHORTCUT: &str = "shortcut";
const COLUMN_DESCRIPTION: &str = "description";
const COLUMN_STATUS: &str = "status";

/// Builds the native table view for the shortcut center.
/// Handles read/write of selection state.
pub(crate) fn build_table_view(
    mtm: MainThreadMarker,
    bridge: &ShortcutCenterBridge,
) -> (Retained<NSScrollView>, Retained<NSTableView>, TableContextMenu) {
    let scroll = NSScrollView::initWithFrame(
        NSScrollView::alloc(mtm),
        NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(660.0, 520.0)),
    );
    scroll.setHasVerticalScroller(true);
    scroll.setHasHorizontalScroller(false);

    let table_view = NSTableView::initWithFrame(
        NSTableView::alloc(mtm),
        NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(660.0, 520.0)),
    );
    table_view.setUsesAlternatingRowBackgroundColors(true);
    table_view.setGridStyleMask(NSTableViewGridLineStyle::SolidHorizontalGridLineMask);
    table_view.setSelectionHighlightStyle(NSTableViewSelectionHighlightStyle::Regular);
    table_view.setRowHeight(28.0);
    table_view.setAllowsMultipleSelection(true);
    table_view.setAllowsTypeSelect(true);
    table_view.setAllowsColumnSelection(false);
    table_view.setAutosaveName(Some(&NSTableViewAutosaveName::from_str(
        "com.zelenya.keyfinder.shortcut-center.shortcuts-table",
    )));
    table_view.setAutosaveTableColumns(true);

    add_column(mtm, &table_view, "Shortcut", COLUMN_SHORTCUT, 160.0, 120.0);
    add_column(mtm, &table_view, "Description", COLUMN_DESCRIPTION, 410.0, 240.0);
    add_column(mtm, &table_view, "Status", COLUMN_STATUS, 90.0, 80.0);

    // Register the table view with the bridge
    let data_source: &ProtocolObject<dyn NSTableViewDataSource> = ProtocolObject::from_ref(bridge);
    let delegate: &ProtocolObject<dyn NSTableViewDelegate> = ProtocolObject::from_ref(bridge);
    // SAFETY: `bridge` is retained by the shortcut-center window for the lifetime of the table view
    // and implements both AppKit protocols required here.
    unsafe {
        table_view.setDataSource(Some(data_source));
        table_view.setDelegate(Some(delegate));
    }

    let (menu, context_menu) = build_context_menu(mtm, bridge);
    // SAFETY: the context menu is retained in the returned `TableContextMenu`, so the native menu
    // object outlives this live table view attachment.
    unsafe {
        table_view.setMenu(Some(&menu));
    }

    bridge.register_table_view(table_view.clone());
    scroll.setDocumentView(Some(&table_view));

    (scroll, table_view, context_menu)
}

/// Reads the current native multi-selection and converts into stable shortcut ids
pub(crate) fn selected_shortcut_ids(
    table_view: &NSTableView,
    shortcuts: &[ManagedShortcut],
) -> Vec<ShortcutId> {
    let indexes = table_view.selectedRowIndexes();
    shortcuts
        .iter()
        .enumerate()
        .filter_map(|(row_index, shortcut)| {
            indexes.containsIndex(row_index as NSUInteger).then_some(shortcut.id)
        })
        .collect()
}

/// Reads the current native selection and converts into a stable shortcut id
pub(crate) fn focused_shortcut_id(
    table_view: &NSTableView,
    shortcuts: &[ManagedShortcut],
) -> Option<ShortcutId> {
    usize::try_from(table_view.selectedRow())
        .ok()
        .and_then(|row| shortcuts.get(row))
        .map(|shortcut| shortcut.id)
}

/// Applies selection back into the native table.
pub(crate) fn sync_table_selection(
    table_view: &NSTableView,
    shortcuts: &[ManagedShortcut],
    selected_ids: &[ShortcutId],
) {
    let indexes = NSMutableIndexSet::indexSet();
    for (row_index, shortcut) in shortcuts.iter().enumerate() {
        if selected_ids.contains(&shortcut.id) {
            indexes.addIndex(row_index as NSUInteger);
        }
    }
    table_view.selectRowIndexes_byExtendingSelection(&indexes, false);
    if selected_ids.is_empty() {
        // SAFETY: clearing selection on a live `NSTableView` with no sender is a normal AppKit call.
        unsafe { table_view.deselectAll(None) };
    }
}

fn add_column(
    mtm: MainThreadMarker,
    table_view: &NSTableView,
    title: &str,
    identifier: &str,
    width: f64,
    min_width: f64,
) {
    let identifier = NSUserInterfaceItemIdentifier::from_str(identifier);
    let column = NSTableColumn::initWithIdentifier(NSTableColumn::alloc(mtm), &identifier);
    column.setTitle(&objc2_foundation::NSString::from_str(title));
    column.setWidth(width);
    column.setMinWidth(min_width);
    column.setResizingMask(
        NSTableColumnResizingOptions::AutoresizingMask | NSTableColumnResizingOptions::UserResizingMask,
    );
    table_view.addTableColumn(&column);
}
