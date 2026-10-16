fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("cargo:rerun-if-changed=proto/intermediate_key_broker.proto");
    tonic_build::configure()
        .compile_protos(&["proto/intermediate_key_broker.proto"], &["proto"])?;
    Ok(())
}
