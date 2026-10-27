use std::path::PathBuf;

#[test]
fn generated_rust_matches_schema() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let schemas = root.join("../../proto");
    let output = tempfile::tempdir().unwrap();
    fn collect(path: &std::path::Path, files: &mut Vec<PathBuf>) {
        for entry in std::fs::read_dir(path).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                collect(&path, files);
            } else if path.extension().is_some_and(|e| e == "proto") {
                files.push(path);
            }
        }
    }
    let mut sources = Vec::new();
    collect(&schemas, &mut sources);
    sources.sort();
    prost_build::Config::new()
        .out_dir(output.path())
        .compile_protos(&sources, &[schemas])
        .unwrap();
    for entry in std::fs::read_dir(output.path()).unwrap() {
        let path = entry.unwrap().path();
        let generated = std::fs::read(&path).unwrap();
        let checked = root.join("src/generated").join(path.file_name().unwrap());
        if std::env::var_os("ENGINE_PROTO_REGENERATE").is_some() {
            std::fs::write(checked, generated).unwrap();
        } else {
            assert_eq!(
                generated,
                std::fs::read(checked).unwrap(),
                "run scripts/engine-proto.sh generate"
            );
        }
    }
}
