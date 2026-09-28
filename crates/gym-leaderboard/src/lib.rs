//! The Gym's published benchmark results.
//!
//! This crate turns committed Terminal-Bench evidence (an experiment's
//! `attempts.json`, a study's round report, and the retained traces) into
//! the two files the Verse Gym and any other reader load:
//!
//! - `leaderboard.v1.json` ([`contract::Leaderboard`]): every board with
//!   its tallies, per-task rows, per-attempt rows, labels, caveats, and the
//!   digest of every evidence file it read.
//! - `traces/<board>/<attempt>.json` ([`contract::TraceBundle`]): one
//!   attempt's instruction, Jev decision, briefing, timed delegate steps,
//!   and verifier result, scrubbed and bounded for a phone.
//!
//! No number is typed by hand: each is computed from the evidence, and
//! each source adapter recomputes the study's verdicts and refuses to
//! build when they disagree with what the study recorded. The output is
//! deterministic, so `check` can regenerate it and compare bytes.
//!
//! Read `docs/verse/gym-leaderboard.md` before changing the contract.

pub mod bundle;
pub mod contract;
pub mod evidence;
pub mod scrub;
pub mod tb21_oos;
pub mod tb4_delegate;

use std::path::Path;

use contract::{
    Board, Generator, INDEX_SCHEMA, Index, LEADERBOARD_SCHEMA, Leaderboard, Publication, TraceRef,
};
use evidence::{Reader, Result, fail, sha256_hex};

/// A beat whose cost or time margin is under this fraction is labeled
/// [`contract::Label::ThinMargin`].
pub const THIN_MARGIN: f64 = 0.05;

/// The leaderboard's serialized size bound. A phone fetches it on entry.
pub const MAX_LEADERBOARD_BYTES: usize = 512 * 1024;

/// Where the publication lives, repository-relative.
pub const PUBLISHED: &str = "bench/terminal-bench/published";

/// The leaderboard's file name inside the publication.
pub const LEADERBOARD_FILE: &str = "leaderboard.v1.json";

/// The index's file name inside the publication.
pub const INDEX_FILE: &str = "index.json";

/// A generated publication, not yet written.
#[derive(Debug)]
pub struct Output {
    pub leaderboard: Leaderboard,
    /// The leaderboard file's bytes.
    pub leaderboard_bytes: Vec<u8>,
    /// Bundle files, as (path relative to the publication, bytes).
    pub bundles: Vec<(String, Vec<u8>)>,
}

/// Builds every board and bundle from the repository at `root`.
pub fn generate(root: &Path) -> Result<Output> {
    let reader = Reader::new(root);
    let (mut delegate, jobs) = tb4_delegate::build(&reader)?;
    let mut bundles = Vec::new();
    for job in &jobs {
        let bundle = bundle::build(
            &reader,
            &bundle::Input {
                board: &delegate.id,
                attempt: &job.attempt,
                bar: &job.bar,
                episode: &job.episode,
                own_entries: &job.own_entries,
            },
        )?;
        let mut bytes = serde_json::to_vec(&bundle).map_err(|e| fail!("serialize: {e}"))?;
        bytes.push(b'\n');
        let path = job
            .attempt
            .trace
            .as_ref()
            .map(|t| t.path.clone())
            .ok_or_else(|| fail!("{}: no trace path", job.attempt.id))?;
        let slot = delegate
            .attempts
            .iter_mut()
            .find(|a| a.id == job.attempt.id)
            .ok_or_else(|| fail!("{}: not on the board", job.attempt.id))?;
        slot.trace = Some(TraceRef {
            path: path.clone(),
            sha256: sha256_hex(&bytes),
            bytes: bytes.len() as u64,
        });
        bundles.push((path, bytes));
    }
    let boards = vec![delegate, tb21_oos::build(&reader)?];
    let leaderboard = leaderboard(boards)?;
    let mut leaderboard_bytes =
        serde_json::to_vec_pretty(&leaderboard).map_err(|e| fail!("serialize: {e}"))?;
    leaderboard_bytes.push(b'\n');
    if leaderboard_bytes.len() > MAX_LEADERBOARD_BYTES {
        return Err(fail!(
            "the leaderboard is {} bytes, over its {MAX_LEADERBOARD_BYTES}-byte bound",
            leaderboard_bytes.len()
        ));
    }
    Ok(Output {
        leaderboard,
        leaderboard_bytes,
        bundles,
    })
}

fn leaderboard(boards: Vec<Board>) -> Result<Leaderboard> {
    let value = serde_json::to_value(&boards).map_err(|e| fail!("serialize: {e}"))?;
    Ok(Leaderboard {
        schema: LEADERBOARD_SCHEMA.into(),
        generator: Generator {
            name: env!("CARGO_PKG_NAME").into(),
            version: env!("CARGO_PKG_VERSION").into(),
        },
        digest: atif::digest(&value),
        boards,
    })
}

