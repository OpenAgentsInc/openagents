//! Print the Kiki and Verse token stylesheet, or write it into the
//! repository with `--write`.

use std::path::Path;

fn main() -> std::io::Result<()> {
    let css = oa_tokens::stylesheet::css_text();
    if std::env::args().any(|arg| arg == "--write") {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join(oa_tokens::stylesheet::STYLESHEET_PATH);
        if std::fs::read_to_string(&path).ok().as_deref() != Some(css.as_str()) {
            std::fs::write(&path, css)?;
            println!("wrote {}", oa_tokens::stylesheet::STYLESHEET_PATH);
        }
    } else {
        print!("{css}");
    }
    Ok(())
}
