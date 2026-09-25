//! Lexical provenance (issue #9655): phrases in live policies and code
//! that also appear in the text of a task a policy was tuned on.
//!
//! The task-id, test-name, and anatomy-fact references can't see a
//! task's lexicon. A marker list can take "accounts for" from one task's
//! code comment and pass them all. This check builds a reference corpus
//! from the comments, docstrings, and instruction of every tuned task
//! ([`TUNED_PATH`] declares which), and compares two kinds of text with
//! it:
//!
//! - **Phrase-list entries.** Every string literal in a `const` or
//!   `static` array of string literals (`[&str; N]` or `&[&str]`) in the
//!   scanned Rust, compared whole when it has at most [`ENTRY_WORDS`]
//!   words.
//! - **Prose.** Every text the static check reads (policy manifests and
//!   their notes, prompt files, the Jev question sets under `questions/`,
//!   and every string literal, so the lean loop's guidance too), compared
//!   in windows of [`WINDOW`] consecutive words.
//!
//! Words are the lowercased runs of letters and digits, so `non-degenerate`
//! is two words and `node_modules` is two. A **content word** is
//! alphabetic, at least three letters long, and not in [`STOP_WORDS`].
//! A phrase is **distinctive** when fewer than [`GENERAL_TASKS`] of all
//! the retained upstream tasks use it, and:
//!
//! - an entry has at least two pieces separated by white space that hold
//!   a letter or a digit, so a phrase and not one word or one identifier,
//!   and a content word;
//! - a prose window has [`WINDOW_CONTENT`] content words.
//!
//! A distinctive match needs a provenance entry in [`PROVENANCE_PATH`];
//! without one, it fails the check. With one, it passes and is listed,
//! and its task can't count as evidence for the policy or pattern the
//! phrase belongs to. A single word is never distinctive on its own: a
//! common English word can't show where it was copied from. A provenance
//! entry can still declare that one was taken from a task, and the check
//! then confirms that the task's text holds it and lists it with the rest.
//!
//! The corpus stores no benchmark text. Each task keeps the SHA-256
//! digest of every file it read and the first [`HASH_HEX`] hexadecimal
//! digits of the SHA-256 digest of each distinctive normalized phrase.
//! The upstream task files carry a training canary, so their words stay
//! out of the repository even though their license would allow them.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::path::{Path, PathBuf};

use regex::Regex;
use serde::Serialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use super::{Text, files_under, read_json, relative, rust_literals, tokens};

/// The tuned-task declaration's schema.
pub const TUNED_SCHEMA: &str = "openagents.coder-one.tuned-tasks.v1";
/// The tuned-task declaration, relative to the repository root.
pub const TUNED_PATH: &str = "crates/coder-one/contamination-tuned.json";
/// The reference corpus's schema.
pub const LEXICON_SCHEMA: &str = "openagents.coder-one.tuned-lexicon.v1";
/// The reference corpus, relative to the repository root.
pub const LEXICON_PATH: &str = "bench/terminal-bench/reference/tuned-lexicon.json";
/// The provenance file's schema.
pub const PROVENANCE_SCHEMA: &str = "openagents.coder-one.contamination-provenance.v1";
/// The provenance file, relative to the repository root.
pub const PROVENANCE_PATH: &str = "crates/coder-one/contamination-provenance.json";
/// The upstream task directory `lexicon` reads by default, under `HOME`.
pub const DEFAULT_TASKS: &str = ".openagents/terminal-bench/upstream/terminal-bench-v4.0.0/tasks";
/// A phrase that at least this many upstream tasks use is general
/// vocabulary, not a task's lexicon.
pub const GENERAL_TASKS: usize = 5;
/// The most words a phrase-list entry has to be compared whole.
pub const ENTRY_WORDS: usize = 3;
/// How many consecutive words of prose make a window.
pub const WINDOW: usize = 4;
/// How many content words a prose window needs to be distinctive.
pub const WINDOW_CONTENT: usize = 3;
/// The shortest word prefix the corpus keeps, so a stem such as
/// `simplif` can match the word it begins.
pub const MIN_PREFIX: usize = 5;
/// How many hexadecimal digits of a phrase's digest the corpus keeps.
pub const HASH_HEX: usize = 12;
/// The largest task file the corpus reads; larger files are data or
/// bundles.
const MAX_SOURCE_BYTES: u64 = 256 * 1024;

/// General English function words. A phrase made only of these, and of
/// words shorter than three letters or with digits, isn't distinctive.
pub const STOP_WORDS: [&str; 147] = [
    "about",
    "above",
    "after",
    "again",
    "against",
    "ain",
    "all",
    "also",
    "and",
    "any",
    "are",
    "aren",
    "because",
    "been",
    "before",
    "being",
    "below",
    "between",
    "both",
    "but",
    "can",
    "cannot",
    "could",
    "couldn",
    "did",
    "didn",
    "does",
    "doesn",
    "doing",
    "don",
    "down",
    "during",
    "each",
    "either",
    "else",
    "etc",
    "even",
    "ever",
    "every",
    "few",
    "for",
    "from",
    "further",
    "had",
    "hadn",
    "has",
    "hasn",
    "have",
    "haven",
    "having",
    "her",
    "here",
    "hers",
    "herself",
    "him",
    "himself",
    "his",
    "how",
    "however",
    "into",
    "isn",
    "its",
    "itself",
    "just",
    "let",
    "may",
    "might",
    "mightn",
    "more",
    "most",
    "must",
    "mustn",
    "myself",
    "need",
    "needn",
    "neither",
    "nor",
    "not",
    "now",
    "off",
    "once",
    "one",
    "only",
    "other",
    "otherwise",
    "our",
    "ours",
    "ourselves",
    "out",
    "over",
    "own",
    "per",
    "same",
    "shall",
    "shan",
    "she",
    "should",
    "shouldn",
    "since",
    "some",
    "such",
    "than",
    "that",
    "the",
    "their",
    "theirs",
    "them",
    "themselves",
    "then",
    "there",
    "these",
    "they",
    "this",
    "those",
    "though",
    "through",
    "thus",
    "too",
    "under",
    "until",
    "upon",
    "very",
    "via",
    "was",
    "wasn",
    "were",
    "weren",
    "what",
    "when",
    "where",
    "whether",
    "which",
    "while",
    "who",
    "whom",
    "why",
    "will",
    "with",
    "within",
    "without",
    "won",
    "would",
    "wouldn",
    "yet",
    "you",
    "your",
    "yours",
];

