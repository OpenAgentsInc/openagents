use std::env;
use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-env-changed=CARGO_FEATURE_GPT_OSS_METAL");
    println!("cargo:rerun-if-env-changed=GPT_OSS_METAL_DIR");
    println!("cargo:rerun-if-env-changed=GPT_OSS_METAL_METALLIB");

    if env::var("CARGO_FEATURE_GPT_OSS_METAL").is_err() {
        return;
    }

    let metallib = match env::var("GPT_OSS_METAL_METALLIB") {
        Ok(path) => PathBuf::from(path),
        Err(_) => {
            let base_dir = env::var("GPT_OSS_METAL_DIR")
                .map(PathBuf::from)
                .ok()
                .or_else(default_metal_dir)
                .unwrap_or_else(|| {
                    panic!(
                        "GPT_OSS_METAL_DIR or GPT_OSS_METAL_METALLIB must be set when gpt-oss-metal is enabled."
                    )
                });
            base_dir.join("default.metallib")
        }
    };

    if !metallib.exists() {
        panic!("GPT-OSS metallib not found at {}", metallib.display());
    }

    println!(
        "cargo:rustc-link-arg=-Wl,-sectcreate,__METAL,__shaders,{}",
        metallib.display()
    );
}

fn default_metal_dir() -> Option<PathBuf> {
    let home = env::var("HOME").ok()?;
    let candidate = PathBuf::from(home).join("code/gpt-oss/gpt_oss/metal/build");
    if candidate.exists() {
        Some(candidate)
    } else {
        None
    }
}
