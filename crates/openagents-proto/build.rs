use std::env;
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};

fn main() -> Result<(), Box<dyn Error>> {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR")?);
    let proto_root = manifest_dir.join("../../proto");
    let out_dir = PathBuf::from(env::var("OUT_DIR")?);

    let mut proto_files = Vec::new();
    collect_proto_files(&proto_root, &mut proto_files)?;
    proto_files.sort();

    println!("cargo:rerun-if-changed={}", proto_root.display());
    println!("cargo:rerun-if-env-changed=OA_PROTO_SNAPSHOT_PATH");
    for proto in &proto_files {
        println!("cargo:rerun-if-changed={}", proto.display());
    }

    let protoc_path = protobuf_src::protoc();

    let mut config = prost_build::Config::new();
    config.btree_map(["."]); // deterministic map ordering in generated types.
    config.compile_well_known_types();
    config.include_file("openagents.rs");
    config.file_descriptor_set_path(out_dir.join("openagents-descriptor-set.bin"));
    config.protoc_executable(protoc_path);

    config.compile_protos(&proto_files, &[proto_root])?;

    if let Ok(snapshot_path) = env::var("OA_PROTO_SNAPSHOT_PATH") {
        let source = out_dir.join("openagents.rs");
        fs::copy(source, snapshot_path)?;
    }

    Ok(())
}

fn collect_proto_files(root: &Path, files: &mut Vec<PathBuf>) -> Result<(), Box<dyn Error>> {
    for entry in fs::read_dir(root)? {
        let entry = entry?;
        let path = entry.path();

        if path.is_dir() {
            collect_proto_files(&path, files)?;
            continue;
        }

        if path.extension().and_then(|ext| ext.to_str()) == Some("proto") {
            files.push(path);
        }
    }

    Ok(())
}
