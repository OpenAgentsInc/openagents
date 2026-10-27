use std::path::PathBuf;

#[test]
fn generated_rust_matches_schema() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let schemas = root.join("../../proto");
    let output = tempfile::tempdir().unwrap();
    let mut sources: Vec<_> = std::fs::read_dir(schemas.join("openagents/engine/v1"))
        .unwrap()
        .map(|p| p.unwrap().path())
        .filter(|p| p.extension().is_some_and(|e| e == "proto"))
        .collect();
    sources.sort();
    prost_build::Config::new()
        .out_dir(output.path())
        .compile_protos(&sources, &[schemas])
        .unwrap();
    let name = "openagents.engine.v1.rs";
    let generated = std::fs::read(output.path().join(name)).unwrap();
    let checked = root.join("src/generated").join(name);
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
