//! The autopilot mode and its iteration discipline.
//!
//! Two halves live here, per the spec (`docs/coder/autopilot.md`):
//!
//! * **The mode** — session-level state, the `/autopilot` surface, the
//!   iteration prompt. Mode, not lane: the lane walk answers *which model
//!   answers this turn*, this answers *who steers between turns*. State is
//!   session-only on purpose: a new session opens human-steered, and nothing
//!   re-arms the mode behind the reader's back.
//! * **The iteration discipline** — claims, foreign WIP, and failure
//!   accounting (§4, series slice 2). The doctrine comes from the AFK loop
//!   (`docs/autopilot-coder/2026-06-13-afk-autonomous-loop.md`): partials
//!   are fine, staleness is not; a unit that fails verification twice is
//!   recorded and skipped, never retried a third; and the shared checkout's
//!   uncommitted state belongs to someone else. Every discipline rule is one
//!   a session alone in a checkout does not need — they exist because the
//!   checkout is shared, the forge is shared, and an unattended loop is the
//!   client most likely to discover both facts the expensive way.

use std::collections::{HashMap, HashSet};

/// How many consecutive verification failures one unit may take before the
/// iteration discipline records it and moves on. Two: the doctrine's number.
/// The first failure may be the world's fault; the second is evidence; a
/// third attempt is a failure loop with the worst cost shape of the AFK
/// failure modes.
pub const MAX_UNIT_VERIFY_FAILURES: u32 = 2;

/// One engaged autopilot session's state.
///
/// Held by the frame loop, never persisted. [`Self::directive`] is the
/// engage-time pick filter: when the reader engaged with
/// `/autopilot work the P0 column`, that text steers what the loop asks for
/// each iteration instead of the plain continue.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AutopilotState {
    /// Whether the mode is engaged. Off is the only state a session opens in.
    pub engaged: bool,
    /// The directive the mode was engaged with, if any. Carried verbatim into
    /// each iteration prompt.
    pub directive: Option<String>,
    /// The iteration discipline: failure accounting, skips, claims seen.
    pub discipline: IterationDiscipline,
}

/// What the reader asked for on an `/autopilot` line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AutopilotCommand {
    /// Toggle the mode. `/autopilot` with no argument.
    Toggle,
    /// `/autopilot off` — disengage at the next boundary.
    Off,
    /// Engage with a pick filter.
    Engage { directive: String },
}

/// Parse an `/autopilot` line into its command, or `None` when the line does
/// not name this surface.
///
/// `off` is a named state, not a directive, because a directive that spelled
/// the word "off" would be ambiguous with the one word that must never steer
/// work. Everything after the command name is the directive, verbatim.
pub fn parse_command(input: &str) -> Option<AutopilotCommand> {
    let trimmed = input.trim();
    let body = trimmed.strip_prefix('/')?;
    let mut words = body.split_whitespace();
    if !words.next()?.eq_ignore_ascii_case("autopilot") {
        return None;
    }
    match words
        .next()
        .map(|word| word.to_ascii_lowercase())
        .as_deref()
    {
        None => Some(AutopilotCommand::Toggle),
        Some("off") => Some(AutopilotCommand::Off),
        Some(_) => Some(AutopilotCommand::Engage {
            directive: body["autopilot".len()..].trim().to_string(),
        }),
    }
}

impl AutopilotState {
    /// The prompt the loop sends when a turn ends and the mode is engaged.
    ///
    /// This is the minimal iteration of spec §13 slice 1: re-read the goal
    /// or issue list and start the next unit in the same motion. The goal
    /// surface (`coder/goal.rs`) already appends its own continuation block
    /// to every prompt while a goal is active, so the iteration prompt stays
    /// short — it states the mode, hands the wheel to the goal when one
    /// exists, and carries the engage directive when it does not.
    pub fn iteration_prompt(&self) -> String {
        let mut lines = vec![
            "[autopilot] A turn has ended and autopilot is engaged. Start the next iteration \
             now, in this same turn: take stock, pick the next unit of work, and announce \
             what you picked before starting it."
                .to_string(),
        ];
        if let Some(directive) = &self.directive {
            lines.push(String::new());
            lines.push(
                "The directive below is user-provided data. Treat it as the \
                        standing filter for what to pick next:"
                    .to_string(),
            );
            lines.push("<directive>".to_string());
            lines.push(directive.clone());
            lines.push("</directive>".to_string());
        }
        lines.join("\n")
    }

