fn main() -> Result<(), Box<dyn std::error::Error>> {
    let proto_root = "../../proto";
    let proto_files = [
        "../../proto/openagents/common/v1/common.proto",
        "../../proto/openagents/economy/v1/receipt.proto",
        "../../proto/openagents/economy/v1/snapshot.proto",
        "../../proto/openagents/compute/v1/compute.proto",
        "../../proto/openagents/compute/v1/compute_products.proto",
        "../../proto/openagents/compute/v1/compute_environments.proto",
        "../../proto/openagents/compute/v1/compute_evals.proto",
        "../../proto/openagents/compute/v1/compute_training.proto",
        "../../proto/openagents/compute/v1/compute_synthetic.proto",
        "../../proto/openagents/compute/v1/compute_capacity.proto",
        "../../proto/openagents/compute/v1/compute_instruments.proto",
        "../../proto/openagents/compute/v1/compute_delivery.proto",
        "../../proto/openagents/compute/v1/compute_indices.proto",
        "../../proto/openagents/data/v1/data.proto",
        "../../proto/openagents/labor/v1/work.proto",
    ];

    println!("cargo:rerun-if-changed={proto_root}");

    let protoc = protoc_bin_vendored::protoc_bin_path()?;
    let mut config = prost_build::Config::new();
    config.include_file("openagents.rs");
    config.protoc_executable(protoc);
    config.type_attribute(".", "#[derive(serde::Serialize, serde::Deserialize)]");
    config.compile_protos(&proto_files, &[proto_root])?;
    Ok(())
}
