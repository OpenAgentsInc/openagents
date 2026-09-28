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
pub mod microcoder;
pub mod reference_boards;
pub mod scrub;
pub mod tb21_oos;
pub mod tb4_delegate;
pub mod tb4_delegate_dev;
pub mod tb4_microcoder_kb;
pub mod tb4_oos;
pub mod view;

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

/// Where a person records credential-shaped matches they inspected.
pub const REVIEWS: &str = "bench/terminal-bench/leaderboard-reviewed-redactions.json";

/// The reviews file's schema.
pub const REVIEWS_SCHEMA: &str = "openagents.gym.reviewed-redactions.v1";

/// A generated publication, not yet written.
#[derive(Debug)]
pub struct Output {
    pub leaderboard: Leaderboard,
    /// The leaderboard file's bytes.
    pub leaderboard_bytes: Vec<u8>,
    /// Bundle files, as (path relative to the publication, bytes).
    pub bundles: Vec<(String, Vec<u8>)>,
    /// Credential-shaped matches a person inspected and recorded.
    pub reviews: Vec<Review>,
}

/// One inspected credential-shaped match. It holds only while the bundle
/// still reads the same source bytes: a changed source lapses it.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Review {
    /// The bundle's path inside the publication.
    pub bundle: String,
    /// The credential rule that matched.
    pub rule: String,
    /// How many matches the bundle's scrub report counts.
    pub matches: u32,
    /// The retained file the match is in, repository-relative, and its
    /// SHA-256 when it was inspected.
    pub source: String,
    pub source_sha256: String,
    /// When and what the person found.
    pub reviewed: String,
    pub finding: String,
}

fn read_reviews(reader: &Reader) -> Result<Vec<Review>> {
    if !reader.exists(REVIEWS) {
        return Ok(Vec::new());
    }
    let (value, _) = reader.json(REVIEWS)?;
    if value.get("schema").and_then(|s| s.as_str()) != Some(REVIEWS_SCHEMA) {
        return Err(fail!("{REVIEWS}: not a {REVIEWS_SCHEMA} file"));
    }
    serde_json::from_value(value.get("reviews").cloned().unwrap_or_default())
        .map_err(|e| fail!("{REVIEWS}: {e}"))
}

/// Builds every board and bundle from the repository at `root`.
pub fn generate(root: &Path) -> Result<Output> {
    let reader = Reader::new(root);
    let mut bundles = Vec::new();

    let (mut delegate, jobs) = tb4_delegate::build(&reader)?;
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
        attach(&mut delegate, &job.attempt.id, &bundle, &mut bundles)?;
    }

    let mut tb21 = tb21_oos::build(&reader)?;
    let manifest = microcoder::Manifest::read(&reader, tb21_oos::RUNS)?;
    bundle_microcoder(&reader, &mut tb21, &manifest, |_| true, &mut bundles)?;

    let (mut dev, jobs) = tb4_delegate_dev::build(&reader)?;
    for job in &jobs {
        let bundle = bundle::build(
            &reader,
            &bundle::Input {
                board: &dev.id,
                attempt: &job.attempt,
                bar: &job.bar,
                episode: &job.episode,
                own_entries: &job.own_entries,
            },
        )?;
        attach(&mut dev, &job.attempt.id, &bundle, &mut bundles)?;
    }

    let mut shared_fact = tb4_microcoder_kb::build(&reader, &delegate)?;
    let manifest = microcoder::Manifest::read(&reader, tb4_microcoder_kb::RUNS)?;
    bundle_microcoder(
        &reader,
        &mut shared_fact,
        &manifest,
        |a| a.passed,
        &mut bundles,
    )?;

    let reviews = read_reviews(&reader)?;
    // In publication order, never by score. The negative board and the
    // reference snapshots stand beside the wins.
    let mut boards = vec![delegate, tb21, dev, shared_fact, tb4_oos::build(&reader)?];
    for (id, rel) in reference_boards::SNAPSHOTS {
        boards.push(reference_boards::build(&reader, id, rel)?);
    }
    let leaderboard = leaderboard(boards)?;
    // Compact: a phone fetches it on entry, and the digest is over the
    // value, not its layout. `jq .` reads it.
    let mut leaderboard_bytes =
        serde_json::to_vec(&leaderboard).map_err(|e| fail!("serialize: {e}"))?;
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
        reviews,
    })
}

