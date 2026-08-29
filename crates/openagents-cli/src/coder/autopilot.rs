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
    /// The budget and stop conditions (§7). Default-armed: one hour of wall
    /// clock, the token ledger as primary signal, visibility-loss and
    /// forge-unreachable counters at zero.
    pub stops: StopConditions,
    /// The stop word, when armed. `None` means only interactive Meta+A can
    /// stop the mode.
    pub stop_word: Option<StopWord>,
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
/// work. A `--stop-word <token>` prefix arms the stop word at engage time
/// (spec §7) and is stripped from the directive; the rest is the directive,
/// verbatim.
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

/// Split an engage directive into its stop-word arm and the remaining pick
/// filter. `--stop-word <token>` at the front of the directive arms the
/// token; everything after it is the directive. Returns the unchanged
/// directive when the prefix is absent. The word `off` as the first bare
/// token was already handled by [`parse_command`] and never reaches here.
pub fn split_stop_word(directive: &str) -> (Option<String>, &str) {
    let trimmed = directive.trim();
    let Some(rest) = trimmed.strip_prefix("--stop-word") else {
        return (None, trimmed);
    };
    let rest = rest.trim_start();
    let Some((token, remainder)) = rest.split_once(char::is_whitespace) else {
        // `--stop-word <token>` with nothing after it: the token arms the
        // mechanism and the directive is empty — the loop picks unfiltered.
        let token = rest.trim();
        if token.is_empty() {
            return (None, trimmed);
        }
        return (Some(token.to_string()), "");
    };
    let token = token.trim().to_string();
    if token.is_empty() {
        return (None, trimmed);
    }
    (Some(token), remainder.trim())
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

    /// The observer's status report (spec §8), assembled from the state the
    /// loop has been keeping. `last_heartbeat` is carried by the frame loop
    /// and passed in, because the heartbeat lives at the boundary, not in
    /// this state.
    pub fn status_report(&self, last_heartbeat: Option<String>) -> StatusReport {
        StatusReport {
            engaged: self.engaged,
            closed: Vec::new(),
            skipped: self.discipline.skipped.clone(),
            elapsed_seconds: self.stops.elapsed_seconds,
            budget_seconds: self.stops.wall_clock_seconds,
            directive: self.directive.clone(),
            last_heartbeat,
            heartbeat_failures: self.stops.heartbeat_failures,
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

    /// Apply one boundary-drain receipt. Returns a stop reason when the
    /// armed stop word is in the consumed mail; otherwise the loop continues.
    /// The mode does not drain again — that would be empty by construction.
    pub fn observe_mail(&self, receipt: &MailReceipt) -> Option<StopReason> {
        self.stop_word
            .as_ref()
            .filter(|word| word.sighted_in_receipt(receipt))
            .map(|_| StopReason::StopWord)
    }
}

/// The budget and stop conditions for one engaged autopilot run (spec §7,
/// series slice 3).
///
/// An autopilot that cannot stop is a liability, not an autopilot. Every
/// condition here stops the mode **on the iteration boundary** — never
/// mid-unit, never mid-verify — and a stop is a report, not a halt: the mode
/// hands the wheel back and waits like a normal session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StopConditions {
    /// The engage-time wall-clock budget, in seconds. The default is one
    /// hour; an unset budget is not an option (spec §7 — `None` is refused
    /// by [`Self::new`], which clamps to the default instead).
    pub wall_clock_seconds: u64,
    /// Seconds the engaged loop has already spent on iterations.
    pub elapsed_seconds: u64,
    /// Consecutive swarm send failures (heartbeats, claim broadcasts). The
    /// visibility mechanism failing silently is the #306 shape; when it
    /// fails this many times in a row, the mode stops rather than run
    /// unseen.
    pub heartbeat_failures: u32,
    /// Consecutive forge ledger failures (an issue read or write that
    /// erred). While the evidence system is dark, a landing cannot satisfy
    /// §6, so the mode must not keep landing.
    pub forge_failures: u32,
}

/// Default wall-clock budget: one hour. The goal's own token budget, when
/// one exists, is checked independently and whichever runs out first stops
/// the mode.
pub const DEFAULT_WALL_CLOCK_SECONDS: u64 = 60 * 60;

/// Consecutive heartbeat failures after which the mode stops. Three: one is
/// a hiccup, two is suspicious, three means nobody can see this loop.
pub const MAX_HEARTBEAT_FAILURES: u32 = 3;

/// Consecutive forge failures after which the mode stops. Three: fewer
/// would stop on a single hiccup while the forge retries.
pub const MAX_FORGE_FAILURES: u32 = 3;

/// Why the mode stopped. The stop report (§7) names the reason in the
/// transcript, so the returning reader knows which condition fired without
/// reconstructing it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StopReason {
    /// The wall-clock budget ran out, or the goal's token budget did.
    BudgetExhausted,
    /// Every remaining open issue needs credentials, boots, or spend.
    OwnerGatedWall,
    /// The same unit failed verification [`MAX_UNIT_VERIFY_FAILURES`] times.
    /// (Recorded by [`IterationDiscipline`]; carried here so the stop report
    /// can name it.)
    RepeatFailure { unit: String },
    /// Swarm sends failed this many times consecutively — the loop went
    /// unseen, so it stopped.
    VisibilityLost,
    /// The forge ledger failed this many times consecutively — landings
    /// cannot be evidenced, so the mode stopped.
    ForgeUnreachable,
    /// The configured stop word was sighted on the boundary drain receipt.
    StopWord,
    /// The reader pressed Meta+A or ran `/autopilot off`. Listed for the
    /// stop report's completeness; an interactive disengage is the reader
    /// taking the wheel, not a condition the loop discovered.
    ReaderDisengage,
}

