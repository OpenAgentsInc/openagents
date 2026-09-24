//! A run's analysis as Markdown, in the shape of the hand-written
//! analyses in `docs/terminal-bench/`.
//!
//! [`render`] writes the document `gym runs analyze` prints and `--write`
//! keeps as `analysis.md`. [`pane_lines`] wraps any Markdown for the Runs
//! pane, which draws it under `A`.

use std::fmt::Write as _;

use crate::runs::date;
use crate::runs_analysis::{Analysis, long, offset, span, usd};

fn percent(part: f64, whole: f64) -> String {
    if whole <= 0.0 {
        "—".to_owned()
    } else {
        format!("{:.1}%", 100.0 * part / whole)
    }
}

fn cell(text: &str) -> String {
    text.replace('|', "\\|").replace('\n', " ")
}

fn yes_no(value: Option<bool>) -> &'static str {
    match value {
        Some(true) => "Passed",
        Some(false) => "Failed",
        None => "—",
    }
}

/// `15:24:10.697` in UTC.
fn clock(ms: i64) -> String {
    let of_day = ms.rem_euclid(86_400_000);
    format!(
        "{:02}:{:02}:{:02}.{:03}",
        of_day / 3_600_000,
        (of_day / 60_000) % 60,
        (of_day / 1000) % 60,
        of_day % 1000
    )
}

/// The analysis as a Markdown document.
#[must_use]
#[allow(clippy::too_many_lines)]
pub fn render(analysis: &Analysis) -> String {
    let mut out = String::new();
    let run = &analysis.run;
    let _ = writeln!(out, "# Run analysis: `{}`, {}\n", run.task, run.agent);
    let _ = writeln!(
        out,
        "Written by `gym runs analyze` ({}). Code computed every number; Jev judged only the acceptance-test and verifier-test pairs the rules left open.\n",
        analysis.version
    );
    let _ = writeln!(out, "- Job: `{}`", run.job);
    let _ = writeln!(out, "- Trial: `{}`", run.trial);
    if let Some(artifact) = &run.artifact {
        let policy = run
            .policy
            .as_ref()
            .map(|policy| {
                format!(
                    ", policy `{policy}`{}",
                    run.policy_digest
                        .as_ref()
                        .map(|d| format!(" (hash `{}`)", &d[..d.len().min(12)]))
                        .unwrap_or_default()
                )
            })
            .unwrap_or_default();
        let _ = writeln!(out, "- Artifact: `{artifact}`{policy}");
    }
    let _ = writeln!(
        out,
        "- Read it with `gym runs show {} --transcript`.\n",
        run.job
    );
    if let Some(origin) = run.origin_ms {
        let _ = writeln!(
            out,
            "Times are offsets from the episode's start, {} UTC on {}, as `minutes:seconds`.\n",
            clock(origin),
            date(origin)
                .split(' ')
                .take(2)
                .collect::<Vec<_>>()
                .join(" ")
        );
    }

    summary(&mut out, analysis);
    outcome(&mut out, analysis);
    timeline(&mut out, analysis);
    suite(&mut out, analysis);
    reversals(&mut out, analysis);
    anomalies(&mut out, analysis);
    fable(&mut out, analysis);

    let jev = &analysis.jev;
    if analysis.suite.is_none() && jev.asked == 0 && jev.cached == 0 {
        return out;
    }
    let _ = writeln!(out, "## Jev\n");
    let _ = writeln!(
        out,
        "Jev ({}) judged the acceptance-test and verifier-test pairs the rules left open: {} requests sent, {} answers reused from the cache, {} failed, {} for {} input tokens.",
        jev.mode,
        jev.asked,
        jev.cached,
        jev.failed,
        usd(jev.cost_usd),
        jev.input_tokens
    );
    for error in &jev.errors {
        let _ = writeln!(out, "\n- {error}");
    }
    out
}

