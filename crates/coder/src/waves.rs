//! The partition's account of itself: a report a caller reads, and the
//! ATIF records a run appends.
//!
//! [`Selection::waves`] decides which work may run beside which — a
//! bounded wave at a time — and what no wave could take. What it returns
//! is a decision, and a decision nobody can inspect is a ruling without
//! a record. A dropped task's reason is the evidence the decision stands
//! on, not noise to be summarized away.
//!
//! This module is that record, in the two shapes this repository keeps
//! records in:
//!
//! - [`report`] renders the partition as lines a caller reads: which wave
//!   admitted what, what each wave left out and why, what no wave could
//!   schedule, and where a wave refused. The lines are pure rendering —
//!   no clock, no filesystem, no I/O — so a terminal and a headless run
//!   show the same partition the same way, and the same `Waves` renders
//!   byte-identical twice.
//! - [`calls`] renders the same partition as the ATIF calls a run
//!   appends: one record per wave naming the admitted and the dropped
//!   with their reasons, then one for the partition itself, so the trace
//!   carries the scheduler's reasoning rather than only its outcome.
//!
//! A drop's reason travels in the source's own words both ways — "comes
//! after 9391" stays "comes after 9391" — because a paraphrase is a
//! second decision nobody made. What the record adds around the words is
//! only which kind of drop it was: `waits`, a place in line the next
//! wave asks again; `unscheduled`, a verdict no wave will revisit; or
//! `refused`, a drop the refusing wave never resolved.

use std::collections::BTreeSet;
use std::fmt;

use atif::{Call, Outcome};
use serde_json::{Map, Value, json};

use crate::runtime::SELECT_CALL;
use crate::source::{Dropped, Overflow, Selection, Waves};

/// The schema a wave-partition record carries in its `extra`: one
/// `task_select` call per wave — each wave is one lookup — and one
/// [`WAVES_CALL`] call for the partition itself.
pub const WAVES_SCHEMA: &str = "openagents.waves.v1";

/// The name the partition's own record carries: the call that says what
/// the waves came to, apart from what any one of them did.
pub const WAVES_CALL: &str = "task_waves";

/// The code a wave that overflowed records — the same one a refused
/// `query` step's lookup answers with.
pub const REFUSED_CODE: &str = "too_many_results";

/// The word a drop carries when it was a place in line: the next wave
/// asks the item again.
const WAITS: &str = "waits";

/// The word a drop carries when it was a verdict: no wave asks again.
const UNSCHEDULED: &str = "unscheduled";

/// The word a drop carries under a wave that refused: the refusal ended
/// the run before the drop's fate was decided.
const REFUSED: &str = "refused";

/// What a partition looks like to a caller: the rendered lines, in
/// order.
///
/// The report is pure data — building it touches nothing but the
/// `Waves` it was given, so the same partition renders the same lines
/// every time. Where the lines go — a terminal pane, a headless log — is
/// the caller's business rather than this type's.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Report {
    /// The lines of the report, in order.
    pub lines: Vec<String>,
}

impl Report {
    /// The report as one text block.
    #[must_use]
    pub fn text(&self) -> String {
        self.lines.join("\n")
    }
}

impl fmt::Display for Report {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.text())
    }
}

/// Renders one partition as the lines a caller reads.
///
/// Per wave: its index, the identifiers it admitted in order, every
/// item it dropped with the source's own reason and which kind of drop
/// it was, and — when the bound shaped the wave — its overflow. A wave
/// that refused names itself and its code rather than listing admitted
/// work, because a refused wave admits nothing. After the waves: every
/// item no wave could schedule, with the reason it was given.
#[must_use]
pub fn report(waves: &Waves) -> Report {
    let verdicts = verdicts(waves);
    let total = waves.waves.len();
    let mut lines = Vec::new();
    match waves.waves.first() {
        Some(first) => lines.push(format!(
            "{} from {}, order {}: {} over {}",
            first.source,
            first.resolved_from,
            first.order.word(),
            counted(first.found, "work item"),
            counted(total, "wave"),
        )),
        None => lines.push("no waves".to_string()),
    }
    for (index, wave) in waves.waves.iter().enumerate() {
        wave_lines(&mut lines, index + 1, total, wave, &verdicts);
    }
    if !waves.unscheduled.is_empty() {
        lines.push("unscheduled:".to_string());
        for dropped in &waves.unscheduled {
            lines.push(format!("  {} — {}", dropped.id, dropped.reason));
        }
    }
    Report { lines }
}

