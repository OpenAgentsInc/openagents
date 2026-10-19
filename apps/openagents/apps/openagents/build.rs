fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos") {
        let plist = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../dist/macos/Info-unbundled.plist");
        println!("cargo:rerun-if-changed={}", plist.display());
        // `cargo run` has no .app Info.plist. NSBundle also reads privacy
        // declarations from this Mach-O section, so source builds can request
        // microphone access without requiring a separate packaging step.
        println!(
            "cargo:rustc-link-arg-bin=openagents=-Wl,-sectcreate,__TEXT,__info_plist,{}",
            plist.display()
        );
    }
    println!("cargo:rerun-if-changed=../../dist/windows/openagents.rc");
    println!("cargo:rerun-if-changed=../../dist/openagents/coder.ico");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        // See src/msvc_compat.rs: used only when MSVC lacks its own.
        if std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc") {
            for n in [1, 2] {
                println!(
                    "cargo:rustc-link-arg=/ALTERNATENAME:__std_find_first_of_trivial_pos_{n}=openagents_find_first_of_trivial_pos_{n}"
                );
            }
        }
        embed_resource::compile_for(
            "../../dist/windows/openagents.rc",
            &["openagents"],
            embed_resource::NONE,
        )
        .manifest_required()
        .expect("Windows app icon resource compilation failed");
    }
}
