//! `microcoder --version` answers as `openagents --version` does.

use std::process::Command;

#[test]
fn version_prints_the_release_the_repository_and_the_commit() {
    let output = Command::new(env!("CARGO_BIN_EXE_microcoder"))
        .arg("--version")
        .output()
        .expect("microcoder runs");
    assert!(output.status.success(), "{output:?}");
    let line = String::from_utf8(output.stdout).expect("UTF-8");
    let line = line.trim_end();
    assert_eq!(
        line,
        coder::identity::program_line("microcoder", env!("CARGO_PKG_VERSION"))
    );
    assert!(
        line.starts_with(&format!("microcoder {} (", env!("CARGO_PKG_VERSION"))),
        "{line}"
    );
    assert!(line.contains("OpenAgentsInc/openagents"), "{line}");
}