/// The path a board's attempt's bundle is published at.
#[must_use]
pub fn trace_path(board: &str, attempt: &str) -> String {
    format!("traces/{board}/{attempt}.json")
}

/// Serializes `bundle`, points the board's attempt at it, and adds it to
/// the publication.
fn attach(
    board: &mut Board,
    attempt: &str,
    bundle: &contract::TraceBundle,
    bundles: &mut Vec<(String, Vec<u8>)>,
) -> Result<()> {
    let mut bytes = serde_json::to_vec(bundle).map_err(|e| fail!("serialize: {e}"))?;
    bytes.push(b'\n');
    let path = trace_path(&board.id, attempt);
    let slot = board
        .attempts
        .iter_mut()
        .find(|a| a.id == attempt)
        .ok_or_else(|| fail!("{attempt}: not on the board"))?;
    slot.trace = Some(TraceRef {
        path: path.clone(),
        sha256: sha256_hex(&bytes),
        bytes: bytes.len() as u64,
    });
    bundles.push((path, bytes));
    Ok(())
}

/// Bundles the Microcoder run record behind each selected attempt. An
/// attempt's `trial` is its record directory, repository-relative.
fn bundle_microcoder(
    reader: &Reader,
    board: &mut Board,
    manifest: &microcoder::Manifest,
    select: impl Fn(&contract::Attempt) -> bool,
    bundles: &mut Vec<(String, Vec<u8>)>,
) -> Result<()> {
    let selected: Vec<contract::Attempt> = board
        .attempts
        .iter()
        .filter(|a| select(a))
        .cloned()
        .collect();
    for attempt in &selected {
        let run = attempt
            .trial
            .strip_prefix(&format!("{}/", manifest.dir))
            .ok_or_else(|| fail!("{}: its record isn't under {}", attempt.id, manifest.dir))?;
        let bar = board
            .tasks
            .iter()
            .find(|t| t.task == attempt.task)
            .map(|t| t.bar.clone())
            .ok_or_else(|| fail!("{}: no task row", attempt.id))?;
        let bundle = microcoder::build(
            reader,
            &microcoder::Input {
                board: &board.id,
                attempt,
                bar: &bar,
                manifest,
                run,
            },
        )?;
        attach(board, &attempt.id, &bundle, bundles)?;
    }
    Ok(())
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
    let mut used = vec![false; output.reviews.len()];
    for (rel, bytes) in &output.bundles {
        let Ok(bundle) = serde_json::from_slice::<contract::TraceBundle>(bytes) else {
            problems.push(format!("{rel} doesn't parse as a trace bundle"));
            continue;
        };
        for (rule, n) in &bundle.scrub.redactions {
            if !credential.contains(&rule.as_str()) {
                continue;
            }
            let review = output.reviews.iter().position(|r| {
                r.bundle == *rel
                    && r.rule == *rule
                    && r.matches == *n
                    && bundle
                        .sources
                        .iter()
                        .any(|s| s.path == r.source && s.sha256 == r.source_sha256)
            });
            match review {
                Some(i) => used[i] = true,
                None => problems.push(format!(
                    "{rel}: {n} match(es) of credential rule {rule}; inspect the retained trace before publishing, and record what you found in {REVIEWS}"
                )),
            }
        }
    }
    for (review, used) in output.reviews.iter().zip(used) {
        if !used {
            problems.push(format!(
                "{REVIEWS}: the review of {} ({}) matches no bundle's redactions and sources; remove or redo it",
                review.bundle, review.rule
            ));
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