impl StopReason {
    /// The one-line transcript text of the stop report (spec §7: a stop is
    /// a report, not a halt).
    pub fn line(&self) -> String {
        match self {
            StopReason::BudgetExhausted => {
                "[autopilot] stopped: budget exhausted — the ledger above is current, the \
                 session waits for you."
                    .to_string()
            }
            StopReason::OwnerGatedWall => {
                "[autopilot] stopped: every remaining open issue is owner-gated \
                 (credentials, boots, or spend). Asks go to the workspace-root \
                 NEEDS_OWNER.md."
                    .to_string()
            }
            StopReason::RepeatFailure { unit } => format!(
                "[autopilot] stopped: unit {unit} failed verification the maximum \
                 number of times and was skipped."
            ),
            StopReason::VisibilityLost => {
                "[autopilot] stopped: swarm sends failed repeatedly — the loop went \
                 unseen, so it stopped rather than run silent."
                    .to_string()
            }
            StopReason::ForgeUnreachable => {
                "[autopilot] stopped: the forge ledger is unreachable — landings \
                 cannot be evidenced while the evidence system is dark."
                    .to_string()
            }
            StopReason::StopWord => {
                "[autopilot] stopped: the stop word was sighted in inbound swarm mail.".to_string()
            }
            StopReason::ReaderDisengage => {
                "[autopilot] stopped: disengaged by the reader.".to_string()
            }
        }
    }
}

impl Default for StopConditions {
    fn default() -> Self {
        Self::new(DEFAULT_WALL_CLOCK_SECONDS)
    }
}

impl StopConditions {
    /// A fresh condition set with the given wall-clock budget. Zero is
    /// clamped to the default: a mode that stops immediately is not a mode,
    /// and "no budget" is not an option (spec §7).
    pub fn new(wall_clock_seconds: u64) -> Self {
        Self {
            wall_clock_seconds: if wall_clock_seconds == 0 {
                DEFAULT_WALL_CLOCK_SECONDS
            } else {
                wall_clock_seconds
            },
            elapsed_seconds: 0,
            heartbeat_failures: 0,
            forge_failures: 0,
        }
    }

    /// Whether any boundary condition has fired. Checked before next-unit
    /// selection, never mid-unit. `goal_budget_exhausted` carries the goal
    /// ledger's answer: the token ledger is the **primary** budget signal
    /// and wall clock secondary, because the lane cycles freely under the
    /// mode and an hour on Pro is different work from an hour on Flash.
    pub fn should_stop(&self, goal_budget_exhausted: bool) -> Option<StopReason> {
        if goal_budget_exhausted {
            return Some(StopReason::BudgetExhausted);
        }
        if self.elapsed_seconds >= self.wall_clock_seconds {
            return Some(StopReason::BudgetExhausted);
        }
        if self.heartbeat_failures >= MAX_HEARTBEAT_FAILURES {
            return Some(StopReason::VisibilityLost);
        }
        if self.forge_failures >= MAX_FORGE_FAILURES {
            return Some(StopReason::ForgeUnreachable);
        }
        None
    }

