//! Discovery and redaction, asserted on values only correct behaviour produces.
//!
//! The defect these cover reported "Redacted size: 141 bytes" — exactly the input
//! plus the length of the two markers it had prepended — while leaving every secret
//! body intact and writing no file. So the assertions here are about *absence*: the
//! secret must not appear in the written output. A test that a marker is present
//! passes against a prefix swap, and that is what let the defect ship.

use openagents_cli::trace::{
    default_trace_stores, discover, is_redacted_copy, path_trace_store, redact_text,
    redact_trace_file, redacted_path_for, resolve_trace_argument, summarize_trace_file,
    DiscoveryBounds, TraceSourceKind,
};
use std::fs;
use std::path::{Path, PathBuf};

/// Planted secrets, one of each shape the rules claim to cover. None is real.
const API_KEY: &str = "sk-liveSECRETVALUE123456789abcdef";
const JWT: &str = "eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiIxMjM0NTY3ODkwIn0.dBjftJeZ4CVPmB92K27uhbUJU1p1r_wW1gFWFOEjXk";
const BEARER_BODY: &str = "REALTOKEN456abcdefghij";
const NSEC: &str = "nsec1vl029mgpspedva04g90vltkh6fvh240zqtv9k0t9af8935ke9laqsnlfe5";
const ENV_VALUE: &str = "hunter2SUPERSECRET";
/// OpenAgents' own personal access token shape, which no TypeScript rule covers.
const OA_PAT: &str = "oa_pat_REALTOKEN456";
const SEED_PHRASE: &str =
    "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";

fn probe_document(home: &str) -> String {
    format!(
        r#"{{
  "api_key": "{API_KEY}",
  "authorization": "Bearer {BEARER_BODY}",
  "jwt": "{JWT}",
  "nsec": "{NSEC}",
  "note": "OPENAI_API_KEY={ENV_VALUE}",
  "pat": "{OA_PAT}",
  "seed": "{SEED_PHRASE}",
  "cwd": "{home}/work/openagents"
}}"#
    )
}

/// Every planted secret, so a single loop can prove each one is gone.
fn planted_secrets() -> Vec<&'static str> {
    vec![
        API_KEY,
        "liveSECRETVALUE123",
        JWT,
        BEARER_BODY,
        "REALTOKEN456",
        NSEC,
        ENV_VALUE,
        SEED_PHRASE,
        OA_PAT,
        "REALTOKEN456",
    ]
}

#[test]
fn redaction_removes_every_planted_secret_body() {
    let home = "/Users/probe-user";
    let document = probe_document(home);
    let redacted = redact_text(&document, home);

    for secret in planted_secrets() {
        assert!(
            !redacted.text.contains(secret),
            "the redacted text still contains a planted secret ({} chars of shape {}...)",
            secret.len(),
            &secret[..secret.len().min(3)]
        );
    }

    // The home path is rewritten, not merely flagged.
    assert!(!redacted.text.contains(home));
    assert!(redacted.text.contains("~/work/openagents"));

    // And the categories are all reported.
    for category in [
        "api_key",
        "bearer_token",
        "jwt",
        "private_key",
        "env_value",
        "seed_phrase",
        "home_path",
        "oa_token",
    ] {
        assert!(
            redacted.counts.contains_key(category),
            "no `{}` match was counted; counts were {:?}",
            category,
            redacted.counts.keys().collect::<Vec<_>>()
        );
    }
    assert_eq!(
        redacted.total,
        redacted.counts.values().sum::<usize>(),
        "the total must be the sum of the per-category counts"
    );
}

#[test]
fn redaction_writes_the_output_file_and_it_holds_no_secret() {
    let directory = tempfile::tempdir().unwrap();
    let input = directory.path().join("probe.json");
    let home = "/Users/probe-user";
    fs::write(&input, probe_document(home)).unwrap();

    let result = redact_trace_file(&input, home).unwrap();

    // The old command wrote nothing at all while reporting success.
    assert_eq!(result.output, directory.path().join("probe.redacted.json"));
    assert!(result.output.is_file(), "no redacted copy was written");

    let written = fs::read_to_string(&result.output).unwrap();
    for secret in planted_secrets() {
        assert!(
            !written.contains(secret),
            "the written file still contains a planted secret"
        );
    }

    // The original is left alone, and still holds what it held.
    assert!(fs::read_to_string(&input).unwrap().contains(API_KEY));

    // A JSON input must still parse after redaction, or the caller is warned.
    assert_eq!(result.valid_json, Some(true));
    assert!(result.total >= 7);
}

