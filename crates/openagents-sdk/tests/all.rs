//! Every integration test of this package in one binary (#12199): each
//! file beside this one is a module here, so a change links the package
//! once instead of once per file. Tests that change process-wide state
//! (environment variables) stay binaries of their own; see Cargo.toml.

mod attention;
mod auth;
mod chats;
mod computers;
mod computers_contract;
mod continuations;
mod copy;
mod dashboard;
mod history;
mod labs;
mod labs_worker;
mod land;
mod payment_required;
mod run_commands;
mod runs;
mod schedules;
mod sessions;
mod voice_agent;
mod work;

/// With `autotests = false`, a new file under `tests/` runs only once it
/// is a module above or a `[[test]]` in Cargo.toml.
#[test]
fn every_test_file_is_built() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests");
    let (all, cargo) = (include_str!("all.rs"), include_str!("../Cargo.toml"));
    for entry in std::fs::read_dir(dir).unwrap().flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        let Some(stem) = name.strip_suffix(".rs") else {
            continue;
        };
        assert!(
            stem == "all"
                || all.contains(&format!("mod {stem};"))
                || cargo.contains(&format!("path = \"tests/{name}\"")),
            "tests/{name} is in no test binary: add `mod {stem};` to tests/all.rs"
        );
    }
}
