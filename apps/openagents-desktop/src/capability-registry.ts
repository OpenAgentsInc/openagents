/**
 * EP250 daily-coding capability registry (#8712).
 *
 * Encodes every capability from the taxonomy tables of
 * `docs/fable/2026-07-11-daily-coding-capability-audit.md` §4 as a typed row,
 * each carrying its desktop status, both oracle references (a UI oracle and a
 * programmatic oracle), the wiring state of each oracle in THIS repo, the
 * highest oracle rung reached, and — for a missing/blocked capability — the
 * blocker. The companion suite `tests/capability-evals.test.ts` iterates this
 * registry so a capability cannot silently regress from `ui_available` without
 * a red test, and drives the real headless oracles.
 *
 * SOURCE-OF-TRUTH NOTE (honest discrepancy). The audit's §4 taxonomy tables —
 * the per-capability authority, since that is where each capability's status is
 * stated — contain FORTY capability rows (A1..K2) with the distribution
 * { ui_available: 15, programmatic_only: 4, partial: 13, missing: 8 }. The
 * audit's prose "Totals" line and its "33 capabilities × 2 oracles" figure say
 * 33 with { 13, 4, 10, 6 }; those numbers are arithmetically inconsistent with
 * the audit's own tables (whose group sizes sum to 40, not 33). This registry
 * follows the TABLES — every capability the audit defines gets a row — and the
 * meta-test locks the table-derived distribution so drift is red. The prose
 * summary figures are recorded in `AUDIT_PROSE_SUMMARY` below and flagged by a
 * test as a known audit inconsistency (not silently "matched").
 */

/** Desktop capability status, exactly the audit's §4 enum. */
export type CapabilityStatus =
  | "ui_available"
  | "programmatic_only"
  | "partial"
  | "missing"

/** The audit's capability groups A..K. */
export type CapabilityGroup =
  | "A" | "B" | "C" | "D" | "E" | "F" | "G" | "H" | "I" | "J" | "K"

/** Highest oracle rung this capability's evals reach today (six-rung target). */
export type CapabilityRung = "fixture" | "live" | "pending"

/**
 * What an oracle reference actually IS in this repo, so the coverage report is
 * honest about wired-vs-pending:
 * - `headless_wired`: a real in-process assertion in capability-evals.test.ts
 *   that drives the typed surface headlessly (no Electron window).
 * - `existing_suite`: an already-green bun-test suite in the repo serves as
 *   this oracle (referenced, not re-implemented).
 * - `smoke_step`: an existing built-Electron smoke journey step (main.ts).
 * - `live_step`: a live-proof driver step (rung-4 PNG + journal receipt).
 * - `pending`: no oracle wired — a blocked/missing capability whose eval is a
 *   skipped-with-reason row that fails loudly if anyone claims it done.
 */
export type OracleWiring =
  | "headless_wired"
  | "existing_suite"
  | "smoke_step"
  | "live_step"
  | "pending"

export type CapabilityRow = Readonly<{
  /** "A1".."K2" — the audit's stable capability id. */
  id: string
  group: CapabilityGroup
  /** Short capability name from the audit §4 tables. */
  capability: string
  status: CapabilityStatus
  /** Repo-relative UI oracle reference (test file or live-proof/smoke driver). */
  uiOracleRef: string
  uiOracleWiring: OracleWiring
  /** Repo-relative programmatic oracle reference. */
  programmaticOracleRef: string
  programmaticOracleWiring: OracleWiring
  rung: CapabilityRung
  /** Required for `missing` (and every blocked row): the honest blocker. */
  blocker?: string
}>