/// Whether a normalized word carries content.
#[must_use]
pub fn content_word(word: &str) -> bool {
    word.chars().count() >= 3
        && word.chars().all(char::is_alphabetic)
        && !STOP_WORDS.contains(&word)
}

/// The corpus's digest of one normalized phrase.
#[must_use]
pub fn phrase_hash(phrase: &str) -> String {
    let digest = Sha256::digest(phrase.as_bytes());
    let mut hex = String::with_capacity(HASH_HEX);
    for byte in digest {
        if hex.len() >= HASH_HEX {
            break;
        }
        hex.push_str(&format!("{byte:02x}"));
    }
    hex.truncate(HASH_HEX);
    hex
}

fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn words(text: &str) -> Vec<String> {
    tokens(text, false)
        .into_iter()
        .map(|(_, _, word)| word)
        .collect()
}

/// The distinctive-candidate phrases of one text, in plain words: every
/// 1-to-[`ENTRY_WORDS`]-word run with a content word, and every
/// [`WINDOW`]-word run with [`WINDOW_CONTENT`] content words. The corpus
/// keeps those that fewer than [`GENERAL_TASKS`] tasks use.
fn candidate_phrases(text: &str, out: &mut HashSet<String>) {
    let words = words(text);
    let content: Vec<bool> = words.iter().map(|word| content_word(word)).collect();
    for n in 1..=ENTRY_WORDS {
        for (index, window) in words.windows(n).enumerate() {
            if content[index..index + n].iter().any(|&c| c) {
                out.insert(window.join(" "));
            }
        }
    }
    for (index, window) in words.windows(WINDOW).enumerate() {
        if content[index..index + WINDOW]
            .iter()
            .filter(|&&c| c)
            .count()
            >= WINDOW_CONTENT
        {
            out.insert(window.join(" "));
        }
    }
}

/// The prefixes, at least [`MIN_PREFIX`] letters and shorter than the
/// word, of every content word in a text.
fn candidate_prefixes(text: &str, out: &mut HashSet<String>) {
    for word in words(text) {
        if !content_word(&word) {
            continue;
        }
        let letters: Vec<char> = word.chars().collect();
        for length in MIN_PREFIX..letters.len() {
            out.insert(letters[..length].iter().collect());
        }
    }
}

/// How a task file's comments are written, from its name.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Syntax {
    /// `#` line comments: shell, Python, YAML, TOML, Dockerfile, Perl,
    /// R, Turtle.
    Hash,
    /// Python: `#` comments and triple-quoted strings (docstrings).
    Python,
    /// `//` and `/* */`: C, C++, CUDA, JavaScript, TypeScript, Rust, Go,
    /// Java, Scala, CSS.
    Slash,
    /// `(* *)`: Coq.
    Coq,
    /// `--` and `/- -/`: Lean.
    Lean,
    /// `--`: SQL.
    Sql,
    /// `'` line comments: Visual Basic.
    Basic,
    /// The whole file is prose: Markdown and README files.
    Prose,
}

fn syntax(path: &Path) -> Option<Syntax> {
    let name = path.file_name()?.to_string_lossy().to_lowercase();
    if name == "dockerfile" || name.starts_with("dockerfile.") {
        return Some(Syntax::Hash);
    }
    if name.starts_with("readme") {
        return Some(Syntax::Prose);
    }
    let ext = name.rsplit_once('.')?.1;
    Some(match ext {
        "py" => Syntax::Python,
        "sh" | "bash" | "yaml" | "yml" | "toml" | "pm" | "pl" | "r" | "ttl" | "cfg" | "ini" => {
            Syntax::Hash
        }
        "c" | "cc" | "cpp" | "h" | "hpp" | "cu" | "js" | "jsx" | "mjs" | "cjs" | "ts" | "tsx"
        | "rs" | "go" | "java" | "scala" | "sbt" | "css" | "kt" | "swift" => Syntax::Slash,
        "v" => Syntax::Coq,
        "lean" => Syntax::Lean,
        "sql" => Syntax::Sql,
        "bas" | "cls" | "frm" => Syntax::Basic,
        "md" | "rst" => Syntax::Prose,
        _ => return None,
    })
}

struct Extractors {
    hash: Regex,
    triple: Regex,
    line_slash: Regex,
    block_slash: Regex,
    coq: Regex,
    dashes: Regex,
    lean_block: Regex,
    quote: Regex,
}

