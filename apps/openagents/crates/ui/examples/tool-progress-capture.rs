//! Draw a chat's working row while it waits on another chat, offscreen, and
//! save a PNG (#11969). Nothing is shown on screen: a headless context renders
//! with Metal into an image.
//!
//! cargo run -p openagents-ui --example tool-progress-capture --features account-fixture -- OUT.png
use std::{path::PathBuf, sync::Arc};

fn main() -> anyhow::Result<()> {
    // A failed check exits instead of aborting, so no crash report opens.
    let report = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        report(info);
        std::process::exit(101);
    }));
    let output = PathBuf::from(
        std::env::args_os()
            .nth(1)
            .expect("pass the PNG path to write"),
    );
    let data = tempfile::tempdir()?;
    let text = gpui_platform::current_platform(true).text_system();
    let mut cx = gpui::HeadlessAppContext::with_platform(
        text,
        Arc::new(openagents_ui::icons::Assets),
        gpui_platform::current_headless_renderer,
    );
    openagents_ui::tool_progress::capture(&mut cx, data.path(), &output)?;
    println!("Wrote {}", output.display());
    Ok(())
}