const EVALS = "apps/openagents-desktop/tests/capability-evals.test.ts"
const SHELL = "apps/openagents-desktop/src/renderer/shell.test.ts"
const TOOLCARDS = "apps/openagents-desktop/src/renderer/tool-cards.test.ts"
const LOCAL_HARNESS = "apps/openagents-desktop/src/renderer/local-harness.test.ts"
const MARKDOWN = "apps/openagents-desktop/src/renderer/markdown.test.ts"
const WORKSPACE = "apps/openagents-desktop/tests/workspace-service.test.ts"
const USAGE = "apps/openagents-desktop/src/usage-ledger.test.ts"
const FABLE_RT = "apps/openagents-desktop/src/fable-local-runtime.test.ts"
const CODEX_CHILD_RT = "apps/openagents-desktop/src/codex-child-runtime.test.ts"
const CODEX_HISTORY = "apps/openagents-desktop/tests/codex-history.test.ts"
const CODING_CATALOG = "apps/openagents-desktop/tests/desktop-coding-catalog.test.ts"
const FLEET = "apps/openagents-desktop/src/renderer/fleet-workspace.test.ts"
const PROVIDER_ACCOUNTS = "apps/openagents-desktop/tests/provider-accounts.test.ts"
const RUNTIME_GATEWAY = "apps/openagents-desktop/tests/runtime-gateway.e2e.test.ts"
const RUNTIME_INTERACTIONS = "apps/openagents-desktop/src/renderer/runtime-interactions.test.ts"
const HISTORY_WORKSPACE = "apps/openagents-desktop/src/renderer/history-workspace.test.ts"
const COMMAND_HOST = "apps/openagents-desktop/tests/desktop-command-host.test.ts"
const SMOKE = "apps/openagents-desktop/src/main.ts"
const LIVE_PROOF = "apps/openagents-desktop/src/live-proof.ts"

/** The audit's prose-summary figures — recorded, and flagged as inconsistent. */
export const AUDIT_PROSE_SUMMARY = {
  total: 33,
  ui_available: 13,
  programmatic_only: 4,
  partial: 10,
  missing: 6,
} as const

/**
 * The table-derived distribution this registry actually encodes (the honest
 * count of the audit's §4 rows). The meta-test locks these; drift is red.
 */
export const CAPABILITY_TABLE_DISTRIBUTION = {
  total: 40,
  ui_available: 15,
  programmatic_only: 4,
  partial: 13,
  missing: 8,
} as const