fn summary(out: &mut String, analysis: &Analysis) {
    let _ = writeln!(out, "## Summary\n");
    let verdict = &analysis.verdict;
    let failing: Vec<String> = verdict
        .tests
        .iter()
        .filter(|t| !t.passed())
        .map(|t| format!("`{}`", t.name))
        .collect();
    let reward = verdict
        .reward
        .map(|r| format!(", reward {r}"))
        .unwrap_or_default();
    let outcome = match verdict.outcome.as_str() {
        "passed" => format!(
            "**Passed.** The verifier passed {} of {} tests{reward}.",
            verdict.passed, verdict.total
        ),
        "failed" => format!(
            "**Failed.** The verifier passed {} of {} tests{reward}{}.",
            verdict.passed,
            verdict.total,
            if failing.is_empty() {
                String::new()
            } else {
                format!("; failing {}", failing.join(", "))
            }
        ),
        other => format!("**{}.**", capitalize(other)),
    };
    let _ = writeln!(out, "- {outcome}");
    if let Some(agent) = analysis.agent_ms {
        let _ = writeln!(
            out,
            "- Agent time {}{}.",
            long(agent),
            analysis
                .trial_ms
                .map(|t| format!(", trial wall time {}", long(t)))
                .unwrap_or_default()
        );
    }
    let cost = &analysis.cost;
    let mut line = format!("- True cost {}", usd(cost.total_usd));
    let mut parts = Vec::new();
    if cost.luna_usd > 0.0 {
        parts.push(format!(
            "Luna {} over {} sessions and {} turns",
            usd(cost.luna_usd),
            cost.sessions.len(),
            cost.luna_turns
        ));
    }
    if cost.jev_requests > 0 {
        parts.push(format!(
            "Jev {} over {} requests",
            usd(cost.jev_usd),
            cost.jev_requests
        ));
    }
    if cost.other_usd > 0.0 {
        parts.push(format!("other {}", usd(cost.other_usd)));
    }
    if parts.is_empty() {
        line = format!("- Cost {}, as Harbor reports it", usd(cost.total_usd));
    } else {
        let _ = write!(line, ": {}", parts.join(", "));
    }
    line.push('.');
    if let Some(harbor) = cost.harbor_usd
        && !parts.is_empty()
    {
        match &cost.harbor_missing {
            Some(missing) => {
                let _ = write!(
                    line,
                    " Harbor reports {}: it leaves out {missing}.",
                    usd(harbor)
                );
            }
            None => {
                let _ = write!(line, " Harbor reports {}, which agrees.", usd(harbor));
            }
        }
    }
    let _ = writeln!(out, "{line}");
    if let Some(fable) = &analysis.fable {
        let mut bits = Vec::new();
        if let Some(cheapest) = &fable.cheapest_pass {
            bits.push(format!(
                "{} of the cheapest passing Fable 5.1 attempt ({})",
                percent(cost.total_usd, cheapest.cost_usd),
                usd(cheapest.cost_usd)
            ));
        }
        if let Some(tier) = &fable.tier {
            bits.push(format!(
                "{} of Fable {}'s mean ({})",
                percent(cost.total_usd, tier.mean_usd),
                tier.effort,
                usd(tier.mean_usd)
            ));
        }
        let time = match (&fable.tier, analysis.agent_ms) {
            (Some(tier), Some(agent)) if tier.mean_sec > 0.0 => format!(
                "; its agent time is {:.1} times Fable {}'s mean of {:.1} min",
                agent as f64 / 1000.0 / tier.mean_sec,
                tier.effort,
                tier.mean_sec / 60.0
            ),
            _ => String::new(),
        };
        if !bits.is_empty() {
            let _ = writeln!(out, "- The cost is {}{time}.", bits.join(" and "));
        }
    }
    if let Some(suite) = &analysis.suite
        && !suite.verifier.is_empty()
    {
        let covered = suite.verifier.iter().filter(|m| m.covered == "yes").count();
        let partly = suite
            .verifier
            .iter()
            .filter(|m| m.covered == "partly")
            .count();
        let _ = writeln!(
            out,
            "- The acceptance suite{} has {} tests; they check {covered} of the verifier's {} tests, {partly} partly, leave {} uncovered{}.",
            suite
                .status
                .as_ref()
                .map(|s| format!(", frozen `{s}`,"))
                .unwrap_or_default(),
            suite.tests.len(),
            suite.verifier.len(),
            suite.uncovered.len(),
            if suite.contradicted.is_empty() {
                String::new()
            } else {
                format!(", and contradict {}", suite.contradicted.len())
            }
        );
    }
    for reversal in analysis.reversals.iter().filter(|r| r.kind == "revert") {
        let _ = writeln!(
            out,
            "- **Reversal.** `{}` undid `{}`'s change to `{}` at {}{}{}.",
            reversal.later,
            reversal.earlier,
            reversal.file,
            offset(reversal.at_ms),
            if reversal.guards.is_empty() {
                String::new()
            } else {
                format!(", driven by the guard `{}`", reversal.guards.join("`, `"))
            },
            reversal
                .restored_by
                .as_ref()
                .map(|r| format!("; `{r}` restored it"))
                .unwrap_or_default()
        );
        if let Some(why) = &reversal.better_then_reverted {
            let _ = writeln!(out, "  It reached a better state, then reverted: {why}.");
        }
    }
    let serious: Vec<&str> = analysis
        .anomalies
        .iter()
        .filter(|a| a.kind != "reversal")
        .map(|a| a.kind.as_str())
        .collect();
    if !serious.is_empty() {
        let mut kinds: Vec<&str> = Vec::new();
        for kind in &serious {
            if !kinds.contains(kind) {
                kinds.push(kind);
            }
        }
        let _ = writeln!(out, "- {} anomalies: {}.", serious.len(), kinds.join(", "));
    }
    out.push('\n');
}

