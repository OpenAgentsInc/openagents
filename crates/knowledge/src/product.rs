//! The OpenAgents product knowledge base: short, sourced entries about the
//! OpenAgents product, under `knowledge/openagents/`, that the chat
//! answers product questions from (`docs/coder/design/2026-09-28-chat-router.md`,
//! "Knowledge routes: product and codebase").
//!
//! An entry here is an ordinary knowledge entry of kind `product`, written
//! from the repository's public documents. It may carry an `answer`: a
//! short, complete reply in the OpenAgents voice ("we"), which the chat can
//! show whole when it fully answers a message. Its body is the reference
//! material a model answers from when no single answer fits.
//!
//! [`Corpus::load`] reads the directory and refuses an entry that
//! [`check`] finds a problem with:
//!
//! - It is kind `product`, with an id under `openagents.`.
//! - Every claim is sourced: it cites at least one repository path, and
//!   each cited path exists in the checkout. A cite is a relative path in
//!   this public repository, never a URL, an absolute path, a path that
//!   leaves the repository, or a private repository's path.
//! - Its answer and body speak in the plural: no first-person singular.
//! - Its answer is at most [`MAX_ANSWER_CHARS`] characters.
//!
//! Only `admitted` entries are served; admission for a product entry is a
//! person's review against its sources, recorded in `evidence`.
//!
//! [`instructions`] turns the entries a retrieval kept into the grounded
//! model's instructions, and [`check_reply`] reads which entries a reply
//! cited. Neither routes anything: which entries are relevant is decided
//! by embeddings and a typed judgment in the caller, not here.

use std::path::{Path, PathBuf};

use crate::{Base, Entry, Kind, Problem, Status, digest};

/// The corpus directory's name under `knowledge/`.
pub const DIR: &str = "openagents";

/// The id prefix every product entry has.
pub const PREFIX: &str = "openagents.";

/// The corpus's name, for evidence and logs.
pub const NAME: &str = "openagents-product";

/// The variable that names the product corpus directory.
pub const DIR_VAR: &str = "OPENAGENTS_PRODUCT_KNOWLEDGE";

/// The longest `answer`, in characters: one to three sentences on a phone.
pub const MAX_ANSWER_CHARS: usize = 600;

/// First-level directories a cite may not name, because they are private
/// repositories' names on the owner's machines or local secrets. A product
/// entry cites this public repository only.
const PRIVATE: &[&str] = &["alpha", ".secrets", "forge", "treasury"];

/// First-person singular words the plural voice never uses.
const SINGULAR: &[&str] = &[
    "i", "i'm", "i'll", "i've", "i'd", "me", "my", "mine", "myself",
];

/// The corpus directory: `OPENAGENTS_PRODUCT_KNOWLEDGE`, or else
/// `knowledge/openagents/` in the checkout this binary was built from.
#[must_use]
pub fn default_dir() -> PathBuf {
    match std::env::var_os(DIR_VAR) {
        Some(dir) if !dir.is_empty() => PathBuf::from(dir),
        _ => repository().join("knowledge").join(DIR),
    }
}

/// The checkout this binary was built from, which cites are relative to.
#[must_use]
pub fn repository() -> PathBuf {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    root.canonicalize().unwrap_or(root)
}

/// The admitted product entries and the corpus's identity.
#[derive(Clone, Debug, Default)]
pub struct Corpus {
    pub base: Base,
    /// `sha256:` over the admitted entries' ids and digests, in id order,
    /// so a changed, added, or withdrawn entry changes it.
    pub digest: String,
}

impl Corpus {
    /// Reads `dir`, checks every entry (cites against `root` when given),
    /// and keeps the admitted ones.
    ///
    /// # Errors
    ///
    /// Any problem [`Base::read`] or [`check`] finds.
    pub fn load(dir: &Path, root: Option<&Path>) -> Result<Self, String> {
        let (entries, mut problems) = Base::read(dir);
        problems.extend(check(&entries, root));
        if let Some(problem) = problems.first() {
            return Err(format!(
                "the product knowledge base in {} has {} problems; the first is {problem}",
                dir.display(),
                problems.len()
            ));
        }
        Ok(Corpus::of(entries))
    }

