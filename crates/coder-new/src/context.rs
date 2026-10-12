//! The context layer: System One picks the instructions, memory and skills
//! in every harness's briefing (#11473, phase 3 of #11451).
//!
//! Before this layer each harness got something different, all of it
//! loaded whether it bore on the work or not: Coder's loop read every
//! `AGENTS.md` and `CLAUDE.md` up the tree plus every memory note, Claude
//! Code loaded its own `CLAUDE.md` files, and Codex read `AGENTS.md` by
//! itself. Now every harness gets one briefing, chosen once per piece of
//! work (episode 286's "conditional AGENTS.md"):
//!
//! 1. **Candidates in code** ([`gather`]). The instruction files Coder
//!    reads today ([`crate::memory::Memory::instructions`], nearest first,
//!    then `~/.openagents/AGENTS.md`) split into parts by heading, long
//!    parts cut at paragraphs and top-level list items. Then the memory
//!    notes of both scopes, and the skills the person has: Claude Code
//!    skills (`~/.claude/skills`, `.claude/skills`), the checkout's vendored
//!    skills (`.agents/skills`), and Coder plugin skills (`plugins/`,
//!    `~/.openagents/skills`). Each candidate has a stable id, where it came
//!    from, and its size.
//! 2. **System One selects** ([`questions`], `context-v1`). One typed
//!    request at a routing point: a Score per candidate, how much it bears
//!    on what the person's latest message asks. It goes to the gateway's
//!    H100 (`/v1/systemone`, `X-Decision-Order: hosted`) with Jev behind it,
//!    through the same doors as the router ([`crate::issue_run::Decider`]).
//!    No words are matched. [`select`] keeps the candidates placed at least
//!    at "background" on average ([`CHOSEN_AT`]), most relevant first,
//!    within [`BRIEFING_BYTES`]. When
//!    System One does not answer within [`BUDGET`], nothing is chosen and
//!    each harness loads what it loaded before.
//! 3. **One briefing for every harness** ([`Briefing::render`]). The Coder
//!    loop reads it in its system prompt, Claude Code as appended system
//!    context with its own loading of those instruction files switched off
//!    (`claudeMdExcludes`), and Codex in its task with its own `AGENTS.md`
//!    reading off (`project_doc_max_bytes=0`).
//! 4. **Stable for the piece of work.** The briefing is chosen on a chat's
//!    first message and again only when the router judges a follow-up a
//!    new piece of work, never every turn, so the prompt cache survives
//!    (owner decision 1).
//! 5. **Recorded.** The routing record's `inputs.context` keeps the
//!    selected ids, their levels, the model and the door ([`Trace`]).

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

/// The question set's frozen name. A changed question is a new version.
pub const QUESTION_SET: &str = "context-v1";

/// How long a message waits for the selection before each harness loads
/// what it loaded before. The selection asks one Score per candidate, so it
/// takes longer than the route question; both run at once on a chat's first
/// message. The gateway door answered a library of 78 candidates, in two
/// requests at once, in about 350 ms on 2026-10-12; Jev behind it in about
/// 700 ms.
pub const BUDGET: Duration = Duration::from_millis(2000);

/// The most a briefing carries, in bytes (about 8,000 tokens).
pub const BRIEFING_BYTES: usize = 32 * 1024;

/// The longest part an instruction file is split into, in bytes.
const PART_MAX: usize = 4 * 1024;

/// The most of a candidate System One reads, in characters.
const EXCERPT_CHARS: usize = 600;

/// The probability-weighted level a candidate needs to be chosen: on
/// average at least "background" ([`LEVELS`]). On the labelled fixture
/// (`fixtures/context/context-v1.json`) this carries 24 of the 30 marked
/// parts against 20 for the argmax "relevant" reading, with about 5.6 KB
/// per briefing; the fixture chose it, so it is not held out.
pub const CHOSEN_AT: f64 = 1.0;

/// The Score levels, from zero.
const LEVELS: [&str; 4] = [
    "Unrelated: it does not bear on what the person asks.",
    "Background: general guidance that could apply here but changes nothing about how to do it.",
    "Relevant: it shapes how what the person asks should be done.",
    "Essential: doing what the person asks without it would likely go wrong.",
];