/// Writes the publication under `out`, replacing earlier generated files,
/// and appends to the index when the digest is new.
pub fn write(out: &Path, output: &Output, commit: Option<&str>) -> Result<()> {
    let io = |e: std::io::Error| fail!("{}: {e}", out.display());
    let traces = out.join("traces");
    if traces.exists() {
        std::fs::remove_dir_all(&traces).map_err(io)?;
    }
    for (rel, bytes) in &output.bundles {
        let path = out.join(rel);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(io)?;
        }
        std::fs::write(&path, bytes).map_err(io)?;
    }
    std::fs::create_dir_all(out).map_err(io)?;
    std::fs::write(out.join(LEADERBOARD_FILE), &output.leaderboard_bytes).map_err(io)?;

    let index_path = out.join(INDEX_FILE);
    let mut index: Index = if index_path.exists() {
        let bytes = std::fs::read(&index_path).map_err(io)?;
        serde_json::from_slice(&bytes).map_err(|e| fail!("{}: {e}", index_path.display()))?
    } else {
        Index {
            schema: INDEX_SCHEMA.into(),
            publications: Vec::new(),
        }
    };
    let digest = &output.leaderboard.digest;
    if index.publications.last().map(|p| &p.digest) != Some(digest) {
        index.publications.push(Publication {
            digest: digest.clone(),
            commit: commit.map(str::to_owned),
            boards: output
                .leaderboard
                .boards
                .iter()
                .map(|b| b.id.clone())
                .collect(),
        });
    }
    let mut bytes = serde_json::to_vec_pretty(&index).map_err(|e| fail!("serialize: {e}"))?;
    bytes.push(b'\n');
    std::fs::write(&index_path, bytes).map_err(io)
}

/// Compares a regenerated publication with the files under `out`, and
/// refuses one whose bundles matched a credential rule. Returns every
/// problem found.
#[must_use]
pub fn check(out: &Path, output: &Output) -> Vec<String> {
    let mut problems = Vec::new();
    let same =
        |rel: &str, want: &[u8], problems: &mut Vec<String>| match std::fs::read(out.join(rel)) {
            Ok(have) if have == want => {}
            Ok(_) => problems.push(format!("{rel} differs from a regenerated copy")),
            Err(e) => problems.push(format!("{rel}: {e}")),
        };
    same(LEADERBOARD_FILE, &output.leaderboard_bytes, &mut problems);
    for (rel, bytes) in &output.bundles {
        same(rel, bytes, &mut problems);
    }
    let credential = scrub::credential_rules();
    for (rel, bytes) in &output.bundles {
        let Ok(bundle) = serde_json::from_slice::<contract::TraceBundle>(bytes) else {
            problems.push(format!("{rel} doesn't parse as a trace bundle"));
            continue;
        };
        for (rule, n) in &bundle.scrub.redactions {
            if credential.contains(&rule.as_str()) {
                problems.push(format!(
                    "{rel}: {n} match(es) of credential rule {rule}; inspect the retained trace before publishing"
                ));
            }
        }
    }
    match std::fs::read(out.join(INDEX_FILE))
        .ok()
        .and_then(|b| serde_json::from_slice::<Index>(&b).ok())
    {
        Some(index)
            if index.publications.last().map(|p| &p.digest) == Some(&output.leaderboard.digest) => {
        }
        _ => problems.push(format!(
            "{INDEX_FILE} doesn't end with digest {}",
            output.leaderboard.digest
        )),
    }
    problems
}

/// A fraction as a percentage: one decimal under 10%, none above, and no
/// trailing `.0`.
#[must_use]
pub fn pct(fraction: f64) -> String {
    let p = fraction * 100.0;
    let text = if p.abs() < 10.0 {
        format!("{p:.1}")
    } else {
        format!("{p:.0}")
    };
    format!("{}%", text.strip_suffix(".0").unwrap_or(&text))
}

/// Dollars: four decimals under $1, two above.
#[must_use]
pub fn usd(amount: f64) -> String {
    if amount.abs() < 1.0 {
        format!("${amount:.4}")
    } else {
        format!("${amount:.2}")
    }
}

/// The median of `values`, or `None` when empty.
#[must_use]
pub fn median(values: &[f64]) -> Option<f64> {
    let mut v: Vec<f64> = values.iter().copied().filter(|x| x.is_finite()).collect();
    if v.is_empty() {
        return None;
    }
    v.sort_by(f64::total_cmp);
    let mid = v.len() / 2;
    Some(if v.len() % 2 == 1 {
        v[mid]
    } else {
        f64::midpoint(v[mid - 1], v[mid])
    })
}
