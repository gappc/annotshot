use std::cell::RefCell;
use std::rc::Rc;

use adw::prelude::*;
use gtk::{gdk, glib};
use relm4::prelude::*;

use crate::canvas::{self, CanvasState, SharedCanvas};
use crate::editor::Tool;
use crate::export::surface_from_texture;
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
    // Mirrors of the canvas state that the toolbar reflects; refreshed by `sync`.
    tool: Tool,
    color: Color,
    width: Width,
    can_undo: bool,
    can_redo: bool,
    has_image: bool,
}

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
    /// Commit the in-place text editor, if open (Escape, canvas resize; Task 8
    /// sends it before copy/save/close or calls `commit_text_if_open`).
    TextCommit,
    /// The canvas state changed outside `update` (pointer gestures).
    Redraw,
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
}

/// Window-level shortcuts. Keys pass through while a text editor is open.
fn key_message(key: gdk::Key, mods: gdk::ModifierType) -> Option<AppMsg> {
    use gdk::ModifierType as M;
    let mods = mods & (M::CONTROL_MASK | M::SHIFT_MASK | M::ALT_MASK | M::SUPER_MASK);
    let key = key.to_lower();
    if mods == M::CONTROL_MASK {
        return match key {
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
            set_title: Some("Annotshot"),
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
                    },
                    pack_end = &gtk::Button {
                        set_icon_name: "edit-copy-symbolic",
                        set_tooltip_text: Some("Copy to clipboard (Ctrl+C)"),
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
            let (state, sender) = (state.clone(), sender.clone());
            move |_, key, _, mods| {
                if state.borrow().text_open {
                    return glib::Propagation::Proceed;
                }
                match key_message(key, mods) {
                    Some(msg) => {
                        sender.input(msg);
                        glib::Propagation::Stop
                    }
                    None => glib::Propagation::Proceed,
                }
            }
        });
        root.add_controller(keys);

        let mut model = AppModel {
            state,
            canvas,
            text,
            toasts,
            tool: Tool::default(),
            color: Color::default(),
            width: Width::default(),
            can_undo: false,
            can_redo: false,
            has_image: false,
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
                let mut st = self.state.borrow_mut();
                st.editor.reset();
                st.editing = None;
                st.image = Some(surface_from_texture(&texture));
            }
            AppMsg::Pasted(None) => self.toast("Clipboard does not contain an image"),
            AppMsg::SetTool(tool) => {
                self.commit_text_if_open();
                self.state.borrow_mut().editor.tool = tool;
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
        }
        self.sync();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gdk::ModifierType as M;

    fn tool_of(key: gdk::Key, mods: M) -> Option<Tool> {
        match key_message(key, mods) {
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
        assert_eq!(tool_of(gdk::Key::x, M::ALT_MASK), None);
    }

    #[test]
    fn ctrl_combos() {
        assert!(matches!(key_message(gdk::Key::v, M::CONTROL_MASK), Some(AppMsg::Paste)));
        assert!(matches!(key_message(gdk::Key::z, M::CONTROL_MASK), Some(AppMsg::Undo)));
        assert!(matches!(key_message(gdk::Key::y, M::CONTROL_MASK), Some(AppMsg::Redo)));
        // Ctrl+C / Ctrl+S belong to later tasks; Ctrl+Shift+Z is not a binding.
        assert!(key_message(gdk::Key::c, M::CONTROL_MASK).is_none());
        assert!(key_message(gdk::Key::Z, M::CONTROL_MASK | M::SHIFT_MASK).is_none());
    }

    #[test]
    fn delete_keys() {
        for k in [gdk::Key::Delete, gdk::Key::BackSpace] {
            assert!(matches!(key_message(k, M::empty()), Some(AppMsg::DeleteSelected)));
        }
    }
}
