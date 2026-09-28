//! Catalog timing, ignored by default. Run with
//! `cargo test -p coder-history --release -- --ignored --nocapture catalog_bench`.
//!
//! It times a first page with nothing remembered (a process that just
//! started), a first page again, and the eight pages a phone reads, against
//! a synthetic history of thousands of sessions and, with
//! `CODER_HISTORY_BENCH_REAL=1`, the real `~/.claude` and `~/.codex`
//! (read-only).

use super::*;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

const CLAUDE_SESSIONS: usize = 3000;
const CODEX_SESSIONS: usize = 3000;

fn uuid(n: usize, salt: u32) -> String {
    format!("{salt:08x}-0000-4000-8000-{n:012x}")
}

/// A synthetic history shaped like a heavy user's: Claude sessions across 30
/// projects, each with a long first prompt and some replies, and Codex
/// sessions across 100 days, each with a ~20 KiB `session_meta` header.
fn synthesize(root: &Path) {
    let filler = "x".repeat(2048);
    for n in 0..CLAUDE_SESSIONS {
        let id = uuid(n, 0xc1a0);
        let dir = root.join(format!("claude/projects/-Users-me-project-{}", n % 30));
        fs::create_dir_all(&dir).unwrap();
        let mut body = format!(
            "{{\"type\":\"summary\",\"sessionId\":\"{id}\"}}\n\
             {{\"type\":\"user\",\"sessionId\":\"{id}\",\"entrypoint\":\"cli\",\
             \"message\":{{\"role\":\"user\",\"content\":\"Fix bug number {n} please\"}}}}\n"
        );
        for _ in 0..16 {
            body.push_str(&format!(
                "{{\"type\":\"assistant\",\"sessionId\":\"{id}\",\
                 \"message\":{{\"role\":\"assistant\",\"content\":\"{filler}\"}}}}\n"
            ));
        }
        fs::write(dir.join(format!("{id}.jsonl")), body).unwrap();
    }
    let instructions = "i".repeat(20 * 1024);
    let mut index = String::new();
    for n in 0..CODEX_SESSIONS {
        let id = uuid(n, 0xc0de);
        let dir = root.join(format!(
            "codex/sessions/2026/{:02}/{:02}",
            1 + n / 100 / 28,
            1 + (n / 100) % 28
        ));
        fs::create_dir_all(&dir).unwrap();
        let mut body = format!(
            "{{\"type\":\"session_meta\",\"payload\":{{\"id\":\"{id}\",\
             \"originator\":\"codex_cli_rs\",\"base_instructions\":{{\"text\":\"{instructions}\"}}}}}}\n\
             {{\"type\":\"response_item\",\"payload\":{{\"type\":\"message\",\"role\":\"user\",\
             \"content\":[{{\"type\":\"input_text\",\"text\":\"Codex task {n}\"}}]}}}}\n"
        );
        for _ in 0..8 {
            body.push_str(&format!(
                "{{\"type\":\"response_item\",\"payload\":{{\"type\":\"message\",\"role\":\"assistant\",\
                 \"content\":[{{\"type\":\"output_text\",\"text\":\"{filler}\"}}]}}}}\n"
            ));
        }
        fs::write(
            dir.join(format!("rollout-2026-01-01T00-00-00-{id}.jsonl")),
            body,
        )
        .unwrap();
        if n % 2 == 0 {
            index.push_str(&format!(
                "{{\"id\":\"{id}\",\"thread_name\":\"Codex thread {n}\",\"updated_at\":\"2026-01-01T00:00:00Z\"}}\n"
            ));
        }
    }
    fs::write(root.join("codex/session_index.jsonl"), index).unwrap();
}

fn timed<T>(f: impl FnOnce() -> T) -> (T, Duration) {
    let start = Instant::now();
    let value = f();
    (value, start.elapsed())
}

fn run(label: &str, config: impl Fn() -> Config, index: Option<PathBuf>) {
    let open = || {
        let history = History::open(config()).unwrap();
        match &index {
            Some(path) => history.with_catalog_index(path.clone()),
            None => history,
        }
    };
    let first = || {
        open()
            .catalog(CatalogRequest {
                cursor: None,
                limit: MAX_CATALOG_PAGE,
            })
            .unwrap()
    };
    catalog::forget();
    let (page, cold) = timed(first);
    let (_, warm) = timed(first);
    let all = || {
        timed(|| {
            let mut request = CatalogRequest {
                cursor: None,
                limit: MAX_CATALOG_PAGE,
            };
            let mut read = 0;
            for _ in 0..8 {
                let page = open().catalog(request.clone()).unwrap();
                read += 1;
                let Some(next) = page.next else { break };
                request.cursor = Some(next);
            }
            read
        })
    };
    // The first time, pages 2 to 8 read their untitled chats' first prompts.
    let (pages, eight) = all();
    let (_, again) = all();
    // A restart: nothing in memory, only what the index kept on disk.
    catalog::forget();
    let (_, restarted) = timed(first);
    let (_, restarted_all) = all();
    eprintln!(
        "{label}: {} entries on page 1; cold first page {cold:.1?}, warm first page {warm:.1?}, \
         {pages} pages first time {eight:.1?}, {pages} pages again {again:.1?}, \
         after restart: first page {restarted:.1?}, {pages} pages {restarted_all:.1?}",
        page.entries.len()
    );
}

#[test]
#[ignore = "timing benchmark; run explicitly with --ignored --nocapture"]
fn catalog_bench() {
    let root = std::env::temp_dir().join(format!("coder-history-bench-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).unwrap();
    synthesize(&root);
    let synthetic = || Config {
        codex: Some(root.join("codex")),
        claude: Some(root.join("claude")),
        coder: None,
        opencode: None,
        devin: None,
    };
    run("synthetic, in memory only", synthetic, None);
    run(
        "synthetic, indexed",
        synthetic,
        Some(root.join("catalog-index.json")),
    );
    if std::env::var_os("CODER_HISTORY_BENCH_REAL").is_some() {
        let home = PathBuf::from(std::env::var_os("HOME").unwrap());
        let real = || Config {
            codex: Some(home.join(".codex")),
            claude: Some(home.join(".claude")),
            coder: None,
            opencode: None,
            devin: None,
        };
        run("real, in memory only", real, None);
        run("real, indexed", real, Some(root.join("real-index.json")));
    }
    let _ = fs::remove_dir_all(&root);
}
