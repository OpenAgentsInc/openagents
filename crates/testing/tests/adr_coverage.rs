//! ADR Coverage Meta-Test
//!
//! This test verifies that every normative rule in Accepted ADRs has
//! corresponding test coverage documented in a Compliance table.
//!
//! How it works:
//! 1. Enumerate all docs/adr/ADR-*.md files with Status = Accepted
//! 2. Extract all rule IDs matching pattern ADR-XXXX.RN
//! 3. Extract all test references from Compliance tables
//! 4. Verify every rule has at least one test reference

use regex::Regex;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;
use walkdir::WalkDir;

/// Get the workspace root directory
fn workspace_root() -> std::path::PathBuf {
    // This test runs from crates/testing, so workspace is ../../
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|_| ".".to_string());
    let path = Path::new(&manifest_dir);

    // Navigate up to workspace root
    if path.ends_with("crates/testing") {
        path.parent().unwrap().parent().unwrap().to_path_buf()
    } else {
        // Fallback: look for docs/adr relative to current dir
        path.to_path_buf()
    }
}

/// Parse an ADR file and extract rule IDs and compliance mappings
fn parse_adr(content: &str) -> (String, Vec<String>, HashMap<String, Vec<String>>) {
    // Extract status
    let status_re = Regex::new(r"\*\*(\w+)\*\*").unwrap();
    let status = content
        .lines()
        .skip_while(|l| !l.contains("## Status"))
        .nth(2)
        .and_then(|l| status_re.captures(l))
        .map(|c| c.get(1).unwrap().as_str().to_string())
        .unwrap_or_default();

    // Extract rule IDs (pattern: ADR-XXXX.RN)
    let rule_re = Regex::new(r"ADR-\d{4}\.R\d+").unwrap();
    let rule_ids: Vec<String> = rule_re
        .find_iter(content)
        .map(|m| m.as_str().to_string())
        .collect::<HashSet<_>>()
        .into_iter()
        .collect();

    // Extract compliance table mappings
    let mut compliance: HashMap<String, Vec<String>> = HashMap::new();

    // Find compliance section
    let compliance_section = content
        .lines()
        .skip_while(|l| !l.contains("## Compliance"))
        .take_while(|l| !l.starts_with("## ") || l.contains("## Compliance"))
        .collect::<Vec<_>>()
        .join("\n");

    // Parse table rows: | ADR-XXXX.RN | test_path | status |
    let table_re = Regex::new(r"\|\s*(ADR-\d{4}\.R\d+)\s*\|\s*`([^`]+)`").unwrap();
    for cap in table_re.captures_iter(&compliance_section) {
        let rule_id = cap.get(1).unwrap().as_str().to_string();
        let test_path = cap.get(2).unwrap().as_str().to_string();
        compliance.entry(rule_id).or_default().push(test_path);
    }

    (status, rule_ids, compliance)
}

/// Test: Every accepted ADR with rules has a Compliance table
#[test]
fn test_accepted_adrs_have_compliance_tables() {
    let root = workspace_root();
    let adr_dir = root.join("docs/adr");

    if !adr_dir.exists() {
        println!("ADR directory not found at {:?}, skipping", adr_dir);
        return;
    }

    let mut missing_tables = Vec::new();

    for entry in WalkDir::new(&adr_dir)
        .max_depth(1)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let path = entry.path();
        if !path.extension().map(|e| e == "md").unwrap_or(false) {
            continue;
        }

        let filename = path.file_name().unwrap().to_string_lossy();
        if !filename.starts_with("ADR-") {
            continue;
        }

        let content = fs::read_to_string(path).unwrap_or_default();
        let (status, rule_ids, compliance) = parse_adr(&content);

        // Only check Accepted ADRs with rule IDs
        if status == "Accepted" && !rule_ids.is_empty() && compliance.is_empty() {
            missing_tables.push(filename.to_string());
        }
    }

    if !missing_tables.is_empty() {
        panic!(
            "The following Accepted ADRs have rules but no Compliance table:\n  - {}",
            missing_tables.join("\n  - ")
        );
    }
}

/// Test: Every rule ID in an ADR has at least one test reference
#[test]
fn test_all_rules_have_test_coverage() {
    let root = workspace_root();
    let adr_dir = root.join("docs/adr");

    if !adr_dir.exists() {
        println!("ADR directory not found at {:?}, skipping", adr_dir);
        return;
    }

    let mut uncovered_rules: HashMap<String, Vec<String>> = HashMap::new();

    for entry in WalkDir::new(&adr_dir)
        .max_depth(1)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let path = entry.path();
        if !path.extension().map(|e| e == "md").unwrap_or(false) {
            continue;
        }

        let filename = path.file_name().unwrap().to_string_lossy();
        if !filename.starts_with("ADR-") {
            continue;
        }

        let content = fs::read_to_string(path).unwrap_or_default();
        let (status, rule_ids, compliance) = parse_adr(&content);

        // Only check Accepted ADRs
        if status != "Accepted" {
            continue;
        }

        // Find rules without test coverage
        for rule_id in rule_ids {
            if !compliance.contains_key(&rule_id) {
                uncovered_rules
                    .entry(filename.to_string())
                    .or_default()
                    .push(rule_id);
            }
        }
    }

    if !uncovered_rules.is_empty() {
        let mut msg = String::from("The following rules lack test coverage:\n");
        for (file, rules) in uncovered_rules {
            msg.push_str(&format!("  {}:\n", file));
            for rule in rules {
                msg.push_str(&format!("    - {}\n", rule));
            }
        }
        panic!("{}", msg);
    }
}