fn capitalize(text: &str) -> String {
    let mut chars = text.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}

fn outcome(out: &mut String, analysis: &Analysis) {
    let _ = writeln!(out, "## Outcome\n");
    let verdict = &analysis.verdict;
    let _ = writeln!(out, "### The verifier\n");
    if verdict.tests.is_empty() {
        let _ = writeln!(
            out,
            "The trial kept no verifier test results. Outcome: {}.\n",
            verdict.outcome
        );
    } else {
        let _ = writeln!(
            out,
            "{} collected, {} passed, {} failed{}.\n",
            verdict.total,
            verdict.passed,
            verdict.failed,
            verdict
                .seconds
                .map(|s| format!(", in {s:.2} seconds"))
                .unwrap_or_default()
        );
        let _ = writeln!(out, "| Verifier test | Result |\n| --- | --- |");
        for test in &verdict.tests {
            let result = if test.passed() {
                "Passed".to_owned()
            } else {
                format!("**{}**", capitalize(&test.status))
            };
            let _ = writeln!(out, "| `{}` | {result} |", test.name);
        }
        out.push('\n');
        for test in verdict.tests.iter().filter(|t| !t.passed()) {
            let _ = writeln!(
                out,
                "**`{}`**{}{}\n",
                test.name,
                test.doc
                    .as_ref()
                    .map(|d| format!(": {d}"))
                    .unwrap_or_default(),
                test.message
                    .as_ref()
                    .map(|m| format!(" The verifier printed `{}`.", cell(m)))
                    .unwrap_or_default()
            );
            if !test.assertions.is_empty() {
                let _ = writeln!(out, "```python");
                for assertion in test.assertions.iter().take(6) {
                    let _ = writeln!(out, "{assertion}");
                }
                let _ = writeln!(out, "```\n");
            }
        }
    }
    if !analysis.spans.is_empty() {
        let _ = writeln!(out, "### Time\n");
        let _ = writeln!(
            out,
            "| Span | Start (UTC) | Duration |\n| --- | --- | ---: |"
        );
        for span_ in &analysis.spans {
            let _ = writeln!(
                out,
                "| {} | {} | {} |",
                span_.span,
                clock(span_.start_ms),
                long_or_short(span_.duration_ms)
            );
        }
        out.push('\n');
    }
    let cost = &analysis.cost;
    let _ = writeln!(out, "### Cost\n");
    if cost.sessions.is_empty() && cost.jev.is_empty() {
        let _ = writeln!(
            out,
            "No per-session records: the cost is {}.\n",
            usd(cost.total_usd)
        );
        return;
    }
    let _ = writeln!(
        out,
        "Luna is priced from each request's `microluna.usage.v1` list-price estimate, and Jev from each request's reported input tokens.\n"
    );
    let _ = writeln!(out, "| Item | Requests | Cost |\n| --- | ---: | ---: |");
    for session in &cost.sessions {
        let _ = writeln!(
            out,
            "| {} `{}` | {} turns | {} |",
            capitalize(&session.role),
            session.session,
            session.turns,
            usd(session.cost_usd)
        );
    }
    if !cost.sessions.is_empty() {
        let _ = writeln!(
            out,
            "| **Luna** | **{} turns** | **{}** |",
            cost.luna_turns,
            usd(cost.luna_usd)
        );
    }
    for group in &cost.jev {
        let _ = writeln!(
            out,
            "| Jev `{}` | {} | {} |",
            group.name,
            group.requests,
            usd(group.cost_usd)
        );
    }
    if cost.jev_requests > 0 {
        let _ = writeln!(
            out,
            "| **Jev** | **{}** | **{}** |",
            cost.jev_requests,
            usd(cost.jev_usd)
        );
    }
    if cost.other_usd > 0.0 {
        let _ = writeln!(
            out,
            "| Other executors and generation | | {} |",
            usd(cost.other_usd)
        );
    }
    let _ = writeln!(out, "| **Total** | | **{}** |", usd(cost.total_usd));
    if let Some(harbor) = cost.harbor_usd {
        let _ = writeln!(out, "| Harbor's `result.json` | | {} |", usd(harbor));
    }
    out.push('\n');
    let luna_input: u64 = cost.sessions.iter().map(|s| s.input_tokens).sum();
    let luna_cached: u64 = cost.sessions.iter().map(|s| s.cached_tokens).sum();
    let luna_output: u64 = cost.sessions.iter().map(|s| s.output_tokens).sum();
    let luna_reasoning: u64 = cost.sessions.iter().map(|s| s.reasoning_tokens).sum();
    if luna_input > 0 {
        let _ = writeln!(
            out,
            "Luna read {} input tokens, {} of them cached ({}), and wrote {}, {} of them reasoning.\n",
            thousands(luna_input),
            thousands(luna_cached),
            percent(luna_cached as f64, luna_input as f64),
            thousands(luna_output),
            thousands(luna_reasoning)
        );
    }
}