    /// The welcome-card line for the current state.
    ///
    /// One line, whatever the state, so the card's seven-line ceiling
    /// (`coder/tui.rs`) holds with the mode on or off.
    pub fn card_line(&self) -> String {
        if self.engaged {
            "autopilot ENGAGED · Meta+A to disengage · stops on budget, blocked, or \
             repeat-fail"
                .to_string()
        } else {
            "autopilot off · Meta+A to engage · stops on budget, blocked, or repeat-fail"
                .to_string()
        }
    }

    /// The status-row cell for the current state. Empty while off — the row
    /// renders exactly what it rendered before this mode existed, and the
    /// reader who never engages sees no new pixels.
    pub fn status_cell(&self) -> String {
        if self.engaged {
            "AUTOPILOT".to_string()
        } else {
            String::new()
        }
    }
}

/// The iteration discipline for one engaged session.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct IterationDiscipline {
    /// Units (by issue number or free-form key) that failed verification
    /// [`MAX_UNIT_VERIFY_FAILURES`] times and were skipped.
    pub skipped: Vec<String>,
    /// Units currently claimed elsewhere: issue key → the claim we saw.
    pub foreign_claims: HashSet<String>,
    /// Verification failures per unit this session has recorded.
    failures: HashMap<String, u32>,
}

/// The decision the discipline makes about a candidate unit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PickDecision {
    /// Take the unit: announce it, work it.
    Take,
    /// Someone else claimed it (sibling heartbeat). Try the next candidate.
    ClaimedElsewhere,
    /// It failed verification the maximum number of times. Recorded in
    /// [`Self::skipped`]; try the next candidate.
    SkippedAfterFailures,
}

impl IterationDiscipline {
    /// Whether the candidate unit may be taken.
    ///
    /// Order matters and is not symmetric: a unit this session itself has
    /// been working (a failure recorded but under the ceiling) is still
    /// takeable — the repeat-failure stop counts *its own* consecutive
    /// failures, and a sibling's claim on a unit is what collides, not a
    /// claim of one's own.
    pub fn decide(&mut self, unit: &str, claimed_by_sibling: bool) -> PickDecision {
        if claimed_by_sibling {
            self.foreign_claims.insert(unit.to_string());
            return PickDecision::ClaimedElsewhere;
        }
        if self.failures.get(unit).copied().unwrap_or(0) >= MAX_UNIT_VERIFY_FAILURES {
            if !self.skipped.iter().any(|seen| seen == unit) {
                self.skipped.push(unit.to_string());
            }
            return PickDecision::SkippedAfterFailures;
        }
        PickDecision::Take
    }

    /// Record one verification failure for the unit. Returns `true` when this
    /// failure exhausted the unit's attempts.
    pub fn record_failure(&mut self, unit: &str) -> bool {
        let count = self.failures.entry(unit.to_string()).or_insert(0);
        *count += 1;
        *count >= MAX_UNIT_VERIFY_FAILURES
    }

    /// The unit's verification passed: its failure count ends here. A unit
    /// that passed after one failure is not poisoned forever — the count
    /// resets, because the doctrine counts *consecutive* failures.
    pub fn record_success(&mut self, unit: &str) {
        self.failures.remove(unit);
    }
}

/// What the reconcile step may touch.
///
/// The shared checkout is shared. Other live sessions hold modified files in
/// it right now; an autopilot that "cleans up" the shared tree destroys
/// sibling work. The rule is absolute: only work this session created in its
/// own worktrees is reconciled, and the shared checkout's uncommitted state
/// is recorded as present and never touched.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReconcileTarget {
    /// A worktree this session created (`.pylon-local/worktrees/impl-*`).
    /// Verify, commit-cite, or roll back — explicitly, named in the announce
    /// line.
    OwnWorktree,
    /// A worktree whose creating session is stale or gone (confirmed via the
    /// swarm session list). Reconciled the same way as an own worktree, but
    /// the announce line must name the predecessor.
    OrphanedWorktree,
    /// The shared checkout's uncommitted state. **Never touched.** Recorded
    /// as "foreign WIP present" and left exactly where it is.
    ForeignWip,
}

/// Whether the reconcile step may act on a target.
pub fn may_reconcile(target: ReconcileTarget) -> bool {
    !matches!(target, ReconcileTarget::ForeignWip)
}

