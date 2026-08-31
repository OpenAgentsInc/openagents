//! Tab completion for the composer.
//!
//! Two things are worth completing in a coder session and neither of them is
//! prose: the session's own commands, and paths on this machine. Both are
//! finite sets that can be enumerated exactly, so a completion here is never a
//! guess — either the candidate exists or it is not offered.
//!
//! Tab **never inserts a candidate that was not the only one**. With several
//! matches it extends the word by however much they all agree on and lists
//! them; the next keystroke narrows the set. That is the readline behaviour,
//! and it means Tab cannot silently choose something you did not mean.
//!
//! [`complete`] is pure apart from reading the directory it is completing in,
//! which is what the tests below give it a temporary one of.

use std::path::{Path, PathBuf};

/// What Tab found.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Completion {
    /// Text to insert at the caret. Empty when nothing can be added.
    pub insert: String,
    /// Every candidate, when there is more than one, for the reader to see.
    /// Empty when the completion was unambiguous or when there was none.
    pub candidates: Vec<String>,
}

impl Completion {
    fn none() -> Self {
        Self::default()
    }

    pub fn is_empty(&self) -> bool {
        self.insert.is_empty() && self.candidates.is_empty()
    }
}

/// Complete the word ending at `caret` in `text`.
///
/// `commands` is the set of slash commands the session actually handles; it is
/// passed in rather than kept here so a command that is added without being
/// wired cannot be offered.
pub fn complete(text: &str, caret: usize, commands: &[&str], cwd: &Path) -> Completion {
    let caret = caret.min(text.len());
    let head = &text[..caret];

    // A slash command is only a command at the very start of the composer, and
    // only while its name is still being typed.
    if let Some(partial) = head.strip_prefix('/')
        && !partial.contains(char::is_whitespace)
    {
        return from_candidates(
            partial,
            commands
                .iter()
                .filter(|name| name.starts_with(partial))
                .map(|name| (*name).to_string())
                .collect(),
        );
    }

    let word = word_at(head);
    // An empty composer has no path prefix to complete. Listing the whole
    // working directory from a stray Tab fills the transcript with unrelated
    // names, so wait until the reader has typed a path prefix.
    if word.is_empty() {
        return Completion::none();
    }
    paths(word, cwd)
}

/// The word the caret is at the end of: everything back to whitespace.
fn word_at(head: &str) -> &str {
    let start = head
        .char_indices()
        .rev()
        .find(|(_, c)| c.is_whitespace())
        .map_or(0, |(index, c)| index + c.len_utf8());
    &head[start..]
}

/// Path candidates for `word`, which may be absolute, `~`-rooted, or relative.
fn paths(word: &str, cwd: &Path) -> Completion {
    // A leading `@` is how a prompt names a file to the model; it is not part
    // of the path being completed.
    let body = word.strip_prefix('@').unwrap_or(word);

    // Split into the directory to read and the stem being matched in it.
    let (directory, stem) = match body.rsplit_once('/') {
        Some(("", stem)) => (PathBuf::from("/"), stem),
        Some(("~", stem)) => (crate::auth::home_directory(), stem),
        Some((dir, stem)) => {
            let base = match dir.strip_prefix("~/") {
                Some(rest) => crate::auth::home_directory().join(rest),
                None if Path::new(dir).is_absolute() => PathBuf::from(dir),
                None => cwd.join(dir),
            };
            (base, stem)
        }
        None if body == "~" => (crate::auth::home_directory(), ""),
        None => (cwd.to_path_buf(), body),
    };

    let Ok(reading) = std::fs::read_dir(&directory) else {
        return Completion::none();
    };
    let mut found: Vec<String> = Vec::new();
    for entry in reading.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if !name.starts_with(stem) {
            continue;
        }
        // A dotfile is offered only once the reader has typed the dot, which
        // is what keeps a bare Tab from listing every `.git` in the tree.
        if name.starts_with('.') && !stem.starts_with('.') {
            continue;
        }
        let is_directory = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);
        found.push(if is_directory {
            format!("{name}/")
        } else {
            name
        });
    }
    found.sort();
    from_candidates(stem, found)
}

/// Turn candidates into what Tab should do about them.
fn from_candidates(typed: &str, candidates: Vec<String>) -> Completion {
    match candidates.len() {
        0 => Completion::none(),
        1 => {
            let only = &candidates[0];
            let mut insert = only[typed.len()..].to_string();
            // A completed command gets its space; a completed directory has
            // already got its slash and is likely to be typed into further.
            if !only.ends_with('/') {
                insert.push(' ');
            }
            Completion {
                insert,
                candidates: Vec::new(),
            }
        }
        _ => {
            let shared = common_prefix(&candidates);
            Completion {
                insert: shared[typed.len().min(shared.len())..].to_string(),
                candidates,
            }
        }
    }
}

/// The longest prefix every candidate shares.
///
/// Built up a character at a time rather than cut down a byte at a time, so a
/// candidate with a multi-byte character in it cannot be split through one.
fn common_prefix(candidates: &[String]) -> String {
    let Some(first) = candidates.first() else {
        return String::new();
    };
    let mut shared = String::new();
    for (index, ch) in first.char_indices() {
        let end = index + ch.len_utf8();
        if candidates[1..]
            .iter()
            .all(|other| other.starts_with(&first[..end]))
        {
            shared.push(ch);
        } else {
            break;
        }
    }
    shared
}

