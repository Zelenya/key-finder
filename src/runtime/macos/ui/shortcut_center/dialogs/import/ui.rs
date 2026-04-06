use super::super::super::window;
use super::spec::{
    presentation_for, ImportDialogKind, ImportDialogPresentation, ImportDialogSpec, ImportDialogState,
    VsCodeImportMode,
};
use super::TAG_MODE_CHANGED;
use super::{TAG_CHOOSE_FILE, TAG_CLEAR_FILE, TAG_CLOSE, TAG_IMPORT};
use crate::domain::errors::AppError;
use crate::runtime::macos::platform::import_sources;
use objc2::rc::Retained;
use objc2::sel;
use objc2::MainThreadMarker;
use objc2::MainThreadOnly;
use objc2_app_kit::{
    NSApplication, NSBackingStoreType, NSButton, NSOpenPanel, NSPathControl, NSPopUpButton,
    NSProgressIndicator, NSProgressIndicatorStyle, NSTextField, NSView, NSWindow, NSWindowButton,
    NSWindowStyleMask,
};
use objc2_foundation::{NSArray, NSPoint, NSRect, NSSize, NSString, NSURL};
use std::path::Path;

pub(super) struct ImportDialogUi {
    pub window: Retained<NSWindow>,
    pub intro_label: Retained<NSTextField>,
    pub file_label: Retained<NSTextField>,
    pub mode_popup: Option<Retained<NSPopUpButton>>,
    pub selected_file_label: Retained<NSTextField>,
    pub selected_path_control: Retained<NSPathControl>,
    pub status_label: Retained<NSTextField>,
    pub detail_label: Retained<NSTextField>,
    pub progress_indicator: Retained<NSProgressIndicator>,
    pub choose_button: Retained<NSButton>,
    pub clear_button: Retained<NSButton>,
    pub import_button: Retained<NSButton>,
    pub close_button: Retained<NSButton>,
}

