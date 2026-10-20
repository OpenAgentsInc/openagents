//! Draw a fixture transcript offscreen in both themes and save PNGs. Moving
//! transcript code must leave them unchanged. Nothing is shown on screen: a
//! headless context renders with Metal into an image.
//!
//! cargo run -p openagents-ui --example transcript-capture --features account-fixture -- DIR
use std::{path::PathBuf, sync::Arc};

fn main() -> anyhow::Result<()> {
    // A failed check exits instead of aborting, so no crash report opens.
    openagents_gpui_test::exit_on_panic();
    let output = PathBuf::from(
        std::env::args_os()
            .nth(1)
            .expect("pass the folder to write the PNGs to"),
    );
    std::fs::create_dir_all(&output)?;
    let data = tempfile::tempdir()?;
    let mut cx = openagents_gpui_test::headless_app(Arc::new(openagents_ui::icons::Assets));
    openagents_ui::transcript_capture::capture(&mut cx, data.path(), &output)?;
    println!("Wrote {}", output.display());
    Ok(())
}
