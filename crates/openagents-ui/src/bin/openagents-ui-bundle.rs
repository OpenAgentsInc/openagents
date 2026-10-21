//! `openagents-ui-bundle OUTDIR [CRATE_DIR]`: writes `openagents-ui.css` and
//! `openagents-ui.js` from this crate's `static/` directory, the same bytes
//! the library compiles in, for a deploy that ships them as content.

use std::path::PathBuf;

fn main() {
    let Some(out) = std::env::args_os().nth(1).map(PathBuf::from) else {
        eprintln!("usage: openagents-ui-bundle OUTDIR [CRATE_DIR]");
        std::process::exit(64);
    };
    let root = std::env::args_os()
        .nth(2)
        .map_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")), PathBuf::from);
    let bundle = openagents_ui::bundle::bundle(&root);
    if !bundle.missing.is_empty() {
        eprintln!(
            "openagents-ui-bundle: missing {}",
            bundle.missing.join(", ")
        );
        std::process::exit(1);
    }
    let write = |name: &str, text: &str| {
        std::fs::create_dir_all(&out)
            .and_then(|()| std::fs::write(out.join(name), text))
            .unwrap_or_else(|error| {
                eprintln!("openagents-ui-bundle: writing {name}: {error}");
                std::process::exit(1);
            });
    };
    write("openagents-ui.css", &bundle.css);
    write("openagents-ui.js", &bundle.js);
}
