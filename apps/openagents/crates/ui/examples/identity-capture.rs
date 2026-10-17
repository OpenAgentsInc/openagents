//! Draw the desktop identity offscreen and save PNGs (#11997): the welcome,
//! Restore from 12 words, the sidebar row and its menu, and Back up your
//! account. Nothing is shown on screen: a headless context renders with
//! Metal into images.
//!
//! cargo run -p openagents-ui --example identity-capture --features account-fixture -- OUT_DIR
use std::{path::PathBuf, sync::Arc};

fn main() -> anyhow::Result<()> {
    // A failed check exits instead of aborting, so no crash report opens.
    openagents_gpui_test::exit_on_panic();
    let output = PathBuf::from(
        std::env::args_os()
            .nth(1)
            .expect("pass the directory to write PNGs into"),
    );
    let data = tempfile::tempdir()?;
    let mut cx = openagents_gpui_test::headless_app(Arc::new(openagents_ui::icons::Assets));
    openagents_ui::openagents_account::capture_identity(&mut cx, data.path(), &output)?;
    println!("Wrote {}", output.display());
    Ok(())
}