/// The ATIF records a run appends for one partition: one `task_select`
/// call per wave naming what it admitted and what it dropped with the
/// reason and the kind of drop, then one [`WAVES_CALL`] record for the
/// partition itself — the unscheduled list, and the refusal when a wave
/// refused.
///
/// The calls arrive with no `id` and no milliseconds: a recorder
/// numbers them as it writes them, and a lookup takes no wall time
/// worth recording.
#[must_use]
pub fn calls(waves: &Waves) -> Vec<Call> {
    let verdicts = verdicts(waves);
    let total = waves.waves.len();
    let mut calls = Vec::with_capacity(total + 1);
    for (index, wave) in waves.waves.iter().enumerate() {
        calls.push(wave_call(index + 1, total, wave, &verdicts));
    }
    calls.push(partition_call(waves));
    calls
}

/// The lines one wave of the partition contributes to the report.
fn wave_lines(
    lines: &mut Vec<String>,
    number: usize,
    total: usize,
    wave: &Selection,
    verdicts: &BTreeSet<(&str, &str)>,
) {
    if wave.overflow == Overflow::Refused {
        lines.push(format!(
            "wave {number} of {total}: refused ({REFUSED_CODE}), {} over the bound",
            counted(wave.work.len(), "work item"),
        ));
        if !wave.work.is_empty() {
            lines.push(format!("  over: {}", wave.selected().join(", ")));
        }
        for dropped in &wave.dropped {
            lines.push(format!(
                "  dropped {} — {} [{REFUSED}]",
                dropped.id, dropped.reason
            ));
        }
        return;
    }
    let admitted = match wave.selected().as_slice() {
        [] => "none".to_string(),
        ids => ids.join(", "),
    };
    lines.push(format!(
        "wave {number} of {total}: admitted {admitted} ({} of {})",
        wave.work.len(),
        wave.found,
    ));
    for dropped in &wave.dropped {
        lines.push(format!(
            "  dropped {} — {} [{}]",
            dropped.id,
            dropped.reason,
            fate(wave, dropped, verdicts),
        ));
    }
    for collision in &wave.collisions {
        lines.push(format!(
            "  collision {} — {}",
            collision.path,
            collision.work.join(", "),
        ));
    }
    if wave.overflow != Overflow::None {
        lines.push(format!("  overflow {}", wave.overflow.word()));
    }
}