/// Where a candidate came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Origin {
    /// A part of an instruction file (`AGENTS.md`, `CLAUDE.md`).
    Instructions,
    /// A saved memory note.
    Memory,
    /// A skill the person has.
    Skill,
}

/// One thing a briefing may carry.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Candidate {
    /// Stable across runs while the file's headings stay: the file's label
    /// and the heading's slug, `memory:SCOPE/NAME`, or `skill:NAME`.
    pub id: String,
    pub origin: Origin,
    /// The file it came from.
    pub path: PathBuf,
    /// The heading path, the note's name, or the skill's name.
    pub title: String,
    /// What the briefing carries when it is chosen.
    pub text: String,
}

impl Candidate {
    /// Its size in bytes, as the budget counts it.
    #[must_use]
    pub fn size(&self) -> usize {
        self.title.len() + self.text.len()
    }

    /// What System One reads about it.
    fn excerpt(&self) -> String {
        let what = match self.origin {
            Origin::Instructions => "A part of an instruction file",
            Origin::Memory => "A note saved in an earlier session",
            Origin::Skill => "A skill the agent can use",
        };
        format!(
            "{what} ({}): {}\n{}",
            self.id,
            self.title,
            gist(&self.text, EXCERPT_CHARS)
        )
    }
}

/// Everything a briefing can be chosen from, for one working directory.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Library {
    pub candidates: Vec<Candidate>,
    /// Every instruction file the candidates were split from.
    pub files: Vec<PathBuf>,
    /// Instruction files elsewhere in the checkout, named rather than read.
    pub nested: Vec<PathBuf>,
}

/// `text`'s gist within `max` characters: the first line of each paragraph
/// and list item, so a long part still shows what it covers.
fn gist(text: &str, max: usize) -> String {
    let mut out = String::new();
    let mut fresh = true;
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            fresh = true;
            continue;
        }
        let item = line.starts_with("- ") || line.starts_with("* ") || line.starts_with("1. ");
        if fresh || item {
            if !out.is_empty() {
                out.push('\n');
            }
            out.push_str(trimmed);
        }
        fresh = false;
    }
    if out.chars().count() <= max {
        return out;
    }
    let mut cut: String = out.chars().take(max).collect();
    cut.push('…');
    cut
}

/// A heading's slug: lowercase words joined by `-`.
fn slug(text: &str) -> String {
    let mut out = String::new();
    for c in text.chars() {
        if c.is_alphanumeric() {
            out.extend(c.to_lowercase());
        } else if !out.ends_with('-') && !out.is_empty() {
            out.push('-');
        }
    }
    let out = out.trim_end_matches('-');
    out.chars().take(60).collect::<String>()
}

/// A file's label in ids: relative to the checkout, `~/` under home, or
/// the full path.
fn label(path: &Path, checkout: &Path, home: Option<&Path>) -> String {
    let real = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    if let Ok(relative) = real.strip_prefix(checkout) {
        return relative.display().to_string();
    }
    if let Some(home) = home
        && let Ok(relative) = real.strip_prefix(home)
    {
        return format!("~/{}", relative.display());
    }
    real.display().to_string()
}

/// One heading's block before it is cut to size.
struct Block {
    title: String,
    lines: Vec<String>,
}