pub(super) fn build_import_window(
    mtm: MainThreadMarker,
    app: &NSApplication,
    app_name: &str,
    spec: &ImportDialogSpec,
) -> Result<ImportDialogUi, AppError> {
    let style = NSWindowStyleMask::Closable | NSWindowStyleMask::Titled;
    let rect = NSRect::new(NSPoint::new(240.0, 200.0), NSSize::new(760.0, 360.0));
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
    window.setTitle(&NSString::from_str(&format!("Import Shortcuts for {app_name}")));
    window.center();
    if let Some(close_button) = window.standardWindowButton(NSWindowButton::CloseButton) {
        close_button.setTag(TAG_CLOSE as _);
        // SAFETY: the close button belongs to this live dialog, `app` is the
        // shared `NSApplication`, and `stopModal` ends this modal flow.
        unsafe {
            close_button.setTarget(Some(app));
            close_button.setAction(Some(sel!(stopModal)));
        }
    }

    let content = window
        .contentView()
        .ok_or_else(|| AppError::StorageOperation("missing import dialog content view".to_string()))?;

    let has_vscode_modes = !spec.kind.vscode_modes().is_empty();
    let default_presentation = presentation_for(spec, spec.kind.default_vscode_mode());

    if has_vscode_modes {
        let label = NSTextField::labelWithString(&NSString::from_str("Import mode"), mtm);
        label.setFrame(NSRect::new(NSPoint::new(20.0, 304.0), NSSize::new(120.0, 20.0)));
        content.addSubview(&label);
    }

    let mode_popup = if has_vscode_modes {
        let popup = NSPopUpButton::initWithFrame_pullsDown(
            NSPopUpButton::alloc(mtm),
            NSRect::new(NSPoint::new(20.0, 272.0), NSSize::new(260.0, 28.0)),
            false,
        );
        popup.setTag(TAG_MODE_CHANGED as _);
        // SAFETY: the popup belongs to this live dialog, `app` is the shared
        // `NSApplication`, and `stopModal` lets the dialog loop re-read the selection.
        unsafe {
            popup.setTarget(Some(app));
            popup.setAction(Some(sel!(stopModal)));
        }
        let items = spec.kind.vscode_modes().iter().map(|mode| mode.title().to_string()).collect::<Vec<_>>();
        window::populate_popup(&popup, &items, 0);
        content.addSubview(&popup);
        Some(popup)
    } else {
        None
    };

    let intro_label =
        NSTextField::wrappingLabelWithString(&NSString::from_str(default_presentation.intro_text), mtm);
    intro_label.setFrame(NSRect::new(NSPoint::new(20.0, 226.0), NSSize::new(720.0, 40.0)));
    content.addSubview(&intro_label);

    let file_label = NSTextField::labelWithString(&NSString::from_str(default_presentation.file_label), mtm);
    file_label.setFrame(NSRect::new(NSPoint::new(20.0, 190.0), NSSize::new(720.0, 20.0)));
    content.addSubview(&file_label);

    let selected_file_label = NSTextField::labelWithString(&NSString::from_str("No file selected"), mtm);
    selected_file_label.setFrame(NSRect::new(NSPoint::new(20.0, 164.0), NSSize::new(500.0, 20.0)));
    content.addSubview(&selected_file_label);

    let selected_path_control = NSPathControl::initWithFrame(
        NSPathControl::alloc(mtm),
        NSRect::new(NSPoint::new(20.0, 136.0), NSSize::new(500.0, 24.0)),
    );
    selected_path_control.setEnabled(false);
    selected_path_control.setHidden(true);
    content.addSubview(&selected_path_control);

    let choose_button = add_action_button(
        &content,
        mtm,
        app,
        "Choose File",
        NSRect::new(NSPoint::new(530.0, 150.0), NSSize::new(110.0, 26.0)),
        TAG_CHOOSE_FILE,
    );
    let clear_button = add_action_button(
        &content,
        mtm,
        app,
        "Clear",
        NSRect::new(NSPoint::new(650.0, 150.0), NSSize::new(90.0, 26.0)),
        TAG_CLEAR_FILE,
    );

    let progress_indicator = NSProgressIndicator::initWithFrame(
        NSProgressIndicator::alloc(mtm),
        NSRect::new(NSPoint::new(20.0, 96.0), NSSize::new(24.0, 24.0)),
    );
    progress_indicator.setStyle(NSProgressIndicatorStyle::Spinning);
    progress_indicator.setIndeterminate(true);
    progress_indicator.setDisplayedWhenStopped(false);
    content.addSubview(&progress_indicator);

    let status_label = NSTextField::labelWithString(&NSString::from_str(""), mtm);
    status_label.setFrame(NSRect::new(NSPoint::new(52.0, 100.0), NSSize::new(688.0, 18.0)));
    content.addSubview(&status_label);

    let detail_label = NSTextField::initWithFrame(
        NSTextField::alloc(mtm),
        NSRect::new(NSPoint::new(20.0, 38.0), NSSize::new(720.0, 44.0)),
    );
    detail_label.setEditable(false);
    detail_label.setSelectable(true);
    detail_label.setBordered(false);
    detail_label.setDrawsBackground(false);
    detail_label.setUsesSingleLineMode(false);
    if let Some(cell) = detail_label.cell() {
        cell.setWraps(true);
        cell.setScrollable(false);
        cell.setUsesSingleLineMode(false);
    }
    content.addSubview(&detail_label);

    let close_button = add_action_button(
        &content,
        mtm,
        app,
        "Cancel",
        NSRect::new(NSPoint::new(560.0, 8.0), NSSize::new(90.0, 30.0)),
        TAG_CLOSE,
    );
    let import_button = add_action_button(
        &content,
        mtm,
        app,
        "Import",
        NSRect::new(NSPoint::new(660.0, 8.0), NSSize::new(90.0, 30.0)),
        TAG_IMPORT,
    );

    Ok(ImportDialogUi {
        window,
        intro_label,
        file_label,
        mode_popup,
        selected_file_label,
        selected_path_control,
        status_label,
        detail_label,
        progress_indicator,
        choose_button,
        clear_button,
        import_button,
        close_button,
    })
}

pub(super) fn apply_dialog_state(
    ui: &ImportDialogUi,
    presentation: &ImportDialogPresentation,
    state: &ImportDialogState,
    selected_path: Option<&str>,
    status_text: &str,
    detail_text: &str,
) {
    ui.intro_label.setStringValue(&NSString::from_str(presentation.intro_text));
    ui.file_label.setStringValue(&NSString::from_str(presentation.file_label));
    ui.import_button.setTitle(&NSString::from_str(presentation.import_button_label));
    let selected_file_name = selected_path
        .and_then(|path| Path::new(path).file_name())
        .and_then(|name| name.to_str())
        .unwrap_or("No file selected");
    ui.selected_file_label.setStringValue(&NSString::from_str(selected_file_name));
    let selected_url = selected_path.and_then(NSURL::from_file_path);
    ui.selected_path_control.setURL(selected_url.as_deref());
    ui.status_label.setStringValue(&NSString::from_str(status_text));
    ui.detail_label.setStringValue(&NSString::from_str(detail_text));

    let has_path = selected_path.is_some_and(|path| !path.trim().is_empty());
    let importing = matches!(state, ImportDialogState::Importing);
    let success = matches!(state, ImportDialogState::Success);
    let show_file_controls = presentation.requires_file;

    ui.file_label.setHidden(!show_file_controls);
    ui.selected_file_label.setHidden(!show_file_controls);
    ui.selected_path_control
        .setHidden(!show_file_controls || selected_path.is_none_or(|path| path.trim().is_empty()));
    ui.choose_button.setHidden(!show_file_controls);
    ui.clear_button.setHidden(!show_file_controls);
    ui.choose_button.setEnabled(show_file_controls && !importing && !success);
    ui.clear_button.setEnabled(show_file_controls && !importing && !success && has_path);
    ui.import_button.setEnabled(!importing && !success && (has_path || !presentation.requires_file));
    ui.close_button.setTitle(&NSString::from_str(if success { "Done" } else { "Cancel" }));
    ui.detail_label.setHidden(detail_text.trim().is_empty());

    if importing {
        // SAFETY: the spinner is a live AppKit control owned by this dialog on the main thread.
        unsafe {
            ui.progress_indicator.startAnimation(None);
        }
    } else {
        // SAFETY: the spinner is a live AppKit control owned by this dialog on the main thread.
        unsafe {
            ui.progress_indicator.stopAnimation(None);
        }
    }
}

