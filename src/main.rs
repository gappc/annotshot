mod app;

use app::AppModel;
use relm4::RelmApp;

fn main() {
    RelmApp::new("net.gappc.Annotshot").run::<AppModel>(());
}
