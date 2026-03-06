fn main() -> Result<(), Box<dyn std::error::Error>> {
    let proto_root = "../../proto";
    let proto_files = [
        "../../proto/openagents/common/v1/common.proto",
        "../../proto/openagents/economy/v1/receipt.proto",
        "../../proto/openagents/economy/v1/snapshot.proto",
        "../../proto/openagents/compute/v1/compute.proto",
        "../../proto/openagents/labor/v1/work.proto",
    ];

    println!("cargo:rerun-if-changed={proto_root}");

    let protoc = protoc_bin_vendored::protoc_bin_path()?;
    let mut config = prost_build::Config::new();
    config.include_file("openagents.rs");
    config.protoc_executable(protoc);
    config.compile_protos(&proto_files, &[proto_root])?;
    Ok(())
}