/// `1,005,824`.
#[must_use]
pub fn thousands(n: u64) -> String {
    let digits = n.to_string();
    let mut out = String::new();
    for (index, digit) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index).is_multiple_of(3) {
            out.push(',');
        }
        out.push(digit);
    }
    out
}

fn long_or_short(ms: u64) -> String {
    if ms >= 60_000 {
        long(ms)
    } else {
        format!("{:.1} s", ms as f64 / 1000.0)
    }
}

fn timeline(out: &mut String, analysis: &Analysis) {
    let Some(timeline) = &analysis.timeline else {
        return;
    };
    let _ = writeln!(out, "## Timeline\n");
    let _ = writeln!(out, "### Per phase\n");
    let _ = writeln!(
        out,
        "| Start | Duration | Phase | What happened |\n| --- | ---: | --- | --- |"
    );
    for row in &timeline.phases {
        let _ = writeln!(
            out,
            "| {} | {} | {}{} | {} |",
            offset(row.start_ms),
            span(row.duration_ms),
            "› ".repeat(row.depth),
            cell(&row.phase),
            cell(&row.what)
        );
    }
    out.push('\n');
    if !timeline.sessions.is_empty() {
        let _ = writeln!(out, "### Per session\n");
        let _ = writeln!(
            out,
            "| Session | Role | Start | Span | Turns | Model | Tools | Edits | Cost | Ending |\n| --- | --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | --- |"
        );
        for session in &timeline.sessions {
            let _ = writeln!(
                out,
                "| `{}` | {} | {} | {} | {} | {:.1} s | {:.1} s | {}{} | {} | {} |",
                session.session,
                session.role,
                offset(session.start_ms),
                span(session.duration_ms),
                session.turns,
                session.model_ms as f64 / 1000.0,
                session.tool_ms as f64 / 1000.0,
                session.patches,
                if session.failed_patches > 0 {
                    format!(" ({} failed)", session.failed_patches)
                } else {
                    String::new()
                },
                usd(session.cost_usd),
                session.ending
            );
        }
        out.push('\n');
        let _ = writeln!(
            out,
            "Each pair is one turn: model latency in seconds, then tool seconds.\n\n```text"
        );
        let width = timeline
            .sessions
            .iter()
            .map(|s| s.session.len())
            .max()
            .unwrap_or(0);
        for session in &timeline.sessions {
            let _ = writeln!(out, "{:<width$}  {}", session.session, session.pairs);
        }
        let _ = writeln!(out, "```\n");
        for session in timeline.sessions.iter().filter(|s| !s.summary.is_empty()) {
            let _ = writeln!(
                out,
                "- `{}`{}: {}",
                session.session,
                if session.directive.is_empty() {
                    String::new()
                } else {
                    format!(" ({})", cell(&session.directive))
                },
                cell(&session.summary)
            );
        }
        out.push('\n');
    }
    let _ = writeln!(out, "### The critical path and overlapping work\n");
    let total: u64 = timeline.critical_path.iter().map(|s| s.duration_ms).sum();
    let _ = writeln!(
        out,
        "The critical path walks back from the end, taking at each point the activity that finished last. It covers {}.\n",
        span(total)
    );
    let _ = writeln!(
        out,
        "| Start | Duration | Share | Segment |\n| --- | ---: | ---: | --- |"
    );
    for segment in &timeline.critical_path {
        let _ = writeln!(
            out,
            "| {} | {} | {} | {} |",
            offset(segment.start_ms),
            span(segment.duration_ms),
            percent(segment.duration_ms as f64, total as f64),
            cell(&segment.label)
        );
    }
    out.push('\n');
    let _ = writeln!(
        out,
        "| Category | Seconds | Share of {:.1} s |\n| --- | ---: | ---: |",
        total as f64 / 1000.0
    );
    for (category, ms) in &timeline.categories {
        let _ = writeln!(
            out,
            "| {} | {:.1} | {} |",
            capitalize(category),
            *ms as f64 / 1000.0,
            percent(*ms as f64, total as f64)
        );
    }
    out.push('\n');
    let _ = writeln!(
        out,
        "Luna sessions ran {:.1} s in {:.1} s of episode: {:.2} sessions at once on average, and at most {}.{}{}\n",
        timeline.session_ms as f64 / 1000.0,
        timeline.episode_ms as f64 / 1000.0,
        timeline.concurrency,
        timeline.peak_sessions,
        timeline
            .serial_tail_ms
            .map(|tail| format!(
                " After the last overlap, {} ({}) ran one thing at a time.",
                span(tail),
                percent(tail as f64, timeline.episode_ms as f64)
            ))
            .unwrap_or_default(),
        timeline
            .first_edit_ms
            .map(|at| format!(" The first workspace edit came at {}.", offset(at)))
            .unwrap_or_default()
    );
    if !timeline.cost_by_phase.is_empty() {
        let _ = writeln!(out, "### Cost by phase\n");
        let _ = writeln!(
            out,
            "| Phase | Luna | Jev | Total |\n| --- | ---: | ---: | ---: |"
        );
        let (mut luna, mut jev) = (0.0, 0.0);
        for phase in &timeline.cost_by_phase {
            luna += phase.luna_usd;
            jev += phase.jev_usd;
            let _ = writeln!(
                out,
                "| {} | {} | {} | {} |",
                cell(&capitalize(&phase.phase)),
                if phase.luna_usd > 0.0 {
                    usd(phase.luna_usd)
                } else {
                    String::new()
                },
                if phase.jev_requests > 0 {
                    format!(
                        "{} ({} request{})",
                        usd(phase.jev_usd),
                        phase.jev_requests,
                        if phase.jev_requests == 1 { "" } else { "s" }
                    )
                } else {
                    String::new()
                },
                usd(phase.luna_usd + phase.jev_usd)
            );
        }
        let _ = writeln!(
            out,
            "| **Total** | **{}** | **{}** | **{}** |\n",
            usd(luna),
            usd(jev),
            usd(luna + jev)
        );
    }
}

