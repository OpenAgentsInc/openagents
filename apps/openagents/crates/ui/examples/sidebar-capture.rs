//! Draw the desktop sidebar's one chat list offscreen and save a PNG (#11960).
//! Nothing is shown on screen: a headless context renders with Metal into an
//! image.
//!
//! cargo run -p openagents-ui --example sidebar-capture --features account-fixture -- OUT.png
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
    openagents_ui::shell::capture_sidebar(&mut cx, data.path(), &output)?;
    println!("Wrote {}", output.display());
    Ok(())
}
