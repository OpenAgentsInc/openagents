//! Every integration test of this package in one binary (#12199): each
//! file beside this one is a module here, so a change links the package
//! once instead of once per file. Tests that change process-wide state
//! (environment variables) stay binaries of their own; see Cargo.toml.

#[path = "../../coder-control/src/tests/relay.rs"]
mod relay;

mod chat_goldens;
mod conformance;
mod delegation;
mod desktop_local_coder;
mod dispatch_ack_eval;
mod eval_author_live;
mod evidence_select;
mod fanout_eval;
mod gateway_stream;
mod headless;
mod host_cli;
mod host_serve;
mod interop_processes;
mod interoperability;
mod issue_pick_eval;
mod phone_engine;
mod plugin_catalog;
mod program_run;
mod relay_binding;
mod relay_delegation;
mod relay_lifecycle;
mod route_map_sources;
mod router_eval;
mod router_suite;
mod runstate_recovery;
mod shell_verdicts;
mod state_caps;
mod suite_questions;
mod task_inbox;
mod task_owner;
mod trace;
mod wallet_eval;
mod worker_lifecycle;

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
