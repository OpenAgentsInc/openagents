//! Chat history to read: a deterministic synthetic one in Claude Code and
//! Codex formats, or this machine's own.

use serde_json::json;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

/// How much synthetic history to write.
#[derive(Clone, Copy, Debug)]
pub struct Scale {
    pub claude: usize,
    pub codex: usize,
    /// The newest Claude chat grows to this many bytes, so opening it shows
    /// what a long chat costs.
    pub large_bytes: usize,
}

impl Scale {
    /// Small enough to write and read in a few seconds.
    pub const CI: Self = Self {
        claude: 240,
        codex: 160,
        large_bytes: 24 * 1024 * 1024,
    };
    /// About the shape of a heavy user's machine.
    pub const LARGE: Self = Self {
        claude: 1000,
        codex: 2000,
        large_bytes: 256 * 1024 * 1024,
    };
}

/// A deterministic generator (xorshift64*), so every run writes the same bytes.
struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_f491_4f6c_dd1d)
    }
    fn range(&mut self, low: usize, high: usize) -> usize {
        low + (self.next() % (high - low) as u64) as usize
    }
    fn uuid(&mut self) -> String {
        let a = self.next();
        let b = self.next();
        format!(
            "{:08x}-{:04x}-4{:03x}-8{:03x}-{:012x}",
            a >> 32,
            (a >> 16) & 0xffff,
            a & 0xfff,
            b >> 52,
            b & 0xffff_ffff_ffff
        )
    }
    fn text(&mut self, low: usize, high: usize) -> String {
        let bytes = self.range(low, high);
        self.words(bytes)
    }
    fn words(&mut self, bytes: usize) -> String {
        const WORDS: [&str; 16] = [
            "the", "relay", "catalog", "page", "reads", "chat", "host", "phone", "cargo", "test",
            "passes", "layout", "row", "error", "fixed", "build",
        ];
        let mut out = String::with_capacity(bytes + 8);
        while out.len() < bytes {
            out.push_str(WORDS[(self.next() % 16) as usize]);
            out.push(if self.next().is_multiple_of(11) {
                '\n'
            } else {
                ' '
            });
        }
        out
    }
}

fn stamp(day: usize, second: usize) -> String {
    format!(
        "2026-09-{:02}T{:02}:{:02}:{:02}.000Z",
        (day % 28) + 1,
        (second / 3600) % 24,
        (second / 60) % 60,
        second % 60
    )
}

/// One Claude Code session: prompts, replies that call tools, and large tool
/// results, which is what most Claude session bytes are.
fn claude_session(rng: &mut Rng, id: &str, target: usize) -> Vec<u8> {
    let mut out = Vec::with_capacity(target + 4096);
    let mut turn = 0;
    while out.len() < target {
        let at = stamp(turn, turn * 37);
        let prompt = rng.text(40, 400);
        let records = [
            json!({"type":"user","message":{"role":"user","content":prompt},
                "uuid":rng.uuid(),"sessionId":id,"timestamp":at,"cwd":"/work/project","entrypoint":"cli"}),
            json!({"type":"assistant","message":{"role":"assistant","content":[
                {"type":"text","text":rng.text(80, 900)},
                {"type":"tool_use","id":format!("toolu_{turn}"),"name":"Bash","input":{"command":"cargo test -p coder-history"}}]},
                "uuid":rng.uuid(),"sessionId":id,"timestamp":at}),
            json!({"type":"user","message":{"role":"user","content":[
                {"type":"tool_result","tool_use_id":format!("toolu_{turn}"),"content":rng.text(400, 12_000)}]},
                "uuid":rng.uuid(),"sessionId":id,"timestamp":at}),
            json!({"type":"assistant","message":{"role":"assistant","content":[
                {"type":"text","text":rng.text(60, 1200)}]},
                "uuid":rng.uuid(),"sessionId":id,"timestamp":at}),
        ];
        for record in records {
            out.extend_from_slice(record.to_string().as_bytes());
            out.push(b'\n');
        }
        turn += 1;
    }
    out
}