fn suite(out: &mut String, analysis: &Analysis) {
    let Some(suite) = &analysis.suite else {
        return;
    };
    let _ = writeln!(out, "## Acceptance tests compared with the verifier\n");
    let _ = writeln!(
        out,
        "The frozen acceptance suite{}{} has {} tests. {} of them are guards: tests that already pass on the untouched workspace.\n",
        suite
            .status
            .as_ref()
            .map(|s| format!(" is `{s}`,"))
            .unwrap_or_default(),
        suite
            .digest
            .as_ref()
            .map(|d| format!(" hash `{}`,", &d[..d.len().min(12)]))
            .unwrap_or_default(),
        suite.tests.len(),
        suite.tests.iter().filter(|t| t.guard).count()
    );
    let _ = writeln!(out, "### Every acceptance test\n");
    let _ = writeln!(
        out,
        "| Test | Requirements | What it asserts | At the start | Last run |\n| --- | --- | --- | --- | --- |"
    );
    for test in &suite.tests {
        let _ = writeln!(
            out,
            "| `{}` | {} | {} | {}{} | {}{} |",
            test.id,
            test.requirements.join(", "),
            cell(&test.what),
            yes_no(test.start),
            if test.guard { " (guard)" } else { "" },
            yes_no(test.last),
            test.last_run
                .as_ref()
                .map(|l| format!(", `{l}`"))
                .unwrap_or_default()
        );
    }
    out.push('\n');
    if suite.verifier.is_empty() {
        return;
    }
    let _ = writeln!(out, "### Which acceptance tests check each verifier test\n");
    let _ = writeln!(
        out,
        "Rules pick candidates by the functions and words two tests share. Jev judged the ambiguous pairs: `checks` is its probability that the acceptance test checks what the verifier test checks, and `contradicts` that it requires behavior the verifier test forbids.\n"
    );
    let _ = writeln!(
        out,
        "| Verifier test | Candidates | Covered | Decided by | Verifier |\n| --- | --- | --- | --- | --- |"
    );
    for mapping in &suite.verifier {
        let candidates: Vec<String> = mapping
            .candidates
            .iter()
            .map(|c| {
                let mut text = format!("`{}` {:.2}", c.test, c.score);
                if let Some(p) = c.checks {
                    let _ = write!(text, ", checks {p:.2}");
                }
                if let Some(p) = c.contradicts
                    && p >= 0.3
                {
                    let _ = write!(text, ", contradicts {p:.2}");
                }
                text
            })
            .collect();
        let covered = match mapping.covered.as_str() {
            "no" => "**No**".to_owned(),
            "contradicted" => format!("**Contradicted** by {}", mapping.by.join(", ")),
            other => format!("{} ({})", capitalize(other), mapping.by.join(", ")),
        };
        let _ = writeln!(
            out,
            "| `{}` | {} | {covered} | {} | {} |",
            mapping.verifier_test,
            if candidates.is_empty() {
                "None".to_owned()
            } else {
                candidates.join("; ")
            },
            mapping.decided_by,
            if mapping.status == "passed" {
                "Passed".to_owned()
            } else {
                format!("**{}**", capitalize(&mapping.status))
            }
        );
    }
    out.push('\n');
    if suite.uncovered.is_empty() {
        let _ = writeln!(
            out,
            "Every verifier test has an acceptance test that checks it.\n"
        );
    } else {
        let _ = writeln!(
            out,
            "Uncovered verifier tests: {}.\n",
            suite
                .uncovered
                .iter()
                .map(|t| format!("`{t}`"))
                .collect::<Vec<_>>()
                .join(", ")
        );
    }
}

