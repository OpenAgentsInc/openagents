//! The installed `openagents` binary keeps the complete CLI surface.

use std::process::Command;

fn run(arguments: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_openagents"))
        .args(arguments)
        .output()
        .expect("run the installed command surface")
}

#[test]
fn root_help_lists_coder_and_cli_namespaces() {
    let output = run(&["--help"]);
    assert!(output.status.success());
    let help = String::from_utf8_lossy(&output.stdout);

    for namespace in ["forge", "trace", "issue", "repo", "auth", "coder"] {
        assert!(
            help.contains(namespace),
            "root help does not expose {namespace}: {help}"
        );
    }
    assert!(help.contains("openagents coder [options]"));
    assert!(!help.contains("does not take that name"));
}

#[test]
fn forge_and_trace_help_dispatch_to_the_shared_cli() {
    for arguments in [
        ["forge", "--help"].as_slice(),
        ["trace", "--help"].as_slice(),
    ] {
        let output = run(arguments);
        assert!(
            output.status.success(),
            "{arguments:?} failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

#[test]
fn explicit_coder_help_uses_the_coder_front_door() {
    let output = run(&["coder", "--help"]);
    assert!(output.status.success());
    let help = String::from_utf8_lossy(&output.stdout);
    assert!(help.contains("Coder options:"));
    assert!(help.contains("--lane <name>"));
}
