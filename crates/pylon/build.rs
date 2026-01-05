use std::env;
use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-env-changed=CARGO_FEATURE_GPT_OSS_METAL");
    println!("cargo:rerun-if-env-changed=GPT_OSS_METAL_DIR");
    println!("cargo:rerun-if-env-changed=GPT_OSS_METAL_METALLIB");

    if env::var("CARGO_FEATURE_GPT_OSS_METAL").is_err() {
        return;
    }

    let metallib = env::var("GPT_OSS_METAL_METALLIB")
        .map(PathBuf::from)
        .or_else(|_| {
            env::var("GPT_OSS_METAL_DIR")
                .map(|dir| PathBuf::from(dir).join("default.metallib"))
        })
        .unwrap_or_else(|_| {
            panic!(
                "GPT_OSS_METAL_DIR or GPT_OSS_METAL_METALLIB must be set when gpt-oss-metal is enabled."
            )
        });

    if !metallib.exists() {
        panic!("GPT-OSS metallib not found at {}", metallib.display());
    }

    println!(
        "cargo:rustc-link-arg=-Wl,-sectcreate,__METAL,__shaders,{}",
        metallib.display()
    );
}
