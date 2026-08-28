//! The staged coder artifacts and the embedded Rust module must match.
//!
//! Rebuild with `cargo run -p openagents-cli --bin coder-surfaces -- --write`.

#[test]
fn staged_artifacts_match_the_embedded_module() {
    if let Err(failures) = openagents_cli::surfaces_build::check() {
        panic!(
            "coder surfaces are out of date:\n{}",
            failures
                .into_iter()
                .map(|f| format!("  - {f}"))
                .collect::<Vec<_>>()
                .join("\n")
        );
    }
}
