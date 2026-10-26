mod app;
mod canvas;
mod editor;
mod export;
mod files;
mod geometry;
mod history;
mod model;
mod render;
mod text_edit;
mod view;

use app::AppModel;
use relm4::RelmApp;

fn main() {
    // The app shows its window itself on activate (see `AppModel::init`).
    RelmApp::new("net.gappc.Annotshot")
        .visible_on_activate(false)
        .run::<AppModel>(());
}