impl Extractors {
    fn new() -> Self {
        let re = |pattern: &str| Regex::new(pattern).expect("a fixed pattern compiles");
        Self {
            hash: re(r"#([^\n]*)"),
            triple: re(r#"(?s)"""(.*?)"""|'''(.*?)'''"#),
            line_slash: re(r"//([^\n]*)"),
            block_slash: re(r"(?s)/\*(.*?)\*/"),
            coq: re(r"(?s)\(\*(.*?)\*\)"),
            dashes: re(r"--([^\n]*)"),
            lean_block: re(r"(?s)/-(.*?)-/"),
            quote: re(r"'([^\n]*)"),
        }
    }

    /// The comments, docstrings, or prose of one file.
    fn prose(&self, syntax: Syntax, text: &str) -> Vec<String> {
        let captures = |re: &Regex| -> Vec<String> {
            re.captures_iter(text)
                .filter_map(|c| {
                    c.iter()
                        .skip(1)
                        .flatten()
                        .next()
                        .map(|m| m.as_str().to_owned())
                })
                .collect()
        };
        match syntax {
            Syntax::Hash => captures(&self.hash),
            Syntax::Python => {
                let mut out = captures(&self.hash);
                out.extend(captures(&self.triple));
                out
            }
            Syntax::Slash => {
                let mut out = captures(&self.line_slash);
                out.extend(captures(&self.block_slash));
                out
            }
            Syntax::Coq => captures(&self.coq),
            Syntax::Lean => {
                let mut out = captures(&self.dashes);
                out.extend(captures(&self.lean_block));
                out
            }
            Syntax::Sql => captures(&self.dashes),
            Syntax::Basic => captures(&self.quote),
            Syntax::Prose => vec![text.to_owned()],
        }
    }
}

/// One upstream task's reference text: its instruction and the comments,
/// docstrings, and prose files of its `environment/`, the workspace an
/// agent receives. Its verifier tests and reference solution aren't read.
#[derive(Clone, Debug, Default)]
struct TaskText {
    /// `path relative to the task`, SHA-256 of the whole file.
    sources: Vec<(String, String)>,
    phrases: HashSet<String>,
    prefixes: HashSet<String>,
}

fn read_task(dir: &Path, extract: &Extractors) -> TaskText {
    let mut task = TaskText::default();
    let mut files = Vec::new();
    let instruction = dir.join("instruction.md");
    if instruction.is_file() {
        files.push((instruction, Some(Syntax::Prose)));
    }
    let mut environment = Vec::new();
    files_under(
        &dir.join("environment"),
        &|path| syntax(path).is_some(),
        &mut environment,
    );
    files.extend(environment.into_iter().map(|path| {
        let syntax = syntax(&path);
        (path, syntax)
    }));
    for (path, syntax) in files {
        let Some(syntax) = syntax else { continue };
        if std::fs::metadata(&path).map_or(true, |meta| meta.len() > MAX_SOURCE_BYTES) {
            continue;
        }
        let Ok(bytes) = std::fs::read(&path) else {
            continue;
        };
        let Ok(text) = std::str::from_utf8(&bytes) else {
            continue;
        };
        let pieces = extract.prose(syntax, text);
        if pieces.iter().all(|piece| piece.trim().is_empty()) {
            continue;
        }
        task.sources
            .push((relative(dir, &path), sha256_hex(&bytes)));
        for piece in pieces {
            candidate_phrases(&piece, &mut task.phrases);
            candidate_prefixes(&piece, &mut task.prefixes);
        }
    }
    task
}

/// One tuned task as the declaration names it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Tuned {
    pub task: String,
    pub roles: Vec<String>,
}