#[test]
fn a_prefix_swap_would_fail_this_test() {
    // The exact defect: `input.replace("sk-", "[REDACTED_KEY]")` grows the text by
    // the marker length and leaves the body. Redaction must shrink this input,
    // because the secret is longer than the marker that replaces it.
    let input = format!("token: {}\n", API_KEY);
    let redacted = redact_text(&input, "");
    assert!(
        redacted.text.len() < input.len(),
        "the output grew, which is what a prefix swap does: {:?}",
        redacted.text
    );
    assert!(!redacted.text.contains("liveSECRETVALUE123"));
    assert_eq!(redacted.text.trim(), "token: [REDACTED:api_key]");
}

#[test]
fn a_twelve_word_run_of_ordinary_prose_is_not_treated_as_a_seed() {
    // The shape rule matches any run of 12 lowercase words. Only a run that is
    // actually BIP-39 is redacted, or every paragraph would be destroyed.
    let prose = "the quick brown fox jumped over the lazy dog while nobody else watched";
    let redacted = redact_text(prose, "");
    assert_eq!(redacted.text, prose);
    assert!(!redacted.counts.contains_key("seed_phrase"));
}

#[test]
fn a_seed_phrase_is_redacted_without_eating_the_prose_around_it() {
    let line = format!("my phrase is {} ok", SEED_PHRASE);
    let redacted = redact_text(&line, "");
    assert!(!redacted.text.contains("abandon"));
    assert!(redacted.text.contains("[REDACTED:seed_phrase]"));
    assert_eq!(redacted.counts.get("seed_phrase"), Some(&1));
}

#[test]
fn redacting_twice_does_not_re_redact_its_own_markers() {
    let input = "OPENAI_API_KEY=hunter2SUPERSECRET";
    let once = redact_text(input, "");
    assert_eq!(once.text, "OPENAI_API_KEY=[REDACTED:env_value]");

    let twice = redact_text(&once.text, "");
    assert_eq!(twice.text, once.text, "the marker was redacted again");
    assert_eq!(twice.total, 0);
}

#[test]
fn nothing_matching_is_reported_as_nothing_matching() {
    let redacted = redact_text("{\"note\":\"a harmless line\"}", "");
    assert_eq!(redacted.total, 0);
    assert!(redacted.counts.is_empty());
    assert_eq!(redacted.text, "{\"note\":\"a harmless line\"}");
}

#[test]
fn redacted_sibling_paths_follow_the_extension() {
    assert_eq!(
        redacted_path_for(Path::new("/t/a.jsonl")),
        PathBuf::from("/t/a.redacted.jsonl")
    );
    assert_eq!(
        redacted_path_for(Path::new("/t/a.json")),
        PathBuf::from("/t/a.redacted.json")
    );
    assert_eq!(
        redacted_path_for(Path::new("/t/a.txt")),
        PathBuf::from("/t/a.txt.redacted.json")
    );
    assert!(is_redacted_copy(Path::new("/t/a.redacted.json")));
    assert!(is_redacted_copy(Path::new("/t/a.redacted.jsonl")));
    assert!(!is_redacted_copy(Path::new("/t/a.json")));
}

// ---------------------------------------------------------------------------
// Discovery
// ---------------------------------------------------------------------------