/// One wave as the call a run appends: the lookup's own fields plus the
/// wave it was and what each of its drops came to.
fn wave_call(
    number: usize,
    total: usize,
    wave: &Selection,
    verdicts: &BTreeSet<(&str, &str)>,
) -> Call {
    let refused = wave.overflow == Overflow::Refused;
    let mut extra = Map::new();
    extra.insert("schema".to_string(), json!(WAVES_SCHEMA));
    extra.insert("wave".to_string(), json!(number));
    extra.insert("waves".to_string(), json!(total));
    extra.insert("source".to_string(), json!(wave.source));
    extra.insert("resolved_from".to_string(), json!(wave.resolved_from));
    extra.insert("order".to_string(), json!(wave.order.word()));
    extra.insert("found".to_string(), json!(wave.found));
    extra.insert("ordered".to_string(), json!(wave.ordered));
    match refused {
        // A refused wave admits nothing: `work` holds what it declined
        // to choose among, not what runs.
        true => {
            extra.insert("admitted".to_string(), json!([]));
            extra.insert("over_bound".to_string(), json!(wave.selected()));
            extra.insert("refusal".to_string(), json!({ "code": REFUSED_CODE }));
        }
        false => {
            extra.insert("admitted".to_string(), json!(wave.selected()));
        }
    }
    extra.insert(
        "dropped".to_string(),
        json!(
            wave.dropped
                .iter()
                .map(|dropped| json!({
                    "id": dropped.id,
                    "reason": dropped.reason,
                    "fate": fate(wave, dropped, verdicts),
                }))
                .collect::<Vec<_>>()
        ),
    );
    if !wave.collisions.is_empty() {
        extra.insert("collisions".to_string(), wave.collisions_value());
    }
    extra.insert("overflow".to_string(), json!(wave.overflow.word()));
    Call {
        id: String::new(),
        name: SELECT_CALL.to_string(),
        arguments: json!({
            "source": wave.source,
            "from": wave.resolved_from,
            "order": wave.order.word(),
            "wave": number,
            "waves": total,
        }),
        output: wave.output(),
        outcome: match refused {
            true => Outcome::Cancelled,
            false => Outcome::Completed,
        },
        milliseconds: 0,
        purpose: Some("Which of the wave's work may run beside itself.".to_string()),
        extra,
    }
}

/// The partition itself as one call: how many waves, what no wave
/// admitted and why, and which wave refused when one did — the record
/// that answers whether the partition ended cleanly.
fn partition_call(waves: &Waves) -> Call {
    let refused = waves
        .waves
        .iter()
        .position(|wave| wave.overflow == Overflow::Refused);
    let mut arguments = Map::new();
    let mut extra = Map::new();
    extra.insert("schema".to_string(), json!(WAVES_SCHEMA));
    if let Some(first) = waves.waves.first() {
        arguments.insert("source".to_string(), json!(first.source));
        arguments.insert("from".to_string(), json!(first.resolved_from));
        arguments.insert("order".to_string(), json!(first.order.word()));
        extra.insert("source".to_string(), json!(first.source));
        extra.insert("resolved_from".to_string(), json!(first.resolved_from));
        extra.insert("order".to_string(), json!(first.order.word()));
    }
    arguments.insert("waves".to_string(), json!(waves.waves.len()));
    extra.insert("waves".to_string(), json!(waves.waves.len()));
    extra.insert(
        "unscheduled".to_string(),
        json!(
            waves
                .unscheduled
                .iter()
                .map(|dropped| json!({
                    "id": dropped.id,
                    "reason": dropped.reason,
                }))
                .collect::<Vec<_>>()
        ),
    );
    if let Some(index) = refused {
        extra.insert(
            "refusal".to_string(),
            json!({ "wave": index + 1, "code": REFUSED_CODE }),
        );
    }
    Call {
        id: String::new(),
        name: WAVES_CALL.to_string(),
        arguments: Value::Object(arguments),
        output: format!(
            "{}, {} unscheduled",
            counted(waves.waves.len(), "wave"),
            waves.unscheduled.len(),
        ),
        outcome: match refused {
            Some(_) => Outcome::Cancelled,
            None => Outcome::Completed,
        },
        milliseconds: 0,
        purpose: Some("What the wave partition came to.".to_string()),
        extra,
    }
}

/// Which kind of drop one dropped item was.
///
/// A wave's verdict drop reaches [`Waves::unscheduled`] verbatim, so
/// pair membership is the partition's own record of which drops were
/// rulings. An ordering drop the next wave asks again never matches —
/// and one the bound or the stall strands later lands in `unscheduled`
/// under a different, drain-time reason, so the wave that dropped it
/// still reads as a place in line. Under a refused wave nothing was
/// resolved: the refusal is the fate every drop shares.
fn fate(wave: &Selection, dropped: &Dropped, verdicts: &BTreeSet<(&str, &str)>) -> &'static str {
    if wave.overflow == Overflow::Refused {
        REFUSED
    } else if verdicts.contains(&(dropped.id.as_str(), dropped.reason.as_str())) {
        UNSCHEDULED
    } else {
        WAITS
    }
}