/// One Codex session: a `session_meta` header with its instructions, then
/// messages, shell calls, and their output.
fn codex_session(rng: &mut Rng, id: &str, target: usize) -> Vec<u8> {
    let mut out = Vec::with_capacity(target + 32 * 1024);
    let header = json!({"timestamp":stamp(0, 0),"type":"session_meta","payload":{
        "id":id,"timestamp":stamp(0, 0),"cwd":"/work/project","originator":"codex_cli_rs",
        "cli_version":"0.40.0","source":"cli","instructions":rng.words(20 * 1024)}});
    out.extend_from_slice(header.to_string().as_bytes());
    out.push(b'\n');
    let mut turn = 0;
    while out.len() < target {
        let at = stamp(turn, turn * 41);
        let records = [
            json!({"timestamp":at,"type":"response_item","payload":{"type":"message","role":"user",
                "content":[{"type":"input_text","text":rng.text(40, 400)}]}}),
            json!({"timestamp":at,"type":"response_item","payload":{"type":"function_call","name":"shell",
                "arguments":"{\"command\":[\"cargo\",\"test\"]}","call_id":format!("call_{turn}")}}),
            json!({"timestamp":at,"type":"response_item","payload":{"type":"function_call_output",
                "call_id":format!("call_{turn}"),"output":rng.text(400, 8_000)}}),
            json!({"timestamp":at,"type":"response_item","payload":{"type":"message","role":"assistant",
                "content":[{"type":"output_text","text":rng.text(60, 1200)}]}}),
        ];
        for record in records {
            out.extend_from_slice(record.to_string().as_bytes());
            out.push(b'\n');
        }
        turn += 1;
    }
    out
}

fn write(path: &Path, bytes: &[u8], age: Duration) -> std::io::Result<()> {
    std::fs::create_dir_all(path.parent().expect("a parent"))?;
    std::fs::write(path, bytes)?;
    // The catalog lists newest first by each file's last write.
    let file = std::fs::File::options().write(true).open(path)?;
    file.set_modified(SystemTime::now() - age)
}

/// Write synthetic Claude and Codex history under `home` and return its
/// source configuration. The newest chat is the large one.
///
/// # Errors
/// When a file cannot be written.
pub fn synthetic(home: &Path, scale: Scale) -> std::io::Result<coder_history::Config> {
    let mut rng = Rng(0x9e37_79b9_7f4a_7c15);
    let claude = home.join(".claude");
    let codex = home.join(".codex");
    for index in 0..scale.claude {
        let id = rng.uuid();
        let project = format!("-work-project-{}", index % 24);
        let size = if index == 0 {
            scale.large_bytes
        } else {
            rng.range(8 * 1024, 160 * 1024)
        };
        let bytes = claude_session(&mut rng, &id, size);
        let age = Duration::from_secs(60 * (index as u64 * 7 + 1));
        write(
            &claude
                .join("projects")
                .join(project)
                .join(format!("{id}.jsonl")),
            &bytes,
            age,
        )?;
    }
    let mut index_lines = String::new();
    for index in 0..scale.codex {
        let id = rng.uuid();
        let size = rng.range(32 * 1024, 200 * 1024);
        let bytes = codex_session(&mut rng, &id, size);
        let age = Duration::from_secs(60 * (index as u64 * 11 + 3));
        let day = 1 + index % 28;
        write(
            &codex
                .join("sessions/2026/09")
                .join(format!("{day:02}"))
                .join(format!("rollout-2026-09-{day:02}T10-00-00-{id}.jsonl")),
            &bytes,
            age,
        )?;
        if index % 2 == 0 {
            index_lines.push_str(
                &json!({"id":id,"thread_name":format!("Codex chat {index}"),"updated_at":stamp(index, 0)})
                    .to_string(),
            );
            index_lines.push('\n');
        }
    }
    std::fs::create_dir_all(&codex)?;
    std::fs::write(codex.join("session_index.jsonl"), index_lines)?;
    Ok(coder_history::Config {
        codex: Some(codex),
        claude: Some(claude),
        coder: None,
        opencode: None,
        devin: None,
    })
}

/// One Coder task attempt's ATIF transcript, shaped like the ones on a
/// working host: a session record, the prompt, then mostly `System` adapter
/// records (admission, controller, capabilities) that show no row, a few
/// `Agent` replies, and an end record.
fn coder_task(rng: &mut Rng, task: &str, target: usize) -> Vec<u8> {
    let mut out = Vec::with_capacity(target + 4096);
    let mut at: u64 = 1_790_000_000_000;
    let line = |out: &mut Vec<u8>, value: serde_json::Value| {
        out.extend_from_slice(value.to_string().as_bytes());
        out.push(b'\n');
    };
    line(
        &mut out,
        json!({"record":"session","schema_version":"ATIF-v1.7","at":at,
        "session":{"id":task,"model":"model","door":"door","repository":"repo",
        "directive":"task","state":"running","seconds":0,"version":"1"}}),
    );
    line(
        &mut out,
        json!({"record":"step","step":{"at":at,"source":"User","message":rng.text(60, 600)}}),
    );
    let mut replies = 0;
    while out.len() < target {
        at += 1_000;
        if rng.next().is_multiple_of(40) && replies < 4 {
            replies += 1;
            line(
                &mut out,
                json!({"record":"step","step":{"at":at,"source":"Agent","message":rng.text(100, 1500)}}),
            );
            continue;
        }
        line(
            &mut out,
            json!({"record":"step","step":{"at":at,"source":"System","message":"adapter",
            "extensions":{"admission":{"grant":rng.uuid(),"note":rng.text(100, 400)},
            "controller":{"state":"running","detail":rng.text(50, 300)},
            "capabilities":{"tools":rng.text(40, 200)}}}}),
        );
    }
    line(
        &mut out,
        json!({"record":"step","step":{"at":at + 1,"source":"Agent","message":rng.text(100, 1500)}}),
    );
    line(
        &mut out,
        json!({"record":"end","at":at + 2,"state":"completed"}),
    );
    out
}