    /// The admitted entries of `entries`, already checked.
    #[must_use]
    pub fn of(entries: Vec<Entry>) -> Self {
        let mut entries: Vec<Entry> = entries
            .into_iter()
            .filter(|e| e.status == Status::Admitted)
            .collect();
        entries.sort_by(|a, b| a.id.cmp(&b.id));
        let listing: String = entries
            .iter()
            .map(|e| format!("{} {}\n", e.id, e.digest))
            .collect();
        Corpus {
            digest: digest(listing.as_bytes()),
            base: Base { entries },
        }
    }

    /// `openagents-product@` and the first 12 hex digits of the digest.
    #[must_use]
    pub fn tag(&self) -> String {
        let hex = self.digest.trim_start_matches("sha256:");
        format!("{NAME}@{}", &hex[..hex.len().min(12)])
    }

    /// The entry with `id`.
    #[must_use]
    pub fn get(&self, id: &str) -> Option<&Entry> {
        self.base.get(id)
    }
}

/// Every problem with `entries` as product entries. Cites are checked
/// against the files under `root` when it is given.
#[must_use]
pub fn check(entries: &[Entry], root: Option<&Path>) -> Vec<Problem> {
    let mut problems = Vec::new();
    for entry in entries {
        let mut problem = |message: String| {
            problems.push(Problem {
                at: entry.id.clone(),
                message,
            });
        };
        if entry.kind != Kind::Product {
            problem(format!("the kind is {}, not product", entry.kind));
        }
        if !entry.id.starts_with(PREFIX) {
            problem(format!("the id must start with {PREFIX}"));
        }
        if entry.cites.is_empty() {
            problem("it cites no source; cite the documents it was written from".to_string());
        }
        for cite in &entry.cites {
            if let Err(message) = public_path(cite) {
                problem(format!("the cite `{cite}` {message}"));
            } else if let Some(root) = root
                && !root.join(path_of(cite)).exists()
            {
                problem(format!("the cite `{cite}` names no file in the repository"));
            }
        }
        match &entry.answer {
            None => {}
            Some(answer) if answer.chars().count() > MAX_ANSWER_CHARS => problem(format!(
                "the answer has {} characters; the most is {MAX_ANSWER_CHARS}",
                answer.chars().count()
            )),
            Some(answer) => {
                if let Some(word) = singular(answer) {
                    problem(format!(
                        "the answer says `{word}`; speak as \"we\", never in the first person singular"
                    ));
                }
                let allowed = entry.tags.iter().any(|tag| tag == IN_APP_TAG)
                    || UNLINKED_ANSWERS.iter().any(|(id, _)| *id == entry.id);
                if !allowed && let Some(step) = unlinked_instruction(answer) {
                    problem(format!(
                        "the answer tells the reader to act (\"{step}\") with no https:// link \
                         and no `command`; add the exact page or the one command to run, or tag \
                         it {IN_APP_TAG} when the steps are screens of the app the reader is in"
                    ));
                }
            }
        }
        if let Some(word) = singular(&entry.body) {
            problem(format!(
                "the body says `{word}`; speak as \"we\", never in the first person singular"
            ));
        }
    }
    problems
}

/// The tag of an entry whose answer walks through screens of the OpenAgents
/// phone or desktop app, which the reader is in: the screen is the action,
/// so its steps need no link or command, and the website's chat never
/// shows that answer whole.
pub const IN_APP_TAG: &str = "in-app";

/// Product entries whose answers read as instructions but tell no one to
/// do anything, each with the reason. Keep this short: fix the answer
/// instead when a person is meant to act on it.
pub const UNLINKED_ANSWERS: &[(&str, &str)] = &[(
    "openagents.ttc-measure-next",
    "an essay's list of our own open problems, not advice to the reader",
)];

/// Words that start an instruction to the reader.
const IMPERATIVES: &[&str] = &[
    "open", "tap", "click", "install", "run", "download", "go to", "visit", "connect", "sign in",
    "sign up", "log in", "choose", "pick", "type", "scan", "enter", "paste", "press", "select",
    "get", "turn on", "add", "update", "build",
];

/// Leading phrases that set up an instruction ("To add a key, open …",
/// "In the Wallet, choose …"): the instruction is what follows the comma.
const SETUPS: &[&str] = &[
    "to ",
    "in ",
    "on ",
    "from ",
    "once ",
    "after ",
    "signed in",
    "while ",
    "when ",
    "if ",
];