fn reversals(out: &mut String, analysis: &Analysis) {
    let _ = writeln!(out, "## Reversals\n");
    if analysis
        .timeline
        .as_ref()
        .is_none_or(|t| t.sessions.is_empty())
    {
        let _ = writeln!(
            out,
            "The run kept no session patches to compare, so reversals can't be rebuilt.\n"
        );
        return;
    }
    if analysis.reversals.is_empty() && analysis.guard_edits.is_empty() {
        let _ = writeln!(
            out,
            "No session removed lines an earlier session had added, as far as the sessions' patches show. Edits made through shell commands aren't tracked.\n"
        );
        return;
    }
    let _ = writeln!(
        out,
        "Rebuilt from the sessions' patches. A revert puts back something like the code the earlier session replaced; a rewrite replaces its lines with new code. Edits made through shell commands aren't tracked.\n"
    );
    for reversal in &analysis.reversals {
        let _ = writeln!(
            out,
            "**`{}` {} `{}`'s change to `{}`** at {}: it removed {} of the {} lines `{}` had added{}.\n",
            reversal.later,
            if reversal.kind == "revert" {
                "reverted"
            } else {
                "rewrote"
            },
            reversal.earlier,
            reversal.file,
            offset(reversal.at_ms),
            reversal.undone,
            reversal.earlier_added,
            reversal.earlier,
            if reversal.symbols.is_empty() {
                String::new()
            } else {
                format!(", in `{}`", reversal.symbols.join("`, `"))
            }
        );
        if !reversal.drivers.is_empty() {
            let _ = writeln!(
                out,
                "- Driven by: {}{}.",
                reversal
                    .drivers
                    .iter()
                    .map(|t| format!("`{t}`"))
                    .collect::<Vec<_>>()
                    .join(", "),
                if reversal.guards.is_empty() {
                    String::new()
                } else {
                    format!(
                        "; {} passed on the untouched workspace, a guard",
                        reversal.guards.join(", ")
                    )
                }
            );
        }
        if let Some(restorer) = &reversal.restored_by {
            let _ = writeln!(out, "- `{restorer}` put the removed lines back.");
        }
        if let Some(why) = &reversal.better_then_reverted {
            let _ = writeln!(out, "- **Reached a better state, then reverted:** {why}.");
        }
        if !reversal.removed.is_empty() {
            let _ = writeln!(out, "\n```diff");
            for line in &reversal.removed {
                let _ = writeln!(out, "- {line}");
            }
            for line in &reversal.added {
                let _ = writeln!(out, "+ {line}");
            }
            let _ = writeln!(out, "```");
        }
        out.push('\n');
    }
    if !analysis.guard_edits.is_empty() {
        let _ = writeln!(
            out,
            "Guards that started failing and led a session to edit code:\n"
        );
        for edit in &analysis.guard_edits {
            let _ = writeln!(
                out,
                "- `{}` passed on the untouched workspace, failed in {}, and `{}` then edited {}.",
                edit.test,
                edit.red_in,
                edit.session,
                edit.files
                    .iter()
                    .map(|f| format!("`{f}`"))
                    .collect::<Vec<_>>()
                    .join(", ")
            );
        }
        out.push('\n');
    }
}