pub(super) fn prompt_for_import_file(
    presentation: &ImportDialogPresentation,
    kind: ImportDialogKind,
    current_value: Option<&str>,
    parent_window: &NSWindow,
) -> Result<Option<String>, AppError> {
    let Some(mtm) = MainThreadMarker::new() else {
        return Err(AppError::MainThreadRequired);
    };

    let panel = NSOpenPanel::openPanel(mtm);
    panel.setTitle(Some(&NSString::from_str("Choose Import File")));
    panel.setMessage(Some(&NSString::from_str(&format!(
        "Select the {} file to import.",
        presentation.file_description
    ))));
    panel.setPrompt(Some(&NSString::from_str("Choose")));
    panel.setCanChooseFiles(true);
    panel.setCanChooseDirectories(false);
    panel.setAllowsMultipleSelection(false);
    panel.setCanCreateDirectories(false);
    panel.setResolvesAliases(true);
    let allowed_file_types = allowed_file_types(presentation.allowed_extensions);
    #[allow(deprecated)]
    panel.setAllowedFileTypes(Some(&allowed_file_types));

    if let Some(directory) = preferred_picker_directory(kind, current_value) {
        panel.setDirectoryURL(Some(&directory));
    }

    let response = panel.runModal();
    parent_window.makeKeyAndOrderFront(None);
    if response != objc2_app_kit::NSModalResponseOK {
        return Ok(None);
    }

    let url = panel
        .URLs()
        .firstObject()
        .ok_or_else(|| AppError::StorageOperation("file picker returned no selected file".to_string()))?;
    let path = url
        .to_file_path()
        .ok_or_else(|| AppError::StorageOperation("selected import file was not a local path".to_string()))?;
    Ok(Some(path.display().to_string()))
}

fn add_action_button(
    content: &NSView,
    mtm: MainThreadMarker,
    app: &NSApplication,
    title: &str,
    frame: NSRect,
    tag: i64,
) -> Retained<NSButton> {
    // SAFETY: `app` is the shared `NSApplication` on the main thread, and
    // `stopModal` is the standard AppKit action for ending this dialog step.
    let button = unsafe {
        NSButton::buttonWithTitle_target_action(
            &NSString::from_str(title),
            Some(app),
            Some(sel!(stopModal)),
            mtm,
        )
    };
    button.setTag(tag as _);
    button.setFrame(frame);
    content.addSubview(&button);
    button
}

fn allowed_file_types(extensions: &[&str]) -> Retained<NSArray<NSString>> {
    let values = extensions.iter().map(|value| NSString::from_str(value)).collect::<Vec<_>>();
    NSArray::from_retained_slice(&values)
}

fn preferred_picker_directory(
    kind: ImportDialogKind,
    current_value: Option<&str>,
) -> Option<Retained<NSURL>> {
    let current_path = current_value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(Path::new)
        .and_then(|path| if path.is_dir() { Some(path) } else { path.parent() })
        .filter(|path| path.exists());

    if let Some(path) = current_path {
        return NSURL::from_directory_path(path);
    }

    let default_dir = match kind {
        ImportDialogKind::CustomCsv => {
            let home = dirs::home_dir()?;
            home.join("Downloads")
        }
        ImportDialogKind::VSCode => import_sources::preferred_vscode_export_directory()?,
        ImportDialogKind::Zed => import_sources::preferred_zed_keymap_directory()?,
        ImportDialogKind::JetBrains => import_sources::preferred_idea_keymap_directory()?,
    };
    default_dir.exists().then_some(default_dir).and_then(NSURL::from_directory_path)
}

pub(super) fn selected_vscode_mode(ui: &ImportDialogUi, spec: &ImportDialogSpec) -> Option<VsCodeImportMode> {
    let popup = ui.mode_popup.as_deref()?;
    let index = usize::try_from(popup.indexOfSelectedItem()).ok()?;
    spec.kind.vscode_modes().get(index).copied()
}
