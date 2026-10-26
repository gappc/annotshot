use adw::prelude::*;
use relm4::prelude::*;

const CSS: &str = "
button.swatch { min-width: 24px; min-height: 24px; padding: 0; border-radius: 12px; }
button.swatch-red { background: #e01b24; }
button.swatch-yellow { background: #f6d32d; }
button.swatch-green { background: #33d17a; }
button.swatch-blue { background: #3584e4; }
button.swatch-black { background: #000000; }
button.swatch:checked { border: 2px solid @window_fg_color; }
";

pub struct AppModel;

#[derive(Debug)]
pub enum AppMsg {}

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
                    },
                    pack_start = &gtk::Button {
                        set_icon_name: "edit-redo-symbolic",
                        set_tooltip_text: Some("Redo (Ctrl+Y)"),
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
                #[name = "toast_overlay"]
                set_content = &adw::ToastOverlay {
                    gtk::Box {
                        set_orientation: gtk::Orientation::Horizontal,

                        gtk::Box {
                            set_orientation: gtk::Orientation::Vertical,
                            set_spacing: 6,
                            set_margin_all: 6,

                            #[name = "tool_select"]
                            gtk::ToggleButton {
                                set_label: "Select",
                                set_active: true,
                                set_tooltip_text: Some("Select (V)"),
                            },
                            gtk::ToggleButton {
                                set_label: "Box",
                                set_group: Some(&tool_select),
                                set_tooltip_text: Some("Box (R)"),
                            },
                            gtk::ToggleButton {
                                set_label: "Arrow",
                                set_group: Some(&tool_select),
                                set_tooltip_text: Some("Arrow (A)"),
                            },
                            gtk::ToggleButton {
                                set_label: "Text",
                                set_group: Some(&tool_select),
                                set_tooltip_text: Some("Text (T)"),
                            },
                            gtk::ToggleButton {
                                set_label: "Redact",
                                set_group: Some(&tool_select),
                                set_tooltip_text: Some("Redact (X)"),
                            },

                            gtk::Separator {},

                            #[name = "color_red"]
                            gtk::ToggleButton {
                                add_css_class: "swatch",
                                add_css_class: "swatch-red",
                                set_active: true,
                                set_tooltip_text: Some("Red (1)"),
                            },
                            gtk::ToggleButton {
                                add_css_class: "swatch",
                                add_css_class: "swatch-yellow",
                                set_group: Some(&color_red),
                                set_tooltip_text: Some("Yellow (2)"),
                            },
                            gtk::ToggleButton {
                                add_css_class: "swatch",
                                add_css_class: "swatch-green",
                                set_group: Some(&color_red),
                                set_tooltip_text: Some("Green (3)"),
                            },
                            gtk::ToggleButton {
                                add_css_class: "swatch",
                                add_css_class: "swatch-blue",
                                set_group: Some(&color_red),
                                set_tooltip_text: Some("Blue (4)"),
                            },
                            gtk::ToggleButton {
                                add_css_class: "swatch",
                                add_css_class: "swatch-black",
                                set_group: Some(&color_red),
                                set_tooltip_text: Some("Black (5)"),
                            },

                            gtk::Separator {},

                            #[name = "width_thin"]
                            gtk::ToggleButton {
                                set_label: "Thin",
                                set_tooltip_text: Some("Thin ([)"),
                            },
                            gtk::ToggleButton {
                                set_label: "Medium",
                                set_active: true,
                                set_group: Some(&width_thin),
                                set_tooltip_text: Some("Medium"),
                            },
                            gtk::ToggleButton {
                                set_label: "Thick",
                                set_group: Some(&width_thin),
                                set_tooltip_text: Some("Thick (])"),
                            },
                        },

                        gtk::Overlay {
                            set_hexpand: true,
                            set_vexpand: true,

                            #[name = "canvas"]
                            gtk::DrawingArea {
                                set_hexpand: true,
                                set_vexpand: true,
                            },

                            add_overlay = &adw::StatusPage {
                                set_icon_name: Some("edit-paste-symbolic"),
                                set_title: "Press Ctrl+V to paste a screenshot",
                                set_can_target: false,
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
        _sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        relm4::set_global_css(CSS);
        let model = AppModel;
        let widgets = view_output!();
        ComponentParts { model, widgets }
    }
}
