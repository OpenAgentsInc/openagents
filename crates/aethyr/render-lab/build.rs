//! With `native`, adds the webgpu.h library directory to the run path of this
//! crate's binaries and tests. `af-gpu` links the library, but link arguments
//! do not propagate to dependents.

fn main() {
    println!("cargo:rerun-if-env-changed=AF_WEBGPU_DIR");
    if std::env::var_os("CARGO_FEATURE_NATIVE").is_none() {
        return;
    }
    let target = std::env::var("TARGET").unwrap_or_default();
    if let Some(dir) = std::env::var_os("AF_WEBGPU_DIR")
        && (target.contains("apple") || target.contains("linux"))
    {
        let lib = std::path::PathBuf::from(dir).join("lib");
        println!("cargo:rustc-link-arg=-Wl,-rpath,{}", lib.display());
    }
}