/// Reads the tuned-task declaration.
///
/// # Errors
/// The file is missing or isn't the declaration.
pub fn tuned_tasks(root: &Path) -> Result<Vec<Tuned>, String> {
    let path = root.join(TUNED_PATH);
    let doc = read_json(&path)?;
    if doc["schema"].as_str() != Some(TUNED_SCHEMA) {
        return Err(format!("{} isn't {TUNED_SCHEMA}", path.display()));
    }
    let roles: BTreeSet<&str> = doc["roles"]
        .as_object()
        .map(|roles| roles.keys().map(String::as_str).collect())
        .unwrap_or_default();
    let mut out = Vec::new();
    for entry in doc["tasks"].as_array().into_iter().flatten() {
        let Some(task) = entry["task"].as_str() else {
            return Err(format!("{}: every task needs a name", path.display()));
        };
        let task_roles: Vec<String> = entry["roles"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .map(str::to_owned)
            .collect();
        if task_roles.is_empty() || task_roles.iter().any(|r| !roles.contains(r.as_str())) {
            return Err(format!(
                "{}: {task} needs roles from the declared roles",
                path.display()
            ));
        }
        out.push(Tuned {
            task: task.to_owned(),
            roles: task_roles,
        });
    }
    Ok(out)
}

/// Builds the reference corpus: reads every task under `tasks_dir` for
/// the general-vocabulary counts, and keeps the distinctive phrases of
/// the declared tuned tasks, as digests.
///
/// # Errors
/// The declaration can't be read, or `tasks_dir` holds no tasks.
pub fn build(root: &Path, tasks_dir: &Path) -> Result<Value, String> {
    let tuned = tuned_tasks(root)?;
    let mut dirs: Vec<PathBuf> = std::fs::read_dir(tasks_dir)
        .map_err(|error| format!("can't read {}: {error}", tasks_dir.display()))?
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.join("instruction.md").is_file())
        .collect();
    dirs.sort();
    if dirs.is_empty() {
        return Err(format!("{} holds no tasks", tasks_dir.display()));
    }
    let extract = Extractors::new();
    let mut texts: BTreeMap<String, TaskText> = BTreeMap::new();
    for dir in &dirs {
        let name = dir
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        texts.insert(name, read_task(dir, &extract));
    }
    let mut phrase_counts: HashMap<&str, usize> = HashMap::new();
    let mut prefix_counts: HashMap<&str, usize> = HashMap::new();
    for task in texts.values() {
        for phrase in &task.phrases {
            *phrase_counts.entry(phrase).or_default() += 1;
        }
        for prefix in &task.prefixes {
            *prefix_counts.entry(prefix).or_default() += 1;
        }
    }
    let mut tasks = serde_json::Map::new();
    let mut missing = Vec::new();
    for Tuned { task, roles } in &tuned {
        let Some(text) = texts.get(task) else {
            missing.push(json!({"task": task, "roles": roles}));
            continue;
        };
        let keep = |set: &HashSet<String>, counts: &HashMap<&str, usize>| {
            let hashes: BTreeSet<String> = set
                .iter()
                .filter(|phrase| counts.get(phrase.as_str()).copied().unwrap_or(0) < GENERAL_TASKS)
                .map(|phrase| phrase_hash(phrase))
                .collect();
            hashes.into_iter().collect::<Vec<_>>().join(" ")
        };
        let sources: Vec<Value> = text
            .sources
            .iter()
            .map(|(path, sha)| json!({"path": path, "sha256": sha}))
            .collect();
        tasks.insert(
            task.clone(),
            json!({
                "roles": roles,
                "sources": sources,
                "phrases": keep(&text.phrases, &phrase_counts),
                "prefixes": keep(&text.prefixes, &prefix_counts),
            }),
        );
    }
    Ok(json!({
        "schema": LEXICON_SCHEMA,
        "note": "The distinctive phrases of each tuned task's instruction and workspace comments, \
                 docstrings, and prose files, stored as digests: the first 12 hexadecimal digits \
                 of the SHA-256 of each phrase's lowercased words joined by one space. No task text \
                 is stored. `coder-one contamination check` compares phrase lists and prose with \
                 it; regenerate with `coder-one contamination lexicon`.",
        "rules": {
            "entry_words": ENTRY_WORDS,
            "window": WINDOW,
            "window_content_words": WINDOW_CONTENT,
            "general_tasks": GENERAL_TASKS,
            "min_prefix": MIN_PREFIX,
            "hash_hex": HASH_HEX,
            "max_source_bytes": MAX_SOURCE_BYTES,
        },
        "background_tasks": texts.len(),
        "tasks": tasks,
        "missing": missing,
    }))
}

/// The reference corpus, loaded.
#[derive(Clone, Debug, Default)]
pub struct Lexicon {
    tasks: Vec<String>,
    phrases: HashMap<String, Vec<usize>>,
    prefixes: HashMap<String, Vec<usize>>,
    missing: Vec<String>,
    digest: String,
}

