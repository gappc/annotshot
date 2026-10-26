use std::cell::{Cell, RefCell};
use std::path::{Path, PathBuf};
use std::rc::Rc;

use adw::prelude::*;
use gtk::{gdk, gio, glib};
use relm4::prelude::*;

use crate::canvas::{self, CanvasState, SharedCanvas};
use crate::editor::Tool;
use crate::export::{self, surface_from_texture, texture_from_surface};
use crate::files::{self, Choice};
use crate::model::{Color, Width};
use crate::text_edit::TextEditor;

const CSS: &str = "
button.swatch { min-width: 24px; min-height: 24px; padding: 0; border-radius: 12px; }
button.swatch-red { background: #e01b24; }
button.swatch-yellow { background: #f6d32d; }
button.swatch-green { background: #33d17a; }
button.swatch-blue { background: #3584e4; }
button.swatch-black { background: #000000; }
button.swatch:checked { border: 2px solid @window_fg_color; }
";

pub struct AppModel {
    state: SharedCanvas,
    canvas: gtk::DrawingArea,
    text: Rc<TextEditor>,
    toasts: adw::ToastOverlay,
    window: adw::ApplicationWindow,
    /// Where the image was last saved; `Ctrl+S` writes here without asking.
    file: Option<PathBuf>,
    /// Action waiting on the unsaved-changes dialog and, after "Save", on the save.
    pending: Option<Pending>,
    /// A save or unsaved-changes dialog is showing.
    busy: bool,
    /// Set right before closing for real so `close-request` lets it through.
    allow_close: Rc<Cell<bool>>,
    /// Set while the window is hidden after closing because our copy is
    /// still on the clipboard.
    linger: SharedLinger,
    // Mirrors of the canvas state that the toolbar reflects; refreshed by `sync`.
    tool: Tool,
    color: Color,
    width: Width,
    can_undo: bool,
    can_redo: bool,
    has_image: bool,
    dirty: bool,
}

/// What to continue with once unsaved changes are dealt with.
#[derive(Debug)]
pub enum Pending {
    NewImage(gdk::Texture),
    Close,
}

/// What closing the window for real does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CloseAction {
    Quit,
    /// Our copy is on the clipboard and would vanish with the process: hide
    /// the window and stay alive until something else owns the clipboard.
    HideAndHold,
}

fn close_action(owns_clipboard: bool) -> CloseAction {
    if owns_clipboard {
        CloseAction::HideAndHold
    } else {
        CloseAction::Quit
    }
}

/// What activating the app (launching it again) does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ActivateAction {
    Present,
    /// The window was closed and only hidden: start over with an empty one.
    ResetAndPresent,
}

fn activate_action(hidden_after_close: bool) -> ActivateAction {
    if hidden_after_close {
        ActivateAction::ResetAndPresent
    } else {
        ActivateAction::Present
    }
}

/// Keeps the process alive while the closed window's copy is on the
/// clipboard. Dropping it stops watching the clipboard and releases the hold.
struct Linger {
    _hold: gio::ApplicationHoldGuard,
    clipboard: gdk::Clipboard,
    handler: Option<glib::SignalHandlerId>,
}

impl Drop for Linger {
    fn drop(&mut self) {
        if let Some(handler) = self.handler.take() {
            self.clipboard.disconnect(handler);
        }
    }
}

type SharedLinger = Rc<RefCell<Option<Linger>>>;

#[derive(Debug)]
pub enum AppMsg {
    Paste,
    Pasted(Option<gdk::Texture>),
    SetTool(Tool),
    SetColor(Color),
    SetWidth(Width),
    Undo,
    Redo,
    DeleteSelected,
    /// Commit the in-place text editor, if open (Escape, canvas resize).
    TextCommit,
    /// The canvas state changed outside `update` (pointer gestures).
    Redraw,
    Copy,
    Save,
    SaveAs,
    /// Result of the save dialog; `None` when cancelled.
    SaveChosen(Option<PathBuf>),
    /// Answer to the replace question for an auto-extended path; `None` when cancelled.
    ReplaceConfirmed(Option<PathBuf>),
    /// The window's close button / Alt+F4 (from `close-request`).
    CloseRequested,
    /// Answer to the unsaved-changes dialog about `pending`.
    Confirmed(Choice),
    /// Launched again after the window was closed but kept hidden.
    Reopen,
}

impl AppModel {
    /// Refresh toolbar mirrors from the editor and repaint the canvas.
    fn sync(&mut self) {
        let st = self.state.borrow();
        let ed = &st.editor;
        self.tool = ed.tool;
        // The editor keeps `style` equal to the selected item's style.
        self.color = ed.style.color;
        self.width = ed.style.width;
        self.can_undo = ed.can_undo();
        self.can_redo = ed.can_redo();
        self.has_image = st.image.is_some();
        self.dirty = ed.is_dirty();
        drop(st);
        self.canvas.queue_draw();
    }

    /// Commit an open text editor into the document. Every action that acts on
    /// the document as a whole (tool switch, paste, undo, copy, save, close)
    /// calls this first.
    pub fn commit_text_if_open(&self) -> bool {
        self.text.commit()
    }

    fn toast(&self, text: &str) {
        self.toasts.add_toast(adw::Toast::new(text));
    }

    /// Replace the document with a new image; it has no file yet.
    fn load(&mut self, texture: &gdk::Texture) {
        let mut st = self.state.borrow_mut();
        st.editor.reset();
        st.editing = None;
        st.image = Some(surface_from_texture(texture));
        drop(st);
        self.file = None;
    }

    /// The image with all annotations at its original pixel size.
    fn flattened(&self) -> Option<gtk::cairo::ImageSurface> {
        let st = self.state.borrow();
        let img = st.image.as_ref()?;
        Some(export::flatten(img, &st.editor.items, &st.measure))
    }

    /// Copy the flattened image. Copying does not count as saving.
    fn copy(&self) {
        self.commit_text_if_open();
        if let Some(surface) = self.flattened() {
            let texture = texture_from_surface(&surface);
            self.canvas.display().clipboard().set_texture(&texture);
            self.toast("Copied");
        }
    }

    /// Save to the remembered file, or ask for one.
    fn save(&mut self, sender: &ComponentSender<Self>) {
        self.commit_text_if_open();
        match self.file.clone() {
            Some(path) => {
                let ok = self.write(&path);
                self.after_save(ok);
            }
            None => self.save_as(sender),
        }
    }

    /// Ask for a file name; the answer arrives as `SaveChosen`.
    fn save_as(&mut self, sender: &ComponentSender<Self>) {
        self.commit_text_if_open();
        if self.state.borrow().image.is_none() {
            self.pending = None;
            return;
        }
        self.busy = true;
        let dialog = files::save_dialog(self.file.as_deref());
        let (window, sender) = (self.window.clone(), sender.clone());
        relm4::spawn_local(async move {
            let path = dialog.save_future(Some(&window)).await.ok().and_then(|f| f.path());
            sender.input(AppMsg::SaveChosen(path));
        });
    }

    /// The chosen name got `.jpg` added and that file exists: ask before
    /// overwriting it; the answer arrives as `ReplaceConfirmed`.
    fn confirm_replace(&mut self, chosen: PathBuf, sender: &ComponentSender<Self>) {
        let (target, _) = export::format_for_path(&chosen);
        let name = target.file_name().unwrap_or_default().to_string_lossy().into_owned();
        self.busy = true;
        let dialog = files::replace_dialog(&name);
        let (window, sender) = (self.window.clone(), sender.clone());
        relm4::spawn_local(async move {
            let response = dialog.choose_future(Some(&window)).await;
            sender.input(AppMsg::ReplaceConfirmed((response == "replace").then_some(chosen)));
        });
    }

    /// Write the flattened image; the extension picks the format (`.jpg`
    /// added when missing). On failure the file and dirty state are kept.
    fn write(&mut self, path: &Path) -> bool {
        let Some(surface) = self.flattened() else { return false };
        let (path, format) = export::format_for_path(path);
        match export::save(&surface, &path, format) {
            Ok(()) => {
                self.state.borrow_mut().editor.mark_saved();
                self.file = Some(path);
                self.toast("Saved");
                true
            }
            Err(e) => {
                self.toast(&format!("Could not save: {e}"));
                false
            }
        }
    }

    /// Continue the pending action only if the save succeeded.
    fn after_save(&mut self, ok: bool) {
        if let Some(pending) = self.pending.take()
            && ok
        {
            self.proceed(pending);
        }
    }

    fn proceed(&mut self, pending: Pending) {
        match pending {
            Pending::NewImage(texture) => self.load(&texture),
            Pending::Close => self.close_window(),
        }
    }

    /// Show the unsaved-changes question for `pending`; the answer arrives as `Confirmed`.
    fn ask(&mut self, pending: Pending, sender: &ComponentSender<Self>) {
        let closing = matches!(pending, Pending::Close);
        let dirty = self.state.borrow().editor.is_dirty();
        self.pending = Some(pending);
        self.busy = true;
        let dialog = files::unsaved_dialog(closing, dirty);
        let (window, sender) = (self.window.clone(), sender.clone());
        relm4::spawn_local(async move {
            let response = dialog.choose_future(Some(&window)).await;
            sender.input(AppMsg::Confirmed(Choice::from_response(&response)));
        });
    }

    /// Empty document, as on a fresh start.
    fn reset(&mut self) {
        self.commit_text_if_open();
        let mut st = self.state.borrow_mut();
        st.editor.reset();
        st.editing = None;
        st.image = None;
        drop(st);
        self.file = None;
        self.pending = None;
        self.busy = false;
    }

    /// The single place where the window really closes. While our copy is on
    /// the clipboard the process must stay alive to serve it, so the window
    /// is only hidden until another app takes over the clipboard.
    fn close_window(&self) {
        let clipboard = self.canvas.display().clipboard();
        match close_action(clipboard.is_local()) {
            CloseAction::Quit => {
                self.allow_close.set(true);
                self.window.close();
            }
            CloseAction::HideAndHold => self.hide_until_clipboard_lost(clipboard),
        }
    }

    fn hide_until_clipboard_lost(&self, clipboard: gdk::Clipboard) {
        self.window.set_visible(false);
        let handler = clipboard.connect_local_notify({
            let (linger, window) = (self.linger.clone(), self.window.clone());
            move |clipboard| {
                if clipboard.is_local() {
                    return;
                }
                // Take it out first so dropping it (disconnect, release) runs
                // outside the borrow; `None` means the window was reopened.
                let lingering = linger.borrow_mut().take();
                if lingering.is_some() {
                    // The hidden window is the app's last one: this quits.
                    window.destroy();
                }
                drop(lingering);
            }
        });
        *self.linger.borrow_mut() = Some(Linger {
            _hold: relm4::main_application().hold(),
            clipboard,
            handler: Some(handler),
        });
    }
}

/// Window-level shortcuts. While a dialog is showing none apply. While a text
/// editor is open only the save shortcuts apply (they commit the text first);
/// other keys go to the editor.
fn key_message(
    key: gdk::Key,
    mods: gdk::ModifierType,
    text_open: bool,
    dialog_open: bool,
) -> Option<AppMsg> {
    if dialog_open {
        return None;
    }
    let msg = shortcut(key, mods)?;
    (!text_open || matches!(msg, AppMsg::Save | AppMsg::SaveAs)).then_some(msg)
}

fn shortcut(key: gdk::Key, mods: gdk::ModifierType) -> Option<AppMsg> {
    use gdk::ModifierType as M;
    let mods = mods & (M::CONTROL_MASK | M::SHIFT_MASK | M::ALT_MASK | M::SUPER_MASK);
    let key = key.to_lower();
    if mods == M::CONTROL_MASK | M::SHIFT_MASK {
        return (key == gdk::Key::s).then_some(AppMsg::SaveAs);
    }
    if mods == M::CONTROL_MASK {
        return match key {
            gdk::Key::c => Some(AppMsg::Copy),
            gdk::Key::s => Some(AppMsg::Save),
            gdk::Key::v => Some(AppMsg::Paste),
            gdk::Key::z => Some(AppMsg::Undo),
            gdk::Key::y => Some(AppMsg::Redo),
            _ => None,
        };
    }
    if !(mods - M::SHIFT_MASK).is_empty() {
        return None;
    }
    match key {
        gdk::Key::v => Some(AppMsg::SetTool(Tool::Select)),
        gdk::Key::r => Some(AppMsg::SetTool(Tool::Rect)),
        gdk::Key::a => Some(AppMsg::SetTool(Tool::Arrow)),
        gdk::Key::t => Some(AppMsg::SetTool(Tool::Text)),
        gdk::Key::x => Some(AppMsg::SetTool(Tool::Redact)),
        gdk::Key::n => Some(AppMsg::SetTool(Tool::Marker)),
        gdk::Key::h => Some(AppMsg::SetTool(Tool::Highlight)),
        gdk::Key::p => Some(AppMsg::SetTool(Tool::Pixelate)),
        gdk::Key::Delete | gdk::Key::KP_Delete | gdk::Key::BackSpace => {
            Some(AppMsg::DeleteSelected)
        }
        _ => None,
    }
}

#[relm4::component(pub)]
impl SimpleComponent for AppModel {
    type Init = ();
    type Input = AppMsg;
    type Output = ();

    view! {
        adw::ApplicationWindow {
            #[watch]
            set_title: Some(&files::window_title(model.file.as_deref(), model.dirty)),
            set_icon_name: Some("net.gappc.Annotshot"),
            set_default_size: (1100, 750),

            adw::ToolbarView {
                add_top_bar = &adw::HeaderBar {
                    pack_start = &gtk::Button {
                        set_icon_name: "edit-undo-symbolic",
                        set_tooltip_text: Some("Undo (Ctrl+Z)"),
                        #[watch]
                        set_sensitive: model.can_undo,
                        connect_clicked => AppMsg::Undo,
                    },
                    pack_start = &gtk::Button {
                        set_icon_name: "edit-redo-symbolic",
                        set_tooltip_text: Some("Redo (Ctrl+Y)"),
                        #[watch]
                        set_sensitive: model.can_redo,
                        connect_clicked => AppMsg::Redo,
                    },
                    pack_end = &gtk::Button {
                        set_label: "Save…",
                        add_css_class: "suggested-action",
                        set_tooltip_text: Some("Save (Ctrl+S)"),
                        #[watch]
                        set_sensitive: model.has_image,
                        connect_clicked => AppMsg::Save,
                    },
                    pack_end = &gtk::Button {
                        set_icon_name: "edit-copy-symbolic",
                        set_tooltip_text: Some("Copy to clipboard (Ctrl+C)"),
                        #[watch]
                        set_sensitive: model.has_image,
                        connect_clicked => AppMsg::Copy,
                    },
                },

                #[wrap(Some)]
                #[local_ref]
                set_content = toasts -> adw::ToastOverlay {
                    gtk::Box {
                        set_orientation: gtk::Orientation::Horizontal,

                        gtk::Box {
                            set_orientation: gtk::Orientation::Vertical,
                            set_spacing: 6,
                            set_margin_all: 6,

                            #[name = "tool_select"]
                            gtk::ToggleButton {
                                set_label: "Select",
                                set_tooltip_text: Some("Select (V)"),
                                #[watch]
                                #[block_signal(h_select)]
                                set_active: model.tool == Tool::Select,
                                connect_toggled[sender] => move |b| if b.is_active() {
                                    sender.input(AppMsg::SetTool(Tool::Select));
                                } @h_select,
                            },
                            gtk::ToggleButton {
                                set_label: "Box",
                                set_group: Some(&tool_select),
                                set_tooltip_text: Some("Box (R)"),
                                #[watch]
                                #[block_signal(h_rect)]
                                set_active: model.tool == Tool::Rect,
                                connect_toggled[sender] => move |b| if b.is_active() {
                                    sender.input(AppMsg::SetTool(Tool::Rect));
                                } @h_rect,
                            },
                            gtk::ToggleButton {
                                set_label: "Arrow",
                                set_group: Some(&tool_select),
                                set_tooltip_text: Some("Arrow (A)"),
                                #[watch]
                                #[block_signal(h_arrow)]
                                set_active: model.tool == Tool::Arrow,
                                connect_toggled[sender] => move |b| if b.is_active() {
                                    sender.input(AppMsg::SetTool(Tool::Arrow));
                                } @h_arrow,
                            },
                            gtk::ToggleButton {
                                set_label: "Text",
                                set_group: Some(&tool_select),
                                set_tooltip_text: Some("Text (T)"),
                                #[watch]
                                #[block_signal(h_text)]
                                set_active: model.tool == Tool::Text,
                                connect_toggled[sender] => move |b| if b.is_active() {
                                    sender.input(AppMsg::SetTool(Tool::Text));
                                } @h_text,
                            },
                            gtk::ToggleButton {
                                set_label: "Marker",
                                set_group: Some(&tool_select),
                                set_tooltip_text: Some("Numbered marker (N)"),
                                #[watch]
                                #[block_signal(h_marker)]
                                set_active: model.tool == Tool::Marker,
                                connect_toggled[sender] => move |b| if b.is_active() {
                                    sender.input(AppMsg::SetTool(Tool::Marker));
                                } @h_marker,
                            },
                            gtk::ToggleButton {
                                set_label: "Highlight",
                                set_group: Some(&tool_select),
                                set_tooltip_text: Some("Highlight (H)"),
                                #[watch]
                                #[block_signal(h_highlight)]
                                set_active: model.tool == Tool::Highlight,
                                connect_toggled[sender] => move |b| if b.is_active() {
                                    sender.input(AppMsg::SetTool(Tool::Highlight));
                                } @h_highlight,
                            },
                            gtk::ToggleButton {
                                set_label: "Pixelate",
                                set_group: Some(&tool_select),
                                set_tooltip_text: Some("Pixelate (P) – not safe for sensitive data, use Redact"),
                                #[watch]
                                #[block_signal(h_pixelate)]
                                set_active: model.tool == Tool::Pixelate,
                                connect_toggled[sender] => move |b| if b.is_active() {
                                    sender.input(AppMsg::SetTool(Tool::Pixelate));
                                } @h_pixelate,
                            },
                            gtk::ToggleButton {
                                set_label: "Redact",
                                set_group: Some(&tool_select),
                                set_tooltip_text: Some("Redact (X)"),
                                #[watch]
                                #[block_signal(h_redact)]
                                set_active: model.tool == Tool::Redact,
                                connect_toggled[sender] => move |b| if b.is_active() {
                                    sender.input(AppMsg::SetTool(Tool::Redact));
                                } @h_redact,
                            },

                            gtk::Separator {},

                            #[name = "color_red"]
                            gtk::ToggleButton {
                                add_css_class: "swatch",
                                add_css_class: "swatch-red",
                                set_tooltip_text: Some("Red"),
                                #[watch]
                                #[block_signal(h_red)]
                                set_active: model.color == Color::Red,
                                connect_toggled[sender] => move |b| if b.is_active() {
                                    sender.input(AppMsg::SetColor(Color::Red));
                                } @h_red,
                            },
                            gtk::ToggleButton {
                                add_css_class: "swatch",
                                add_css_class: "swatch-yellow",
                                set_group: Some(&color_red),
                                set_tooltip_text: Some("Yellow"),
                                #[watch]
                                #[block_signal(h_yellow)]
                                set_active: model.color == Color::Yellow,
                                connect_toggled[sender] => move |b| if b.is_active() {
                                    sender.input(AppMsg::SetColor(Color::Yellow));
                                } @h_yellow,
                            },
                            gtk::ToggleButton {
                                add_css_class: "swatch",
                                add_css_class: "swatch-green",
                                set_group: Some(&color_red),
                                set_tooltip_text: Some("Green"),
                                #[watch]
                                #[block_signal(h_green)]
                                set_active: model.color == Color::Green,
                                connect_toggled[sender] => move |b| if b.is_active() {
                                    sender.input(AppMsg::SetColor(Color::Green));
                                } @h_green,
                            },
                            gtk::ToggleButton {
                                add_css_class: "swatch",
                                add_css_class: "swatch-blue",
                                set_group: Some(&color_red),
                                set_tooltip_text: Some("Blue"),
                                #[watch]
                                #[block_signal(h_blue)]
                                set_active: model.color == Color::Blue,
                                connect_toggled[sender] => move |b| if b.is_active() {
                                    sender.input(AppMsg::SetColor(Color::Blue));
                                } @h_blue,
                            },
                            gtk::ToggleButton {
                                add_css_class: "swatch",
                                add_css_class: "swatch-black",
                                set_group: Some(&color_red),
                                set_tooltip_text: Some("Black"),
                                #[watch]
                                #[block_signal(h_black)]
                                set_active: model.color == Color::Black,
                                connect_toggled[sender] => move |b| if b.is_active() {
                                    sender.input(AppMsg::SetColor(Color::Black));
                                } @h_black,
                            },

                            gtk::Separator {},

                            #[name = "width_thin"]
                            gtk::ToggleButton {
                                set_label: "Thin",
                                set_tooltip_text: Some("Thin"),
                                #[watch]
                                #[block_signal(h_thin)]
                                set_active: model.width == Width::Thin,
                                connect_toggled[sender] => move |b| if b.is_active() {
                                    sender.input(AppMsg::SetWidth(Width::Thin));
                                } @h_thin,
                            },
                            gtk::ToggleButton {
                                set_label: "Medium",
                                set_group: Some(&width_thin),
                                set_tooltip_text: Some("Medium"),
                                #[watch]
                                #[block_signal(h_medium)]
                                set_active: model.width == Width::Medium,
                                connect_toggled[sender] => move |b| if b.is_active() {
                                    sender.input(AppMsg::SetWidth(Width::Medium));
                                } @h_medium,
                            },
                            gtk::ToggleButton {
                                set_label: "Thick",
                                set_group: Some(&width_thin),
                                set_tooltip_text: Some("Thick"),
                                #[watch]
                                #[block_signal(h_thick)]
                                set_active: model.width == Width::Thick,
                                connect_toggled[sender] => move |b| if b.is_active() {
                                    sender.input(AppMsg::SetWidth(Width::Thick));
                                } @h_thick,
                            },
                        },

                        #[name = "canvas_overlay"]
                        gtk::Overlay {
                            set_hexpand: true,
                            set_vexpand: true,

                            #[local_ref]
                            canvas -> gtk::DrawingArea {
                                set_hexpand: true,
                                set_vexpand: true,
                            },

                            add_overlay = &adw::StatusPage {
                                set_icon_name: Some("edit-paste-symbolic"),
                                set_title: "Press Ctrl+V to paste a screenshot",
                                set_can_target: false,
                                #[watch]
                                set_visible: !model.has_image,
                            },
                        },
                    },
                },
            },
        }
    }

    fn init(
        _init: Self::Init,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        relm4::set_global_css(CSS);
        let state: SharedCanvas = Rc::new(RefCell::new(CanvasState::new()));
        let canvas = gtk::DrawingArea::new();
        let toasts = adw::ToastOverlay::new();
        let text = Rc::new(TextEditor::new(state.clone(), {
            let sender = sender.clone();
            move || sender.input(AppMsg::TextCommit)
        }));
        // The editor is placed for the current view; a resize changes the view,
        // so commit rather than reposition (deferred: not during allocation).
        canvas.connect_resize({
            let (text, sender) = (text.clone(), sender.clone());
            move |_, _, _| {
                if text.is_open() {
                    sender.input(AppMsg::TextCommit);
                }
            }
        });
        canvas::attach(&canvas, state.clone(), text.clone(), {
            let sender = sender.clone();
            move || sender.input(AppMsg::Redraw)
        });

        let keys = gtk::EventControllerKey::new();
        keys.connect_key_pressed({
            let (state, sender, window) = (state.clone(), sender.clone(), root.downgrade());
            move |_, key, _, mods| {
                // Keys must not change the document under an alert dialog.
                let dialog_open =
                    window.upgrade().is_some_and(|w| w.visible_dialog().is_some());
                match key_message(key, mods, state.borrow().text_open, dialog_open) {
                    Some(msg) => {
                        sender.input(msg);
                        glib::Propagation::Stop
                    }
                    None => glib::Propagation::Proceed,
                }
            }
        });
        root.add_controller(keys);

        // Every close goes through `CloseRequested`, which asks about unsaved
        // changes and then closes for real via `close_window`.
        let allow_close = Rc::new(Cell::new(false));
        root.connect_close_request({
            let (allow_close, sender) = (allow_close.clone(), sender.clone());
            move |_| {
                if allow_close.get() {
                    return glib::Propagation::Proceed;
                }
                sender.input(AppMsg::CloseRequested);
                glib::Propagation::Stop
            }
        });

        // Single instance: launching again activates this process. The window
        // is shown here (relm4's own showing is off, see main.rs) so a window
        // that was closed and hidden can be emptied before it reappears.
        let linger: SharedLinger = Rc::default();
        relm4::main_application().connect_activate({
            let (linger, window, sender) = (linger.clone(), root.clone(), sender.clone());
            move |_| {
                // Reopened: stop waiting for the clipboard; the next close decides anew.
                let hidden = linger.borrow_mut().take().is_some();
                match activate_action(hidden) {
                    ActivateAction::Present => window.present(),
                    ActivateAction::ResetAndPresent => sender.input(AppMsg::Reopen),
                }
            }
        });

        let mut model = AppModel {
            state,
            canvas,
            text,
            toasts,
            window: root.clone(),
            file: None,
            pending: None,
            busy: false,
            allow_close,
            linger,
            tool: Tool::default(),
            color: Color::default(),
            width: Width::default(),
            can_undo: false,
            can_redo: false,
            has_image: false,
            dirty: false,
        };
        model.sync();
        let (canvas, toasts) = (&model.canvas, &model.toasts);
        let widgets = view_output!();
        widgets.canvas_overlay.add_overlay(model.text.widget());
        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: Self::Input, sender: ComponentSender<Self>) {
        match msg {
            AppMsg::Paste => {
                let clipboard = self.canvas.display().clipboard();
                relm4::spawn_local(async move {
                    let texture = clipboard.read_texture_future().await.ok().flatten();
                    sender.input(AppMsg::Pasted(texture));
                });
            }
            AppMsg::Pasted(Some(texture)) => {
                self.commit_text_if_open();
                if self.busy {
                    // A dialog is already deciding about the current image.
                } else if self.state.borrow().image.is_none() {
                    self.load(&texture);
                } else {
                    self.ask(Pending::NewImage(texture), &sender);
                }
            }
            AppMsg::Pasted(None) => self.toast("Clipboard does not contain an image"),
            AppMsg::SetTool(tool) => {
                self.commit_text_if_open();
                self.state.borrow_mut().editor.set_tool(tool);
            }
            AppMsg::SetColor(color) => {
                // An edited existing label is selected, so this recolors it;
                // the open editor follows the new color either way.
                self.state.borrow_mut().editor.set_color(color);
                if self.text.is_open() {
                    self.text.set_color(color);
                }
            }
            AppMsg::SetWidth(width) => {
                self.state.borrow_mut().editor.set_width(width);
                if self.text.is_open() {
                    // Labels have no width; just keep typing.
                    self.text.widget().grab_focus();
                }
            }
            AppMsg::Undo => {
                self.commit_text_if_open();
                self.state.borrow_mut().editor.undo();
            }
            AppMsg::Redo => {
                self.commit_text_if_open();
                self.state.borrow_mut().editor.redo();
            }
            AppMsg::DeleteSelected => {
                self.commit_text_if_open();
                self.state.borrow_mut().editor.delete_selected();
            }
            AppMsg::TextCommit => {
                self.commit_text_if_open();
            }
            AppMsg::Redraw => {}
            AppMsg::Copy => self.copy(),
            AppMsg::Save if !self.busy => self.save(&sender),
            AppMsg::SaveAs if !self.busy => self.save_as(&sender),
            AppMsg::Save | AppMsg::SaveAs => {}
            AppMsg::SaveChosen(Some(path)) if files::would_replace_other_file(&path) => {
                self.busy = false;
                self.confirm_replace(path, &sender);
            }
            AppMsg::SaveChosen(path) | AppMsg::ReplaceConfirmed(path) => {
                self.busy = false;
                let ok = path.is_some_and(|p| self.write(&p));
                self.after_save(ok);
            }
            AppMsg::CloseRequested if !self.busy => {
                self.commit_text_if_open();
                if self.state.borrow().editor.is_dirty() {
                    self.ask(Pending::Close, &sender);
                } else {
                    self.close_window();
                }
            }
            AppMsg::CloseRequested => {}
            AppMsg::Confirmed(choice) => {
                self.busy = false;
                match choice {
                    Choice::Cancel => self.pending = None,
                    Choice::Discard => {
                        if let Some(pending) = self.pending.take() {
                            self.proceed(pending);
                        }
                    }
                    Choice::Save => self.save(&sender),
                }
            }
            AppMsg::Reopen => {
                self.reset();
                self.window.present();
            }
        }
        self.sync();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gdk::ModifierType as M;

    fn tool_of(key: gdk::Key, mods: M) -> Option<Tool> {
        match shortcut(key, mods) {
            Some(AppMsg::SetTool(t)) => Some(t),
            _ => None,
        }
    }

    #[test]
    fn single_letters_pick_tools() {
        assert_eq!(tool_of(gdk::Key::v, M::empty()), Some(Tool::Select));
        assert_eq!(tool_of(gdk::Key::R, M::SHIFT_MASK), Some(Tool::Rect));
        assert_eq!(tool_of(gdk::Key::a, M::empty()), Some(Tool::Arrow));
        assert_eq!(tool_of(gdk::Key::t, M::empty()), Some(Tool::Text));
        assert_eq!(tool_of(gdk::Key::x, M::empty()), Some(Tool::Redact));
        assert_eq!(tool_of(gdk::Key::n, M::empty()), Some(Tool::Marker));
        assert_eq!(tool_of(gdk::Key::h, M::empty()), Some(Tool::Highlight));
        assert_eq!(tool_of(gdk::Key::p, M::empty()), Some(Tool::Pixelate));
        assert_eq!(tool_of(gdk::Key::x, M::ALT_MASK), None);
    }

    #[test]
    fn ctrl_combos() {
        assert!(matches!(shortcut(gdk::Key::v, M::CONTROL_MASK), Some(AppMsg::Paste)));
        assert!(matches!(shortcut(gdk::Key::z, M::CONTROL_MASK), Some(AppMsg::Undo)));
        assert!(matches!(shortcut(gdk::Key::y, M::CONTROL_MASK), Some(AppMsg::Redo)));
        assert!(matches!(shortcut(gdk::Key::c, M::CONTROL_MASK), Some(AppMsg::Copy)));
        assert!(matches!(shortcut(gdk::Key::s, M::CONTROL_MASK), Some(AppMsg::Save)));
        assert!(matches!(
            shortcut(gdk::Key::S, M::CONTROL_MASK | M::SHIFT_MASK),
            Some(AppMsg::SaveAs)
        ));
        // Ctrl+Shift+Z is not a binding.
        assert!(shortcut(gdk::Key::Z, M::CONTROL_MASK | M::SHIFT_MASK).is_none());
    }

    #[test]
    fn delete_keys() {
        for k in [gdk::Key::Delete, gdk::Key::BackSpace] {
            assert!(matches!(shortcut(k, M::empty()), Some(AppMsg::DeleteSelected)));
        }
    }

    #[test]
    fn close_hides_only_while_owning_clipboard() {
        assert_eq!(close_action(false), CloseAction::Quit);
        assert_eq!(close_action(true), CloseAction::HideAndHold);
    }

    #[test]
    fn activate_resets_only_a_hidden_window() {
        assert_eq!(activate_action(false), ActivateAction::Present);
        assert_eq!(activate_action(true), ActivateAction::ResetAndPresent);
    }

    #[test]
    fn open_text_editor_only_lets_save_shortcuts_through() {
        let ctrl = M::CONTROL_MASK;
        assert!(matches!(key_message(gdk::Key::s, ctrl, true, false), Some(AppMsg::Save)));
        assert!(matches!(
            key_message(gdk::Key::S, ctrl | M::SHIFT_MASK, true, false),
            Some(AppMsg::SaveAs)
        ));
        for (k, m) in [
            (gdk::Key::c, ctrl),
            (gdk::Key::v, ctrl),
            (gdk::Key::z, ctrl),
            (gdk::Key::y, ctrl),
            (gdk::Key::r, M::empty()),
            (gdk::Key::Delete, M::empty()),
        ] {
            assert!(key_message(k, m, true, false).is_none(), "{k:?} must reach the text editor");
        }
        assert!(matches!(key_message(gdk::Key::c, ctrl, false, false), Some(AppMsg::Copy)));
    }

    #[test]
    fn open_dialog_blocks_all_shortcuts() {
        let ctrl = M::CONTROL_MASK;
        for (k, m) in [
            (gdk::Key::Delete, M::empty()),
            (gdk::Key::BackSpace, M::empty()),
            (gdk::Key::z, ctrl),
            (gdk::Key::y, ctrl),
            (gdk::Key::v, ctrl),
            (gdk::Key::c, ctrl),
            (gdk::Key::s, ctrl),
            (gdk::Key::S, ctrl | M::SHIFT_MASK),
            (gdk::Key::r, M::empty()),
        ] {
            for text_open in [false, true] {
                assert!(key_message(k, m, text_open, true).is_none(), "{k:?} acts under a dialog");
            }
        }
    }
}