    /// Record one iteration's wall-clock cost.
    pub fn record_elapsed(&mut self, seconds: u64) {
        self.elapsed_seconds = self.elapsed_seconds.saturating_add(seconds);
    }

    /// Record one swarm send outcome; a success resets the consecutive
    /// count, matching the doctrine's consecutive-failure shape.
    pub fn record_heartbeat(&mut self, delivered: bool) {
        if delivered {
            self.heartbeat_failures = 0;
        } else {
            self.heartbeat_failures = self.heartbeat_failures.saturating_add(1);
        }
    }

    /// Record one forge ledger outcome; a success resets the count.
    pub fn record_forge(&mut self, succeeded: bool) {
        if succeeded {
            self.forge_failures = 0;
        } else {
            self.forge_failures = self.forge_failures.saturating_add(1);
        }
    }
}

/// The stop word: the token that ends the mode from the network (spec §7).
///
/// The mechanism is a token carried by a swarm message, not a sender
/// register — sessions carry no identity metadata, so "a message from the
/// owner's session" is not checkable today; "a message carrying this token"
/// is. The boundary drain is the primary delivery path (#310): sighting
/// the token on that receipt ends the mode. An explicit later drain is
/// an edge case for mail that arrived with no boundary (wake catch-up).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StopWord {
    pub token: String,
}

impl StopWord {
    /// A stop word from the engage flag or config. An empty token disables
    /// the mechanism: with no token, only interactive Meta+A can stop the
    /// mode, and the welcome card says so.
    pub fn new(token: impl Into<String>) -> Self {
        Self {
            token: token.into().trim().to_string(),
        }
    }

    /// Whether the mechanism is armed.
    pub fn armed(&self) -> bool {
        !self.token.is_empty()
    }

    /// Whether inbound mail (one message body, or a whole drained batch)
    /// sights the token.
    pub fn sighted_in(&self, mail: &str) -> bool {
        self.armed() && mail.contains(&self.token)
    }

    /// Whether the boundary-drain receipt carries the token in any body.
    pub fn sighted_in_receipt(&self, receipt: &MailReceipt) -> bool {
        receipt.bodies.iter().any(|body| self.sighted_in(body))
    }
}

/// Mail the boundary drain just processed. After #303 the drain *is*
/// delivery: `consumed` names the ids stamped read, in inject order.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MailReceipt {
    pub consumed: Vec<String>,
    pub bodies: Vec<String>,
}

impl MailReceipt {
    /// Parse an `openagents.swarm.inbox.v1` tool result. Requires a
    /// `consumed` array so an empty or unrelated tool dump is not a receipt.
    pub fn from_inbox_tool_output(output: &str) -> Option<Self> {
        let value: serde_json::Value = serde_json::from_str(output).ok()?;
        if value.get("schema").and_then(|schema| schema.as_str())
            != Some("openagents.swarm.inbox.v1")
        {
            return None;
        }
        let consumed = value
            .get("consumed")?
            .as_array()?
            .iter()
            .filter_map(|id| id.as_str().map(str::to_string))
            .collect::<Vec<_>>();
        let bodies = value
            .get("messages")
            .and_then(|messages| messages.as_array())
            .map(|messages| {
                messages
                    .iter()
                    .filter_map(|message| {
                        message
                            .get("body")
                            .and_then(|body| body.as_str().map(str::to_string))
                    })
                    .collect()
            })
            .unwrap_or_default();
        Some(Self { consumed, bodies })
    }

