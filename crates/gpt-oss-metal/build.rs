use std::env;
use std::path::PathBuf;

fn main() {
    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    if target_os != "macos" {
        return;
    }

    println!("cargo:rerun-if-env-changed=GPT_OSS_METAL_DIR");
    println!("cargo:rerun-if-env-changed=GPT_OSS_METAL_BUILD_DIR");
    println!("cargo:rerun-if-env-changed=GPT_OSS_METAL_LIB_DIR");
    println!("cargo:rerun-if-env-changed=GPT_OSS_METAL_METALLIB");

    if env::var("CARGO_FEATURE_LINK").is_err() {
        return;
    }

    let base_dir = env::var("GPT_OSS_METAL_DIR")
        .or_else(|_| env::var("GPT_OSS_METAL_BUILD_DIR"))
        .or_else(|_| env::var("GPT_OSS_METAL_LIB_DIR"))
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            panic!(
                "GPT_OSS_METAL_DIR is not set. Point it at the gpt-oss metal build output \
                (contains libgptoss.a, libmetal-kernels.a, default.metallib)."
            )
        });

    let libgptoss = base_dir.join("libgptoss.a");
    if !libgptoss.exists() {
        panic!("GPT-OSS metal library not found at {}", libgptoss.display());
    }

    let libkernels = base_dir.join("libmetal-kernels.a");
    if !libkernels.exists() {
        panic!(
            "GPT-OSS metal-kernels library not found at {}",
            libkernels.display()
        );
    }

    let metallib = env::var("GPT_OSS_METAL_METALLIB")
        .map(PathBuf::from)
        .unwrap_or_else(|_| base_dir.join("default.metallib"));
    if !metallib.exists() {
        panic!("GPT-OSS metallib not found at {}", metallib.display());
    }

    println!("cargo:rustc-link-search=native={}", base_dir.display());
    println!("cargo:rustc-link-lib=static=gptoss");
    println!("cargo:rustc-link-lib=static=metal-kernels");
    println!("cargo:rustc-link-lib=framework=Foundation");
    println!("cargo:rustc-link-lib=framework=Metal");
    println!("cargo:rustc-link-lib=framework=IOKit");
    println!(
        "cargo:rustc-link-arg=-Wl,-sectcreate,__METAL,__shaders,{}",
        metallib.display()
    );
}