#[test]
fn discovery_reports_the_files_that_are_there_and_nothing_else() {
    let root = tempfile::tempdir().unwrap();
    let store = root.path().join("store");
    fs::create_dir_all(store.join("nested")).unwrap();
    fs::write(store.join("one.json"), "{}").unwrap();
    fs::write(store.join("nested").join("two.jsonl"), "{}\n").unwrap();
    // Neither extension matches, so neither is listed.
    fs::write(store.join("notes.txt"), "hello").unwrap();

    let bounds = DiscoveryBounds::default();
    let (scans, candidates) = discover(&[path_trace_store(store.clone())], bounds);

    assert_eq!(scans.len(), 1);
    assert!(scans[0].present);
    assert_eq!(scans[0].matched, 2);
    assert_eq!(scans[0].listed, 2);
    assert!(!scans[0].truncated);

    assert_eq!(candidates.len(), 2);
    let mut names: Vec<String> = candidates
        .iter()
        .map(|c| c.path.file_name().unwrap().to_string_lossy().into_owned())
        .collect();
    names.sort();
    assert_eq!(names, vec!["one.json", "two.jsonl"]);
    assert!(candidates.iter().all(|c| c.kind == TraceSourceKind::TracePath));
    // The reported size is the file's real size.
    let one = candidates.iter().find(|c| c.path.ends_with("one.json")).unwrap();
    assert_eq!(one.bytes, 2);
    assert!(
        one.modified_at.ends_with('Z') && one.modified_at.len() == 24,
        "modified_at is not an ISO-8601 instant: {}",
        one.modified_at
    );
}

#[test]
fn a_missing_store_is_reported_as_missing_not_as_empty_success() {
    let (scans, candidates) = discover(
        &[path_trace_store(PathBuf::from("/nonexistent-store-xyz"))],
        DiscoveryBounds::default(),
    );
    assert!(!scans[0].present);
    assert_eq!(scans[0].matched, 0);
    assert!(candidates.is_empty());
}

#[test]
fn symlinks_are_skipped_and_counted_never_followed() {
    let root = tempfile::tempdir().unwrap();
    let store = root.path().join("store");
    let outside = root.path().join("outside");
    fs::create_dir_all(&store).unwrap();
    fs::create_dir_all(&outside).unwrap();
    fs::write(store.join("real.json"), "{}").unwrap();
    fs::write(outside.join("secret.json"), "{}").unwrap();

    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(outside.join("secret.json"), store.join("link.json")).unwrap();
        std::os::unix::fs::symlink(&outside, store.join("escape")).unwrap();
    }

    let (scans, candidates) = discover(&[path_trace_store(store)], DiscoveryBounds::default());

    #[cfg(unix)]
    {
        assert_eq!(scans[0].skipped_symlinks, 2);
        assert_eq!(scans[0].matched, 1, "a symlink must not be counted as a file");
    }
    assert!(candidates.iter().all(|c| c.path.ends_with("real.json")));
}

#[test]
fn the_listing_cap_limits_what_is_listed_but_not_what_is_counted() {
    let root = tempfile::tempdir().unwrap();
    let store = root.path().join("store");
    fs::create_dir_all(&store).unwrap();
    for index in 0..7 {
        fs::write(store.join(format!("t{}.json", index)), "{}").unwrap();
    }

    let bounds = DiscoveryBounds {
        max_files_per_store: 3,
        ..Default::default()
    };
    let (scans, candidates) = discover(&[path_trace_store(store)], bounds);
    assert_eq!(scans[0].matched, 7, "matched counts every file found");
    assert_eq!(scans[0].listed, 3, "listed honours the cap");
    assert_eq!(candidates.len(), 3);
}

#[test]
fn the_default_stores_are_the_three_real_agent_directories() {
    let home = Path::new("/home/probe");
    let specs = default_trace_stores(home);
    let roots: Vec<PathBuf> = specs.iter().map(|s| s.root.clone()).collect();
    assert_eq!(
        roots,
        vec![
            home.join(".openagents").join("exports"),
            home.join(".claude").join("projects"),
            home.join(".codex").join("sessions"),
        ]
    );
    assert_eq!(specs[0].kind, TraceSourceKind::OpenagentsExport);
    assert_eq!(specs[1].kind, TraceSourceKind::ClaudeSession);
    assert_eq!(specs[2].kind, TraceSourceKind::CodexSession);
}

// ---------------------------------------------------------------------------
// Resolution and summary
// ---------------------------------------------------------------------------