/// The first instruction to the reader in `text` that carries neither an
/// absolute `https://` link nor a code-formatted command, or `None` when
/// every instruction can be acted on as written.
///
/// This is a lint heuristic over reviewed, user-facing copy (the owner's
/// rule of 2026-10-09: advice to do something always carries the link or
/// the one command to run). It reads our own text, never a person's
/// message, and decides nothing about routing.
#[must_use]
pub fn unlinked_instruction(text: &str) -> Option<String> {
    if text.contains("https://") || has_code(text) {
        return None;
    }
    instruction(text)
}

/// Whether `text` has a non-empty `` `code` `` span.
fn has_code(text: &str) -> bool {
    text.split('`')
        .enumerate()
        .any(|(n, part)| n % 2 == 1 && !part.trim().is_empty() && n + 1 < text.split('`').count())
}

/// The first clause of `text` that starts with an instruction.
fn instruction(text: &str) -> Option<String> {
    let mut clauses: Vec<String> = Vec::new();
    let mut current = String::new();
    let chars: Vec<char> = text.chars().collect();
    for (n, c) in chars.iter().enumerate() {
        let ends = matches!(c, '.' | '!' | '?' | ';' | ':')
            && chars.get(n + 1).is_none_or(|next| next.is_whitespace());
        if ends || matches!(c, '(' | ')') {
            clauses.push(std::mem::take(&mut current));
        } else {
            current.push(*c);
        }
    }
    clauses.push(current);
    let pieces = clauses.into_iter().flat_map(|clause| {
        clause
            .replace(", then ", "\u{1}")
            .replace(" then ", "\u{1}")
            .split('\u{1}')
            .map(str::to_string)
            .collect::<Vec<_>>()
    });
    for piece in pieces {
        let mut low = piece
            .trim()
            .trim_matches('*')
            .trim()
            .to_lowercase()
            .replace('\u{2019}', "'");
        if SETUPS.iter().any(|setup| low.starts_with(setup))
            && let Some((_, rest)) = low.split_once(", ")
        {
            low = rest.trim().to_string();
        }
        for lead in [
            "and ", "or ", "then ", "just ", "please ", "first, ", "first ",
        ] {
            if let Some(rest) = low.strip_prefix(lead) {
                low = rest.to_string();
            }
        }
        let low = low.trim_start_matches('*').to_string();
        if IMPERATIVES.iter().any(|verb| {
            low == *verb
                || low.starts_with(&format!("{verb} "))
                || low.starts_with(&format!("{verb},"))
        }) {
            return Some(piece.trim().chars().take(80).collect());
        }
    }
    None
}

/// The file a cite names: the cite without a `#section` anchor.
fn path_of(cite: &str) -> &str {
    cite.split_once('#').map_or(cite, |(path, _)| path)
}

/// Whether a cite is a relative path in this repository that names no
/// private repository.
fn public_path(cite: &str) -> Result<(), &'static str> {
    let path = path_of(cite);
    if path.is_empty() || path.contains("://") {
        return Err("is not a repository path");
    }
    if path.starts_with('/') || path.starts_with('~') || path.contains('\\') {
        return Err("must be relative to the repository root");
    }
    if path.split('/').any(|part| part == "..") {
        return Err("leaves the repository");
    }
    let first = path.split('/').next().unwrap_or_default();
    if PRIVATE.contains(&first) {
        return Err("names a private repository");
    }
    Ok(())
}

/// `text` without its code spans (`` `…` ``) and bold interface labels
/// (`**My reports**`), which quote names rather than speak.
fn prose(text: &str) -> String {
    let outside = |text: &str, mark: &str| -> String {
        text.split(mark)
            .enumerate()
            .filter(|(n, _)| n % 2 == 0)
            .map(|(_, part)| part)
            .collect::<Vec<_>>()
            .join(" ")
    };
    outside(&outside(text, "`"), "**")
}

/// The first first-person singular word in `text`, outside code spans and
/// bold interface labels.
fn singular(text: &str) -> Option<String> {
    let prose = prose(text);
    prose
        .split(|c: char| !(c.is_alphanumeric() || c == '\'' || c == '\u{2019}'))
        .map(|word| word.replace('\u{2019}', "'"))
        .find(|word| SINGULAR.contains(&word.to_lowercase().as_str()))
}