impl Lexicon {
    /// A corpus from plain phrases per task, for tests: each task's text
    /// is taken as-is, with no general-vocabulary filter.
    #[must_use]
    pub fn from_texts<'a>(texts: impl IntoIterator<Item = (&'a str, &'a str)>) -> Self {
        let mut lexicon = Self::default();
        for (task, text) in texts {
            let index = lexicon.tasks.len();
            lexicon.tasks.push(task.to_owned());
            let mut phrases = HashSet::new();
            let mut prefixes = HashSet::new();
            candidate_phrases(text, &mut phrases);
            candidate_prefixes(text, &mut prefixes);
            for phrase in phrases {
                lexicon
                    .phrases
                    .entry(phrase_hash(&phrase))
                    .or_default()
                    .push(index);
            }
            for prefix in prefixes {
                lexicon
                    .prefixes
                    .entry(phrase_hash(&prefix))
                    .or_default()
                    .push(index);
            }
        }
        lexicon
    }

    /// Reads [`LEXICON_PATH`] under `root`.
    ///
    /// # Errors
    /// The corpus is missing or isn't the corpus.
    pub fn load(root: &Path) -> Result<Self, String> {
        let path = root.join(LEXICON_PATH);
        let bytes = std::fs::read(&path).map_err(|error| {
            format!(
                "can't read {}: {error}; build it with `coder-one contamination lexicon`",
                path.display()
            )
        })?;
        let doc: Value = serde_json::from_slice(&bytes)
            .map_err(|error| format!("{} isn't JSON: {error}", path.display()))?;
        if doc["schema"].as_str() != Some(LEXICON_SCHEMA) {
            return Err(format!("{} isn't {LEXICON_SCHEMA}", path.display()));
        }
        let mut lexicon = Self {
            digest: format!("sha256:{}", sha256_hex(&bytes)),
            ..Self::default()
        };
        for (task, body) in doc["tasks"].as_object().into_iter().flatten() {
            let index = lexicon.tasks.len();
            lexicon.tasks.push(task.clone());
            for (field, map) in [
                ("phrases", &mut lexicon.phrases),
                ("prefixes", &mut lexicon.prefixes),
            ] {
                for hash in body[field].as_str().unwrap_or_default().split_whitespace() {
                    map.entry(hash.to_owned()).or_default().push(index);
                }
            }
        }
        lexicon.missing = doc["missing"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|m| m["task"].as_str().map(str::to_owned))
            .collect();
        Ok(lexicon)
    }

    fn tasks_of(&self, indices: &[usize]) -> Vec<String> {
        let names: BTreeSet<&String> = indices.iter().filter_map(|&i| self.tasks.get(i)).collect();
        names.into_iter().cloned().collect()
    }

    /// The tuned tasks whose text holds a phrase-list entry, when the
    /// entry is distinctive: at least two words apart by white space.
    #[must_use]
    pub fn entry_tasks(&self, entry: &str) -> Vec<String> {
        let pieces = entry
            .split_whitespace()
            .filter(|piece| piece.chars().any(char::is_alphanumeric))
            .count();
        if pieces < 2 {
            return Vec::new();
        }
        self.holding(entry)
    }

    /// The tuned tasks whose text holds an entry of at most
    /// [`ENTRY_WORDS`] words with a content word, whatever its length. An
    /// entry whose text ends inside a word, such as `simplif`, also
    /// matches the words it begins.
    #[must_use]
    pub fn holding(&self, entry: &str) -> Vec<String> {
        let words = words(entry);
        if words.is_empty() || words.len() > ENTRY_WORDS || !words.iter().any(|w| content_word(w)) {
            return Vec::new();
        }
        let phrase = words.join(" ");
        let mut indices: Vec<usize> = self
            .phrases
            .get(&phrase_hash(&phrase))
            .cloned()
            .unwrap_or_default();
        let open_ended = entry.chars().last().is_some_and(char::is_alphanumeric);
        if words.len() == 1 && open_ended && phrase.chars().count() >= MIN_PREFIX {
            indices.extend(
                self.prefixes
                    .get(&phrase_hash(&phrase))
                    .into_iter()
                    .flatten(),
            );
        }
        self.tasks_of(&indices)
    }

    /// The distinctive windows of a prose text that a tuned task's text
    /// holds: per task, each run of overlapping matched windows as its
    /// words and its byte span.
    #[must_use]
    pub fn prose_matches(&self, text: &str) -> Vec<(String, String, usize, usize)> {
        let tokens = tokens(text, false);
        if tokens.len() < WINDOW {
            return Vec::new();
        }
        let content: Vec<bool> = tokens.iter().map(|(_, _, w)| content_word(w)).collect();
        // Task, then the matched window starts.
        let mut hits: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
        let mut key = String::new();
        for start in 0..=tokens.len() - WINDOW {
            if content[start..start + WINDOW]
                .iter()
                .filter(|&&c| c)
                .count()
                < WINDOW_CONTENT
            {
                continue;
            }
            key.clear();
            for (offset, (_, _, word)) in tokens[start..start + WINDOW].iter().enumerate() {
                if offset > 0 {
                    key.push(' ');
                }
                key.push_str(word);
            }
            if let Some(indices) = self.phrases.get(&phrase_hash(&key)) {
                for &index in indices {
                    hits.entry(index).or_default().push(start);
                }
            }
        }
        let mut out = Vec::new();
        for (index, starts) in hits {
            let mut runs: Vec<(usize, usize)> = Vec::new();
            for start in starts {
                match runs.last_mut() {
                    Some((_, end)) if start <= *end => *end = start + WINDOW,
                    _ => runs.push((start, start + WINDOW)),
                }
            }
            for (from, to) in runs {
                let phrase = tokens[from..to]
                    .iter()
                    .map(|(_, _, w)| w.as_str())
                    .collect::<Vec<_>>()
                    .join(" ");
                out.push((
                    self.tasks[index].clone(),
                    phrase,
                    tokens[from].0,
                    tokens[to - 1].1,
                ));
            }
        }
        out
    }
}

/// One array of string literals in the scanned Rust.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PhraseList {
    /// `path::NAME`.
    pub name: String,
    pub path: String,
    /// Each entry's line and text.
    pub entries: Vec<(usize, String)>,
}

/// The `const` and `static` arrays of string literals in one Rust file,
/// less those inside `#[cfg(test)]` items.
#[must_use]
pub fn phrase_lists(path: &str, source: &str) -> Vec<PhraseList> {
    static DECLARATION: std::sync::OnceLock<Regex> = std::sync::OnceLock::new();
    let declaration = DECLARATION.get_or_init(|| {
        Regex::new(
            r"(?:const|static)\s+([A-Z][A-Z0-9_]*)\s*:\s*&?\s*(?:'static\s+)?\[\s*&\s*(?:'static\s+)?str\s*(?:;[^\]]*)?\]\s*=",
        )
        .expect("a fixed pattern compiles")
    });
    let literals = rust_literals(source);
    let chars: Vec<char> = source.chars().collect();
    // Byte offsets to character offsets, for the regex's matches.
    let char_at: HashMap<usize, usize> = source
        .char_indices()
        .enumerate()
        .map(|(index, (byte, _))| (byte, index))
        .collect();
    let mut out = Vec::new();
    for capture in declaration.captures_iter(source) {
        let whole = capture.get(0).expect("the whole match");
        let Some(&from) = char_at.get(&whole.end()) else {
            continue;
        };
        // The declaration ends at the first `;` outside a literal.
        let mut end = chars.len();
        let mut index = from;
        let mut next = literals.partition_point(|literal| literal.start < from);
        while index < chars.len() {
            if let Some(literal) = literals.get(next)
                && literal.start == index
            {
                index = literal.end;
                next += 1;
                continue;
            }
            if chars[index] == ';' {
                end = index;
                break;
            }
            index += 1;
        }
        let entries: Vec<(usize, String)> = literals
            .iter()
            .filter(|literal| literal.start >= from && literal.end <= end)
            .map(|literal| (literal.line, literal.text.clone()))
            .collect();
        if entries.is_empty() {
            continue;
        }
        out.push(PhraseList {
            name: format!("{path}::{}", &capture[1]),
            path: path.to_owned(),
            entries,
        });
    }
    out
}

/// What the lexical comparison reads: every phrase list in the scanned
/// Rust, and every text the static check reads, as prose.
#[derive(Clone, Debug, Default)]
pub struct Scanned {
    pub lists: Vec<PhraseList>,
    pub prose: Vec<Text>,
}

