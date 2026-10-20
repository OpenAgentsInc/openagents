//! Rasterize the fixed chat icons on the build machine, without shipping an SVG parser.

use std::{env, fs, path::PathBuf};

fn main() {
    let source = PathBuf::from("../../crates/ui/assets/icons");
    let out = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    println!("cargo:rerun-if-changed={}", source.display());
    let mut files: Vec<_> = fs::read_dir(&source)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .collect();
    files.sort();
    println!("cargo:rerun-if-changed=src/assets.rs");
    let assets = fs::read_to_string("src/assets.rs").unwrap();
    let mut generated =
        String::from("fn mask(path: &str) -> Option<&'static [u8]> { match path {\n");
    for path in files {
        if path.extension().and_then(|s| s.to_str()) != Some("svg") {
            continue;
        }
        let name = path.file_name().unwrap().to_str().unwrap();
        // Only icons embedded by the web view belong in its module.
        if !assets.contains(&format!("\"icons/{name}\"")) {
            continue;
        }
        let tree = resvg::usvg::Tree::from_data(
            &fs::read(&path).unwrap(),
            &resvg::usvg::Options::default(),
        )
        .unwrap();
        let mut pixels = resvg::tiny_skia::Pixmap::new(64, 64).unwrap();
        let scale = resvg::tiny_skia::Transform::from_scale(
            64. / tree.size().width(),
            64. / tree.size().height(),
        );
        resvg::render(&tree, scale, &mut pixels.as_mut());
        let alpha: Vec<_> = pixels.pixels().iter().map(|pixel| pixel.alpha()).collect();
        let file = out.join(format!("{name}.alpha"));
        fs::write(&file, alpha).unwrap();
        generated.push_str(&format!(
            "\"icons/{name}\" => Some(include_bytes!({:?})),\n",
            file
        ));
    }
    generated.push_str("_ => None, } }\n");
    fs::write(out.join("icons.rs"), generated).unwrap();
}
