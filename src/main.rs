mod app;
#[allow(dead_code)] // consumed by later tasks
mod editor;
#[allow(dead_code)] // consumed by later tasks
mod export;
#[allow(dead_code)] // consumed by later tasks
mod geometry;
#[allow(dead_code)] // consumed by later tasks
mod history;
#[allow(dead_code)] // consumed by later tasks
mod model;
#[allow(dead_code)] // consumed by later tasks
mod render;

use app::AppModel;
use relm4::RelmApp;

fn main() {
    RelmApp::new("net.gappc.Annotshot").run::<AppModel>(());
}