fn anomalies(out: &mut String, analysis: &Analysis) {
    let _ = writeln!(out, "## Anomalies\n");
    if analysis.anomalies.is_empty() {
        let _ = writeln!(out, "None found.\n");
        return;
    }
    for (index, anomaly) in analysis.anomalies.iter().enumerate() {
        let _ = writeln!(
            out,
            "{}. **{}.** {}{}.",
            index + 1,
            capitalize(&anomaly.kind),
            anomaly.text.trim_end_matches('.'),
            anomaly
                .seconds
                .filter(|s| *s > 0.0)
                .map(|s| format!(", {s:.1} s in all"))
                .unwrap_or_default()
        );
    }
    out.push('\n');
}

fn fable(out: &mut String, analysis: &Analysis) {
    let Some(fable) = &analysis.fable else {
        return;
    };
    let _ = writeln!(out, "## Compared with Fable 5.1\n");
    let _ = writeln!(
        out,
        "Fable 5.1 passed {} of {} public attempts on `{}`. Fable's times are trial wall times from the public records.\n",
        fable.passes, fable.attempts, analysis.run.task
    );
    let verdict = &analysis.verdict;
    let mut header = vec!["Measure".to_owned(), "This run".to_owned()];
    if fable.cheapest_pass.is_some() {
        header.push("Fable cheapest pass".to_owned());
    }
    if let Some(tier) = &fable.tier {
        header.push(format!("Fable {}, mean", tier.effort));
    }
    header.push("Fable, all efforts".to_owned());
    let _ = writeln!(out, "| {} |", header.join(" | "));
    let _ = writeln!(out, "| --- |{}", " --- |".repeat(header.len() - 1));
    let mut row = |cells: Vec<String>| {
        let _ = writeln!(out, "| {} |", cells.join(" | "));
    };
    let mut verifier = vec![
        "Verifier".to_owned(),
        format!("{} of {}", verdict.passed, verdict.total),
    ];
    if fable.cheapest_pass.is_some() {
        verifier.push("Pass".to_owned());
    }
    if let Some(tier) = &fable.tier {
        verifier.push(format!("{} of {}", tier.passes, tier.attempts));
    }
    verifier.push(format!("{} of {}", fable.passes, fable.attempts));
    row(verifier);
    let mut time = vec![
        "Time".to_owned(),
        analysis.agent_ms.map_or("—".to_owned(), long),
    ];
    if let Some(cheapest) = &fable.cheapest_pass {
        time.push(long((cheapest.seconds * 1000.0) as u64));
    }
    if let Some(tier) = &fable.tier {
        time.push(long((tier.mean_sec * 1000.0) as u64));
    }
    time.push(long((fable.all_mean_sec * 1000.0) as u64));
    row(time);
    let mut cost = vec!["Cost".to_owned(), usd(analysis.cost.total_usd)];
    if let Some(cheapest) = &fable.cheapest_pass {
        cost.push(usd(cheapest.cost_usd));
    }
    if let Some(tier) = &fable.tier {
        cost.push(usd(tier.mean_usd));
    }
    cost.push(usd(fable.all_mean_usd));
    row(cost);
    if let Some(cheapest) = &fable.cheapest_pass {
        let mut steps = vec![
            "Steps".to_owned(),
            analysis.timeline.as_ref().map_or("—".to_owned(), |t| {
                format!(
                    "{} turns in {} sessions",
                    t.sessions.iter().map(|s| s.turns).sum::<usize>(),
                    t.sessions.len()
                )
            }),
            format!("{} ({})", cheapest.steps, cheapest.effort),
        ];
        if fable.tier.is_some() {
            steps.push(String::new());
        }
        steps.push(String::new());
        row(steps);
    }
    out.push('\n');
}