#[cfg(test)]
mod tests {
    use super::*;

    const COMMANDS: &[&str] = &["clear", "diff", "export", "help", "run"];

    fn scratch() -> tempfile::TempDir {
        let dir = tempfile::tempdir().expect("a temporary directory");
        std::fs::create_dir(dir.path().join("crates")).expect("crates/");
        std::fs::create_dir(dir.path().join("credentials")).expect("credentials/");
        std::fs::write(dir.path().join("README.md"), "").expect("README.md");
        std::fs::write(dir.path().join("Cargo.toml"), "").expect("Cargo.toml");
        std::fs::write(dir.path().join(".hidden"), "").expect(".hidden");
        std::fs::write(dir.path().join("crates").join("one.rs"), "").expect("one.rs");
        dir
    }

    fn at(text: &str, dir: &Path) -> Completion {
        complete(text, text.len(), COMMANDS, dir)
    }

    #[test]
    fn one_matching_command_completes_and_adds_its_space() {
        let dir = scratch();
        assert_eq!(
            at("/ex", dir.path()),
            Completion {
                insert: "port ".to_string(),
                candidates: Vec::new()
            }
        );
    }

    #[test]
    fn several_matching_commands_are_all_listed_and_none_is_chosen() {
        let dir = scratch();
        let found = at("/", dir.path());
        assert_eq!(
            found.candidates,
            vec!["clear", "diff", "export", "help", "run"]
        );
        assert_eq!(
            found.insert, "",
            "Tab chose a command when five matched: {found:?}"
        );
    }

    #[test]
    fn a_shared_prefix_is_inserted_and_the_choice_is_left_open() {
        let dir = scratch();
        // Both directories in the scratch tree start with `cr`.
        let found = at("cr", dir.path());
        assert_eq!(found.insert, "");
        assert_eq!(found.candidates, vec!["crates/", "credentials/"]);

        let found = at("c", dir.path());
        assert_eq!(found.insert, "r", "the shared prefix was not offered");
    }

    #[test]
    fn a_command_that_matches_nothing_offers_nothing() {
        let dir = scratch();
        assert!(at("/nope", dir.path()).is_empty());
    }

    #[test]
    fn a_path_completes_inside_the_working_directory() {
        let dir = scratch();
        assert_eq!(at("REA", dir.path()).insert, "DME.md ");
    }

    #[test]
    fn a_directory_keeps_its_slash_and_gets_no_space() {
        let dir = scratch();
        let found = at("crat", dir.path());
        assert_eq!(found.insert, "es/");
        assert!(found.candidates.is_empty());
    }

    #[test]
    fn completing_continues_inside_a_directory() {
        let dir = scratch();
        assert_eq!(at("crates/o", dir.path()).insert, "ne.rs ");
    }

    #[test]
    fn a_path_argument_after_a_command_completes_as_a_path() {
        let dir = scratch();
        assert_eq!(at("/diff REA", dir.path()).insert, "DME.md ");
    }

    #[test]
    fn an_empty_composer_does_not_list_the_working_directory() {
        let dir = scratch();
        assert!(at("", dir.path()).is_empty());
    }

    #[test]
    fn a_dotfile_is_offered_only_once_the_dot_is_typed() {
        let dir = scratch();
        assert_eq!(at(".hid", dir.path()).insert, "den ");
    }

    #[test]
    fn an_at_mention_completes_the_path_after_the_sigil() {
        let dir = scratch();
        assert_eq!(at("look at @Car", dir.path()).insert, "go.toml ");
    }

    #[test]
    fn tab_after_a_space_offers_nothing_rather_than_the_whole_directory() {
        let dir = scratch();
        assert!(at("tell me about ", dir.path()).is_empty());
    }

    #[test]
    fn a_slash_command_stops_being_completable_once_it_has_an_argument() {
        let dir = scratch();
        // `/diff cr` completes a path, not a command, even though `cr` would
        // match no command anyway — the point is which set was consulted.
        let found = at("/diff cr", dir.path());
        assert_eq!(found.candidates, vec!["crates/", "credentials/"]);
    }

    #[test]
    fn the_caret_is_where_completion_happens_not_the_end_of_the_line() {
        let dir = scratch();
        let text = "REA and more";
        let found = complete(text, 3, COMMANDS, dir.path());
        assert_eq!(found.insert, "DME.md ");
    }

    #[test]
    fn a_directory_that_cannot_be_read_offers_nothing() {
        let dir = scratch();
        assert!(at("no/such/place/x", dir.path()).is_empty());
    }

    #[test]
    fn the_shared_prefix_of_multibyte_names_is_cut_on_a_character() {
        assert_eq!(
            common_prefix(&["café-one".to_string(), "café-two".to_string()]),
            "café-"
        );
        assert_eq!(
            common_prefix(&["日本語".to_string(), "日本".to_string()]),
            "日本"
        );
    }
}