    /// The most recent turn-boundary inbox receipt in the replayed
    /// messages, walking newest-first. Explicit `swarm_inbox` dumps are
    /// skipped: Autopilot's primary path is the drain the runtime already
    /// ran at the hop.
    pub fn from_tool_messages<'a>(
        messages: impl IntoIterator<Item = &'a crate::runtime::ChatMessage>,
    ) -> Option<Self> {
        for message in messages.into_iter().collect::<Vec<_>>().into_iter().rev() {
            if message.role != "tool" {
                continue;
            }
            let Some(content) = message.content.as_deref() else {
                continue;
            };
            let Ok(value) = serde_json::from_str::<serde_json::Value>(content) else {
                continue;
            };
            if value.get("source").and_then(|source| source.as_str()) != Some("turn_boundary") {
                continue;
            }
            return Self::from_inbox_tool_output(content);
        }
        None
    }

    /// The announce line spec §4.3 / §11 skims. Empty `consumed` is still
    /// a receipt — the observer should see that the drain ran and saw
    /// nothing, only when the caller asks for the line.
    pub fn announce_line(&self) -> String {
        if self.consumed.is_empty() {
            "[autopilot] mail: drain consumed []".to_string()
        } else {
            format!(
                "[autopilot] mail: drain consumed [{}]",
                self.consumed.join(", ")
            )
        }
    }
}

/// One heartbeat: the boundary status message the engaged loop sends to the
/// swarm (spec §5, series slice #311).
///
/// The heartbeat names the unit just closed and the unit just picked, and
/// refreshes any open claim (§4.2). It is the cheapest possible "still
/// alive, still productive" signal, and the thing that makes a stuck
/// autopilot visible from outside its own process. A send failure counts
/// against [`StopConditions::record_heartbeat`]: the visibility mechanism
/// must not fail silently.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Heartbeat {
    /// The unit closed at this boundary, when one was.
    pub closed: Option<String>,
    /// The unit about to be worked, when one was picked.
    pub picked: Option<String>,
    /// The claim this heartbeat refreshes, when the session holds one.
    pub claim: Option<String>,
}

impl Heartbeat {
    /// The message body. One line — the swarm carries it at machine speed,
    /// and siblings parse it by eye.
    pub fn body(&self) -> String {
        let closed = self.closed.as_deref().unwrap_or("none");
        let picked = self.picked.as_deref().unwrap_or("none");
        let mut line = format!("[autopilot] boundary: closed {closed}; picked {picked}");
        if let Some(claim) = &self.claim {
            line.push_str("; claim on ");
            line.push_str(claim);
            line.push_str(" (live)");
        }
        line
    }
}

/// The state `/autopilot status` renders (spec §8, §11): the observer's
/// return surface. One screen, in-terminal — what the reader's eyes land on
/// first when they walk back in.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StatusReport {
    /// Whether the mode is engaged right now.
    pub engaged: bool,
    /// Units closed this session, most recent last, as "unit (commit)".
    pub closed: Vec<String>,
    /// Units skipped after repeated verification failure.
    pub skipped: Vec<String>,
    /// Wall-clock seconds burned against the budget.
    pub elapsed_seconds: u64,
    /// The wall-clock budget the burn is measured against.
    pub budget_seconds: u64,
    /// The standing pick filter, when one was engaged with.
    pub directive: Option<String>,
    /// The last heartbeat body, when one was sent.
    pub last_heartbeat: Option<String>,
    /// Consecutive heartbeat failures, when any.
    pub heartbeat_failures: u32,
}

