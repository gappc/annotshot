mod app;
#[allow(dead_code)] // consumed by later tasks
mod geometry;
#[allow(dead_code)] // consumed by later tasks
mod model;

use app::AppModel;
use relm4::RelmApp;

fn main() {
    RelmApp::new("net.gappc.Annotshot").run::<AppModel>(());
}
