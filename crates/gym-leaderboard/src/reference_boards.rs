//! Reference boards: dated snapshots of Harbor Hub's public Terminal-Bench
//! leaderboards, committed as `bench/terminal-bench/reference/*.json`.
//!
//! A reference board isn't a subject's board. Its rows are other agents'
//! published results under their own conditions, one [`ReferenceRow`] per
//! agent, model, and effort, copied as published, with the snapshot's two
//! reconciliation flags per row: whether the row's per-task counts add up
//! to its totals, and whether its per-task costs add up to its cost. It
//! has no attempts, beats, or tasks, and no row is ever merged into a
//! subject board.

use crate::contract::{
    Benchmark, Board, BoardKind, Caveat, CostBasis, Provenance, Reference, ReferenceRow, Snapshot,
    Spend, Subject, Tally,
};
use crate::evidence::{Reader, Result, array, at, fail, opt_number, opt_string, opt_u64, string};

/// The snapshots and their boards' identities, in publication order.
pub const SNAPSHOTS: [(&str, &str); 2] = [
    (
        "reference-harbor-hub-tb4",
        "bench/terminal-bench/reference/tb4-leaderboard.json",
    ),
    (
        "reference-harbor-hub-tb21",
        "bench/terminal-bench/reference/tb21-leaderboard.json",
    ),
];

const SCHEMA: &str = "openagents.tbench.reference.v1";