export const capabilityRegistry: ReadonlyArray<CapabilityRow> = [
  // --- A. Conversation & steering -----------------------------------------
  {
    id: "A1", group: "A", capability: "Multi-turn streaming chat", status: "ui_available",
    uiOracleRef: SMOKE, uiOracleWiring: "smoke_step",
    programmaticOracleRef: EVALS, programmaticOracleWiring: "headless_wired",
    rung: "live",
  },
  {
    id: "A2", group: "A", capability: "Mid-turn interrupt / steer", status: "partial",
    uiOracleRef: SHELL, uiOracleWiring: "existing_suite",
    programmaticOracleRef: EVALS, programmaticOracleWiring: "headless_wired",
    rung: "live",
    // The Stop button landed in THIS lane; the interrupt IPC path is wired and
    // proven. Residual partial: no queued-follow-up steering (see A3).
    blocker: "audit A2: interrupt now UI-driven via the composer Stop button; queued follow-up steering remains unbuilt (A3)",
  },
  {
    id: "A3", group: "A", capability: "Queue follow-up while turn runs", status: "partial",
    uiOracleRef: "", uiOracleWiring: "pending",
    programmaticOracleRef: "", programmaticOracleWiring: "pending",
    rung: "pending",
    blocker: "audit A3: no local-lane message queue; sending a second prompt mid-turn is not implemented",
  },
  {
    id: "A4", group: "A", capability: "Model selection / mix", status: "partial",
    uiOracleRef: SHELL, uiOracleWiring: "existing_suite",
    programmaticOracleRef: EVALS, programmaticOracleWiring: "headless_wired",
    rung: "fixture",
    blocker: "audit A4: model pinned to FABLE_LOCAL_MODEL; model_effective visibility exists but there is no picker",
  },

  // --- B. Code reading & search -------------------------------------------
  {
    id: "B1", group: "B", capability: "File reading", status: "ui_available",
    uiOracleRef: SMOKE, uiOracleWiring: "smoke_step",
    programmaticOracleRef: EVALS, programmaticOracleWiring: "headless_wired",
    rung: "fixture",
  },
  {
    id: "B2", group: "B", capability: "Code search (grep/rg)", status: "ui_available",
    uiOracleRef: TOOLCARDS, uiOracleWiring: "existing_suite",
    programmaticOracleRef: TOOLCARDS, programmaticOracleWiring: "existing_suite",
    rung: "fixture",
  },
  {
    id: "B3", group: "B", capability: "Structure navigation (glob/find/ls)", status: "ui_available",
    uiOracleRef: TOOLCARDS, uiOracleWiring: "existing_suite",
    programmaticOracleRef: TOOLCARDS, programmaticOracleWiring: "existing_suite",
    rung: "fixture",
  },

  // --- C. Editing & patching ----------------------------------------------
  {
    id: "C1", group: "C", capability: "Targeted edits", status: "ui_available",
    uiOracleRef: TOOLCARDS, uiOracleWiring: "existing_suite",
    programmaticOracleRef: TOOLCARDS, programmaticOracleWiring: "existing_suite",
    rung: "fixture",
  },
  {
    id: "C2", group: "C", capability: "New-file creation", status: "ui_available",
    uiOracleRef: TOOLCARDS, uiOracleWiring: "existing_suite",
    programmaticOracleRef: TOOLCARDS, programmaticOracleWiring: "existing_suite",
    rung: "fixture",
  },
  {
    id: "C3", group: "C", capability: "Human file edit + save", status: "ui_available",
    uiOracleRef: LIVE_PROOF, uiOracleWiring: "live_step",
    programmaticOracleRef: EVALS, programmaticOracleWiring: "headless_wired",
    rung: "live",
  },

  // --- D. Execution & terminal --------------------------------------------
  {
    id: "D1", group: "D", capability: "Shell execution", status: "ui_available",
    uiOracleRef: SMOKE, uiOracleWiring: "smoke_step",
    programmaticOracleRef: TOOLCARDS, programmaticOracleWiring: "existing_suite",
    rung: "live",
  },
  {
    id: "D2", group: "D", capability: "Background processes + monitoring", status: "partial",
    uiOracleRef: "", uiOracleWiring: "pending",
    programmaticOracleRef: FABLE_RT, programmaticOracleWiring: "existing_suite",
    rung: "fixture",
    blocker: "audit D2: delegate children run async with caps, but there is no general background-process surface/indicator",
  },
  {
    id: "D3", group: "D", capability: "Interactive terminal / stdin steering", status: "partial",
    uiOracleRef: "", uiOracleWiring: "pending",
    programmaticOracleRef: "", programmaticOracleWiring: "pending",
    rung: "pending",
    blocker: "audit D3: workspace.terminal command exists but there is no agent-attached PTY stdin seam",
  },

  // --- E. Git & GitHub ----------------------------------------------------
  {
    id: "E1", group: "E", capability: "Repo inspection (status/diff/log)", status: "ui_available",
    uiOracleRef: LIVE_PROOF, uiOracleWiring: "live_step",
    programmaticOracleRef: EVALS, programmaticOracleWiring: "headless_wired",
    rung: "live",
  },
  {
    id: "E2", group: "E", capability: "Commit / push (fetch-rebase-push retry)", status: "programmatic_only",
    uiOracleRef: "", uiOracleWiring: "pending",
    programmaticOracleRef: TOOLCARDS, programmaticOracleWiring: "existing_suite",
    rung: "pending",
    blocker: "audit E2: agent-mediated Bash only; no commit/push UI or gateway command (live commit-push rung blocked)",
  },
  {
    id: "E3", group: "E", capability: "Worktree / branch isolation", status: "programmatic_only",
    uiOracleRef: "", uiOracleWiring: "pending",
    programmaticOracleRef: TOOLCARDS, programmaticOracleWiring: "existing_suite",
    rung: "fixture",
    blocker: "audit E3: worktree/branch isolation is agent-driven Bash only; no typed surface",
  },
  {
    id: "E4", group: "E", capability: "GitHub issues (gh issue)", status: "programmatic_only",
    uiOracleRef: "", uiOracleWiring: "pending",
    programmaticOracleRef: TOOLCARDS, programmaticOracleWiring: "existing_suite",
    rung: "fixture",
    blocker: "audit E4: gh issue via agent Bash only; no UI (one live gh receipt rung pending)",
  },
  {
    id: "E5", group: "E", capability: "GitHub PRs (gh pr)", status: "programmatic_only",
    uiOracleRef: "", uiOracleWiring: "pending",
    programmaticOracleRef: TOOLCARDS, programmaticOracleWiring: "existing_suite",
    rung: "fixture",
    blocker: "audit E5: gh pr via agent Bash only; no UI (one live gh receipt rung pending)",
  },

  // --- F. Web research ----------------------------------------------------
  {
    id: "F1", group: "F", capability: "Web search", status: "ui_available",
    uiOracleRef: TOOLCARDS, uiOracleWiring: "existing_suite",
    programmaticOracleRef: TOOLCARDS, programmaticOracleWiring: "existing_suite",
    rung: "fixture",
  },
  {
    id: "F2", group: "F", capability: "URL fetch", status: "ui_available",
    uiOracleRef: TOOLCARDS, uiOracleWiring: "existing_suite",
    programmaticOracleRef: TOOLCARDS, programmaticOracleWiring: "existing_suite",
    rung: "fixture",
  },

  // --- G. Delegation & multi-agent ----------------------------------------
  {
    id: "G1", group: "G", capability: "Subagent spawn (same provider)", status: "ui_available",
    uiOracleRef: TOOLCARDS, uiOracleWiring: "existing_suite",
    programmaticOracleRef: CODEX_CHILD_RT, programmaticOracleWiring: "existing_suite",
    rung: "fixture",
  },
  {
    id: "G2", group: "G", capability: "Cross-provider delegation", status: "ui_available",
    uiOracleRef: SMOKE, uiOracleWiring: "smoke_step",
    programmaticOracleRef: FABLE_RT, programmaticOracleWiring: "existing_suite",
    rung: "live",
  },
  {
    id: "G3", group: "G", capability: "Background agents + completion notify", status: "partial",
    uiOracleRef: "", uiOracleWiring: "pending",
    programmaticOracleRef: FABLE_RT, programmaticOracleWiring: "existing_suite",
    rung: "fixture",
    blocker: "audit G3: children run async with caps/timeouts, but there is no notification-on-complete surface",
  },
  {
    id: "G4", group: "G", capability: "Steer/message running children", status: "missing",
    uiOracleRef: "", uiOracleWiring: "pending",
    programmaticOracleRef: "", programmaticOracleWiring: "pending",
    rung: "pending",
    blocker: "audit G4: no child-steer channel in fable-local-contract.ts; the app can spawn but not talk to or stop a child",
  },
  {
    id: "G5", group: "G", capability: "Scheduled / fleet automation", status: "partial",
    uiOracleRef: FLEET, uiOracleWiring: "existing_suite",
    programmaticOracleRef: PROVIDER_ACCOUNTS, programmaticOracleWiring: "existing_suite",
    rung: "live",
    blocker: "audit G5: fleet workspace + pylon registry exist, but there is no local scheduling/cron",
  },

  // --- H. Session lifecycle -----------------------------------------------
  {
    id: "H1", group: "H", capability: "Resume / continuation", status: "partial",
    uiOracleRef: "", uiOracleWiring: "pending",
    programmaticOracleRef: FABLE_RT, programmaticOracleWiring: "existing_suite",
    rung: "pending",
    blocker: "audit H1: SDK resume is automatic per thread; no resume picker, and Codex children never resume (thread-resume live rung pending)",
  },
  {
    id: "H2", group: "H", capability: "Session fork", status: "missing",
    uiOracleRef: "", uiOracleWiring: "pending",
    programmaticOracleRef: "", programmaticOracleWiring: "pending",
    rung: "pending",
    blocker: "audit H2: no fork surface or seam",
  },
  {
    id: "H3", group: "H", capability: "History import / browse", status: "partial",
    uiOracleRef: HISTORY_WORKSPACE, uiOracleWiring: "existing_suite",
    programmaticOracleRef: CODEX_HISTORY, programmaticOracleWiring: "existing_suite",
    rung: "fixture",
    blocker: "audit H3: imports ~/.codex history only; no ~/.claude importer",
  },
  {
    id: "H4", group: "H", capability: "Session search", status: "partial",
    uiOracleRef: HISTORY_WORKSPACE, uiOracleWiring: "existing_suite",
    programmaticOracleRef: CODING_CATALOG, programmaticOracleWiring: "existing_suite",
    rung: "fixture",
    blocker: "audit H4: structured catalog search only; no free-text transcript search",
  },
  {
    id: "H5", group: "H", capability: "Context compaction", status: "partial",
    uiOracleRef: "", uiOracleWiring: "pending",
    programmaticOracleRef: "", programmaticOracleWiring: "pending",
    rung: "pending",
    blocker: "audit H5: SDK auto-compacts, but there is no UI marker or control and no compaction-boundary integrity harness",
  },

  // --- I. Context & inputs (all missing) ----------------------------------
  {
    id: "I1", group: "I", capability: "Image input (screenshots)", status: "missing",
    uiOracleRef: "", uiOracleWiring: "pending",
    programmaticOracleRef: "", programmaticOracleWiring: "pending",
    rung: "pending",
    blocker: "audit I1: no composer image path; main.ts disallows webview attachments; fable-local input schema carries no image block",
  },
  {
    id: "I2", group: "I", capability: "User-configured MCP servers", status: "missing",
    uiOracleRef: "", uiOracleWiring: "pending",
    programmaticOracleRef: "", programmaticOracleWiring: "pending",
    rung: "pending",
    blocker: "audit I2: only the internal delegate SDK-MCP server exists; no MCP config UI",
  },
  {
    id: "I3", group: "I", capability: "Skills / slash commands", status: "missing",
    uiOracleRef: "", uiOracleWiring: "pending",
    programmaticOracleRef: "", programmaticOracleWiring: "pending",
    rung: "pending",
    blocker: "audit I3: Skill is in FABLE_LOCAL_DISALLOWED_TOOLS and skills is []; no slash/skill invocation path",
  },
  {
    id: "I4", group: "I", capability: "File attachments / mentions", status: "missing",
    uiOracleRef: "", uiOracleWiring: "pending",
    programmaticOracleRef: "", programmaticOracleWiring: "pending",
    rung: "pending",
    blocker: "audit I4: no attachment or @-mention path in the composer",
  },

  // --- J. Interactive control ---------------------------------------------
  {
    id: "J1", group: "J", capability: "Agent asks user a question", status: "ui_available",
    uiOracleRef: SMOKE, uiOracleWiring: "smoke_step",
    programmaticOracleRef: LOCAL_HARNESS, programmaticOracleWiring: "existing_suite",
    rung: "fixture",
  },
  {
    id: "J2", group: "J", capability: "Plan mode / plan review", status: "missing",
    uiOracleRef: "", uiOracleWiring: "pending",
    programmaticOracleRef: "", programmaticOracleWiring: "pending",
    rung: "pending",
    blocker: "audit J2: EnterPlanMode/ExitPlanMode are disallowed; no plan surface",
  },
  {
    id: "J3", group: "J", capability: "Tool approval / permission modes", status: "partial",
    uiOracleRef: RUNTIME_INTERACTIONS, uiOracleWiring: "existing_suite",
    programmaticOracleRef: RUNTIME_GATEWAY, programmaticOracleWiring: "existing_suite",
    rung: "fixture",
    blocker: "audit J3: local lane is allow-all canUseTool; signed-in decideInteraction exists but there is no local permission-mode UI",
  },
  {
    id: "J4", group: "J", capability: "Task/todo progress tracking", status: "missing",
    uiOracleRef: "", uiOracleWiring: "pending",
    programmaticOracleRef: "", programmaticOracleWiring: "pending",
    rung: "pending",
    blocker: "audit J4: no todo/plan-progress rendering",
  },

  // --- K. Workspace & observability ---------------------------------------
  {
    id: "K1", group: "K", capability: "Multi-repo / workspace switching", status: "partial",
    uiOracleRef: COMMAND_HOST, uiOracleWiring: "existing_suite",
    programmaticOracleRef: COMMAND_HOST, programmaticOracleWiring: "existing_suite",
    rung: "fixture",
    blocker: "audit K1: workspace.choose exists, but only a single active workspace — no multi-workspace switching",
  },
  {
    id: "K2", group: "K", capability: "Usage / token observability", status: "ui_available",
    uiOracleRef: FLEET, uiOracleWiring: "existing_suite",
    programmaticOracleRef: EVALS, programmaticOracleWiring: "headless_wired",
    rung: "live",
  },
]

/** Count the registry by status (used by the meta-test and the coverage report). */
export const capabilityStatusCounts = (): Record<CapabilityStatus, number> => {
  const counts: Record<CapabilityStatus, number> = {
    ui_available: 0,
    programmatic_only: 0,
    partial: 0,
    missing: 0,
  }
  for (const row of capabilityRegistry) counts[row.status] += 1
  return counts
}

/** A capability oracle is "wired" (drivable/green) rather than pending. */
export const isWired = (wiring: OracleWiring): boolean => wiring !== "pending"