/// Splits one instruction file into parts: at headings of levels one to
/// three outside code fences, and a part over [`PART_MAX`] again at
/// paragraphs and top-level list items. A heading with nothing under it
/// before the next one names the parts below it.
#[must_use]
pub fn sections(file_label: &str, path: &Path, text: &str) -> Vec<Candidate> {
    let mut blocks: Vec<Block> = Vec::new();
    let mut trail: Vec<(usize, String)> = Vec::new();
    let mut current = Block {
        title: String::new(),
        lines: Vec::new(),
    };
    let mut fenced = false;
    for line in text.lines() {
        if line.trim_start().starts_with("```") {
            fenced = !fenced;
        }
        let level = line.chars().take_while(|c| *c == '#').count();
        if !fenced && (1..=3).contains(&level) && line[level..].starts_with(' ') {
            let heading = line[level..].trim().to_owned();
            if current.lines.iter().any(|line| !line.trim().is_empty()) {
                blocks.push(std::mem::replace(
                    &mut current,
                    Block {
                        title: String::new(),
                        lines: Vec::new(),
                    },
                ));
            }
            trail.retain(|(depth, _)| *depth < level);
            trail.push((level, heading));
            // The file's own title names the file, not its parts.
            current.title = trail
                .iter()
                .filter(|(depth, _)| *depth > 1 || trail.len() == 1)
                .map(|(_, heading)| heading.as_str())
                .collect::<Vec<_>>()
                .join(" › ");
            current.lines.clear();
            continue;
        }
        current.lines.push(line.to_owned());
    }
    if current.lines.iter().any(|line| !line.trim().is_empty()) {
        blocks.push(current);
    }
    let mut out = Vec::new();
    let mut seen = BTreeSet::new();
    for block in blocks {
        let title = if block.title.is_empty() {
            "Opening".to_owned()
        } else {
            block.title
        };
        let pieces = pieces(&block.lines.join("\n"));
        let several = pieces.len() > 1;
        for (index, piece) in pieces.into_iter().enumerate() {
            let mut base = slug(&title);
            if base.is_empty() {
                base = "part".into();
            }
            if several {
                // A long part's pieces are named by what each starts with
                // (a list item's first code span), so adding an item to
                // one piece leaves the others' ids alone.
                let lead = first_code_span(&piece).map(|span| slug(&span));
                base = match lead {
                    Some(lead) if !lead.is_empty() => format!("{base}-{lead}"),
                    _ => format!("{base}-{}", index + 1),
                };
            }
            let mut id = format!("{file_label}#{base}");
            let mut n = 2;
            while !seen.insert(id.clone()) {
                id = format!("{file_label}#{base}-{n}");
                n += 1;
            }
            out.push(Candidate {
                id,
                origin: Origin::Instructions,
                path: path.to_path_buf(),
                title: if several {
                    format!("{title} (part {})", index + 1)
                } else {
                    title.clone()
                },
                text: piece.trim().to_owned(),
            });
        }
    }
    out
}

/// The first `code span` on `text`'s first non-blank line.
fn first_code_span(text: &str) -> Option<String> {
    let line = text.lines().find(|line| !line.trim().is_empty())?;
    let start = line.find('`')? + 1;
    let end = line[start..].find('`')? + start;
    Some(line[start..end].to_owned())
}

/// `text` in pieces of at most [`PART_MAX`] bytes, cut where a paragraph
/// or a top-level list item starts; a single paragraph over the limit is
/// cut at a line.
fn pieces(text: &str) -> Vec<String> {
    if text.len() <= PART_MAX {
        return vec![text.to_owned()];
    }
    // Units: paragraphs, with each top-level list item its own unit.
    let mut units: Vec<String> = Vec::new();
    let mut unit = String::new();
    let mut fenced = false;
    for line in text.lines() {
        if line.trim_start().starts_with("```") {
            fenced = !fenced;
        }
        let starts = !fenced
            && (line.trim().is_empty()
                || line.starts_with("- ")
                || line.starts_with("* ")
                || line.starts_with("| ") && unit.is_empty());
        if starts && !unit.trim().is_empty() {
            units.push(std::mem::take(&mut unit));
        }
        unit.push_str(line);
        unit.push('\n');
    }
    if !unit.trim().is_empty() {
        units.push(unit);
    }
    let mut out = Vec::new();
    let mut piece = String::new();
    for unit in units {
        if !piece.is_empty() && piece.len() + unit.len() > PART_MAX {
            out.push(std::mem::take(&mut piece));
        }
        if unit.len() > PART_MAX {
            for line in unit.lines() {
                if !piece.is_empty() && piece.len() + line.len() + 1 > PART_MAX {
                    out.push(std::mem::take(&mut piece));
                }
                piece.push_str(line);
                piece.push('\n');
            }
        } else {
            piece.push_str(&unit);
        }
    }
    if !piece.trim().is_empty() {
        out.push(piece);
    }
    out
}