/// Builds one reference board from its snapshot file.
pub fn build(reader: &Reader, id: &str, rel: &str) -> Result<Board> {
    let (doc, file) = reader.json(rel)?;
    if at(&doc, "schema").as_str() != Some(SCHEMA) {
        return Err(fail!("{rel}: not a {SCHEMA} snapshot"));
    }
    let benchmark = string(&doc, "benchmark")?;
    let version = benchmark
        .rsplit(' ')
        .next()
        .ok_or_else(|| fail!("{rel}: no benchmark version"))?
        .to_owned();
    let url = string(&doc, "leaderboard")?;
    let host = url
        .split("://")
        .nth(1)
        .and_then(|rest| rest.split('/').next())
        .ok_or_else(|| fail!("{rel}: can't read the host of {url}"))?
        .to_owned();
    let fetched_at = string(&doc, "fetched_at")?;
    let method = string(&doc, "source")?;

    let mut rows = Vec::new();
    for entry in array(&doc, "entries")? {
        let trials = opt_u64(entry, "metrics/n_trials").map(|n| n as u32);
        let passes = opt_u64(entry, "metrics/successes").map(|n| n as u32);
        if let (Some(p), Some(t)) = (passes, trials)
            && p > t
        {
            return Err(fail!("{rel}: a row passes {p} of {t} trials"));
        }
        rows.push(ReferenceRow {
            rank: opt_u64(entry, "rank")
                .map(|r| r as u32)
                .ok_or_else(|| fail!("{rel}: a row without a rank"))?,
            agent: string(entry, "agent")?,
            agent_version: opt_string(entry, "agent_version"),
            model: string(entry, "model")?,
            effort: opt_string(entry, "reasoning_effort"),
            date: opt_string(entry, "date"),
            passes,
            trials,
            accuracy_pct: opt_number(entry, "metrics/accuracy"),
            cost_usd: opt_number(entry, "metrics/total_cost_usd"),
            mean_trial_seconds: opt_number(entry, "metrics/avg_trial_duration_sec"),
            per_task_consistent: at(entry, "per_task/consistent").as_bool(),
            per_task_cost_consistent: at(entry, "per_task/cost_consistent").as_bool(),
        });
    }
    if rows.is_empty() {
        return Err(fail!("{rel}: no rows"));
    }
    let date = fetched_at
        .split('T')
        .next()
        .unwrap_or(&fetched_at)
        .to_owned();

    let name = |r: &ReferenceRow| {
        format!(
            "{} {}{}",
            r.agent,
            r.model,
            r.effort
                .as_deref()
                .map_or_else(String::new, |e| format!(" {e}"))
        )
    };
    let flagged = |f: &dyn Fn(&ReferenceRow) -> Option<bool>| -> Vec<String> {
        rows.iter()
            .filter(|r| f(r) == Some(false))
            .map(name)
            .collect()
    };
    let inconsistent = flagged(&|r| r.per_task_consistent);
    let cost_inconsistent = flagged(&|r| r.per_task_cost_consistent);
    let no_passes = rows.iter().filter(|r| r.passes.is_none()).count();
    let list = |names: &[String]| {
        if names.len() <= 3 {
            names.join(", ")
        } else {
            format!("{}, and {} more", names[..3].join(", "), names.len() - 3)
        }
    };
    let mut caveats = vec![
        Caveat {
            code: "snapshot".into(),
            text: format!(
                "A dated snapshot, not live: fetched from {host} at {fetched_at} ({url}). The live leaderboard may have changed since."
            ),
        },
        Caveat {
            code: "reconciliation".into(),
            text: format!(
                "Per row, the snapshot checked whether the per-task counts add up to the row's totals and whether the per-task costs add up to its cost. Counts don't reconcile on {} of {} rows{}; costs don't on {}{}.",
                inconsistent.len(),
                rows.len(),
                if inconsistent.is_empty() {
                    String::new()
                } else {
                    format!(" ({})", list(&inconsistent))
                },
                cost_inconsistent.len(),
                if cost_inconsistent.is_empty() {
                    String::new()
                } else {
                    format!(" ({})", list(&cost_inconsistent))
                },
            ),
        },
    ];
    if no_passes > 0 {
        caveats.push(Caveat {
            code: "passes_unpublished".into(),
            text: format!(
                "The leaderboard publishes no pass count for {no_passes} of {} rows; their passes are unknown, not zero, and only the published accuracy is shown.",
                rows.len()
            ),
        });
    }
    caveats.push(Caveat {
        code: "not_a_subject_board".into(),
        text: "These are other agents' published results under their own conditions and cost bases. No row is merged into, ranked against, or pooled with a board of OpenAgents' own runs.".into(),
    });

    Ok(Board {
        id: id.to_owned(),
        title: format!("{host} {benchmark} leaderboard, snapshot of {date}"),
        benchmark: Benchmark {
            name: benchmark
                .strip_suffix(&format!(" {version}"))
                .unwrap_or(&benchmark)
                .to_owned(),
            version,
        },
        kind: BoardKind::Reference,
        question: format!(
            "What did {host}'s public {benchmark} leaderboard show when it was read?"
        ),
        headline: format!(
            "A dated snapshot of {host}'s {benchmark} leaderboard, fetched {date}: {} rows, not live.",
            rows.len()
        ),
        provenance: Provenance {
            issues: vec![9844],
            report: rel.to_owned(),
            frozen_commit: None,
            evidence: vec![file],
        },
        subject: Subject {
            agent: "each row's own".into(),
            arm: "none".into(),
            model: "each row's own".into(),
            effort: None,
            artifact: None,
        },
        reference: Reference {
            name: host.clone(),
            rule: "No bar: a reference board lists published rows and has no beats.".into(),
            conditions: method.clone(),
        },
        labels: vec![crate::contract::Label::ReferenceOtherConditions],
        caveats,
        totals: Tally::default(),
        splits: Vec::new(),
        // OpenAgents ran nothing for a reference board.
        spend: Spend {
            basis: CostBasis::Published,
            reported_usd: 0.0,
            estimated_lower_bound_usd: None,
            estimated_upper_bound_usd: None,
        },
        tasks: Vec::new(),
        attempts: Vec::new(),
        reference_rows: rows,
        snapshot: Some(Snapshot {
            host,
            url,
            fetched_at,
            method,
        }),
    })
}
