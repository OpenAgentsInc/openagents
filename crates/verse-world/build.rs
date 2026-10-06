use std::{collections::BTreeMap, env, process::Command};

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    let rustc = Command::new(env::var_os("RUSTC").expect("Rust compiler"))
        .arg("--version")
        .arg("--verbose")
        .output()
        .expect("Read Rust compiler identity");
    assert!(rustc.status.success(), "Read Rust compiler identity");
    let compiler = String::from_utf8(rustc.stdout).expect("Compiler identity is UTF-8");
    println!(
        "cargo:rustc-env=VERSE_REPLAY_COMPILER={}",
        compiler.trim().replace('\n', " | ")
    );
    println!(
        "cargo:rustc-env=VERSE_REPLAY_TARGET={}",
        env::var("TARGET").expect("Cargo target")
    );
    let mut config = BTreeMap::new();
    for name in [
        "PROFILE",
        "OPT_LEVEL",
        "DEBUG",
        "CARGO_ENCODED_RUSTFLAGS",
        "CARGO_CFG_TARGET_FEATURE",
    ] {
        println!("cargo:rerun-if-env-changed={name}");
        config.insert(name.to_owned(), env::var(name).unwrap_or_default());
    }
    for (name, value) in env::vars().filter(|(name, _)| name.starts_with("CARGO_FEATURE_")) {
        config.insert(name, value);
    }
    let config = format!("{config:?}");
    println!("cargo:rustc-env=VERSE_REPLAY_BUILD={config}");
}