/// A skill's name and description from its `SKILL.md` front matter.
fn skill(path: &Path) -> Option<(String, String)> {
    let text = std::fs::read_to_string(path).ok()?;
    let mut lines = text.lines();
    let mut name = None;
    let mut description = None;
    if lines.next().map(str::trim) == Some("---") {
        for line in lines.by_ref() {
            if line.trim() == "---" {
                break;
            }
            if let Some(value) = line.strip_prefix("name:") {
                name = Some(value.trim().trim_matches('"').to_owned());
            } else if let Some(value) = line.strip_prefix("description:") {
                description = Some(value.trim().trim_matches('"').to_owned());
            }
        }
    }
    let name = name.filter(|name| !name.is_empty()).or_else(|| {
        path.parent()?
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
    })?;
    let description = description
        .filter(|text| !text.is_empty())
        .unwrap_or_else(|| gist(lines.collect::<Vec<_>>().join("\n").trim(), 300));
    Some((name, description))
}

/// Every `SKILL.md` under `dir`, at most three folders down.
fn skill_files(dir: &Path, depth: usize, out: &mut Vec<PathBuf>) {
    let Ok(read) = std::fs::read_dir(dir) else {
        return;
    };
    let mut entries: Vec<PathBuf> = read.flatten().map(|entry| entry.path()).collect();
    entries.sort();
    for path in entries {
        if path.file_name().is_some_and(|name| name == "SKILL.md") && path.is_file() {
            out.push(path);
        } else if depth > 0 && path.is_dir() {
            skill_files(&path, depth - 1, out);
        }
    }
}

/// Where the person's skills live, for a checkout and a home folder.
#[must_use]
pub fn skill_dirs(checkout: &Path, home: Option<&Path>) -> Vec<PathBuf> {
    let mut dirs = vec![
        checkout.join(".claude/skills"),
        checkout.join(".agents/skills"),
        checkout.join("plugins"),
    ];
    if let Some(home) = home {
        dirs.push(home.join(".claude/skills"));
        dirs.push(home.join(".openagents/skills"));
    }
    dirs
}

/// The candidates for `memory`'s working directory: its instruction files'
/// parts, its notes, and the skills in `skill_dirs`.
#[must_use]
pub fn gather(memory: &crate::memory::Memory, skill_dirs: &[PathBuf]) -> Library {
    let checkout = memory.checkout().to_path_buf();
    let home = memory.home().map(Path::to_path_buf);
    let files = memory.instructions();
    let nested = memory.nested_instructions(&files);
    let mut candidates = Vec::new();
    for file in &files {
        let file_label = label(&file.path, &checkout, home.as_deref());
        candidates.extend(sections(&file_label, &file.path, &file.text));
    }
    for entry in memory.all() {
        candidates.push(Candidate {
            id: format!("memory:{}/{}", entry.scope.word(), slug(&entry.name)),
            origin: Origin::Memory,
            path: entry.file.clone(),
            title: format!("{} ({})", entry.name, entry.kind.word()),
            text: format!("{}\n{}", entry.description, entry.body.trim()),
        });
    }
    let mut names = BTreeSet::new();
    for dir in skill_dirs {
        let mut found = Vec::new();
        skill_files(dir, 3, &mut found);
        for path in found {
            let Some((name, description)) = skill(&path) else {
                continue;
            };
            if !names.insert(name.clone()) {
                continue;
            }
            candidates.push(Candidate {
                id: format!("skill:{}", slug(&name)),
                origin: Origin::Skill,
                path,
                title: name,
                text: description,
            });
        }
    }
    Library {
        candidates,
        files: files.into_iter().map(|file| file.path).collect(),
        nested: nested.into_iter().map(|path| checkout.join(path)).collect(),
    }
}

/// The library for `cwd` on this computer, or `None` when memory and
/// instruction loading are off (`OPENAGENTS_MEMORY=off`) or there is
/// nothing to choose from. A unit test reads only the library it set
/// ([`set_test_library`]).
#[must_use]
pub fn library(cwd: &Path) -> Option<Library> {
    #[cfg(test)]
    {
        let _ = cwd;
        TEST_LIBRARY.with(|library| library.borrow().clone())
    }
    #[cfg(not(test))]
    {
        let memory = crate::memory::Memory::discover(cwd)?;
        let dirs = skill_dirs(memory.checkout(), memory.home());
        Some(gather(&memory, &dirs)).filter(|library| !library.candidates.is_empty())
    }
}