impl StatusReport {
    /// The rendered screen. One block of lines; no decorative progress
    /// bars — the transcript is the progress bar, this is the summary.
    pub fn render(&self) -> String {
        let mut lines = vec![format!(
            "Autopilot: {}",
            if self.engaged { "ENGAGED" } else { "off" }
        )];
        let burned = if self.budget_seconds > 0 {
            format!(
                " ({}m of {}m budget)",
                self.elapsed_seconds / 60,
                self.budget_seconds / 60
            )
        } else {
            String::new()
        };
        lines.push(format!("Budget burned: {}s{burned}", self.elapsed_seconds));
        match (&self.directive, self.closed.last()) {
            (Some(directive), _) => {
                lines.push(format!("Next unit filter: {directive}"));
            }
            (None, Some(last)) => {
                lines.push(format!("Last unit closed: {last}"));
            }
            (None, None) => {
                lines.push("Next unit: none picked yet".to_string());
            }
        }
        if !self.closed.is_empty() {
            lines.push(format!("Units closed: {}", self.closed.len()));
        }
        if !self.skipped.is_empty() {
            lines.push(format!(
                "Skipped (repeat-fail): {}",
                self.skipped.join(", ")
            ));
        }
        match &self.last_heartbeat {
            Some(body) => lines.push(format!("Last heartbeat: {body}")),
            None => lines.push("Last heartbeat: none sent".to_string()),
        }
        if self.heartbeat_failures > 0 {
            lines.push(format!(
                "Heartbeat failures: {} consecutive",
                self.heartbeat_failures
            ));
        }
        lines.join("\n")
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
            stops: StopConditions::default(),
            stop_word: None,
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
            stops: StopConditions::default(),
            stop_word: None,
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
            stops: StopConditions::default(),
            stop_word: None,
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

    // ───────────────────────────────────── budget and stop conditions

    #[test]
    fn the_default_budget_exists_and_zero_clamps_to_it() {
        let default = StopConditions::default();
        assert_eq!(default.wall_clock_seconds, DEFAULT_WALL_CLOCK_SECONDS);
        // "No budget" is not an option: a zero engage-time budget clamps to
        // the default rather than producing a mode that stops immediately.
        assert_eq!(
            StopConditions::new(0).wall_clock_seconds,
            DEFAULT_WALL_CLOCK_SECONDS
        );
        assert_eq!(StopConditions::new(120).wall_clock_seconds, 120);
    }

    #[test]
    fn wall_clock_is_secondary_to_the_token_ledger() {
        // The goal ledger's answer wins: an exhausted token budget stops the
        // mode even with wall clock left, and wall clock alone never stops a
        // mode the ledger still funds.
        let mut conditions = StopConditions::new(1_000);
        assert!(
            conditions
                .should_stop(true)
                .is_some_and(|reason| { reason == StopReason::BudgetExhausted })
        );
        conditions.record_elapsed(500);
        assert!(conditions.should_stop(false).is_none());
        conditions.record_elapsed(500);
        assert_eq!(
            conditions.should_stop(false),
            Some(StopReason::BudgetExhausted)
        );
    }

    #[test]
    fn consecutive_failures_stop_the_mode_and_successes_reset() {
        let mut conditions = StopConditions::new(1_000);
        conditions.record_heartbeat(false);
        conditions.record_heartbeat(false);
        assert!(conditions.should_stop(false).is_none());
        conditions.record_heartbeat(false);
        assert_eq!(
            conditions.should_stop(false),
            Some(StopReason::VisibilityLost)
        );
        // A success resets the consecutive count: recovery is possible.
        conditions.record_heartbeat(true);
        assert!(conditions.should_stop(false).is_none());

        let mut forge = StopConditions::new(1_000);
        for _ in 0..(MAX_FORGE_FAILURES - 1) {
            forge.record_forge(false);
        }
        assert!(forge.should_stop(false).is_none());
        forge.record_forge(false);
        assert_eq!(forge.should_stop(false), Some(StopReason::ForgeUnreachable));
        forge.record_forge(true);
        assert!(forge.should_stop(false).is_none());
    }

    #[test]
    fn the_stop_word_sights_on_the_drain_receipt_and_disarms_cleanly() {
        let word = StopWord::new("flywheel-home");
        assert!(word.armed());
        let hit = MailReceipt {
            consumed: vec!["msg_1".to_string()],
            bodies: vec!["status from a sibling: flywheel-home please".to_string()],
        };
        let miss = MailReceipt {
            consumed: vec!["msg_2".to_string()],
            bodies: vec!["nothing here".to_string()],
        };
        assert!(word.sighted_in_receipt(&hit));
        assert!(!word.sighted_in_receipt(&miss));

        let engaged = AutopilotState {
            engaged: true,
            directive: None,
            discipline: IterationDiscipline::default(),
            stops: StopConditions::default(),
            stop_word: Some(word.clone()),
        };
        assert_eq!(engaged.observe_mail(&hit), Some(StopReason::StopWord));
        assert!(engaged.observe_mail(&miss).is_none());

        let disarmed = StopWord::new("   ");
        assert!(!disarmed.armed());
        assert!(!disarmed.sighted_in_receipt(&hit));
    }

    #[test]
    fn a_mail_receipt_names_consumed_ids_and_parses_the_boundary_json() {
        let output = serde_json::json!({
            "schema": "openagents.swarm.inbox.v1",
            "source": "turn_boundary",
            "consumed": ["msg_a", "msg_b"],
            "messages": [
                {"id": "msg_a", "body": "first"},
                {"id": "msg_b", "body": "second"}
            ]
        })
        .to_string();
        let receipt = MailReceipt::from_inbox_tool_output(&output).expect("receipt");
        assert_eq!(
            receipt.consumed,
            vec!["msg_a".to_string(), "msg_b".to_string()]
        );
        assert_eq!(
            receipt.announce_line(),
            "[autopilot] mail: drain consumed [msg_a, msg_b]"
        );
        assert!(MailReceipt::from_inbox_tool_output("not json").is_none());
        assert!(MailReceipt::from_inbox_tool_output(r#"{"schema":"other"}"#).is_none());
    }

    #[test]
    fn stop_lines_say_what_fired() {
        assert!(
            StopReason::BudgetExhausted
                .line()
                .contains("budget exhausted")
        );
        assert!(
            StopReason::RepeatFailure {
                unit: "309".to_string()
            }
            .line()
            .contains("309")
        );
        assert!(StopReason::StopWord.line().contains("stop word"));
        assert!(StopReason::VisibilityLost.line().contains("unseen"));
        assert!(StopReason::ForgeUnreachable.line().contains("evidence"));
        assert!(StopReason::OwnerGatedWall.line().contains("NEEDS_OWNER"));
        assert!(StopReason::ReaderDisengage.line().contains("reader"));
    }

    #[test]
    fn the_stop_word_prefix_splits_from_the_directive() {
        assert_eq!(
            split_stop_word("--stop-word flywheel-home work the P0 column"),
            (Some("flywheel-home".to_string()), "work the P0 column")
        );
        // No prefix: the directive passes through whole.
        assert_eq!(
            split_stop_word("work the P0 column"),
            (None, "work the P0 column")
        );
        // A token with nothing after it still arms — the directive is simply
        // empty, meaning the loop picks without a filter.
        assert_eq!(
            split_stop_word("--stop-word flywheel-home"),
            (Some("flywheel-home".to_string()), "")
        );
        // A bare `--stop-word` with no token does not arm half a mechanism.
        assert_eq!(split_stop_word("--stop-word"), (None, "--stop-word"));
    }

    #[test]
    fn heartbeat_lines_carry_closed_picked_and_claim() {
        let bare = Heartbeat {
            closed: Some("#308".to_string()),
            picked: Some("#310".to_string()),
            claim: None,
        };
        assert_eq!(
            bare.body(),
            "[autopilot] boundary: closed #308; picked #310"
        );

        let claiming = Heartbeat {
            closed: None,
            picked: Some("#311".to_string()),
            claim: Some("#311".to_string()),
        };
        assert_eq!(
            claiming.body(),
            "[autopilot] boundary: closed none; picked #311; claim on #311 (live)"
        );
    }

    #[test]
    fn the_status_report_renders_the_observer_screen() {
        let mut report = StatusReport {
            engaged: true,
            closed: vec!["#307 (3d5837bbe0)".to_string()],
            skipped: vec!["#309".to_string()],
            elapsed_seconds: 1_500,
            budget_seconds: 3_600,
            directive: Some("work the P0 column".to_string()),
            last_heartbeat: Some("[autopilot] boundary: closed #308; picked #310".to_string()),
            heartbeat_failures: 1,
        };
        let screen = report.render();
        assert!(screen.contains("Autopilot: ENGAGED"), "{screen}");
        assert!(screen.contains("1500s (25m of 60m budget)"), "{screen}");
        assert!(
            screen.contains("Next unit filter: work the P0 column"),
            "{screen}"
        );
        assert!(screen.contains("Units closed: 1"), "{screen}");
        assert!(screen.contains("Skipped (repeat-fail): #309"), "{screen}");
        assert!(
            screen.contains("Last heartbeat: [autopilot] boundary"),
            "{screen}"
        );
        assert!(
            screen.contains("Heartbeat failures: 1 consecutive"),
            "{screen}"
        );

        // A fresh, disengaged session renders the same screen honestly.
        report.engaged = false;
        report.closed.clear();
        report.skipped.clear();
        report.elapsed_seconds = 0;
        report.directive = None;
        report.last_heartbeat = None;
        report.heartbeat_failures = 0;
        let bare = report.render();
        assert!(bare.contains("Autopilot: off"), "{bare}");
        assert!(bare.contains("Next unit: none picked yet"), "{bare}");
        assert!(bare.contains("Last heartbeat: none sent"), "{bare}");
        assert!(!bare.contains("consecutive"), "{bare}");
    }
}