/// Markdown as lines for the Runs pane, each marked as a heading or not,
/// wrapped to `width`. Table rows and code keep their lines, clipped.
#[must_use]
pub fn pane_lines(markdown: &str, width: usize) -> Vec<(String, bool)> {
    let width = width.max(20);
    let mut lines = Vec::new();
    let mut code = false;
    for line in markdown.lines() {
        if line.starts_with("```") {
            code = !code;
            continue;
        }
        if code || line.starts_with('|') {
            if line.starts_with("| ---") {
                continue;
            }
            let text = if line.starts_with('|') {
                line.trim_matches('|')
                    .split(" | ")
                    .map(|cell| cell.trim().replace("**", "").replace('`', ""))
                    .collect::<Vec<_>>()
                    .join("  ·  ")
            } else {
                format!("  {line}")
            };
            let clipped: String = text.chars().take(width).collect();
            lines.push((clipped, false));
            continue;
        }
        if let Some(heading) = line.strip_prefix('#') {
            lines.push((
                heading.trim_start_matches('#').trim().replace('`', ""),
                true,
            ));
            continue;
        }
        if line.trim().is_empty() {
            lines.push((String::new(), false));
            continue;
        }
        let plain = line.replace("**", "").replace('`', "");
        let indent: String = plain.chars().take_while(|c| *c == ' ').collect();
        let mut current = String::new();
        for word in plain.split_whitespace() {
            if !current.is_empty() && current.chars().count() + 1 + word.chars().count() > width {
                lines.push((std::mem::take(&mut current), false));
                current.push_str(&indent);
                current.push_str("  ");
            }
            if !current.is_empty() && !current.ends_with(' ') {
                current.push(' ');
            }
            current.push_str(word);
        }
        if !current.is_empty() {
            lines.push((current, false));
        }
    }
    lines
}