/// The `context-v1` questions: one Score per candidate, `c0`, `c1`, … in
/// the library's order.
#[must_use]
pub fn questions(candidates: &[Candidate]) -> jev::Questions {
    let mut questions = jev::Questions::new();
    for (index, candidate) in candidates.iter().enumerate() {
        let mut score = jev::Score::new(
            format!(
                "How much does this matter for doing what the person's latest message asks?\n\n{}",
                candidate.excerpt()
            ),
            Vec::new(),
        );
        for level in LEVELS {
            score = score.level(level);
        }
        questions = questions.with(format!("c{index}"), score);
    }
    questions
}

/// One candidate's judgment.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Scored {
    pub id: String,
    /// The level System One named ([`LEVELS`], from zero).
    pub level: u8,
    /// The probability-weighted level.
    pub position: f64,
}

/// The level a Score answer names: its `selected` level, else the highest
/// level among the most probable.
fn level_of(answer: &jev::ScoreAnswer) -> Option<u8> {
    if let Some(selected) = answer.selected.as_deref().and_then(|s| s.parse().ok()) {
        return Some(selected);
    }
    let best = answer
        .probabilities
        .values()
        .copied()
        .filter(|p| p.is_finite())
        .fold(f64::NEG_INFINITY, f64::max);
    answer
        .probabilities
        .iter()
        .filter(|(_, p)| **p >= best)
        .map(|(level, _)| *level)
        .max()
        .and_then(|level| u8::try_from(level).ok())
        .or_else(|| {
            // A door that sends no distribution: the nearest level.
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            Some(answer.score.round().clamp(0.0, 3.0) as u8)
        })
}

/// Reads a response to [`questions`] for `candidates`; question `cN` is
/// the Nth of them.
///
/// # Errors
/// A candidate went unanswered.
pub fn read(
    response: &jev::SystemOneResponse,
    candidates: &[Candidate],
) -> Result<Vec<Scored>, String> {
    candidates
        .iter()
        .enumerate()
        .map(|(index, candidate)| {
            let answer = response
                .score(&format!("c{index}"))
                .map_err(|error| format!("c{index}: {error}"))?;
            Ok(Scored {
                id: candidate.id.clone(),
                level: level_of(answer).unwrap_or(0),
                position: answer.score,
            })
        })
        .collect()
}

/// The candidates chosen from `scored`: those placed at [`CHOSEN_AT`] or
/// above, the highest first (library order breaks ties), each taken while
/// it fits in `budget` bytes. Returned in library order.
#[must_use]
pub fn select(candidates: &[Candidate], scored: &[Scored], budget: usize) -> Vec<usize> {
    let mut ranked: Vec<(usize, f64)> = candidates
        .iter()
        .enumerate()
        .filter_map(|(index, candidate)| {
            scored
                .iter()
                .find(|s| s.id == candidate.id)
                .filter(|s| s.position >= CHOSEN_AT)
                .map(|s| (index, s.position))
        })
        .collect();
    ranked.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
    let mut left = budget;
    let mut chosen = Vec::new();
    for (index, _) in ranked {
        let size = candidates[index].size();
        if size <= left {
            left -= size;
            chosen.push(index);
        }
    }
    chosen.sort_unstable();
    chosen
}

/// What one selection did, as the routing record keeps it.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Trace {
    pub question_set: String,
    /// How many candidates there were.
    pub candidates: usize,
    /// The chosen candidates with their judgments, in library order.
    #[serde(default)]
    pub selected: Vec<Scored>,
    /// The briefing's size in bytes, and its budget.
    pub bytes: usize,
    pub budget_bytes: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub door: Option<String>,
    /// How long System One took, or how long the message waited for it.
    pub ms: u64,
    /// Why nothing was chosen, scrubbed, when nothing was.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub missing: Option<String>,
    /// The turn kept the briefing chosen for this piece of work earlier.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub kept: bool,
}

