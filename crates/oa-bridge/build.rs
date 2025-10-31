fn main() {
    // Nothing to generate here; we keep the build script to allow future hooks.
    println!("cargo:rerun-if-changed=build.rs");
}
