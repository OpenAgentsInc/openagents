//! Export a transcript table without making provider or tool calls.

use coder_new::{App, Mode, live::Entry, snapshot};

fn main() {
    let mut app = App::default();
    app.set_mode(Mode::Live);
    app.plugins.enabled = true;
    app.plugins.key_configured = true;
    app.plugins.connection = coder_new::plugins::Connection::Verified;
    app.plugins.model = "openai/gpt-6-luna".into();
    app.plugins.options.reasoning = Some("low".into());
    app.live.entries.extend([
        Entry::User("Explain the local coding loop.".into()),
        Entry::Assistant {
            text: include_str!("../tests/fixtures/microcoder-table.md").into(),
            model: Some("openai/gpt-6-luna:low".into()),
        },
    ]);
    print!("{}", snapshot::svg(&mut app, 110, 46));
}