/// Reads the phrase lists of `sources` (paths under `root`) beside the
/// static check's texts.
#[must_use]
pub fn scanned(root: &Path, sources: &[PathBuf], prose: Vec<Text>) -> Scanned {
    let mut lists = Vec::new();
    for path in sources {
        if let Ok(source) = std::fs::read_to_string(path) {
            lists.extend(phrase_lists(&relative(root, path), &source));
        }
    }
    Scanned { lists, prose }
}

/// One provenance entry: a phrase the source holds on purpose, the task
/// whose text it matches, and what that means for evidence.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Provenance {
    pub phrase: String,
    pub task: String,
    pub paths: Vec<String>,
    /// `source`: the phrase was taken from the task's text. `coincident`:
    /// it came from elsewhere, and the task's text holds it anyway.
    pub relation: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub commits: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pattern: Option<String>,
    pub note: String,
}

/// Reads [`PROVENANCE_PATH`] under `root`; none when it doesn't exist.
///
/// # Errors
/// The file exists but an entry is incomplete.
pub fn provenance(root: &Path) -> Result<Vec<Provenance>, String> {
    let path = root.join(PROVENANCE_PATH);
    if !path.is_file() {
        return Ok(Vec::new());
    }
    let doc = read_json(&path)?;
    if doc["schema"].as_str() != Some(PROVENANCE_SCHEMA) {
        return Err(format!("{} isn't {PROVENANCE_SCHEMA}", path.display()));
    }
    let mut out = Vec::new();
    for entry in doc["provenance"].as_array().into_iter().flatten() {
        let text = |field: &str| entry[field].as_str().map(str::to_owned);
        let list = |field: &str| -> Vec<String> {
            entry[field]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
                .map(str::to_owned)
                .collect()
        };
        let (Some(phrase), Some(task), Some(relation), Some(note)) =
            (text("phrase"), text("task"), text("relation"), text("note"))
        else {
            return Err(format!(
                "{}: every entry needs a phrase, a task, a relation, and a note",
                path.display()
            ));
        };
        let paths = list("paths");
        if paths.is_empty() {
            return Err(format!("{}: {phrase:?} names no path", path.display()));
        }
        if relation != "source" && relation != "coincident" {
            return Err(format!(
                "{}: {phrase:?} has relation {relation:?}; use source or coincident",
                path.display()
            ));
        }
        let commits = list("commits");
        if relation == "source" && commits.is_empty() {
            return Err(format!(
                "{}: {phrase:?} was taken from {task}, so name the commits that added it",
                path.display()
            ));
        }
        out.push(Provenance {
            phrase: words(&phrase).join(" "),
            task,
            paths,
            relation,
            commits,
            pattern: text("pattern"),
            note,
        });
    }
    Ok(out)
}

/// One distinctive phrase that a tuned task's text holds.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Match {
    /// `entry` for a distinctive phrase-list entry, `prose` for a window
    /// of prose, and `declared` for an entry that isn't distinctive on its
    /// own but that a provenance entry declares and the task's text holds.
    pub kind: &'static str,
    /// The matched words, normalized.
    pub phrase: String,
    pub task: String,
    /// `path:line`.
    pub location: String,
    /// For an entry, the list it belongs to.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub list: Option<String>,
    /// The provenance entry that annotates it, if any.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provenance: Option<Provenance>,
}

impl Match {
    fn file(&self) -> &str {
        self.location
            .split_once(':')
            .map_or(self.location.as_str(), |(file, _)| file)
    }
}

/// Every distinctive match in what was scanned, each with its provenance
/// entry when one covers it.
#[must_use]
pub fn compare(lexicon: &Lexicon, scanned: &Scanned, provenance: &[Provenance]) -> Vec<Match> {
    let mut out = Vec::new();
    for list in &scanned.lists {
        for (line, entry) in &list.entries {
            for task in lexicon.entry_tasks(entry) {
                out.push(Match {
                    kind: "entry",
                    phrase: words(entry).join(" "),
                    task,
                    location: format!("{}:{line}", list.path),
                    list: Some(list.name.clone()),
                    provenance: None,
                });
            }
        }
    }
    for text in &scanned.prose {
        for (task, phrase, _, _) in lexicon.prose_matches(&text.text) {
            out.push(Match {
                kind: "prose",
                phrase,
                task,
                location: text.location.clone(),
                list: None,
                provenance: None,
            });
        }
    }
    for found in &mut out {
        found.provenance = provenance
            .iter()
            .find(|entry| {
                entry.phrase == found.phrase
                    && entry.task == found.task
                    && entry.paths.iter().any(|path| path == found.file())
            })
            .cloned();
    }
    // A declared entry that no distinctive match used, such as one word:
    // confirm that a list in its files holds it and the task's text does.
    for entry in provenance {
        if out
            .iter()
            .any(|found| found.provenance.as_ref() == Some(entry))
        {
            continue;
        }
        for list in &scanned.lists {
            if !entry.paths.contains(&list.path) {
                continue;
            }
            for (line, raw) in &list.entries {
                if words(raw).join(" ") == entry.phrase
                    && lexicon.holding(raw).contains(&entry.task)
                {
                    out.push(Match {
                        kind: "declared",
                        phrase: entry.phrase.clone(),
                        task: entry.task.clone(),
                        location: format!("{}:{line}", list.path),
                        list: Some(list.name.clone()),
                        provenance: Some(entry.clone()),
                    });
                }
            }
        }
    }
    out
}

