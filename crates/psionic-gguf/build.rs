fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").ok().as_deref() == Some("macos") {
        println!("cargo:rerun-if-changed=src/metal_dispatch.c");
        cc::Build::new()
            .file("src/metal_dispatch.c")
            .compile("oa_metal_dispatch");
        println!("cargo:rustc-link-lib=objc");
    }
}
