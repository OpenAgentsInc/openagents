//! Values that recur across data files: columns in different tables whose
//! values overlap, found by code.
//!
//! A per-file profile describes each table on its own. Records in
//! different files often describe the same things, and a column whose
//! values recur in another file's column says so: a shared identifier, a
//! foreign key, or a field two sources both carry. The pass reads each
//! table the profile read, keeps its columns with many distinct values,
//! and reports the pairs of columns in different files with the most
//! values in common. It states what it counted and nothing else.

use std::collections::HashSet;
use std::path::Path;

use super::{FileProfile, Kind, Params, count, read_head, split_row};

/// The label of the evidence item the pass adds.
pub const LABEL: &str = "Values shared across data files";

/// A column needs at least this many distinct values to be compared.
pub const MIN_DISTINCT: usize = 50;

/// And at least one distinct value per this many filled rows.
pub const MIN_SPREAD: usize = 5;

/// A pair is reported when its columns share at least this many values.
pub const MIN_SHARED: usize = 20;

/// Distinct values kept per column, at most.
pub const MAX_VALUES: usize = 100_000;

/// Normalized values shorter than this aren't compared.
pub const MIN_LEN: usize = 4;

/// Pairs reported, at most.
pub const MAX_PAIRS: usize = 12;

/// One pair of columns in different files with values in common.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pair {
    pub a_file: String,
    pub a_column: String,
    pub a_distinct: usize,
    pub b_file: String,
    pub b_column: String,
    pub b_distinct: usize,
    pub shared: usize,
}

/// A value as the pass compares it: lowercase letters and digits only,
/// so `555-0142` and `5550142` match. `None` when too short to compare.
#[must_use]
pub fn normalize(value: &str) -> Option<String> {
    let kept: String = value
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .map(|c| c.to_ascii_lowercase())
        .collect();
    (kept.len() >= MIN_LEN).then_some(kept)
}

/// The comparable columns of one table: its header's names and each
/// column's normalized values. A table without a header row has none.
#[must_use]
pub fn columns(text: &str, delimiter: u8) -> Vec<(String, HashSet<String>)> {
    let mut lines = text.lines().filter(|l| !l.trim().is_empty());
    let Some(first) = lines.next() else {
        return Vec::new();
    };
    let names = split_row(first.trim_end_matches('\r'), delimiter);
    let header = names
        .iter()
        .all(|n| !n.trim().is_empty() && n.trim().parse::<f64>().is_err());
    if !header {
        return Vec::new();
    }
    let mut values: Vec<HashSet<String>> = vec![HashSet::new(); names.len()];
    let mut filled = vec![0usize; names.len()];
    for line in lines.take(super::MAX_ROWS) {
        for (c, field) in split_row(line.trim_end_matches('\r'), delimiter)
            .into_iter()
            .enumerate()
        {
            let Some(set) = values.get_mut(c) else {
                continue;
            };
            if let Some(value) = normalize(&field) {
                filled[c] += 1;
                if set.len() < MAX_VALUES {
                    set.insert(value);
                }
            }
        }
    }
    names
        .into_iter()
        .zip(values)
        .zip(filled)
        .filter(|((_, set), filled)| set.len() >= MIN_DISTINCT && set.len() * MIN_SPREAD >= *filled)
        .map(|((name, set), _)| (name.trim().to_string(), set))
        .collect()
}

/// The column pairs in different profiled tables with the most values in
/// common, most first.
#[must_use]
pub fn pairs(root: &Path, files: &[FileProfile], params: &Params) -> Vec<Pair> {
    let mut tables = Vec::new();
    for file in files {
        let delimiter = match file.kind {
            Kind::Csv => b',',
            Kind::Tsv => b'\t',
            _ => continue,
        };
        let Ok(bytes) = read_head(&root.join(&file.path), params.read_bytes) else {
            continue;
        };
        let text = String::from_utf8_lossy(&bytes);
        let text = if file.whole {
            &text[..]
        } else {
            text.rsplit_once('\n').map_or(&text[..], |(head, _)| head)
        };
        let found = columns(text, delimiter);
        if !found.is_empty() {
            tables.push((file.path.clone(), found));
        }
    }
    let mut out = Vec::new();
    for (i, (a_file, a_columns)) in tables.iter().enumerate() {
        for (b_file, b_columns) in &tables[i + 1..] {
            for (a_column, a) in a_columns {
                for (b_column, b) in b_columns {
                    let (small, large) = if a.len() <= b.len() { (a, b) } else { (b, a) };
                    let shared = small.iter().filter(|v| large.contains(*v)).count();
                    if shared >= MIN_SHARED {
                        out.push(Pair {
                            a_file: a_file.clone(),
                            a_column: a_column.clone(),
                            a_distinct: a.len(),
                            b_file: b_file.clone(),
                            b_column: b_column.clone(),
                            b_distinct: b.len(),
                            shared,
                        });
                    }
                }
            }
        }
    }
    out.sort_by(|x, y| {
        y.shared
            .cmp(&x.shared)
            .then_with(|| x.a_file.cmp(&y.a_file))
    });
    out.truncate(MAX_PAIRS);
    out
}

/// The evidence text for `pairs`, or `None` when there are none.
#[must_use]
pub fn text(pairs: &[Pair]) -> Option<String> {
    if pairs.is_empty() {
        return None;
    }
    let mut lines = vec![format!(
        "Columns in different files whose values overlap, counted after keeping only letters \
         and digits in lowercase, and comparing columns with at least {MIN_DISTINCT} distinct \
         values:"
    )];
    for pair in pairs {
        lines.push(format!(
            "- {} `{}` and {} `{}`: {} values in common ({} and {} distinct)",
            pair.a_file,
            pair.a_column,
            pair.b_file,
            pair.b_column,
            count(pair.shared),
            count(pair.a_distinct),
            count(pair.b_distinct)
        ));
    }
    Some(lines.join("\n"))
}