/// The briefing every harness reads for one piece of work.
#[derive(Clone, Debug, PartialEq)]
pub struct Briefing {
    /// The chosen candidates, in library order.
    pub parts: Vec<Candidate>,
    /// Every instruction file the choice was made over: no harness loads
    /// them again on its own.
    pub files: Vec<PathBuf>,
    /// Instruction files elsewhere in the checkout, named for the model.
    pub nested: Vec<PathBuf>,
    /// The names of the notes left out, so the model can still recall one.
    pub other_notes: Vec<String>,
    /// How it was chosen.
    pub trace: Trace,
}

impl Briefing {
    /// The briefing from `chosen` candidates of `library`.
    #[must_use]
    pub fn from_choice(library: &Library, chosen: &[usize], trace: Trace) -> Self {
        let parts: Vec<Candidate> = chosen
            .iter()
            .map(|index| library.candidates[*index].clone())
            .collect();
        let other_notes = library
            .candidates
            .iter()
            .enumerate()
            .filter(|(index, c)| c.origin == Origin::Memory && !chosen.contains(index))
            .map(|(_, c)| c.title.clone())
            .collect();
        Self {
            parts,
            files: library.files.clone(),
            nested: library.nested.clone(),
            other_notes,
            trace,
        }
    }

    /// A short key that changes when the chosen parts do.
    #[must_use]
    pub fn key(&self) -> String {
        self.parts
            .iter()
            .map(|part| part.id.as_str())
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// The briefing as the model reads it. `memory_tools` says whether the
    /// harness offers the remember, forget and recall tools.
    #[must_use]
    pub fn render(&self, memory_tools: bool) -> String {
        let mut text = String::new();
        let instructions: Vec<&Candidate> = self
            .parts
            .iter()
            .filter(|part| part.origin == Origin::Instructions)
            .collect();
        if !self.files.is_empty() || !self.nested.is_empty() {
            text.push_str("# Project instructions\n\nThese parts of the repository's and the user's instruction files were chosen for this piece of work. Follow them. Nearer files come first and take precedence over farther ones; the user's messages in this chat take precedence over all of them.\n");
            let mut file: Option<&Path> = None;
            for part in &instructions {
                if file != Some(part.path.as_path()) {
                    file = Some(part.path.as_path());
                    text.push_str(&format!("\n## {}\n", part.path.display()));
                }
                text.push_str(&format!("\n### {}\n\n{}\n", part.title, part.text));
            }
            if instructions.is_empty() {
                text.push_str("\nNo part of them bears on this piece of work.\n");
            }
            let files: Vec<String> = self
                .files
                .iter()
                .map(|path| path.display().to_string())
                .collect();
            if !files.is_empty() {
                text.push_str(&format!(
                    "\nThe rest of these files was left out as unrelated to this work. Read a file when something in it may matter: {}\n",
                    files.join(", ")
                ));
            }
            if !self.nested.is_empty() {
                let nested: Vec<String> = self
                    .nested
                    .iter()
                    .map(|path| path.display().to_string())
                    .collect();
                text.push_str(&format!(
                    "\nOther directories have their own instructions. Read the one for a directory before changing files under it: {}\n",
                    nested.join(", ")
                ));
            }
        }
        let notes: Vec<&Candidate> = self
            .parts
            .iter()
            .filter(|part| part.origin == Origin::Memory)
            .collect();
        if memory_tools || !notes.is_empty() {
            text.push_str("\n# Memory\n\n");
            text.push_str(if memory_tools {
                crate::memory::TOOLS_GUIDE
            } else {
                crate::memory::NOTES_GUIDE
            });
            for note in &notes {
                text.push_str(&format!("- {}:", note.title));
                for line in note.text.lines() {
                    text.push_str("\n  ");
                    text.push_str(line);
                }
                text.push('\n');
            }
            if !self.other_notes.is_empty() {
                text.push_str(&format!(
                    "Other saved notes, left out as unrelated to this work: {}.{}\n",
                    self.other_notes.join(", "),
                    if memory_tools {
                        " Use recall to read one by name."
                    } else {
                        ""
                    }
                ));
            }
        }
        let skills: Vec<&Candidate> = self
            .parts
            .iter()
            .filter(|part| part.origin == Origin::Skill)
            .collect();
        if !skills.is_empty() {
            text.push_str("\n# Skills\n\nThese skills fit this work. Read a skill's file before you use it.\n");
            for skill in skills {
                text.push_str(&format!(
                    "- {}: {} ({})\n",
                    skill.title,
                    skill.text.trim(),
                    skill.path.display()
                ));
            }
        }
        text
    }
}

/// A System One answer to one selection request.
#[derive(Clone, Debug, PartialEq)]
pub struct Answered {
    pub scored: Vec<Scored>,
    pub model: String,
    pub door: String,
    pub ms: u64,
}

/// The briefing and its trace from System One's answer, or no briefing
/// (each harness loads what it loaded before) and a trace saying why.
#[must_use]
pub fn decide(
    answer: Result<Answered, String>,
    library: &Library,
    waited: Duration,
) -> (Option<Briefing>, Trace) {
    let mut trace = Trace {
        question_set: QUESTION_SET.into(),
        candidates: library.candidates.len(),
        budget_bytes: BRIEFING_BYTES,
        ms: u64::try_from(waited.as_millis()).unwrap_or(u64::MAX),
        ..Trace::default()
    };
    match answer {
        Ok(answered) => {
            let chosen = select(&library.candidates, &answered.scored, BRIEFING_BYTES);
            trace.selected = chosen
                .iter()
                .filter_map(|index| {
                    let id = &library.candidates[*index].id;
                    answered.scored.iter().find(|s| &s.id == id).cloned()
                })
                .collect();
            trace.bytes = chosen
                .iter()
                .map(|index| library.candidates[*index].size())
                .sum();
            trace.model = Some(answered.model);
            trace.door = Some(answered.door);
            trace.ms = answered.ms;
            let briefing = Briefing::from_choice(library, &chosen, trace.clone());
            (Some(briefing), trace)
        }
        Err(why) => {
            trace.missing = Some(crate::routing_record::scrub(&why));
            (None, trace)
        }
    }
}

/// The most questions one request may ask: the gateway door refuses more
/// than 64, so a larger library goes in several requests at once.
pub const BATCH: usize = 48;

/// The requests for `library` about the latest message: one per
/// [`BATCH`] candidates, each with its candidates' offset.
#[must_use]
pub fn requests(
    library: &Library,
    earlier: &[crate::router::Said],
    latest: &str,
) -> Vec<(usize, jev::SystemOneRequest)> {
    let state = crate::router::state(earlier, latest);
    library
        .candidates
        .chunks(BATCH)
        .enumerate()
        .map(|(n, chunk)| {
            (
                n * BATCH,
                jev::SystemOneRequest::new(state.clone(), questions(chunk)),
            )
        })
        .collect()
}

/// One selection, its requests asked at once and answered or given up on
/// within [`BUDGET`].
///
/// # Errors
/// A request had no answer in time, or an answer could not be read.
pub fn ask_now(
    requests: Vec<(usize, jev::SystemOneRequest)>,
    candidates: &[Candidate],
) -> Result<Answered, String> {
    let started = Instant::now();
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|error| error.to_string())?;
    let answers = runtime
        .block_on(async {
            tokio::time::timeout(
                BUDGET,
                futures::future::join_all(requests.into_iter().map(
                    |(offset, request)| async move {
                        crate::issue_run::decide::ask(crate::issue_run::Decider::Auto, request)
                            .await
                            .map(|answer| (offset, answer))
                    },
                )),
            )
            .await
        })
        .map_err(|_| "System One did not answer in time.".to_owned())?;
    let mut scored = Vec::new();
    let mut model = String::new();
    let mut door = String::new();
    for answer in answers {
        let (offset, (response, by)) = answer?;
        let end = (offset + BATCH).min(candidates.len());
        scored.extend(read(&response, &candidates[offset..end])?);
        if model.is_empty() {
            model.clone_from(&response.model);
            door = by;
        } else if door != by && !door.contains(&by) {
            door = format!("{door}; {by}");
        }
    }
    Ok(Answered {
        scored,
        model,
        door,
        ms: u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX),
    })
}