/// The provenance entries no match used: stale after a phrase moved or
/// left the source.
#[must_use]
pub fn unused<'a>(provenance: &'a [Provenance], matches: &[Match]) -> Vec<&'a Provenance> {
    provenance
        .iter()
        .filter(|entry| {
            !matches
                .iter()
                .any(|found| found.provenance.as_ref() == Some(*entry))
        })
        .collect()
}

/// The lexical comparison's part of a check report.
#[must_use]
pub fn report(
    lexicon: &Lexicon,
    scanned: &Scanned,
    matches: &[Match],
    stale: &[&Provenance],
) -> Value {
    let (annotated, unannotated): (Vec<&Match>, Vec<&Match>) =
        matches.iter().partition(|found| found.provenance.is_some());
    let mut excluded: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for found in &annotated {
        let owner = found
            .provenance
            .as_ref()
            .and_then(|p| p.pattern.clone())
            .unwrap_or_else(|| found.file().to_owned());
        excluded
            .entry(owner)
            .or_default()
            .insert(found.task.clone());
    }
    json!({
        "lexicon": lexicon.digest,
        "tuned_tasks": lexicon.tasks.len(),
        "tuned_without_source": lexicon.missing,
        "phrase_lists": scanned.lists.len(),
        "entries": scanned.lists.iter().map(|l| l.entries.len()).sum::<usize>(),
        "prose_texts": scanned.prose.len(),
        "clean": unannotated.is_empty(),
        "unannotated": unannotated,
        "annotated": annotated,
        "not_evidence": excluded,
        "stale_provenance": stale,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lexicon() -> Lexicon {
        Lexicon::from_texts([
            (
                "drift-task",
                "The window adapts over time to reflect the most recent data. \
                 It accounts for natural distributional evolution. The estimator \
                 assumes normalized input and is sufficient for monitoring. Simplified here.",
            ),
            (
                "other-task",
                "Nothing in common because it is unrelated prose.",
            ),
        ])
    }

    #[test]
    fn content_words_exclude_stop_words_numbers_and_short_words() {
        assert!(content_word("sufficient"));
        assert!(!content_word("because"));
        assert!(!content_word("the"));
        assert!(!content_word("x2"));
        assert!(!content_word("is"));
    }

    #[test]
    fn a_distinctive_entry_matches_and_a_stop_word_entry_does_not() {
        let lexicon = lexicon();
        assert_eq!(lexicon.entry_tasks("accounts for"), vec!["drift-task"]);
        assert_eq!(lexicon.entry_tasks("to reflect"), vec!["drift-task"]);
        // One word: the task holds it, but it isn't distinctive.
        assert!(lexicon.entry_tasks("assumes").is_empty());
        assert_eq!(lexicon.holding("assumes"), vec!["drift-task"]);
        // One identifier is one word, however it splits, and punctuation
        // around it isn't a word.
        assert!(lexicon.entry_tasks("drift-task").is_empty());
        assert!(lexicon.entry_tasks("= assumes =").is_empty());
        // Only function words: never distinctive, even when the task says it.
        assert!(lexicon.entry_tasks("because").is_empty());
        assert!(lexicon.holding("because").is_empty());
        // Absent from every task.
        assert!(lexicon.entry_tasks("good enough").is_empty());
        // Longer than an entry: compared as prose, not whole.
        assert!(
            lexicon
                .entry_tasks("accounts for natural distributional evolution")
                .is_empty()
        );
    }

    #[test]
    fn a_stem_entry_matches_the_words_it_begins_and_a_closed_one_does_not() {
        let lexicon = lexicon();
        assert_eq!(lexicon.holding("simplif"), vec!["drift-task"]);
        // A trailing space closes the word: `assume ` isn't `assumes`.
        assert!(lexicon.holding("assume ").is_empty());
    }

    #[test]
    fn copied_prose_matches_as_one_run_and_general_prose_does_not() {
        let lexicon = lexicon();
        let matches = lexicon.prose_matches(
            "Guidance: the reference accounts for natural distributional evolution, so keep it.",
        );
        assert_eq!(matches.len(), 1, "{matches:?}");
        assert_eq!(matches[0].0, "drift-task");
        assert_eq!(
            matches[0].1,
            "accounts for natural distributional evolution"
        );
        assert!(
            lexicon
                .prose_matches("Run the tests before and after every edit to the code.")
                .is_empty()
        );
    }

    #[test]
    fn phrase_lists_read_const_and_static_arrays_but_not_test_items() {
        let source = r#"
pub const MARKS: [&str; 3] = [
    "accounts for",
    "semi;colon",
    "good enough",
];
const OTHER: &[&str] = &["assumes"];
static SPLIT: &'static [&'static str] = &["simplif"];
const NOT_A_LIST: &str = "accounts for";
const PAIRS: &[(&str, &str)] = &[("a", "b")];
#[cfg(test)]
mod tests {
    const FIXTURE: [&str; 1] = ["to reflect"];
}
"#;
        let lists = phrase_lists("src/m.rs", source);
        let names: Vec<&str> = lists.iter().map(|l| l.name.as_str()).collect();
        assert_eq!(
            names,
            vec!["src/m.rs::MARKS", "src/m.rs::OTHER", "src/m.rs::SPLIT"]
        );
        let entries: Vec<&str> = lists[0].entries.iter().map(|(_, e)| e.as_str()).collect();
        assert_eq!(entries, vec!["accounts for", "semi;colon", "good enough"]);
        assert_eq!(lists[0].entries[0].0, 3);
    }

    fn provenance_entry(phrase: &str, path: &str) -> Provenance {
        Provenance {
            phrase: phrase.to_owned(),
            task: "drift-task".to_owned(),
            paths: vec![path.to_owned()],
            relation: "source".to_owned(),
            commits: vec!["abc".to_owned()],
            pattern: Some("defended-comment-suspects".to_owned()),
            note: "chosen after reading the task's comment".to_owned(),
        }
    }

    #[test]
    fn an_annotated_match_passes_and_is_listed_and_an_unannotated_one_fails() {
        let lexicon = lexicon();
        let scanned = Scanned {
            lists: phrase_lists(
                "src/m.rs",
                r#"const MARKS: [&str; 3] = ["accounts for", "to reflect", "because"];"#,
            ),
            prose: vec![Text::new("src/p.rs:9", "Plain guidance about tests.")],
        };
        let provenance = vec![
            provenance_entry("accounts for", "src/m.rs"),
            // Covers a different file, so it doesn't annotate this match.
            provenance_entry("to reflect", "src/elsewhere.rs"),
        ];
        let matches = compare(&lexicon, &scanned, &provenance);
        assert_eq!(matches.len(), 2, "{matches:?}");
        let annotated: Vec<&str> = matches
            .iter()
            .filter(|m| m.provenance.is_some())
            .map(|m| m.phrase.as_str())
            .collect();
        assert_eq!(annotated, vec!["accounts for"]);
        let stale = unused(&provenance, &matches);
        assert_eq!(stale.len(), 1);
        assert_eq!(stale[0].phrase, "to reflect");
        let report = report(&lexicon, &scanned, &matches, &stale);
        assert_eq!(report["clean"], json!(false));
        assert_eq!(report["unannotated"][0]["phrase"], json!("to reflect"));
        assert_eq!(
            report["not_evidence"]["defended-comment-suspects"],
            json!(["drift-task"])
        );
    }

    #[test]
    fn a_declared_single_word_is_confirmed_and_listed_or_else_stale() {
        let lexicon = lexicon();
        let scanned = Scanned {
            lists: phrase_lists(
                "src/m.rs",
                r#"const MARKS: [&str; 2] = ["assumes", "heuristic"];"#,
            ),
            prose: Vec::new(),
        };
        let provenance = vec![
            provenance_entry("assumes", "src/m.rs"),
            // The task's text never says it: nothing to confirm.
            provenance_entry("heuristic", "src/m.rs"),
        ];
        let matches = compare(&lexicon, &scanned, &provenance);
        assert_eq!(matches.len(), 1, "{matches:?}");
        assert_eq!(matches[0].kind, "declared");
        assert_eq!(matches[0].phrase, "assumes");
        assert_eq!(matches[0].location, "src/m.rs:1");
        let stale = unused(&provenance, &matches);
        assert_eq!(stale.len(), 1);
        assert_eq!(stale[0].phrase, "heuristic");
    }

    #[test]
    fn a_source_entry_without_commits_is_refused() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join(PROVENANCE_PATH);
        std::fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");
        std::fs::write(
            &path,
            json!({"schema": PROVENANCE_SCHEMA, "provenance": [
                {"phrase": "Accounts for", "task": "t", "paths": ["a.rs"],
                 "relation": "source", "note": "n"}
            ]})
            .to_string(),
        )
        .expect("write");
        assert!(provenance(dir.path()).is_err());
        std::fs::write(
            &path,
            json!({"schema": PROVENANCE_SCHEMA, "provenance": [
                {"phrase": "Accounts for", "task": "t", "paths": ["a.rs"],
                 "relation": "coincident", "note": "n"}
            ]})
            .to_string(),
        )
        .expect("write");
        let entries = provenance(dir.path()).expect("load");
        assert_eq!(entries[0].phrase, "accounts for");
    }

    #[test]
    fn the_corpus_stores_digests_and_drops_general_vocabulary() {
        let dir = tempfile::tempdir().expect("tempdir");
        let root = dir.path().join("repo");
        let tasks = dir.path().join("tasks");
        std::fs::create_dir_all(root.join("crates/coder-one")).expect("mkdir");
        std::fs::write(
            root.join(TUNED_PATH),
            json!({"schema": TUNED_SCHEMA,
                   "roles": {"dev": "iterated on"},
                   "tasks": [{"task": "drift", "roles": ["dev"]},
                             {"task": "gone", "roles": ["dev"]}]})
            .to_string(),
        )
        .expect("write");
        for (index, name) in ["drift", "b", "c", "d", "e"].iter().enumerate() {
            let task = tasks.join(name);
            std::fs::create_dir_all(task.join("environment/pkg")).expect("mkdir");
            std::fs::write(task.join("instruction.md"), "Fix the monitor module.").expect("write");
            let comment = if index == 0 {
                "\"\"\"The window accounts for natural distributional evolution.\"\"\"\n"
            } else {
                "# plain\n"
            };
            std::fs::write(task.join("environment/pkg/window.py"), comment).expect("write");
            std::fs::write(task.join("environment/data.csv"), "a,b\n1,2\n").expect("write");
        }
        let doc = build(&root, &tasks).expect("build");
        let drift = &doc["tasks"]["drift"];
        let phrases = drift["phrases"].as_str().expect("phrases");
        assert!(phrases.contains(&phrase_hash("accounts for")));
        // Every task's instruction says it: general vocabulary.
        assert!(!phrases.contains(&phrase_hash("monitor module")));
        assert!(!phrases.contains("accounts"));
        let sources: Vec<&str> = drift["sources"]
            .as_array()
            .expect("sources")
            .iter()
            .filter_map(|s| s["path"].as_str())
            .collect();
        assert_eq!(sources, vec!["instruction.md", "environment/pkg/window.py"]);
        assert_eq!(doc["missing"][0]["task"], json!("gone"));
    }
}
