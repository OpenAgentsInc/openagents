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
 * stated — contain FORTY capability rows (A1..K2) with the audit-time
 * distribution { ui_available: 15, programmatic_only: 4, partial: 13,
 * missing: 8 } (I1 and I2 have since flipped missing -> ui_available on the
 * EP250 wave-2 lanes, so the live registry is now { 17, 4, 13, 6 }). The
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
const TOOLCARDS = "apps/openagents-desktop/src/renderer/tool-cards.test.ts"
const LOCAL_HARNESS = "apps/openagents-desktop/src/renderer/local-harness.test.ts"
const MARKDOWN = "apps/openagents-desktop/src/renderer/markdown.test.ts"
const WORKSPACE = "apps/openagents-desktop/tests/workspace-service.test.ts"
const WORKSPACE_EDITOR = "apps/openagents-desktop/src/renderer/workspace-editor.test.ts"
const USAGE = "apps/openagents-desktop/src/usage-ledger.test.ts"
const FABLE_RT = "apps/openagents-desktop/src/fable-local-runtime.test.ts"
const FABLE_CAPS_RT = "apps/openagents-desktop/src/fable-local-runtime-caps.test.ts"
const RUNTIME_CARDS = "apps/openagents-desktop/src/renderer/runtime-cards.test.ts"
const CODEX_CHILD_RT = "apps/openagents-desktop/src/codex-child-runtime.test.ts"
const FLEET = "apps/openagents-desktop/src/renderer/fleet-workspace.test.ts"
const PROVIDER_ACCOUNTS = "apps/openagents-desktop/tests/provider-accounts.test.ts"
const RUNTIME_GATEWAY = "apps/openagents-desktop/tests/runtime-gateway.e2e.test.ts"
const RUNTIME_INTERACTIONS = "apps/openagents-desktop/src/renderer/runtime-interactions.test.ts"
const HISTORY_WORKSPACE = "apps/openagents-desktop/src/renderer/history-workspace.test.ts"
const CODEX_HISTORY = "apps/openagents-desktop/tests/codex-subagent-history.test.ts"
const HISTORY_ACTIONS = "apps/openagents-desktop/src/history-thread-actions.test.ts"
const COMMAND_HOST = "apps/openagents-desktop/tests/desktop-command-host.test.ts"
const SMOKE = "apps/openagents-desktop/src/main.ts"
const LIVE_PROOF = "apps/openagents-desktop/src/live-proof.ts"
const MCP_SETTINGS = "apps/openagents-desktop/src/renderer/settings.test.ts"
const MCP_HOST = "apps/openagents-desktop/src/mcp-config-host.test.ts"
const SKILL_UI = "apps/openagents-desktop/src/renderer/skill-invocation.test.ts"
const SKILL_HOST = "apps/openagents-desktop/tests/plugin-config.test.ts"
const SHELL = "apps/openagents-desktop/src/renderer/shell.test.ts"
const GIT_PANEL = "apps/openagents-desktop/src/renderer/git-panel.test.ts"
const GIT_HOST = "apps/openagents-desktop/src/git-github-host.test.ts"
const TERMINAL_HOST = "apps/openagents-desktop/src/terminal-host.test.ts"

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
  // EP250 wave-2 flips from the audit-time { 15, 4, 13, 8 } baseline:
  // I2 (user-configured MCP servers) landed settings UI + persistence host next
  // to the runtime passthrough, and I1 (image input) landed composer
  // attach/drop/paste + both-lane wiring (fixture-proven). Both flipped
  // missing -> ui_available. Then the typed Git/GitHub UI surface landed
  // (git-github-host.ts + git-panel.ts + git-review smoke step, both oracles):
  // E2 (commit/push), E4 (gh issue), and E5 (gh pr) flip programmatic_only ->
  // ui_available, taking ui_available from 17 to 20.
  // CUT-20 (#8700): D3 (interactive terminal / stdin steering) flipped
  // partial -> ui_available (typed workspace-bounded PTY host + bounded/redacted
  // terminal UI + adversarial suite + built-host/dev-preview receipts), taking
  // ui_available from 20 to 21 and partial from 16 to 15.
  // #8712 H1/H2: history now offers a typed local-thread resume picker and a
  // refs-only fork-from-here action backed by a bounded host re-read. Both
  // carry renderer and programmatic oracles, taking UI from 24 to 26. CUT-16
  // then wires I4's grant-scoped editor Mention-in-chat path through the
  // shared ChatHost boundary, taking UI to 27 and missing to zero.
  ui_available: 27,
  // The Git/GitHub UI surface emptied the programmatic_only bucket: E2/E4/E5
  // are now ui_available and E3 (worktree/branch isolation) is partial (branch
  // UI wired, worktree creation still agent-only), so programmatic_only is 0.
  programmatic_only: 0,
  // EP250 wave-2 (#8712): the renderer surfaces landed for the queued follow-up
  // (A3), steer-a-running-child (G4), and task/todo progress (J4) capabilities,
  // so G4 and J4 moved missing -> partial (A3 was already partial). Combined
  // with I2/I1 (missing -> ui_available), E3 (programmatic_only -> partial), and
  // CUT-20's D3 (partial -> ui_available): from the audit-time { 15, 4, 13, 8 }
  // baseline the live registry became { 21, 0, 15, 4 }. CUT-23 R1 then wired
  // I3's explicit slash grammar + host-validated SDK skill catalog, moving it
  // missing -> ui_available: { 22, 0, 15, 3 }.
  partial: 13,
  missing: 0,
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
    // Stop and typed current-turn steer are wired. Residual partial: provider
    // support differs by lane, while A3 is queue-until-idle rather than true
    // injection into an already-running provider turn.
    blocker: "audit A2: Stop and current-turn steer are typed and UI-driven; true mid-stream injection remains provider-dependent, while A3 is queue-until-idle",
  },
  {
    id: "A3", group: "A", capability: "Queue follow-up while turn runs", status: "partial",
    uiOracleRef: RUNTIME_CARDS, uiOracleWiring: "existing_suite",
    programmaticOracleRef: FABLE_CAPS_RT, programmaticOracleWiring: "existing_suite",
    rung: "fixture",
    // EP250 wave-2: the composer stays usable while a turn streams; a mid-turn
    // submit enqueues via fableLocal.queueFollowup, renders a queued chip, and
    // the promoted follow-up becomes the next turn. Residual (still partial):
    // delivery is queue-until-idle, not mid-stream steering (the single-string
    // turn cannot inject), and it is local-lane only.
    blocker: "audit A3: queued follow-up is queue-until-idle (delivered at turn completion), not mid-stream steering; local lane only",
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
    id: "D3", group: "D", capability: "Interactive terminal / stdin steering", status: "ui_available",
    // CUT-20 (#8700): a workspace-bounded PTY host landed (terminal-host.ts +
    // terminal-contract.ts) with a typed create/input/resize/interrupt/restart/
    // close lifecycle bound to the authorized workspace, bounded+redacted ring
    // buffers, exactly-once process-tree disposal, restart recovery, and a local
    // preview lifecycle. The renderer terminal workspace (terminal-workspace.ts,
    // mounted in the 'terminal' switch) gives a bounded text terminal with a
    // typed input line + interrupt/restart. UI oracle: the built-Electron smoke
    // routes to the terminal workspace and runs a REAL command through the real
    // PTY host (output captured, tree disposed). Programmatic oracle: the
    // adversarial suite (shell injection, secret env, runaway output, orphan
    // children with real process-tree kill, duplicate start, port collision,
    // revoked grants) + built-host + dev-preview receipts.
    uiOracleRef: SMOKE, uiOracleWiring: "smoke_step",
    programmaticOracleRef: TERMINAL_HOST, programmaticOracleWiring: "existing_suite",
    rung: "live",
    // Honest residual: the shipped backend is a child-process-group terminal
    // (real stdin steering + real tree kill, zero native deps, runs under bun
    // test AND Electron). The node-pty pseudo-TTY (line editing / colors /
    // isatty) is a documented one-file TerminalBackend swap, deferred because
    // node-pty fails to spawn under Bun and needs electron-rebuild in the #8574
    // packaging lane; a full xterm.js render is the follow-up UI enhancement.
    blocker: "audit D3: typed PTY stdin seam + bounded/redacted terminal UI landed and proven (adversarial suite + built-host + dev-preview receipts); residual — node-pty pseudo-TTY + xterm.js render deferred to the packaging lane (documented TerminalBackend swap)",
  },

  // --- E. Git & GitHub ----------------------------------------------------
  {
    id: "E1", group: "E", capability: "Repo inspection (status/diff/log)", status: "ui_available",
    uiOracleRef: LIVE_PROOF, uiOracleWiring: "live_step",
    programmaticOracleRef: EVALS, programmaticOracleWiring: "headless_wired",
    rung: "live",
  },
  {
    id: "E2", group: "E", capability: "Commit / push (fetch-rebase-push retry)", status: "ui_available",
    // EP250 (#8712): a typed Git/GitHub UI surface landed — git-panel.ts renders
    // the commit box + push button with SHA/push receipts, and git-github-host.ts
    // runs the real commit/push path against a real local bare remote (no mocks).
    uiOracleRef: GIT_PANEL, uiOracleWiring: "existing_suite",
    programmaticOracleRef: GIT_HOST, programmaticOracleWiring: "existing_suite",
    rung: "fixture",
  },
  {
    id: "E3", group: "E", capability: "Worktree / branch isolation", status: "partial",
    // EP250 (#8712): the branch list/create/checkout UI shipped (git-panel branch
    // switcher; git-github-host branch-name validation + listing). Residual gap
    // keeps this partial: worktree CREATION is still agent-only (no typed surface).
    uiOracleRef: GIT_PANEL, uiOracleWiring: "existing_suite",
    programmaticOracleRef: GIT_HOST, programmaticOracleWiring: "existing_suite",
    rung: "fixture",
    blocker: "audit E3: branch list/create/checkout UI is wired, but worktree CREATION is still agent-driven Bash only (no typed surface)",
  },
  {
    id: "E4", group: "E", capability: "GitHub issues (gh issue)", status: "ui_available",
    // EP250 (#8712): git-panel renders the issues/PRs section with a gh Create
    // affordance + issue-create URL receipt; git-github-host parses the gh issue
    // list and gates it behind the typed gh-unavailable reason.
    uiOracleRef: GIT_PANEL, uiOracleWiring: "existing_suite",
    programmaticOracleRef: GIT_HOST, programmaticOracleWiring: "existing_suite",
    rung: "fixture",
  },
  {
    id: "E5", group: "E", capability: "GitHub PRs (gh pr)", status: "ui_available",
    // EP250 (#8712): git-panel renders the issues/PRs section with the gh Create
    // affordance + receipt; git-github-host runs the gh pr path behind the typed
    // gh-availability gate.
    uiOracleRef: GIT_PANEL, uiOracleWiring: "existing_suite",
    programmaticOracleRef: GIT_HOST, programmaticOracleWiring: "existing_suite",
    rung: "fixture",
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
    id: "G4", group: "G", capability: "Steer/message running children", status: "partial",
    uiOracleRef: RUNTIME_CARDS, uiOracleWiring: "existing_suite",
    programmaticOracleRef: FABLE_CAPS_RT, programmaticOracleWiring: "existing_suite",
    rung: "fixture",
    // EP250 wave-2: a running child card offers an Interrupt control that drives
    // fableLocal.steerChild(action:"interrupt") and renders the child_steered
    // outcome. Residual (still partial): MESSAGE-ing an in-flight child is
    // capability-unsupported (codex exec is non-interactive; the SDK Agent tool
    // exposes no per-child message API), so only Interrupt is offered.
    blocker: "audit G4: interrupt-a-running-child is UI-driven; messaging an in-flight child stays unsupported (codex exec non-interactive; no per-subagent SDK message API)",
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
    id: "H1", group: "H", capability: "Resume / continuation", status: "ui_available",
    uiOracleRef: HISTORY_WORKSPACE, uiOracleWiring: "existing_suite",
    programmaticOracleRef: FABLE_RT, programmaticOracleWiring: "existing_suite",
    rung: "fixture",
  },
  {
    id: "H2", group: "H", capability: "Session fork", status: "ui_available",
    uiOracleRef: HISTORY_WORKSPACE, uiOracleWiring: "existing_suite",
    programmaticOracleRef: HISTORY_ACTIONS, programmaticOracleWiring: "existing_suite",
    rung: "fixture",
  },
  {
    id: "H3", group: "H", capability: "History import / browse", status: "partial",
    uiOracleRef: HISTORY_WORKSPACE, uiOracleWiring: "existing_suite",
    programmaticOracleRef: EVALS, programmaticOracleWiring: "headless_wired",
    rung: "fixture",
    // ~/.claude import LANDED (claude-history.ts + merged-history.ts): Claude
    // sessions surface in the SAME catalog as Codex, source-badged, with the
    // same loss-accounted completeness equation and the ~3% orphan class shown
    // as explicit topology gaps. Residual keeps this partial (not full browse):
    blocker: "audit H3: ~/.claude import landed and merged with ~/.codex (source-tagged, loss-accounted, orphan-counted); residual — Claude workflow-journal edges and async background-agent lifecycle are surfaced as gap/unknown rather than fully reconstructed",
  },
  {
    id: "H4", group: "H", capability: "Session search", status: "partial",
    uiOracleRef: HISTORY_WORKSPACE, uiOracleWiring: "existing_suite",
    programmaticOracleRef: EVALS, programmaticOracleWiring: "headless_wired",
    rung: "fixture",
    // Free-text search LANDED (history-search.ts + merged-history.ts): ranked
    // title + bounded content matching over Codex AND Claude sessions, opening
    // a content result windowed on its matching item. Residual keeps partial:
    blocker: "audit H4: free-text title + bounded content search landed (ranked by match then recency, opens at the matching item); residual — the rebuildable content index is bounded to the most-recent sessions, not the whole archive, and there is no fuzzy/semantic ranking",
  },
  {
    id: "H5", group: "H", capability: "Context compaction", status: "partial",
    uiOracleRef: HISTORY_WORKSPACE, uiOracleWiring: "existing_suite",
    programmaticOracleRef: CODEX_HISTORY, programmaticOracleWiring: "existing_suite",
    rung: "fixture",
    blocker: "audit H5: automatic Codex compaction is visible and loss-accounted across its persisted boundary; explicit user-triggered compaction remains unsupported by the pinned app-server",
  },

  // --- I. Context & inputs (all missing) ----------------------------------
  {
    id: "I1", group: "I", capability: "Image input (screenshots)", status: "ui_available",
    uiOracleRef: SMOKE, uiOracleWiring: "smoke_step",
    programmaticOracleRef: EVALS, programmaticOracleWiring: "headless_wired",
    rung: "fixture",
    // Composer image path landed in THIS lane: paperclip picker + drag-drop +
    // paste (bounded ≤8, ≤10MB, PNG/JPEG/WebP/GIF) hold base64 in the renderer,
    // thread through the additive fable-local start `images` field, and reach
    // BOTH lanes — Fable as an SDK image content block (streaming-input user
    // message), Codex as `codex exec -i <path>` files written to the turn
    // workspace. Residual partial→ui_available: proven at fixture rung (smoke
    // image-attach step + headless payload oracle); a real live provider image
    // turn is deferred to a live-proof run.
    blocker: "audit I1: image input now UI-driven via composer attach/drop/paste; Fable sends SDK base64 image blocks, Codex passes `-i <path>`; live provider image turn deferred to live-proof",
  },
  {
    // Landed EP250 wave-2: the MCP-config SETTINGS UI + persistence host now
    // exist alongside the runtime passthrough, so I2 flips off `missing`.
    id: "I2", group: "I", capability: "User-configured MCP servers", status: "ui_available",
    uiOracleRef: MCP_SETTINGS, uiOracleWiring: "existing_suite",
    programmaticOracleRef: MCP_HOST, programmaticOracleWiring: "existing_suite",
    rung: "fixture",
  },
  {
    id: "I3", group: "I", capability: "Skills / slash commands", status: "ui_available",
    uiOracleRef: SKILL_UI, uiOracleWiring: "existing_suite",
    programmaticOracleRef: SKILL_HOST, programmaticOracleWiring: "existing_suite",
    rung: "fixture",
  },
  {
    id: "I4", group: "I", capability: "File attachments / mentions", status: "ui_available",
    uiOracleRef: WORKSPACE_EDITOR, uiOracleWiring: "existing_suite",
    programmaticOracleRef: SHELL, programmaticOracleWiring: "existing_suite",
    rung: "fixture",
  },

  // --- J. Interactive control ---------------------------------------------
  {
    id: "J1", group: "J", capability: "Agent asks user a question", status: "ui_available",
    uiOracleRef: SMOKE, uiOracleWiring: "smoke_step",
    programmaticOracleRef: LOCAL_HARNESS, programmaticOracleWiring: "existing_suite",
    rung: "fixture",
  },
  {
    id: "J2", group: "J", capability: "Plan mode / plan review", status: "ui_available",
    uiOracleRef: SHELL, uiOracleWiring: "existing_suite",
    programmaticOracleRef: FABLE_CAPS_RT, programmaticOracleWiring: "existing_suite",
    rung: "fixture",
  },
  {
    id: "J3", group: "J", capability: "Tool approval / permission modes", status: "ui_available",
    uiOracleRef: SHELL, uiOracleWiring: "existing_suite",
    programmaticOracleRef: RUNTIME_GATEWAY, programmaticOracleWiring: "existing_suite",
    rung: "fixture",
  },
  {
    id: "J4", group: "J", capability: "Task/todo progress tracking", status: "partial",
    uiOracleRef: RUNTIME_CARDS, uiOracleWiring: "existing_suite",
    programmaticOracleRef: FABLE_CAPS_RT, programmaticOracleWiring: "existing_suite",
    rung: "fixture",
    // EP250 wave-2: TodoWrite plan_updated events render a compact task-progress
    // card (status glyphs from the exact enum) that updates in place live, and
    // the final plan state persists into the finalized transcript. Residual
    // (still partial): the J2 plan-mode toggle/review is not built.
    blocker: "audit J4: live-updating plan/todo card renders and the final plan state persists; the J2 plan-mode toggle/review is unbuilt (shipped as residual)",
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