/// A selection under way.
pub struct Asking {
    pub receiver: mpsc::Receiver<Result<Answered, String>>,
    pub started: Instant,
    pub library: Library,
    /// The answer, or why there is none, once that is known.
    pub answer: Option<Result<Answered, String>>,
}

impl Asking {
    /// Moves the answer in once it is known; returns whether it is.
    pub fn settled(&mut self) -> bool {
        if self.answer.is_none() {
            self.answer = self.poll();
        }
        self.answer.is_some()
    }

    /// Waits for the answer, for callers with no screen to keep drawing.
    pub fn settle(&mut self) {
        if self.answer.is_none() {
            self.answer = Some(self.wait());
        }
    }

    /// The briefing and its trace from the answer.
    #[must_use]
    pub fn decide(self) -> (Option<Briefing>, Trace) {
        let waited = self.started.elapsed();
        let answer = self
            .answer
            .unwrap_or_else(|| Err("System One did not answer in time.".into()));
        decide(answer, &self.library, waited)
    }

    /// The answer, or why there is none, once it is in; `None` while it is
    /// still worth waiting.
    #[must_use]
    pub fn poll(&self) -> Option<Result<Answered, String>> {
        match self.receiver.try_recv() {
            Ok(answer) => Some(answer),
            Err(mpsc::TryRecvError::Disconnected) => {
                Some(Err("System One could not be asked.".into()))
            }
            Err(mpsc::TryRecvError::Empty) if self.started.elapsed() >= BUDGET => {
                Some(Err("System One did not answer in time.".into()))
            }
            Err(mpsc::TryRecvError::Empty) => None,
        }
    }

