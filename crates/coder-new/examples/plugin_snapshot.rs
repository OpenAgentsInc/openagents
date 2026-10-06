//! Export bundled plugin screens using temporary executable fixtures.

use coder_new::{App, Mode, plugin_definition::DEFINITIONS, snapshot};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

fn main() {
    let mut app = App::default();
    app.set_mode(Mode::Live);
    app.open_plugins();
    if let Some(id) = std::env::args().nth(1) {
        let temporary = tempfile::tempdir().expect("Create preview fixture directory.");
        if id == "acp-subagents" {
            for name in ["grok", "devin", "opencode", "cursor-agent", "omp"] {
                let program = temporary.path().join(if cfg!(windows) {
                    format!("{name}.exe")
                } else {
                    name.into()
                });
                std::fs::write(&program, "Preview fixture; never executed.")
                    .expect("Write preview fixture.");
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o700))
                        .expect("Set fixture permissions.");
                }
            }
            app.plugins.bundled.discover_acp(&|name| match name {
                "PATH" | "HOME" => Some(temporary.path().as_os_str().to_owned()),
                _ => None,
            });
        }
        app.plugins.selected = DEFINITIONS
            .iter()
            .position(|plugin| plugin.id == id)
            .expect("Use a bundled plugin ID.");
        app.open_plugin_settings();
        if id == "jev" && std::env::args().nth(2).as_deref() == Some("vercel") {
            app.plugins.bundled.focus = coder_new::plugins::SettingsFocus::Gateway;
            app.plugins
                .bundled
                .handle(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
            app.plugins.bundled.focus = coder_new::plugins::SettingsFocus::ApiKey;
        }
    }
    print!("{}", snapshot::svg(&mut app, 110, 36));
}