#[test]
fn an_id_that_resolves_to_nothing_is_refused() {
    let home = tempfile::tempdir().unwrap();
    // The fabricated `trace show` accepted any id, including this one.
    let result = resolve_trace_argument("claude_sess_01", home.path());
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("No trace file exists"));
}

#[test]
fn a_bare_name_resolves_inside_the_exports_directory() {
    let home = tempfile::tempdir().unwrap();
    let exports = home.path().join(".openagents").join("exports");
    fs::create_dir_all(&exports).unwrap();
    fs::write(exports.join("run.json"), "{}").unwrap();

    assert_eq!(
        resolve_trace_argument("run.json", home.path()).unwrap(),
        exports.join("run.json")
    );
    // A name with a separator is not looked up in exports.
    assert!(resolve_trace_argument("sub/run.json", home.path()).is_err());
}

#[test]
fn an_atif_document_is_summarized_from_its_own_contents() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("run.json");
    fs::write(
        &path,
        r#"{
          "schema_version": "1.7",
          "session_id": "sess-abc",
          "agent": { "name": "coder", "model_name": "opus" },
          "steps": [
            { "source": "user", "timestamp": "2026-08-01T00:00:00Z" },
            { "source": "assistant", "model_name": "opus", "timestamp": "2026-08-01T00:01:00Z",
              "tool_calls": [{"name":"shell"},{"name":"read"}],
              "metrics": { "prompt_tokens": 100, "completion_tokens": 20 } }
          ]
        }"#,
    )
    .unwrap();

    let summary = summarize_trace_file(&path).unwrap();
    assert_eq!(summary.format, "atif");
    assert_eq!(summary.schema_version.as_deref(), Some("1.7"));
    assert_eq!(summary.session_id.as_deref(), Some("sess-abc"));
    assert_eq!(summary.agent_name.as_deref(), Some("coder"));
    assert_eq!(summary.steps, Some(2));
    assert_eq!(summary.tool_calls, Some(2));
    assert_eq!(summary.models.as_deref(), Some(&["opus".to_string()][..]));
    assert_eq!(summary.total_prompt_tokens, Some(100));
    assert_eq!(summary.total_completion_tokens, Some(20));
    assert_eq!(summary.first_timestamp.as_deref(), Some("2026-08-01T00:00:00Z"));
    assert_eq!(summary.last_timestamp.as_deref(), Some("2026-08-01T00:01:00Z"));
    let by_source = summary.steps_by_source.unwrap();
    assert_eq!(by_source.get("user"), Some(&1));
    assert_eq!(by_source.get("assistant"), Some(&1));
}

#[test]
fn a_foreign_log_is_reported_as_a_foreign_log_not_given_invented_steps() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("session.jsonl");
    fs::write(&path, "{\"a\":1}\n\n{\"b\":2}\n").unwrap();

    let summary = summarize_trace_file(&path).unwrap();
    assert_eq!(summary.format, "jsonl");
    assert_eq!(summary.lines, Some(2), "blank lines are not counted");
    assert_eq!(summary.steps, None, "a foreign log has no step count to report");
    assert_eq!(summary.tool_calls, None);
    assert_eq!(summary.models, None);
}

#[test]
fn tokens_absent_from_the_document_are_omitted_not_reported_as_zero() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("run.json");
    fs::write(&path, r#"{"steps":[{"source":"user"}]}"#).unwrap();

    let summary = summarize_trace_file(&path).unwrap();
    assert_eq!(summary.total_prompt_tokens, None);
    assert_eq!(summary.total_completion_tokens, None);
    assert_eq!(summary.steps, Some(1));
}

#[test]
fn the_openagents_personal_access_token_is_redacted() {
    // The rule set ported from the TypeScript CLI has no `oa_pat_` pattern, so this
    // token survived a redaction that reported success. Issue #95's own probe file
    // plants it, and it is the credential that reaches openagents.com.
    for token in [
        "oa_pat_REALTOKEN456",
        "oa_agent_abc123def456",
        "smct_abcdef123456",
        "oa-x-abcd1234",
    ] {
        let redacted = redact_text(&format!("token: {}", token), "");
        assert!(
            !redacted.text.contains(token),
            "`{}` survived redaction",
            token
        );
        assert!(redacted.total > 0);
    }
}
