//! Export bundled plugin screens without credentials, files, or network work.

use coder_new::{App, Mode, plugin_definition::DEFINITIONS, snapshot};

fn main() {
    let mut app = App::default();
    app.set_mode(Mode::Live);
    app.open_plugins();
    if let Some(id) = std::env::args().nth(1) {
        app.plugins.selected = DEFINITIONS
            .iter()
            .position(|plugin| plugin.id == id)
            .expect("Use a bundled plugin ID.");
        app.open_plugin_settings();
    }
    print!("{}", snapshot::svg(&mut app, 110, 36));
}
