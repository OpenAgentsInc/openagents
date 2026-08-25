//! The OpenAgents knowledge base, as a `packet-v0` guest plugin.
//!
//! The corpus is generated ahead of time by `build-kb.mjs` — curated stances
//! (reviewed positions on what is built, parked, or planned, each dated) plus
//! summaries of the public docs — and embedded into the artifact with
//! `include_str!`, so answering needs no mounts, no network, and no host
//! imports at all. The guest is a ranking function over that frozen corpus.
//!
//! Scoring is deliberately simple lexical overlap: the query's distinct
//! words of three or more characters are matched against each entry's title,
//! questions, tags, state, and body text. A stance whose curated `questions`
//! overlap the query is what the corpus exists for, so question hits weigh
//! heaviest and stances outrank docs at equal overlap. The harness surfaces
//! the top hits as context; it never asks the model to call this as a tool.

use openagents_pdk::{plugin_entry, Refusal, RefusalCode};
use serde::{Deserialize, Serialize};

/// The generated corpus, frozen into the artifact at build time.
const KB: &str = include_str!("../kb.json");

/// Most hits one lookup returns; the harness attaches at most the top few.
const LIMIT_BOUND: usize = 10;
const DEFAULT_LIMIT: usize = 3;

#[derive(Deserialize)]
struct Input {
    query: String,
    #[serde(default)]
    limit: Option<usize>,
}

#[derive(Deserialize)]
struct Corpus {
    entries: Vec<Entry>,
}

#[derive(Deserialize, Clone)]
#[serde(tag = "kind", rename_all = "lowercase")]
enum Entry {
    Stance {
        title: String,
        questions: Vec<String>,
        state: String,
        answer: String,
        #[serde(default)]
        sources: Vec<String>,
        date: String,
    },
    Doc {
        title: String,
        source: String,
        #[serde(default)]
        headings: Vec<String>,
        summary: String,
        #[serde(default)]
        tags: Vec<String>,
    },
}

#[derive(Serialize)]
struct Hit {
    kind: &'static str,
    title: String,
    /// The stance's position or the doc's summary — the text worth attaching.
    body: String,
    /// The stance's lifecycle state ("built", "parked", …); absent for docs.
    #[serde(skip_serializing_if = "Option::is_none")]
    state: Option<String>,
    /// Where the claim comes from: doc paths, or a stance's cited sources.
    sources: Vec<String>,
    /// The date the stance was last reviewed; absent for docs.
    #[serde(skip_serializing_if = "Option::is_none")]
    date: Option<String>,
    score: u32,
}

#[derive(Serialize)]
struct Output {
    hits: Vec<Hit>,
}

/// The query's distinct lowercase words of three or more characters — the
/// same shape the capability matcher uses, so the two rails rank alike.
fn terms(query: &str) -> Vec<String> {
    let mut words: Vec<String> = query
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| w.len() >= 3)
        .map(|w| w.to_lowercase())
        .collect();
    words.sort();
    words.dedup();
    words
}

fn contains_word(haystack: &str, word: &str) -> bool {
    haystack
        .split(|c: char| !c.is_alphanumeric())
        .any(|w| w.eq_ignore_ascii_case(word))
}

/// Overlap score: question and title hits carry the corpus's intent, so they
/// weigh 3; tags and state weigh 2; body text weighs 1.
fn score(entry: &Entry, terms: &[String]) -> u32 {
    let mut total = 0;
    for term in terms {
        total += match entry {
            Entry::Stance {
                title,
                questions,
                state,
                answer,
                ..
            } => {
                let questioned = questions.iter().any(|q| contains_word(q, term));
                if questioned || contains_word(title, term) {
                    3
                } else if contains_word(state, term) {
                    2
                } else if contains_word(answer, term) {
                    1
                } else {
                    0
                }
            }
            Entry::Doc {
                title,
                headings,
                summary,
                tags,
                ..
            } => {
                if contains_word(title, term) {
                    3
                } else if tags.iter().any(|t| t.eq_ignore_ascii_case(term))
                    || headings.iter().any(|h| contains_word(h, term))
                {
                    2
                } else if contains_word(summary, term) {
                    1
                } else {
                    0
                }
            }
        };
    }
    total
}

fn hit(entry: Entry, score: u32) -> Hit {
    match entry {
        Entry::Stance {
            title,
            state,
            answer,
            sources,
            date,
            ..
        } => Hit {
            kind: "stance",
            title,
            body: answer,
            state: Some(state),
            sources,
            date: Some(date),
            score,
        },
        Entry::Doc {
            title,
            source,
            summary,
            ..
        } => Hit {
            kind: "doc",
            title,
            body: summary,
            state: None,
            sources: vec![source],
            date: None,
            score,
        },
    }
}

fn handle(input: Input) -> Result<Output, Refusal> {
    let terms = terms(&input.query);
    if terms.is_empty() {
        return Err(Refusal {
            code: RefusalCode::Unsupported,
            reason: "The query held no words of three or more characters to match on.".to_string(),
        });
    }

    let corpus: Corpus = serde_json::from_str(KB).map_err(|error| Refusal {
        code: RefusalCode::Internal,
        reason: format!("The embedded knowledge base did not parse: {error}"),
    })?;

    let limit = input.limit.unwrap_or(DEFAULT_LIMIT).clamp(1, LIMIT_BOUND);

    let mut scored: Vec<(u32, u32, Entry)> = corpus
        .entries
        .into_iter()
        .filter_map(|entry| {
            let overlap = score(&entry, &terms);
            if overlap == 0 {
                return None;
            }
            // Stances outrank docs at equal overlap: they are the reviewed
            // positions the corpus exists to carry.
            let stance = matches!(entry, Entry::Stance { .. }) as u32;
            Some((overlap, stance, entry))
        })
        .collect();
    scored.sort_by(|a, b| (b.0, b.1).cmp(&(a.0, a.1)));
    scored.truncate(limit);

    Ok(Output {
        hits: scored
            .into_iter()
            .map(|(overlap, _, entry)| hit(entry, overlap))
            .collect(),
    })
}

plugin_entry!(handle);

#[cfg(test)]
mod tests;
