//! Corrections for a command the shell did not find (#10694): the names in
//! the shell's own inventory closest to the typed word, as choices. Taking
//! one types the corrected line at the prompt and runs nothing; Enter is
//! still the person's.

use crate::route::Table;

/// The most choices offered.
pub const MAX_CHOICES: usize = 3;
/// The most `PATH` directories and entries per directory read.
const MAX_DIRS: usize = 64;
const MAX_ENTRIES: usize = 8192;

/// The closest names to a missing command.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Choices {
    /// The corrected lines, closest first and then in name order, at most
    /// [`MAX_CHOICES`].
    pub lines: Vec<String>,
    /// How many names were equally close, shown or not.
    pub total: usize,
}

/// The edit distance between `a` and `b`, counting an adjacent swap as one
/// edit (optimal string alignment).
#[must_use]
pub fn distance(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let mut rows = vec![vec![0usize; b.len() + 1]; a.len() + 1];
    for (i, row) in rows.iter_mut().enumerate() {
        row[0] = i;
    }
    for j in 0..=b.len() {
        rows[0][j] = j;
    }
    for i in 1..=a.len() {
        for j in 1..=b.len() {
            let cost = usize::from(a[i - 1] != b[j - 1]);
            let mut best = (rows[i - 1][j] + 1)
                .min(rows[i][j - 1] + 1)
                .min(rows[i - 1][j - 1] + cost);
            if i > 1 && j > 1 && a[i - 1] == b[j - 2] && a[i - 2] == b[j - 1] {
                best = best.min(rows[i - 2][j - 2] + 1);
            }
            rows[i][j] = best;
        }
    }
    rows[a.len()][b.len()]
}

/// The farthest a correction may be from `word`: one edit for a short word,
/// two for a longer one.
fn reach(word: &str) -> usize {
    if word.chars().count() >= 5 { 2 } else { 1 }
}

/// Corrections for `line`, whose first word the shell did not find, from
/// `table`'s aliases and functions, the shell builtins, and the executables
/// on its `PATH`. `None` when nothing is close.
#[must_use]
pub fn choices(table: &Table, line: &str) -> Option<Choices> {
    let line = line.trim_start();
    let end = line.find(char::is_whitespace).unwrap_or(line.len());
    let (word, rest) = line.split_at(end);
    if word.is_empty() || word.contains('/') || word.chars().count() > 64 {
        return None;
    }
    let reach = reach(word);
    let close = |name: &str| {
        name != word
            && name.chars().count().abs_diff(word.chars().count()) <= reach
            && distance(word, name) <= reach
    };
    let mut names = std::collections::BTreeMap::<String, usize>::new();
    let mut consider = |name: &str| {
        if close(name) {
            names.insert(name.to_owned(), distance(word, name));
        }
    };
    table.aliases.iter().for_each(|name| consider(name));
    table.functions.iter().for_each(|name| consider(name));
    crate::route::BUILTINS
        .iter()
        .for_each(|name| consider(name));
    let path = table
        .path
        .clone()
        .or_else(|| std::env::var("PATH").ok())
        .unwrap_or_default();
    for dir in path.split(':').filter(|dir| !dir.is_empty()).take(MAX_DIRS) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            continue;
        };
        for entry in entries.take(MAX_ENTRIES).flatten() {
            let name = entry.file_name();
            let Some(name) = name.to_str() else {
                continue;
            };
            if close(name) && crate::route::executable(&entry.path()) {
                consider(name);
            }
        }
    }
    let best = names.values().copied().min()?;
    let closest: Vec<&String> = names
        .iter()
        .filter(|(_, d)| **d == best)
        .map(|(name, _)| name)
        .collect();
    Some(Choices {
        total: closest.len(),
        lines: closest
            .into_iter()
            .take(MAX_CHOICES)
            .map(|name| format!("{name}{rest}"))
            .collect(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_swap_is_one_edit() {
        assert_eq!(distance("gti", "git"), 1);
        assert_eq!(distance("sl", "ls"), 1);
        assert_eq!(distance("carg", "cargo"), 1);
        assert_eq!(distance("abc", "xyz"), 3);
        assert_eq!(distance("", "ls"), 2);
    }

    #[cfg(unix)]
    #[test]
    fn choices_come_from_the_shells_own_inventory_and_stay_bounded() {
        use std::os::unix::fs::PermissionsExt;
        let bin = tempfile::tempdir().unwrap();
        for (name, mode) in [("git", 0o755), ("gist", 0o755), ("gtx", 0o644)] {
            let path = bin.path().join(name);
            std::fs::write(&path, "").unwrap();
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(mode)).unwrap();
        }
        let mut table = Table::parse(&format!(
            "p:{}\na:ll la lr lz\nf:deploy",
            bin.path().display()
        ));
        // A swap, with the rest of the line kept; a file that is not
        // executable is not a command.
        let fix = choices(&table, "gti status --short").unwrap();
        assert_eq!(fix.lines, ["git status --short"]);
        assert_eq!(fix.total, 1);
        // Functions count; a longer word reaches two edits.
        assert_eq!(choices(&table, "delpoy now").unwrap().lines, ["deploy now"]);
        // Equally close aliases: at most three, with the true count.
        let many = choices(&table, "lk").unwrap();
        assert_eq!(many.lines, ["la", "ll", "lr"]);
        assert_eq!(many.total, 4);
        // Nothing close, a path, and an empty line offer nothing.
        assert_eq!(choices(&table, "zzzzzz"), None);
        assert_eq!(choices(&table, "./gti"), None);
        assert_eq!(choices(&table, "  "), None);
        // The inventory the shell reports now is the one that counts.
        table = Table::parse("p:/nonexistent\na:\nf:");
        assert_eq!(choices(&table, "gti"), None);
    }
}
