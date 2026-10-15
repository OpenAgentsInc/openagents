use code_index::episodes::{self, Activity, Corpus, Limits, Phase};
use coder_history::Harness;
use serde_json::{Value, json};
use std::path::Path;

fn decode(harness: Harness, rows: &[Value], tail: &str) -> Corpus {
    let bytes = rows.iter().map(|r| format!("{r}\n")).collect::<String>() + tail;
    let mut corpus = Corpus::default();
    episodes::decode(
        &mut corpus,
        "source",
        Some("native"),
        harness,
        false,
        bytes.as_bytes(),
        Path::new("/repo"),
        Limits::default(),
    )
    .unwrap();
    corpus
}

#[test]
fn claude_keeps_all_calls_deduplicates_and_tracks_recovery() {
    let rows = vec![
        json!({"type":"user","cwd":"/repo","message":{"role":"user","content":"Add a model"},"parentSessionId":"parent"}),
        json!({"type":"assistant","message":{"id":"request","usage":{"input_tokens":10,"output_tokens":0},"content":[
            {"type":"tool_use","id":"a","name":"Read","input":{"file_path":"a.rs"}},
            {"type":"tool_use","id":"b","name":"Edit","input":{}},
            {"type":"tool_use","id":"c","name":"Bash","input":{"command":"cargo test"}}]}}),
        json!({"type":"assistant","message":{"id":"request","usage":{"input_tokens":10,"output_tokens":8},"content":[{"type":"tool_use","id":"a","name":"Read","input":{}}]}}),
        json!({"type":"user","message":{"content":[{"type":"tool_result","tool_use_id":"c","is_error":true,"content":"failed"}]}}),
        json!({"type":"assistant","message":{"content":[{"type":"tool_use","id":"d","name":"Grep","input":{}}]}}),
        json!({"type":"new-provider-record","secret":"LOCAL_ONLY"}),
    ];
    let corpus = decode(Harness::Claude, &rows, "{\"unfinished\":");
    let e = &corpus.episodes[0];
    assert_eq!(e.family, "parent");
    assert!(e.revision.is_none());
    assert_eq!(e.operations.len(), 4);
    assert_eq!(e.operations[0].phase, Phase::Discovery);
    assert_eq!(e.operations[1].phase, Phase::Editing);
    assert_eq!(e.operations[3].phase, Phase::Recovery);
    assert_eq!((e.usage.prompt, e.usage.completion), (10, 8));
    assert_eq!(corpus.unknown.len(), 2);
    assert!(
        !episodes::summary(&corpus)
            .to_string()
            .contains("LOCAL_ONLY")
    );
}

#[test]
fn codex_deltas_and_changed_working_directory_are_explicit() {
    let corpus = decode(
        Harness::Codex,
        &[
            json!({"type":"session_meta","payload":{"cwd":"/repo","git":{"commit_hash":"abc"}}}),
            json!({"type":"response_item","payload":{"role":"user","content":[{"text":"Where is it?"}]}}),
            json!({"type":"event_msg","payload":{"type":"token_count","info":{"total_token_usage":{"input_tokens":100,"output_tokens":20}}}}),
            json!({"type":"event_msg","payload":{"type":"token_count","info":{"total_token_usage":{"input_tokens":100,"output_tokens":20}}}}),
            json!({"type":"token_usage_record","payload":{"thread_token_usage":{"input_tokens":120,"output_tokens":25}}}),
            json!({"type":"response_item","payload":{"type":"custom_tool_call","call_id":"a","name":"exec","input":"tools.exec_command({cmd:\"rg model crates\"})"}}),
            json!({"type":"turn_context","payload":{"cwd":"/other"}}),
            json!({"type":"response_item","payload":{"type":"function_call","call_id":"private","name":"Read","arguments":"{}"}}),
        ],
        "",
    );
    let e = &corpus.episodes[0];
    assert_eq!((e.usage.prompt, e.usage.completion), (120, 25));
    assert_eq!(e.operations.len(), 1);
    assert_eq!(e.operations[0].activity, Activity::Search);
    assert_eq!(e.revision.as_deref(), Some("abc"));
}

#[test]
fn atif_calls_and_usage_use_the_shared_types() {
    let session = atif::Session::opening("s", "test", "test", "/repo", "test");
    let mut step = atif::Step::said(atif::Source::Agent, "");
    step.tokens = Some((12, 3));
    step.call = Some(atif::Call {
        id: "a".into(),
        name: "shell".into(),
        arguments: json!({"command":"cat src/lib.rs"}),
        output: "source".into(),
        outcome: atif::Outcome::Completed,
        milliseconds: 4,
        purpose: None,
        extra: Default::default(),
    });
    let corpus = decode(
        Harness::Coder,
        &[
            json!({"record":"session","session":session}),
            json!({"record":"step","step":atif::Step::said(atif::Source::User,"Find the handler")}),
            json!({"record":"step","step":step}),
        ],
        "",
    );
    let e = &corpus.episodes[0];
    assert_eq!(e.usage.prompt, 12);
    assert_eq!(e.operations[0].duration_ms, Some(4));
    assert_eq!(e.operations[0].output_bytes, Some(6));
}

#[test]
fn selected_history_pages_reassemble_and_private_files_stay_private() {
    let dir = tempfile::tempdir().unwrap();
    let claude = dir.path().join("claude");
    std::fs::create_dir_all(claude.join("projects/p")).unwrap();
    std::fs::write(claude.join("projects/p/s.jsonl"),format!("{}\n{}\n",json!({"type":"user","cwd":"/repo","sessionId":"s","message":{"role":"user","content":"x"}}),json!({"type":"assistant","message":{"content":[{"type":"tool_use","id":"a","name":"Read","input":{"data":"x".repeat(50000)}}]}}))).unwrap();
    let history = coder_history::History::open(coder_history::Config {
        claude: Some(claude),
        ..Default::default()
    })
    .unwrap();
    let corpus = episodes::import(&history, Path::new("/repo"), Limits::default()).unwrap();
    assert_eq!(corpus.episodes.len(), 1);
    assert_eq!(corpus.episodes[0].operations.len(), 1);
    let out = dir.path().join("private");
    episodes::save(&out, &corpus).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            std::fs::metadata(out.join("episodes.json"))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
        assert_eq!(
            std::fs::metadata(out).unwrap().permissions().mode() & 0o777,
            0o700
        );
    }
}