/// One entry a retrieval kept, as the grounded model sees it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Reference {
    /// The entry's id and version, `openagents.wallet-send@1`.
    pub id: String,
    pub title: String,
    /// What the model may answer from.
    pub text: String,
    /// The documents it was written from.
    pub source: String,
}

impl Reference {
    /// The reference for `entry`: its summary, then its body.
    #[must_use]
    pub fn of(entry: &Entry) -> Self {
        Reference {
            id: format!("{}@{}", entry.id, entry.version),
            title: entry.title.clone(),
            text: format!("{}\n\n{}", entry.summary, entry.body),
            source: entry.cites.join(", "),
        }
    }

    /// The id without its version.
    #[must_use]
    pub fn base_id(&self) -> &str {
        self.id.split_once('@').map_or(&self.id, |(id, _)| id)
    }
}

/// What the model is told when retrieval kept nothing, so it says what it
/// does not know instead of inventing product facts.
pub const NO_DOCUMENTED_ANSWER: &str = "We have no documented answer to this question about \
OpenAgents in our product notes. If the user asks about the OpenAgents app or its services, say \
plainly that we don't have that documented yet; do not guess or state product facts, features, \
prices, or dates that are not already in this conversation.";

/// The grounded model's instructions: answer only from `references`, cite
/// each one used by its id in square brackets, and say so when they do not
/// answer. With no references this is [`NO_DOCUMENTED_ANSWER`].
#[must_use]
pub fn instructions(references: &[Reference]) -> String {
    if references.is_empty() {
        return NO_DOCUMENTED_ANSWER.to_string();
    }
    let mut out = String::from(
        "We are OpenAgents, answering in the OpenAgents app's chat. Always speak as \"we\" and \
\"us\", never \"I\" or \"me\". Answer the user's latest message using only the reference entries \
below, which are our reviewed product notes. After each sentence that uses an entry, cite it by \
its id in square brackets, such as [openagents.wallet-send]. Do not add facts about OpenAgents, \
its app, prices, dates, or plans that the entries do not state. If the entries do not answer what \
the user asked, say plainly that we don't have that documented yet, and give only what the \
entries do say. When you summarize or describe a document an entry links, such as one of our \
essays, give its link from the entry. When you tell the user to do something, give the exact \
https:// link or the command to run that the entries give, with a command in a code span. Keep it short for a phone screen, and use Markdown only when \
it helps.\n",
    );
    for reference in references {
        out.push_str(&format!(
            "\n<entry id=\"{}\" title=\"{}\" sources=\"{}\">\n{}\n</entry>\n",
            reference.base_id(),
            reference.title.replace('"', "'"),
            reference.source.replace('"', "'"),
            reference.text.trim()
        ));
    }
    out
}

/// Which entries a grounded reply cited.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Cited {
    /// Cited ids that name a reference the reply was given, in order, once
    /// each.
    pub known: Vec<String>,
    /// Cited ids under [`PREFIX`] that name no reference it was given: a
    /// citation the model made up.
    pub unknown: Vec<String>,
}

impl Cited {
    /// Whether the reply cited at least one given entry and invented none.
    #[must_use]
    pub fn grounded(&self) -> bool {
        !self.known.is_empty() && self.unknown.is_empty()
    }
}

/// The `[openagents.…]` citations in `reply`, sorted into the ones that
/// name one of `references` and the ones that don't. This parses the
/// model's own output for a bounded shape after the route was chosen; it
/// decides nothing about the user's message.
#[must_use]
pub fn check_reply(reply: &str, references: &[Reference]) -> Cited {
    let mut cited = Cited::default();
    for piece in reply.split('[').skip(1) {
        let Some((inside, _)) = piece.split_once(']') else {
            continue;
        };
        for id in inside.split([',', ';']).map(str::trim) {
            let id = id.split_once('@').map_or(id, |(id, _)| id);
            if !id.starts_with(PREFIX) || !crate::valid_id(id) {
                continue;
            }
            let list = if references.iter().any(|r| r.base_id() == id) {
                &mut cited.known
            } else {
                &mut cited.unknown
            };
            if !list.iter().any(|seen| seen == id) {
                list.push(id.to_string());
            }
        }
    }
    cited
}

#[cfg(test)]
mod tests;