    /// Waits for the answer up to what is left of the budget.
    pub fn wait(&self) -> Result<Answered, String> {
        let left = BUDGET.saturating_sub(self.started.elapsed());
        match self.receiver.recv_timeout(left) {
            Ok(answer) => answer,
            Err(mpsc::RecvTimeoutError::Timeout) => {
                Err("System One did not answer in time.".into())
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                Err("System One could not be asked.".into())
            }
        }
    }
}

/// Asks System One to choose from `library` on a thread of its own.
#[must_use]
pub fn ask(library: Library, earlier: &[crate::router::Said], latest: &str) -> Asking {
    let (sender, receiver) = mpsc::channel();
    let request = requests(&library, earlier, latest);
    #[cfg(test)]
    {
        if let Some(Some(scored)) = TEST_JUDGE.with(|judge| judge.borrow().clone()) {
            let _ = sender.send(Ok(Answered {
                scored,
                model: "test".into(),
                door: "test".into(),
                ms: 0,
            }));
        }
        drop(request);
    }
    #[cfg(not(test))]
    {
        let candidates = library.candidates.clone();
        std::thread::spawn(move || {
            let _ = sender.send(ask_now(request, &candidates));
        });
    }
    Asking {
        receiver,
        started: Instant::now(),
        library,
        answer: None,
    }
}

/// Whether routing points choose a briefing. `CODER_CONTEXT=off` turns it
/// off, and each harness loads what it loaded before. Unit tests ask only a
/// judge they set ([`set_test_judge`]).
#[must_use]
pub fn enabled() -> bool {
    #[cfg(test)]
    {
        TEST_JUDGE.with(|judge| judge.borrow().is_some())
    }
    #[cfg(not(test))]
    {
        std::env::var("CODER_CONTEXT").map_or(true, |value| value.trim() != "off")
    }
}

#[cfg(test)]
thread_local! {
    static TEST_JUDGE: std::cell::RefCell<Option<Option<Vec<Scored>>>> =
        const { std::cell::RefCell::new(None) };
    static TEST_LIBRARY: std::cell::RefCell<Option<Library>> =
        const { std::cell::RefCell::new(None) };
}

/// Sets this test thread's System One for selections: `None` turns them
/// off, `Some(None)` never answers, `Some(Some(scores))` answers with
/// `scores`.
#[cfg(test)]
pub(crate) fn set_test_judge(judge: Option<Option<Vec<Scored>>>) {
    TEST_JUDGE.with(|cell| *cell.borrow_mut() = judge);
}

/// Sets the library this test thread's chats choose from.
#[cfg(test)]
pub(crate) fn set_test_library(library: Option<Library>) {
    TEST_LIBRARY.with(|cell| *cell.borrow_mut() = library);
}

#[cfg(test)]
#[path = "context_tests.rs"]
mod tests;