/// Test: ADR-0016, 0017, 0018 have rule IDs (the ones we just added)
#[test]
fn test_new_adrs_have_rule_ids() {
    let root = workspace_root();
    let adr_dir = root.join("docs/adr");

    if !adr_dir.exists() {
        println!("ADR directory not found at {:?}, skipping", adr_dir);
        return;
    }

    let expected_adrs = vec![
        ("ADR-0016-privacy-defaults-swarm-dispatch.md", 5),
        ("ADR-0017-telemetry-trace-contract.md", 7),
        ("ADR-0018-forge-adapter-contract.md", 8),
    ];

    for (filename, expected_rule_count) in expected_adrs {
        let path = adr_dir.join(filename);
        if !path.exists() {
            println!("ADR {} not found, skipping", filename);
            continue;
        }

        let content = fs::read_to_string(&path).unwrap();
        let (_, rule_ids, _) = parse_adr(&content);

        assert!(
            rule_ids.len() >= expected_rule_count,
            "{} should have at least {} rule IDs, found {}: {:?}",
            filename,
            expected_rule_count,
            rule_ids.len(),
            rule_ids
        );
    }
}

/// Lint: No hardcoded NIP-90 kinds next to schema IDs
#[test]
fn lint_no_hardcoded_nip90_kinds_in_docs() {
    let root = workspace_root();
    let docs_dir = root.join("docs");

    if !docs_dir.exists() {
        println!("Docs directory not found, skipping");
        return;
    }

    // Pattern that would indicate hardcoded kind next to schema
    // e.g., "oa.sandbox_run.v1 (kind 5102)"
    let pattern = Regex::new(r"oa\.\w+\.v\d+.*kind\s*\d+").unwrap();

    let mut violations = Vec::new();

    for entry in WalkDir::new(&docs_dir).into_iter().filter_map(|e| e.ok()) {
        let path = entry.path();
        if !path.extension().map(|e| e == "md").unwrap_or(false) {
            continue;
        }

        let content = fs::read_to_string(path).unwrap_or_default();
        if pattern.is_match(&content) {
            violations.push(path.to_string_lossy().to_string());
        }
    }

    // This is a soft lint - warn but don't fail
    if !violations.is_empty() {
        println!(
            "WARNING: Found potential hardcoded NIP-90 kinds in:\n  - {}",
            violations.join("\n  - ")
        );
    }
}

/// Lint: trajectory_hash is used, not replay_hash (in forge-related code)
#[test]
fn lint_trajectory_hash_not_replay_hash() {
    let root = workspace_root();

    // Check forge-related files
    let files_to_check = vec![
        root.join("docs/adr/ADR-0018-forge-adapter-contract.md"),
        root.join("docs/PROTOCOL_SURFACE.md"),
    ];

    for path in files_to_check {
        if !path.exists() {
            continue;
        }

        let content = fs::read_to_string(&path).unwrap_or_default();

        // replay_hash should not appear except in negation context
        // Valid patterns: "not `replay_hash`", "Use `replay_hash` instead — rejected"
        let replay_hash_count = content.matches("replay_hash").count();
        let not_replay_hash_count = content.matches("not `replay_hash`").count()
            + content.matches("not \"replay_hash\"").count()
            + content.matches("not replay_hash").count()
            + content.matches("replay_hash` instead").count() // "Use X instead — rejected"
            + content.matches("replay_hash\" instead").count();

        if replay_hash_count > not_replay_hash_count {
            panic!(
                "{}: Uses 'replay_hash' {} times but only {} times in negation context. \
                 Use 'trajectory_hash' (the canonical name).",
                path.display(),
                replay_hash_count,
                not_replay_hash_count
            );
        }
    }
}

/// Summary test: Print coverage statistics
#[test]
fn test_adr_coverage_summary() {
    let root = workspace_root();
    let adr_dir = root.join("docs/adr");

    if !adr_dir.exists() {
        println!("ADR directory not found at {:?}", adr_dir);
        return;
    }

    let mut total_rules = 0;
    let mut covered_rules = 0;
    let mut passing_tests = 0;
    let mut ignored_tests = 0;

    for entry in WalkDir::new(&adr_dir)
        .max_depth(1)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let path = entry.path();
        if !path.extension().map(|e| e == "md").unwrap_or(false) {
            continue;
        }

        let filename = path.file_name().unwrap().to_string_lossy();
        if !filename.starts_with("ADR-") {
            continue;
        }

        let content = fs::read_to_string(path).unwrap_or_default();
        let (status, rule_ids, compliance) = parse_adr(&content);

        if status != "Accepted" {
            continue;
        }

        total_rules += rule_ids.len();
        covered_rules += compliance.len();

        // Count passing vs ignored
        if content.contains("✅ Pass") {
            passing_tests += content.matches("✅ Pass").count();
        }
        if content.contains("⏳ Ignored") {
            ignored_tests += content.matches("⏳ Ignored").count();
        }
    }

    println!("\n=== ADR Coverage Summary ===");
    println!("Total rules defined: {}", total_rules);
    println!("Rules with test coverage: {}", covered_rules);
    println!("Passing tests: {}", passing_tests);
    println!("Ignored tests (pending impl): {}", ignored_tests);
    println!(
        "Coverage: {:.1}%",
        if total_rules > 0 {
            (covered_rules as f64 / total_rules as f64) * 100.0
        } else {
            0.0
        }
    );
    println!("============================\n");
}