/// Write `count` synthetic Coder task transcripts under `home` and return a
/// configuration with only the Coder source, as a phone that shows only
/// Coder chats reads it.
///
/// # Errors
/// When a file cannot be written.
pub fn coder(home: &Path, count: usize) -> std::io::Result<coder_history::Config> {
    let mut rng = Rng(0x51_7cc1_b727_220a);
    let tasks = home.join(".openagents/tasks");
    for index in 0..count {
        let task = format!(
            "{:016x}{:016x}{:016x}{:016x}",
            rng.next(),
            rng.next(),
            rng.next(),
            rng.next()
        );
        let size = rng.range(40 * 1024, 400 * 1024);
        let bytes = coder_task(&mut rng, &task, size);
        let age = Duration::from_secs(60 * (index as u64 * 13 + 1));
        write(&tasks.join(format!("{task}.1.atif.jsonl")), &bytes, age)?;
    }
    Ok(coder_history::Config {
        coder: Some(tasks),
        ..coder_history::Config::default()
    })
}

/// This machine's Coder task transcripts only (`~/.openagents/tasks`).
pub fn real_coder() -> Option<coder_history::Config> {
    let home = PathBuf::from(std::env::var_os("HOME")?);
    let tasks = home.join(".openagents/tasks");
    tasks.is_dir().then(|| coder_history::Config {
        coder: Some(tasks),
        ..coder_history::Config::default()
    })
}

/// This machine's history, as `coder host serve` reads it
/// (`coder_host::tailnet::default_sources`): `~/.codex`, `~/.claude`,
/// `~/.openagents/tasks`, and the OpenCode and Devin mirrors when present.
pub fn real() -> Option<coder_history::Config> {
    let home = PathBuf::from(std::env::var_os("HOME")?);
    let dir = |relative: &str| Some(home.join(relative)).filter(|p| p.is_dir());
    let config = coder_history::Config {
        codex: dir(".codex"),
        claude: dir(".claude"),
        coder: dir(".openagents/tasks"),
        opencode: dir(".openagents/opencode/mirror"),
        devin: dir(".openagents/devin/mirror"),
    };
    (config.codex.is_some() || config.claude.is_some() || config.coder.is_some()).then_some(config)
}

/// JSONL files and bytes under each configured root, for the report.
pub fn describe(config: &coder_history::Config) -> String {
    fn walk(path: &Path, files: &mut u64, bytes: &mut u64, largest: &mut u64) {
        let Ok(entries) = std::fs::read_dir(path) else {
            return;
        };
        for entry in entries.flatten() {
            let Ok(kind) = entry.file_type() else {
                continue;
            };
            if kind.is_dir() {
                walk(&entry.path(), files, bytes, largest);
            } else if kind.is_file() && entry.path().extension().is_some_and(|e| e == "jsonl") {
                let size = entry.metadata().map_or(0, |m| m.len());
                *files += 1;
                *bytes += size;
                *largest = (*largest).max(size);
            }
        }
    }
    let mut parts = vec![];
    for (name, root, subdirs) in [
        (
            "Codex",
            &config.codex,
            &["sessions", "archived_sessions"][..],
        ),
        ("Claude", &config.claude, &["projects"][..]),
        ("Coder", &config.coder, &[""][..]),
        ("OpenCode", &config.opencode, &[""][..]),
        ("Devin", &config.devin, &[""][..]),
    ] {
        let Some(root) = root else { continue };
        let (mut files, mut bytes, mut largest) = (0, 0, 0);
        for sub in subdirs {
            walk(&root.join(sub), &mut files, &mut bytes, &mut largest);
        }
        parts.push(format!(
            "{name}: {files} files, {:.1} GB, largest {:.0} MB",
            bytes as f64 / 1e9,
            largest as f64 / 1e6
        ));
    }
    parts.join("; ")
}
