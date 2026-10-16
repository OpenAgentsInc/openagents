//! The conformance matrix names exact tests; each must exist where it says,
//! and every contract family in the proposal must have a section.
use std::path::Path;

const MATRIX: &str = include_str!("../../../docs/research/dhruv/conformance-matrix.md");

fn root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .unwrap()
}

/// `crates/<crate>/<path>.rs::<test>` references, in order.
fn references() -> Vec<(String, String)> {
    let mut out = Vec::new();
    for piece in MATRIX.split('`') {
        let Some((file, test)) = piece.split_once(".rs::") else {
            continue;
        };
        if file.starts_with("crates/")
            && !file.contains(' ')
            && test
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
        {
            out.push((format!("{file}.rs"), test.to_string()));
        }
    }
    out
}

#[test]
fn every_referenced_test_exists_in_its_file() {
    let refs = references();
    assert!(refs.len() >= 60, "the matrix lost rows: {}", refs.len());
    let mut missing = Vec::new();
    for (file, test) in &refs {
        let source = std::fs::read_to_string(root().join(file)).unwrap_or_default();
        if !source.contains(&format!("fn {test}(")) {
            missing.push(format!("{file}::{test}"));
        }
    }
    assert!(
        missing.is_empty(),
        "the matrix names tests that do not exist: {missing:#?}"
    );
}

#[test]
fn every_contract_family_has_a_section_and_fixtures_are_not_called_live() {
    for family in [
        "ATIF",
        "REG and EXT",
        "WORK, COORD, WS",
        "ENV, SESS, LIVE",
        "SOV",
        "MKT and LAB",
        "VAULT",
        "ATT",
        "Cross-contract runs",
    ] {
        assert!(MATRIX.contains(family), "no section for {family}");
    }
    assert!(MATRIX.contains("Fixture support is not qualification"));
    // Every table row says how far it has been shown.
    for line in MATRIX
        .lines()
        .filter(|l| l.starts_with("| ") && l.contains(".rs::"))
    {
        let last = line
            .trim_end_matches('|')
            .rsplit('|')
            .next()
            .unwrap_or_default();
        assert!(
            !last.trim().is_empty(),
            "a row does not say how far it was shown: {line}"
        );
    }
}
