use std::{env, fs, path::PathBuf};

fn main() {
    let dir = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap()).join("../../knowledge");
    println!("cargo:rerun-if-changed={}", dir.display());
    let mut paths: Vec<_> = fs::read_dir(&dir)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| {
            path.extension().is_some_and(|ext| ext == "md")
                && path.file_name().unwrap() != "README.md"
        })
        .collect();
    paths.sort();
    let mut generated = String::from("const BUNDLED: &[(&str, &str)] = &[\n");
    for path in paths {
        let name = path.file_name().unwrap().to_str().unwrap();
        let text = fs::read_to_string(&path).unwrap();
        generated.push_str(&format!("({name:?}, {text:?}),\n"));
    }
    generated.push_str("];\n");
    fs::write(
        PathBuf::from(env::var_os("OUT_DIR").unwrap()).join("bundled.rs"),
        generated,
    )
    .unwrap();
}
