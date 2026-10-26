//! Save and unsaved-changes dialogs, plus the pure helpers behind them.

use std::path::Path;

use adw::prelude::*;
use gtk::{gio, glib};

pub const APP_NAME: &str = "Annotshot";

/// Window title: app name, then `• <file name>` once saved, `*` prefix when dirty.
pub fn window_title(file: Option<&Path>, dirty: bool) -> String {
    let mut title = String::new();
    if dirty {
        title.push('*');
    }
    title.push_str(APP_NAME);
    if let Some(name) = file.and_then(Path::file_name) {
        title.push_str(" • ");
        title.push_str(&name.to_string_lossy());
    }
    title
}

/// `screenshot-YYYY-MM-DD-HHMMSS.jpg` for the given time.
pub fn default_file_name(t: &glib::DateTime) -> String {
    t.format("screenshot-%Y-%m-%d-%H%M%S.jpg")
        .map(String::from)
        .unwrap_or_else(|_| "screenshot.jpg".to_owned())
}

/// Save dialog with JPEG (default) and PNG filters, a timestamped name, and
/// the folder of the current file, if any.
pub fn save_dialog(current: Option<&Path>) -> gtk::FileDialog {
    let jpeg = gtk::FileFilter::new();
    jpeg.set_name(Some("JPEG image"));
    jpeg.add_suffix("jpg");
    jpeg.add_suffix("jpeg");
    let png = gtk::FileFilter::new();
    png.set_name(Some("PNG image"));
    png.add_suffix("png");
    let filters = gio::ListStore::new::<gtk::FileFilter>();
    filters.append(&jpeg);
    filters.append(&png);

    let dialog = gtk::FileDialog::new();
    dialog.set_title("Save Image");
    dialog.set_filters(Some(&filters));
    dialog.set_default_filter(Some(&jpeg));
    let name = glib::DateTime::now_local()
        .map(|t| default_file_name(&t))
        .unwrap_or_else(|_| "screenshot.jpg".to_owned());
    dialog.set_initial_name(Some(&name));
    if let Some(dir) = current.and_then(Path::parent) {
        dialog.set_initial_folder(Some(&gio::File::for_path(dir)));
    }
    dialog
}

/// What the user chose in an unsaved-changes dialog.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Choice {
    Cancel,
    /// Discard changes (or "New Image" when there were none).
    Discard,
    /// Save first, then continue.
    Save,
}

impl Choice {
    pub fn from_response(id: &str) -> Self {
        match id {
            "save" => Choice::Save,
            "discard" | "new" => Choice::Discard,
            _ => Choice::Cancel,
        }
    }
}

const UNSAVED_BODY: &str = "Your changes to the current image have not been saved.";

/// Question asked before replacing the current image (`closing == false`) or
/// closing the window. Without unsaved changes only Cancel / New Image are offered.
pub fn unsaved_dialog(closing: bool, dirty: bool) -> adw::AlertDialog {
    let heading = if closing { "Save changes before closing?" } else { "Start a new image?" };
    let dialog = adw::AlertDialog::new(Some(heading), dirty.then_some(UNSAVED_BODY));
    if dirty {
        dialog.add_responses(&[("cancel", "Cancel"), ("discard", "Discard"), ("save", "Save")]);
        dialog.set_response_appearance("discard", adw::ResponseAppearance::Destructive);
        dialog.set_response_appearance("save", adw::ResponseAppearance::Suggested);
        dialog.set_default_response(Some("save"));
    } else {
        dialog.add_responses(&[("cancel", "Cancel"), ("new", "New Image")]);
        dialog.set_response_appearance("new", adw::ResponseAppearance::Suggested);
        dialog.set_default_response(Some("new"));
    }
    dialog.set_close_response("cancel");
    dialog
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn title_reflects_file_and_dirty_state() {
        let f = PathBuf::from("/tmp/shots/name.jpg");
        assert_eq!(window_title(None, false), "Annotshot");
        assert_eq!(window_title(None, true), "*Annotshot");
        assert_eq!(window_title(Some(&f), false), "Annotshot • name.jpg");
        assert_eq!(window_title(Some(&f), true), "*Annotshot • name.jpg");
    }

    #[test]
    fn default_name_is_timestamped_jpeg() {
        let t = glib::DateTime::new(&glib::TimeZone::utc(), 2026, 3, 4, 5, 6, 7.0).unwrap();
        assert_eq!(default_file_name(&t), "screenshot-2026-03-04-050607.jpg");
    }

    #[test]
    fn responses_map_to_choices() {
        assert_eq!(Choice::from_response("save"), Choice::Save);
        assert_eq!(Choice::from_response("discard"), Choice::Discard);
        assert_eq!(Choice::from_response("new"), Choice::Discard);
        assert_eq!(Choice::from_response("cancel"), Choice::Cancel);
        assert_eq!(Choice::from_response(""), Choice::Cancel);
    }
}