/// The `(id, reason)` pairs no wave will ask again, borrowed so the
/// lookup allocates nothing.
fn verdicts(waves: &Waves) -> BTreeSet<(&str, &str)> {
    waves
        .unscheduled
        .iter()
        .map(|dropped| (dropped.id.as_str(), dropped.reason.as_str()))
        .collect()
}

/// "3 waves", "1 wave" — the count and the word agreeing, the way the
/// report and the trace both spell it.
fn counted(n: usize, word: &str) -> String {
    match n {
        1 => format!("1 {word}"),
        _ => format!("{n} {word}s"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::delegate::Task;
    use crate::source::{OnOverflow, Source, Work};

    fn work(id: &str, touches: &[&str], after: &[&str]) -> Work {
        Work {
            id: id.to_string(),
            touches: touches.iter().map(|path| (*path).to_string()).collect(),
            after: after.iter().map(|id| (*id).to_string()).collect(),
            blocked: None,
            task: Task::reading(&format!("do {id}"), touches.first().unwrap_or(&"a.rs")),
        }
    }

    /// A writing item, the shape a work list spells with `"writes": true`.
    fn writes(id: &str, touches: &[&str]) -> Work {
        let mut item = work(id, touches, &[]);
        item.task.writes = true;
        item
    }

    /// A work item the source itself marked as unable to run.
    fn blocked(id: &str, reason: &str) -> Work {
        let mut item = work(id, &[], &[]);
        item.blocked = Some(reason.to_string());
        item
    }

    fn partition(
        items: Vec<Work>,
        max_per_wave: usize,
        max_waves: usize,
        on_overflow: OnOverflow,
    ) -> Waves {
        Selection::waves(
            &Source::request(),
            items,
            max_per_wave,
            max_waves,
            on_overflow,
        )
    }

    /// Six read-only tasks over six files, the fan-out the first
    /// burndown runs.
    fn six() -> Vec<Work> {
        (1..=6)
            .map(|n| work(&format!("t{n}"), &[&format!("{n}.rs")], &[]))
            .collect()
    }

    #[test]
    fn zero_tasks_report_no_waves() {
        let waves = partition(Vec::new(), 6, 6, OnOverflow::Truncate);

        assert_eq!(report(&waves).lines, ["no waves"]);
        let calls = calls(&waves);
        assert_eq!(calls.len(), 1, "the partition itself still records");
        assert_eq!(calls[0].name, WAVES_CALL);
        assert_eq!(calls[0].extra["waves"], json!(0));
        assert_eq!(calls[0].extra["unscheduled"], json!([]));
        assert_eq!(calls[0].outcome, Outcome::Completed);
    }

    #[test]
    fn one_task_reports_one_wave() {
        let waves = partition(vec![work("a", &["a.rs"], &[])], 6, 6, OnOverflow::Truncate);
        let report = report(&waves);

        assert_eq!(
            report.lines,
            [
                "request from the request, order given: 1 work item over 1 wave",
                "wave 1 of 1: admitted a (1 of 1)",
            ]
        );
    }

    #[test]
    fn six_tasks_report_one_wave_of_six() {
        let waves = partition(six(), 12, 6, OnOverflow::Truncate);
        let report = report(&waves);

        assert_eq!(report.lines.len(), 2);
        assert_eq!(
            report.lines[1],
            "wave 1 of 1: admitted t1, t2, t3, t4, t5, t6 (6 of 6)"
        );
    }

    /// Twelve tasks against a wave bound of six partition into two full
    /// waves, and the first wave's truncation drops read as places in
    /// line rather than rulings.
    #[test]
    fn twelve_tasks_report_two_waves() {
        let items: Vec<Work> = (1..=12)
            .map(|n| work(&format!("t{n}"), &[&format!("{n}.rs")], &[]))
            .collect();
        let waves = partition(items, 6, 6, OnOverflow::Truncate);
        let report = report(&waves);

        assert_eq!(waves.waves.len(), 2);
        assert_eq!(
            report.lines[1],
            "wave 1 of 2: admitted t1, t2, t3, t4, t5, t6 (6 of 12)"
        );
        for (line, id) in report.lines[2..8]
            .iter()
            .zip(["t7", "t8", "t9", "t10", "t11", "t12"])
        {
            assert_eq!(
                *line,
                format!("  dropped {id} — over the step's max_results of 6 [{WAITS}]"),
                "the bound's drops are places in line, each naming it"
            );
        }
        assert_eq!(report.lines[8], "  overflow truncated");
        assert_eq!(
            report.lines[9],
            "wave 2 of 2: admitted t7, t8, t9, t10, t11, t12 (6 of 6)"
        );
    }

    /// A cycle waits on itself: neither side ever runs, the drops are
    /// verdicts, and each side's own reason names the other.
    #[test]
    fn a_dependency_cycle_is_unscheduled_with_the_cycle_named() {
        let waves = partition(
            vec![work("a", &["a.rs"], &["b"]), work("b", &["b.rs"], &["a"])],
            6,
            6,
            OnOverflow::Truncate,
        );
        let report = report(&waves);

        assert_eq!(report.lines[1], "wave 1 of 1: admitted none (0 of 2)");
        assert_eq!(
            report.lines[2],
            "  dropped a — comes after b, which this lookup also found [unscheduled]"
        );
        assert_eq!(
            report.lines[3],
            "  dropped b — comes after a, which this lookup also found [unscheduled]"
        );
        assert_eq!(report.lines[4], "unscheduled:");
        assert!(
            report.lines[5].contains("comes after b"),
            "{}",
            report.lines[5]
        );
        assert!(
            report.lines[6].contains("comes after a"),
            "{}",
            report.lines[6]
        );
    }

    /// Work that comes after an identifier no source answered with runs
    /// in the first wave: the dependency already landed somewhere, and
    /// the partition does not wait on work it cannot see.
    #[test]
    fn a_missing_dependency_schedules_in_the_first_wave() {
        let waves = partition(
            vec![work("a", &["a.rs"], &[]), work("b", &["b.rs"], &["9391"])],
            6,
            6,
            OnOverflow::Truncate,
        );
        let report = report(&waves);

        assert_eq!(waves.waves.len(), 1);
        assert_eq!(
            report.lines[1], "wave 1 of 1: admitted a, b (2 of 2)",
            "the missing identifier drops nothing"
        );
        assert_eq!(report.lines.len(), 2);
    }

    /// Items the source ruled out keep their own reasons: a stale
    /// revision is a verdict, recorded once and never asked again.
    #[test]
    fn items_behind_stale_revisions_report_their_reasons() {
        let waves = partition(
            vec![
                work("ok", &["a.rs"], &[]),
                blocked("stale-1", "revision moved: expected digest aa, found bb"),
                blocked("stale-2", "closed upstream since the snapshot"),
            ],
            6,
            6,
            OnOverflow::Truncate,
        );
        let report = report(&waves);

        assert_eq!(
            report.lines[2],
            "  dropped stale-1 — revision moved: expected digest aa, found bb [unscheduled]"
        );
        assert_eq!(
            report.lines[3],
            "  dropped stale-2 — closed upstream since the snapshot [unscheduled]"
        );
        assert_eq!(report.lines[4], "unscheduled:");
        assert_eq!(
            report.lines[5], "  stale-1 — revision moved: expected digest aa, found bb",
            "the verdict's own words, carried whole"
        );
    }

    /// A drop that was a place in line resolves in a later wave: the
    /// report shows it waiting where it fell and admitted where it ran —
    /// scheduled, not lost.
    #[test]
    fn a_resolved_drop_shows_as_scheduled_not_lost() {
        let waves = partition(
            vec![
                work("a", &["a.rs"], &[]),
                work("b", &["b.rs"], &["a"]),
                work("c", &["c.rs"], &["b"]),
            ],
            6,
            6,
            OnOverflow::Truncate,
        );
        let report = report(&waves);

        assert_eq!(
            report.lines[2],
            "  dropped b — comes after a, which this lookup also found [waits]"
        );
        assert_eq!(
            report.lines[3],
            "  dropped c — comes after b, which this lookup also found [waits]"
        );
        assert_eq!(report.lines[4], "wave 2 of 3: admitted b (1 of 2)");
        assert_eq!(
            report.lines[5], "  dropped c — comes after b, which this lookup also found [waits]",
            "b sat in this wave's own list, so c waits one more wave"
        );
        assert_eq!(report.lines[6], "wave 3 of 3: admitted c (1 of 1)");
    }

    /// A writer dropped behind a kept reader re-offers next wave and is
    /// admitted there: the conflict was ordering, not a verdict.
    #[test]
    fn a_write_conflict_resolves_in_the_next_wave() {
        let waves = partition(
            vec![
                work("read", &["src/lib.rs"], &[]),
                writes("edit", &["src/lib.rs"]),
            ],
            6,
            6,
            OnOverflow::Truncate,
        );
        let report = report(&waves);

        assert_eq!(report.lines[1], "wave 1 of 2: admitted read (1 of 2)");
        assert_eq!(
            report.lines[2],
            "  dropped edit — writes src/lib.rs, which read also touches [waits]"
        );
        assert_eq!(report.lines[3], "wave 2 of 2: admitted edit (1 of 1)");
    }

    /// Candidates still in line when `max_waves` runs out are
    /// unscheduled with the bound named — and the wave's own drop still
    /// reads as the place in line it was.
    #[test]
    fn the_wave_bound_reports_who_it_left_waiting() {
        let waves = partition(
            vec![
                work("a", &["a.rs"], &[]),
                work("b", &["b.rs"], &["a"]),
                work("c", &["c.rs"], &["b"]),
            ],
            6,
            2,
            OnOverflow::Truncate,
        );
        let report = report(&waves);

        assert_eq!(waves.waves.len(), 2);
        assert_eq!(
            report.lines[5],
            "  dropped c — comes after b, which this lookup also found [waits]"
        );
        assert_eq!(
            report.lines.last().expect("the bound leaves a line"),
            &"  c — over the run's max_waves of 2".to_string(),
            "the cap names itself in the item's own reason"
        );
    }

    /// A wave that overflows under `refuse` refuses the whole call, and
    /// the report names the refusing wave and its code rather than
    /// listing admitted work — a refused wave admits nothing.
    #[test]
    fn a_refusal_names_the_refusing_wave_and_its_code() {
        let waves = partition(
            vec![work("a", &["a.rs"], &[]), work("b", &["b.rs"], &[])],
            1,
            6,
            OnOverflow::Refuse,
        );
        let report = report(&waves);

        assert_eq!(
            report.lines[1],
            "wave 1 of 1: refused (too_many_results), 2 work items over the bound"
        );
        assert_eq!(report.lines[2], "  over: a, b");
        assert_eq!(report.lines.len(), 3, "nothing else claims to run");

        let calls = calls(&waves);
        assert_eq!(calls[0].outcome, Outcome::Cancelled);
        assert_eq!(calls[0].extra["refusal"]["code"], json!(REFUSED_CODE));
        assert_eq!(calls[0].extra["admitted"], json!([]));
        assert_eq!(calls[0].extra["over_bound"], json!(["a", "b"]));
        assert_eq!(calls[1].outcome, Outcome::Cancelled);
        assert_eq!(calls[1].extra["refusal"]["wave"], json!(1));
        assert_eq!(calls[1].extra["refusal"]["code"], json!(REFUSED_CODE));
    }

    /// The same partition renders the same report twice, byte for byte:
    /// a report that moved between readings could not be compared
    /// across runs.
    #[test]
    fn the_report_is_deterministic() {
        let build = || {
            partition(
                vec![
                    work("a", &["a.rs"], &[]),
                    work("b", &["b.rs"], &["a"]),
                    writes("c", &["c.rs"]),
                    writes("d", &["c.rs"]),
                    blocked("held", "blocked by #9999, which is still open"),
                ],
                6,
                6,
                OnOverflow::Truncate,
            )
        };

        assert_eq!(report(&build()).text(), report(&build()).text());
        let first = calls(&build());
        let second = calls(&build());
        assert_eq!(first.len(), second.len());
        for (one, two) in first.iter().zip(second.iter()) {
            assert_eq!(
                json!(one).to_string(),
                json!(two).to_string(),
                "the trace record is byte-identical too"
            );
        }
    }

    /// Each wave records as a `task_select` call — a wave is one lookup —
    /// carrying the admitted identifiers, every drop's own reason and
    /// what came of it, and the partition record carrying the
    /// unscheduled list whole.
    #[test]
    fn the_atif_records_carry_the_reasoning() {
        let waves = partition(
            vec![
                work("a", &["a.rs"], &[]),
                work("b", &["b.rs"], &["a"]),
                blocked("held", "blocked by #9999, which is still open"),
            ],
            6,
            6,
            OnOverflow::Truncate,
        );
        let calls = calls(&waves);

        assert_eq!(calls.len(), 3, "two waves and the partition record");
        assert_eq!(calls[0].name, SELECT_CALL);
        assert_eq!(calls[0].extra["schema"], json!(WAVES_SCHEMA));
        assert_eq!(calls[0].extra["wave"], json!(1));
        assert_eq!(calls[0].extra["waves"], json!(2));
        assert_eq!(calls[0].extra["admitted"], json!(["a"]));
        assert_eq!(calls[0].extra["dropped"][0]["id"], json!("b"));
        assert_eq!(
            calls[0].extra["dropped"][0]["reason"],
            json!("comes after a, which this lookup also found")
        );
        assert_eq!(calls[0].extra["dropped"][0]["fate"], json!(WAITS));
        assert_eq!(calls[0].extra["dropped"][1]["id"], json!("held"));
        assert_eq!(
            calls[0].extra["dropped"][1]["reason"],
            json!("blocked by #9999, which is still open")
        );
        assert_eq!(calls[0].extra["dropped"][1]["fate"], json!(UNSCHEDULED));
        assert_eq!(calls[1].extra["admitted"], json!(["b"]));

        let partition = &calls[2];
        assert_eq!(partition.name, WAVES_CALL);
        assert_eq!(partition.extra["waves"], json!(2));
        assert_eq!(
            partition.extra["unscheduled"],
            json!([{ "id": "held", "reason": "blocked by #9999, which is still open" }])
        );
        assert!(partition.extra.get("refusal").is_none());
    }

    /// A wave that admits nothing still records: its drops say why, and
    /// the stall's drain reason lands in the partition record rather
    /// than rewriting the wave's.
    #[test]
    fn a_stalled_partition_keeps_both_reasons() {
        let waves = partition(
            vec![work("a", &["a.rs"], &[]), work("b", &["b.rs"], &[])],
            0,
            6,
            OnOverflow::Truncate,
        );
        let report = report(&waves);

        assert_eq!(report.lines[1], "wave 1 of 1: admitted none (0 of 2)");
        assert_eq!(
            report.lines[2], "  dropped a — over the step's max_results of 0 [waits]",
            "at the wave it was a place in line"
        );
        assert_eq!(
            report.lines.last().expect("the stall leaves a line"),
            &"  b — a wave admitted nothing, and nothing behind it can run".to_string(),
        );
    }
}
