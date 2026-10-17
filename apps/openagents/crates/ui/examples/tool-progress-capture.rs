//! Draw a chat's working row while it waits on another chat, offscreen, and
//! save a PNG (#11969). Nothing is shown on screen: a headless context renders
//! with Metal into an image.
//!
//! cargo run -p openagents-ui --example tool-progress-capture --features account-fixture -- OUT.png
use std::{path::PathBuf, sync::Arc};

fn main() -> anyhow::Result<()> {
    // A failed check exits instead of aborting, so no crash report opens.
    openagents_gpui_test::exit_on_panic();
    let output = PathBuf::from(
        std::env::args_os()
            .nth(1)
            .expect("pass the PNG path to write"),
    );
    let data = tempfile::tempdir()?;
    let mut cx = openagents_gpui_test::headless_app(Arc::new(openagents_ui::icons::Assets));
    openagents_ui::tool_progress::capture(&mut cx, data.path(), &output)?;
    println!("Wrote {}", output.display());
    Ok(())
}