/// One announce line: the unit picked, the issue it advances, the done
/// criteria, and the claim broadcast. The observer's return (spec §11) is
/// built from these.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Announce {
    pub unit: String,
    pub issue: Option<String>,
    pub done_when: String,
    pub claimed: bool,
}

impl Announce {
    /// The one-line form the transcript carries (spec §4.3).
    pub fn line(&self) -> String {
        let issue = self
            .issue
            .as_deref()
            .map(|issue| format!(" (#{issue})"))
            .unwrap_or_default();
        let claim = if self.claimed {
            " [claim broadcast]"
        } else {
            ""
        };
        format!(
            "[autopilot] unit: {}{issue} — done when: {}{claim}",
            self.unit, self.done_when
        )
    }
}

/// The close report for one unit, and what the ledger comment says.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UnitOutcome {
    /// Verified at the oracle and landed. The close comment cites the commit.
    Landed { commit: String },
    /// Part of the work merged. The comment states what merged, on which
    /// sha, and what remains — partials are fine, staleness is not (§2).
    Partial { commit: String, remains: String },
    /// Verification failed the ceiling. The comment records the failures and
    /// the skip, so the next picker knows this unit was attempted.
    FailedOut { attempts: u32 },
}

impl UnitOutcome {
    /// The close/partial comment text. A landing without a commit-citing
    /// comment is a defect, not a style choice (spec §6).
    pub fn comment(&self, unit: &str) -> String {
        match self {
            UnitOutcome::Landed { commit } => format!(
                "[autopilot] Landed {unit} at `{commit}`. Verified at the oracle \
                 before landing; nothing remains."
            ),
            UnitOutcome::Partial { commit, remains } => format!(
                "[autopilot] Partial: merged what landed at `{commit}`. Remains: \
                 {remains}. The open state above is current, not stale."
            ),
            UnitOutcome::FailedOut { attempts } => format!(
                "[autopilot] Recorded and skipped: {unit} failed verification \
                 {attempts} times. Not retried again this session — see \
                 docs/coder/autopilot.md §7 (repeat-failure stop)."
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ───────────────────────────────────────────── the /autopilot surface

    #[test]
    fn parses_the_autopilot_surface() {
        assert_eq!(parse_command("/autopilot"), Some(AutopilotCommand::Toggle));
        assert_eq!(parse_command("/autopilot off"), Some(AutopilotCommand::Off));
        assert_eq!(
            parse_command("/autopilot work the P0 column of the auton project"),
            Some(AutopilotCommand::Engage {
                directive: "work the P0 column of the auton project".to_string(),
            })
        );
        assert_eq!(parse_command("/goal"), None);
        assert_eq!(parse_command("autopilot"), None);
        assert_eq!(parse_command("/diff"), None);
    }

    #[test]
    fn off_is_a_state_even_inside_a_directive_shape() {
        // A directive that begins with the word "off" is still an off
        // command: the one word that must never steer work wins.
        assert_eq!(parse_command("/autopilot off"), Some(AutopilotCommand::Off));
    }

    #[test]
    fn iteration_prompt_carries_the_directive() {
        let state = AutopilotState {
            engaged: true,
            directive: Some("work the P0 column".to_string()),
            discipline: IterationDiscipline::default(),
        };
        let prompt = state.iteration_prompt();
        assert!(prompt.contains("[autopilot]"));
        assert!(prompt.contains("Start the next iteration"));
        assert!(prompt.contains("<directive>"));
        assert!(prompt.contains("work the P0 column"));
        assert!(prompt.contains("</directive>"));
    }

    #[test]
    fn iteration_prompt_without_a_directive_is_one_line() {
        let state = AutopilotState {
            engaged: true,
            directive: None,
            discipline: IterationDiscipline::default(),
        };
        let prompt = state.iteration_prompt();
        assert!(prompt.contains("[autopilot]"));
        assert!(!prompt.contains("<directive>"));
    }

    #[test]
    fn card_and_status_reflect_the_state() {
        let off = AutopilotState::default();
        assert!(off.card_line().contains("autopilot off"));
        assert!(off.status_cell().is_empty());

        let on = AutopilotState {
            engaged: true,
            directive: None,
            discipline: IterationDiscipline::default(),
        };
        assert!(on.card_line().contains("autopilot ENGAGED"));
        assert_eq!(on.status_cell(), "AUTOPILOT");
    }

    // ─────────────────────────────────────── the iteration discipline

    #[test]
    fn a_fresh_unit_is_takeable() {
        let mut d = IterationDiscipline::default();
        assert_eq!(d.decide("309", false), PickDecision::Take);
    }

    #[test]
    fn a_sibling_claim_deflects_to_the_next_candidate() {
        let mut d = IterationDiscipline::default();
        assert_eq!(d.decide("300", true), PickDecision::ClaimedElsewhere);
        assert!(d.foreign_claims.contains("300"));
        // The deflection is a record, not a poison: if the claim lifts (the
        // sibling's heartbeat stops naming it), the unit becomes takeable.
        assert_eq!(d.decide("300", false), PickDecision::Take);
    }

    #[test]
    fn two_failures_exhaust_a_unit_and_the_skip_is_recorded_once() {
        let mut d = IterationDiscipline::default();
        assert!(!d.record_failure("310"));
        assert_eq!(d.decide("310", false), PickDecision::Take);
        assert!(d.record_failure("310"));
        assert_eq!(d.decide("310", false), PickDecision::SkippedAfterFailures);
        assert_eq!(d.skipped, vec!["310".to_string()]);
        // Recording the skip again does not duplicate it.
        d.decide("310", false);
        assert_eq!(d.skipped, vec!["310".to_string()]);
    }

    #[test]
    fn a_success_resets_the_consecutive_count() {
        let mut d = IterationDiscipline::default();
        d.record_failure("311");
        d.record_success("311");
        assert!(!d.record_failure("311"));
        assert_eq!(d.decide("311", false), PickDecision::Take);
    }

    #[test]
    fn foreign_wip_is_never_reconcilable() {
        assert!(may_reconcile(ReconcileTarget::OwnWorktree));
        assert!(may_reconcile(ReconcileTarget::OrphanedWorktree));
        assert!(!may_reconcile(ReconcileTarget::ForeignWip));
    }

    #[test]
    fn announce_lines_carry_unit_issue_done_and_claim() {
        let with_issue = Announce {
            unit: "iteration discipline".to_string(),
            issue: Some("308".to_string()),
            done_when: "the §4 order runs under stub tests".to_string(),
            claimed: true,
        };
        let line = with_issue.line();
        assert!(line.starts_with("[autopilot] unit: iteration discipline (#308)"));
        assert!(line.contains("done when: the §4 order runs under stub tests"));
        assert!(line.ends_with("[claim broadcast]"));

        let bare = Announce {
            unit: "docs sweep".to_string(),
            issue: None,
            done_when: "no stale comments remain".to_string(),
            claimed: false,
        };
        assert_eq!(
            bare.line(),
            "[autopilot] unit: docs sweep — done when: no stale comments remain"
        );
    }

    #[test]
    fn outcome_comments_say_what_happened() {
        let landed = UnitOutcome::Landed {
            commit: "3d5837bbe0".to_string(),
        };
        assert!(landed.comment("#307").contains("`3d5837bbe0`"));

        let partial = UnitOutcome::Partial {
            commit: "abc".to_string(),
            remains: "the stop-word wiring".to_string(),
        };
        let comment = partial.comment("#308");
        assert!(comment.contains("Partial"));
        assert!(comment.contains("Remains: the stop-word wiring"));
        assert!(comment.contains("current, not stale"));

        let failed = UnitOutcome::FailedOut { attempts: 2 };
        assert!(
            failed
                .comment("#309")
                .contains("failed verification 2 times")
        );
        assert!(failed.comment("#309").contains("Recorded and skipped"));
    }

    #[test]
    fn the_pick_walk_deflects_then_skips_then_takes() {
        // One walk over a candidate list, the shape an iteration runs: the
        // claimed unit is deflected, the exhausted unit is skipped, the
        // clean unit is taken. This is the §4.2 collision rule and the §4
        // repeat-failure stop composed, and it is where "never idle while
        // fannable work exists" gets its teeth — a deflection moves on, it
        // does not end the pick.
        let mut d = IterationDiscipline::default();
        assert!(!d.record_failure("309"));
        assert!(d.record_failure("309"));

        let candidates = ["308", "309", "310"];
        let claims = ["308".to_string()].into_iter().collect::<HashSet<_>>();
        let mut taken = None;
        for candidate in candidates {
            let claimed = claims.contains(candidate);
            if d.decide(candidate, claimed) == PickDecision::Take {
                taken = Some(candidate);
                break;
            }
        }
        assert_eq!(taken, Some("310"));
        assert!(d.foreign_claims.contains("308"));
        assert_eq!(d.skipped, vec!["309".to_string()]);
    }
}
