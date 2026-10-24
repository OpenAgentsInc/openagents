//! Print the JSON Schema, or write the schema and TypeScript types into the
//! repository with `--write`.

use aethyr_ui::codegen;
use std::path::Path;

fn main() -> std::io::Result<()> {
    if std::env::args().any(|arg| arg == "--write") {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        for (path, text) in [
            (codegen::SCHEMA_PATH, codegen::schema_text()),
            (codegen::TYPESCRIPT_PATH, codegen::typescript()),
        ] {
            let file = root.join(path);
            if std::fs::read_to_string(&file).ok().as_deref() != Some(text.as_str()) {
                if let Some(parent) = file.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                std::fs::write(&file, text)?;
                println!("wrote {path}");
            }
        }
    } else {
        print!("{}", codegen::schema_text());
    }
    Ok(())
}
