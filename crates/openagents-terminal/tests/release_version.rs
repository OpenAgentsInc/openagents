//! Audit CLI-03: `scripts/release/terminal.sh` refuses a release unless
//! openagents-cli, openagents-terminal, and microcoder carry the same
//! version, so the three manifests must move together. The welcome card
//! prints this crate's version, and `openagents --version` prints the CLI's.

fn manifest_version(manifest: &str) -> &str {
    manifest
        .lines()
        .find_map(|line| line.strip_prefix("version = \""))
        .and_then(|rest| rest.strip_suffix('"'))
        .expect("the manifest names its own version")
}

#[test]
fn terminal_cli_and_microcoder_share_one_release_version() {
    let terminal = env!("CARGO_PKG_VERSION");
    let cli = manifest_version(include_str!("../../openagents-cli/Cargo.toml"));
    let microcoder = manifest_version(include_str!("../../microcoder/Cargo.toml"));
    assert_eq!(
        terminal, cli,
        "openagents-terminal and openagents-cli versions differ"
    );
    assert_eq!(
        terminal, microcoder,
        "openagents-terminal and microcoder versions differ"
    );
}
