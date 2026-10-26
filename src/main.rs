mod app;
mod canvas;
#[allow(dead_code)] // consumed by later tasks
mod editor;
#[allow(dead_code)] // consumed by later tasks
mod export;
mod geometry;
mod history;
#[allow(dead_code)] // consumed by later tasks
mod model;
mod render;
mod text_edit;
mod view;

use app::AppModel;
use relm4::RelmApp;

fn main() {
    RelmApp::new("net.gappc.Annotshot").run::<AppModel>(());
}
