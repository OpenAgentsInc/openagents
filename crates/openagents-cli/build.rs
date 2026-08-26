//! Records the version this binary was published as.
//!
//! The crate manifest names the version of the source; `ops/release-cli.sh`
//! names the version of the release, and for a release candidate the two
//! differ. `oa update` compares what the binary reports against what the
//! channel pointer resolves to, so the binary has to report the name it was
//! published under or the comparison is meaningless.
//!
//! Cargo does not otherwise track environment variables read through
//! `option_env!`, so a second build with a different version would reuse the
//! first one's artifact. This declares the dependency.

fn main() {
    println!("cargo:rerun-if-env-changed=OPENAGENTS_CLI_RELEASE_VERSION");
}
