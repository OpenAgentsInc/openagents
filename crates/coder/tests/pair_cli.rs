//! Pairing dispatch must stay independent from model credentials and agent turns.
use std::process::Command;

#[test]
fn installed_pair_command_exposes_its_own_help() {
    let output = Command::new(env!("CARGO_BIN_EXE_coder"))
        .args(["pair", "--help"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let help = String::from_utf8(output.stdout).unwrap();
    assert!(help.starts_with("coder pair "));
    assert!(help.contains("read-only phone access"));
    assert!(!help.contains("Classify"));
}

#[test]
fn installed_pair_command_cannot_be_used_for_other_host_operations() {
    let output = Command::new(env!("CARGO_BIN_EXE_coder"))
        .args(["pair", "revoke"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(
        String::from_utf8(output.stderr)
            .unwrap()
            .contains("unexpected positional argument")
    );
}
