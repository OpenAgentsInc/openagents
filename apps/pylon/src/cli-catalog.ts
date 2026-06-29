// Machine-readable command catalog (issue #5035). `pylon help --json` emits
// this so an agent can DISCOVER the full steering surface — every command, its
// args, and whether it mutates state or spends money — the headless equivalent
// of "seeing the GUI". `pylon <cmd> --help` projects a single entry.
//
// `mutates` = changes node/server/remote state. `spends` = can move money
// (sats). A CLI verb is a control surface only; `spends` flags the wallet
// send/payout paths so an agent can reason about authority before invoking.

export type PylonCommandArg = {
  name: string
  required: boolean
  description: string
  // A repeatable option (e.g. --capability-ref) or a positional.
  kind?: "flag" | "option" | "positional"
}

export type PylonCommandEntry = {
  command: string
  summary: string
  mutates: boolean
  spends: boolean
  // Needs a running `pylon node` over the loopback control API.
  needsNode?: boolean
  // Hits the openagents.com network API (needs --base-url / PYLON_OPENAGENTS_BASE_URL).
  needsNetwork?: boolean
  json: boolean
  args: PylonCommandArg[]
}

const opt = (name: string, description: string, required = false): PylonCommandArg => ({
  name,
  required,
  description,
  kind: "option",
})
const pos = (name: string, description: string, required = true): PylonCommandArg => ({
  name,
  required,
  description,
  kind: "positional",
})
const flag = (name: string, description: string): PylonCommandArg => ({
  name,
  required: false,
  description,
  kind: "flag",
})

export const PYLON_COMMAND_CATALOG: readonly PylonCommandEntry[] = [
  {
    command: "help",
    summary: "Print the machine-readable command catalog.",
    mutates: false,
    spends: false,
    json: true,
    args: [flag("--json", "Emit the full catalog as JSON.")],
  },
  {
    command: "bootstrap",
    summary: "Write Pylon local state + identity and project status.",
    mutates: true,
    spends: false,
    json: true,
    args: [flag("--json", "Emit JSON."), opt("--pylon-ref", "Stable Pylon ref."), opt("--display-name", "Display name.")],
  },
  {
    command: "auth",
    summary: "Connect OpenAgents and Codex accounts with the minimum device-login flow.",
    mutates: true,
    spends: false,
    needsNetwork: true,
    json: true,
    args: [
      pos("openagents|codex", "Target to connect. codex also ensures the OpenAgents Pylon link first."),
      opt("--account", "codex: stable local account ref. Omit to use codex, codex-2, codex-3, ... automatically."),
      opt("--agent-token", "Optional existing OpenAgents agent token. Stored locally with private file permissions."),
      opt("--base-url", "OpenAgents base URL."),
      opt("--timeout-seconds", "Maximum time to wait for device confirmations."),
      flag("--force-device-login", "codex: run Codex device auth even when that account home already has auth.json."),
      flag("--json", "Emit JSON instead of only verification URL/code lines."),
    ],
  },
  {
    command: "status",
    summary: "Project public node status (identity, inventory, psionic); read-only, never binds the control port.",
    mutates: false,
    spends: false,
    json: true,
    args: [
      flag("--json", "Emit JSON."),
      flag("--remote", "Read a running node over the control API (error if none reachable)."),
      flag("--connect", "Alias for --remote."),
    ],
  },
  {
    command: "doctor",
    summary: "Read-only health diagnostic: resolved node home + source, seed presence, running-node + wallet state. Never binds the control port.",
    mutates: false,
    spends: false,
    json: true,
    args: [
      flag("--remote", "Read a running node over the control API (error if none reachable)."),
      flag("--connect", "Alias for --remote."),
    ],
  },
  {
    command: "inventory",
    summary: "Project discovered host inventory (backends, platform).",
    mutates: false,
    spends: false,
    json: true,
    args: [flag("--json", "Emit JSON (required).")],
  },
  {
    command: "accounts",
    summary: "Connect, list, or inspect local provider accounts.",
    mutates: true,
    spends: false,
    json: true,
    args: [
      pos("connect|list|usage|status", "Subcommand."),
      pos("codex", "connect: provider to connect.", false),
      opt("--account", "connect/usage/status: stable local account ref."),
      opt("--account-label", "connect --openagents-link: provider-account label."),
      opt("--agent-token", "connect --openagents-link: OpenAgents agent token (or OPENAGENTS_AGENT_TOKEN)."),
      opt("--base-url", "connect --openagents-link: OpenAgents base URL."),
      opt("--home", "connect: existing or derived local Codex home."),
      opt("--openagents-attempt-id", "connect: poll an OpenAgents-linked Codex device-login attempt."),
      opt("--provider-account-ref", "connect --openagents-link: reconnect an existing OpenAgents provider account ref."),
      flag("--force-device-login", "connect: run Codex device auth even when auth.json already exists."),
      flag("--openagents-link", "connect: start the OpenAgents-linked Codex provider-account device flow with the Pylon agent token."),
      flag("--skip-device-login", "connect: only register the home; do not run Codex device auth."),
      flag("--json", "Emit JSON (required)."),
      flag("--refresh", "usage: refresh provider snapshots."),
      flag("--reset", "status: consume one manual quota reset for the selected account."),
    ],
  },
  {
    command: "codex",
    summary: "Codex account namespace alias; `codex accounts list --json` plus local fleet offload planning.",
    mutates: true,
    spends: false,
    json: true,
    args: [
      pos("accounts", "Codex account sub-namespace; `fleet offload-plan` is also supported."),
      pos("list|usage|status|connect|offload-plan", "Account command or fleet offload planner."),
      opt("--account", "connect/usage/status: stable local Codex account ref."),
      opt("--accounts", "fleet offload-plan: comma-separated Codex account refs to move."),
      opt("--base-url", "connect --openagents-link: OpenAgents base URL."),
      opt("--bundle-dir", "fleet offload-plan: local directory for tar bundles."),
      opt("--home", "connect: existing or derived local Codex home."),
      opt("--remote-home", "fleet offload-plan: target PYLON_HOME on each host."),
      opt("--remote-repo", "fleet offload-plan: target openagents checkout on each host."),
      opt("--target", "fleet offload-plan: Tailnet host capacity, e.g. imac-pro-bertha:2."),
      flag("--json", "Emit JSON."),
      flag("--refresh", "usage: refresh provider snapshots."),
      flag("--reset", "status: consume one manual quota reset for the selected account."),
      flag("--include-private-paths", "fleet offload-plan: include local tar/scp/launch commands with private paths."),
    ],
  },
  {
    command: "balance",
    summary: "Read projection-safe wallet/earnings balance (never seed/offers).",
    mutates: false,
    spends: false,
    needsNetwork: true,
    json: true,
    args: [opt("--base-url", "OpenAgents base URL.")],
  },
  {
    command: "wallet",
    summary: "Spark-primary wallet status plus legacy receive/send/payout-target commands.",
    mutates: true,
    spends: true,
    needsNetwork: false,
    json: true,
    args: [
      pos("status|receive|send|admit-payout-target|register-payout-target|request-payout-target-admission|report-readiness|migrate-spark|recover-mdk|backup-receive|backup-status|backup-claim|spark-selftest", "Subcommand."),
      opt("--amount", "send/receive amount in sats."),
      opt("--rail", "send rail: mdk (legacy default) or spark."),
      opt("--destination-ref", "Legacy MDK send destination ref."),
      opt("--destination", "Spark send raw BOLT11/Spark payment request or Lightning Address (LOCAL/PRIVATE input)."),
      opt("--payment-request", "Spark send raw payment request alias (LOCAL/PRIVATE input)."),
      opt("--lightning-address", "Spark send Lightning Address alias (LOCAL/PRIVATE input)."),
      opt("--kind", "payout-target kind; register-payout-target kind (spark-address); or backup-receive kind (spark-address|lightning-address)."),
      opt("--ref", "payout-target ref."),
      flag("--confirm-send", "wallet send --rail spark: explicit owner consent to spend Spark balance."),
      flag("--show-local-target", "backup-receive/backup-status: print the raw local Spark target (LOCAL/PRIVATE only)."),
      flag("--sweep", "migrate-spark: dry-run reconcile of OWN received Spark backup funds (no consent, no movement)."),
      flag("--confirm-sweep", "migrate-spark: explicit consent to sweep OWN received Spark backup funds into OWN MDK wallet (receive-side reconcile, NOT a payout)."),
      flag("--destination-ready", "migrate-spark --confirm-sweep: assert the MDK destination is ready to receive the swept funds."),
      flag("--execute", "recover-mdk/migrate-spark: leave dry-run mode after reviewing the local recovery plan."),
      flag("--yes", "recover-mdk/migrate-spark: explicit owner consent for the local recovery operation."),
    ],
  },
  {
    command: "sessions",
    summary: "List/spawn/reply/batch/exec/cancel coding sessions on the running node (control API).",
    mutates: true,
    spends: false,
    needsNode: true,
    json: true,
    args: [
      pos("list|spawn|reply|batch|exec|cancel", "Subcommand. reply = continuation turn; batch = capped fan-out; exec = blocking run-to-completion one-shot."),
      opt("--adapter", "spawn/exec: codex|claude_agent."),
      opt("--objective", "spawn/exec/reply: objective text."),
      opt("--verify", "spawn/exec: verification command (repeatable for exec)."),
      opt("--worktree", "spawn/exec/batch: worktree path."),
      flag("--managed-worktree", "spawn/exec/batch: create a Pylon-managed isolated worktree from a git base ref."),
      opt("--repo", "spawn/exec/batch --managed-worktree: GitHub owner/name (defaults from origin)."),
      opt("--base-ref", "spawn/exec/batch --managed-worktree: git ref to materialize (default origin/main)."),
      opt("--lane", "spawn/exec/batch: execution lane auto|local|cloud-gcp|cloud-shc."),
      opt("--tasks", "batch: JSON file containing task strings or task objects."),
      opt("--concurrency", "batch: maximum concurrent sessions (default 2)."),
      opt("--on-approval", "exec: manual|deny|auto (default manual; auto = BOUNDED owner-local auto-approve, audited)."),
      opt("--approval-policy", "exec: alias for --on-approval (manual|deny|auto)."),
      opt("--max-auto-approvals", "exec: cap on auto-approvals before escalating (auto policy; default 50)."),
      opt("--auto-window-seconds", "exec: wall-clock bound for auto-approve before escalating (auto policy; default 1800)."),
      opt("--auto-out-of-bounds", "exec: escalate|deny for out-of-bounds approvals (auto policy; default escalate)."),
      opt("--timeout-seconds", "reply/exec: bound the session + driver (default 600)."),
      opt("--session-ref", "reply/cancel: session ref."),
      flag("--wait", "reply: wait for the continuation session to reach a terminal state."),
    ],
  },
  {
    command: "approvals",
    summary: "List/approve/deny the node's pending operator approval queue.",
    mutates: true,
    spends: false,
    needsNode: true,
    json: true,
    args: [
      pos("list|approve|deny", "Subcommand."),
      opt("--approval-ref", "approve/deny: approval ref."),
      opt("--answer", "answer text (for answer decisions)."),
    ],
  },
  {
    command: "vmq",
    summary: "Plan Pylon virtual merge queue supervisor operations from local JSON inputs.",
    mutates: false,
    spends: false,
    json: true,
    args: [
      pos("pr-fast-forward-plan", "Subcommand."),
      opt("--projection", "Path to a virtual merge queue projection JSON file.", true),
      opt("--request", "Path to a PR fast-forward request JSON file.", true),
    ],
  },
  {
    command: "deploy",
    summary: "Trigger/inspect a node cloud deploy (gated by OA_DEPLOY_ENABLE=1).",
    mutates: true,
    spends: false,
    needsNode: true,
    json: true,
    args: [
      pos("cloud|status", "Subcommand."),
      opt("--target", "cloud: deploy target."),
      opt("--ref", "cloud: ref to deploy."),
      opt("--env", "cloud: deploy environment."),
    ],
  },
  {
    command: "training",
    summary:
      "Drive the training cockpit lane (preflight/plan/activate/claim/admit/reconcile/closeout/status/submit-trace/validate).",
    mutates: true,
    spends: false,
    needsNetwork: true,
    json: true,
    args: [
      pos(
        "preflight|plan|activate|claim|admit|reconcile|closeout|status|submit-trace|validate",
        "Subcommand.",
      ),
      opt("--base-url", "OpenAgents base URL."),
      opt("--admin-token", "Admin token (or OA_TRAINING_ADMIN_TOKEN) for admin verbs."),
      opt(
        "--agent-token",
        "Agent token (or OPENAGENTS_AGENT_TOKEN) for submit-trace/validate.",
      ),
      opt("--window-ref", "activate/reconcile/closeout: window ref."),
      opt("--run-ref", "admit: training run ref."),
      opt("--pylon-ref", "claim: pylon ref."),
      opt("--packet", "admit: evidence packet JSON path."),
      opt("--lease-ref", "submit-trace/validate: claimed training window lease ref."),
      opt(
        "--workload",
        "submit-trace/validate: dispatch workload JSON path (auto-discovery pending #5053).",
      ),
      opt(
        "--workload-family",
        "submit-trace/validate: article_closeout|sudoku_trace|hungarian_trace|kernel_trace.",
      ),
      opt(
        "--device-ref",
        "submit-trace/validate: device ref (defaults to the local node id).",
      ),
      opt("--assignment-ref", "submit-trace: optional assignment ref."),
    ],
  },
  {
    command: "activity",
    summary: "Read/tail the public OpenAgents activity timeline for agent retrieval.",
    mutates: false,
    spends: false,
    needsNetwork: true,
    json: true,
    args: [
      flag("--watch", "Poll the public timeline repeatedly."),
      flag("--json", "Emit JSON."),
      opt("--since", "Timeline cursor to resume from."),
      opt("--filter", "Comma-separated event family filter: work, verify, settle, pylon, forum, artanis, capacity, gap, trace, window."),
      opt("--kind", "Comma-separated concrete event kind filter."),
      opt("--source", "Comma-separated source family filter."),
      opt("--limit", "Maximum events per timeline page."),
      opt("--max-iterations", "Bound --watch polling iterations for scripts/tests."),
      opt("--interval-ms", "Delay between --watch polls."),
      opt("--base-url", "OpenAgents base URL."),
    ],
  },
  {
    command: "timeline",
    summary: "Fetch a bounded public activity timeline range.",
    mutates: false,
    spends: false,
    needsNetwork: true,
    json: true,
    args: [
      opt("--from", "Inclusive ISO timestamp lower bound.", true),
      opt("--to", "Inclusive ISO timestamp upper bound.", true),
      flag("--json", "Emit JSON."),
      opt("--kind", "Comma-separated concrete event kind filter."),
      opt("--source", "Comma-separated source family filter."),
      opt("--limit", "Maximum events per page."),
      opt("--base-url", "OpenAgents base URL."),
    ],
  },
  {
    command: "replay",
    summary: "Fetch a generated public activity replay bundle and print an agent-readable event track.",
    mutates: false,
    spends: false,
    needsNetwork: true,
    json: true,
    args: [
      opt("--from", "Inclusive ISO timestamp lower bound for generated replay.", true),
      opt("--to", "Inclusive ISO timestamp upper bound for generated replay.", true),
      opt("--run", "Filter generated replay to a training run ref."),
      opt("--window", "Filter generated replay to a training window ref."),
      opt("--actor", "Filter generated replay to an actor ref; comma-separated values accepted."),
      opt("--pair", "Filter generated replay to two actor refs separated by comma, colon, or plus."),
      opt("--kind", "Comma-separated concrete event kind filter."),
      opt("--filter", "Comma-separated event family filter: work, verify, settle, pylon, forum, artanis, capacity, gap, trace, window."),
      opt("--source", "Comma-separated source family filter."),
      opt("--since", "Timeline cursor to resume from inside the bounded range."),
      opt("--limit", "Maximum source timeline events for the generated bundle."),
      opt("--format", "text or json."),
      flag("--json", "Emit JSON; equivalent to --format json."),
      opt("--base-url", "OpenAgents base URL."),
    ],
  },
  {
    command: "multi-earning",
    summary:
      "Project the local cross-mode earning ledger (settled receipts across >=2 modes). INERT unless PYLON_MULTI_EARNING_LEDGER_ENABLED=1.",
    mutates: false,
    spends: false,
    needsNetwork: false,
    json: true,
    args: [
      pos("subcommand", "ledger (the only subcommand).", false),
      flag("--json", "Emit JSON; this command is JSON-only."),
    ],
  },
  {
    command: "receipts",
    summary: "Fetch public settlement receipts for a training run.",
    mutates: false,
    spends: false,
    needsNetwork: true,
    json: true,
    args: [
      opt("--run", "Training run ref.", true),
      flag("--json", "Emit JSON."),
      opt("--base-url", "OpenAgents base URL."),
    ],
  },
  {
    command: "evidence-pack",
    summary: "Fetch agent-readable public evidence for a training run.",
    mutates: false,
    spends: false,
    needsNetwork: true,
    json: true,
    args: [
      opt("--run", "Training run ref.", true),
      flag("--json", "Emit JSON."),
      opt("--challenge-ref", "Verification challenge ref to include."),
      opt("--replay-ref", "Proof replay ref to include."),
      opt("--base-url", "OpenAgents base URL."),
    ],
  },
  {
    command: "khala",
    summary: "Issue/resume/status/proof Khala requests and run roadmap burndown through linked Pylon capacity.",
    mutates: true,
    spends: false,
    needsNetwork: true,
    json: true,
    args: [
      pos("request|resume|status|proof|burndown", "Subcommand."),
      opt("--prompt", "request: prompt text for openagents/khala."),
      opt("--objective", "request: objective text alias for --prompt."),
      opt("--workflow", "request: typed workflow class claude_agent_task|cloud_coding_session|codex_agent_task."),
      opt("--pylon-ref", "request: target a specific caller-owned Pylon ref."),
      opt("--target-pylon-ref", "request: alias for --pylon-ref."),
      flag("--fixture", "request: explicit codex_agent_task fixture smoke intent."),
      opt("--repo", "request: public GitHub owner/repo for workspace-backed codex_agent_task work."),
      opt("--commit", "request: pinned 40-character commit SHA for workspace-backed codex_agent_task work."),
      opt("--verify", "request: bounded verification argv for workspace-backed codex_agent_task work."),
      opt("--issues", "burndown: comma-separated GitHub issue numbers to assign."),
      opt("--roadmap", "burndown: roadmap markdown file to parse for active issues."),
      opt("--max-parallel", "burndown: maximum concurrent ready Codex accounts to use."),
      opt("--iterations", "burndown: refill rounds to run."),
      flag("--execute", "burndown: dispatch, run, and proof assignments; omitted means dry-run plan."),
      opt("--resume", "request/resume: durable request id to resume."),
      opt("--assignment-ref", "proof: assignment ref to resolve."),
      opt("--offset", "resume/status: durable byte offset."),
      opt("--base-url", "OpenAgents base URL."),
      opt("--agent-token", "Agent token or OPENAGENTS_AGENT_TOKEN."),
      flag("--json", "Emit JSON."),
    ],
  },
  {
    command: "mcp",
    summary: "Run the Khala MCP stdio server or emit local/remote MCP config.",
    mutates: true,
    spends: false,
    needsNetwork: true,
    json: true,
    args: [
      pos("config", "Optional subcommand to emit MCP config instead of running stdio.", false),
      opt("--base-url", "OpenAgents base URL."),
      opt("--agent-token", "Agent token or OPENAGENTS_AGENT_TOKEN."),
      opt("--command", "config: local pylon command/path for stdio clients."),
      flag("--json", "config: emit JSON."),
    ],
  },
  {
    command: "assignment",
    summary: "Poll/accept/progress/closeout OpenAgents assignments.",
    mutates: true,
    spends: false,
    needsNetwork: true,
    json: true,
    args: [
      pos("poll|accept|progress|closeout|run-no-spend", "Subcommand."),
      opt("--account", "run-no-spend: Codex account ref to use for codex_agent_task leases."),
      opt("--account-home", "run-no-spend: direct Codex home path when no registered account ref is used."),
      opt("--assignment-ref", "run-no-spend: accept only this assignment ref; prevents stale leases from being claimed."),
      opt("--base-url", "OpenAgents base URL."),
      opt("--lease-ref", "run-no-spend: alias for --assignment-ref."),
      flag("--json", "Emit final JSON on stdout; run-no-spend lifecycle JSONL is emitted on stderr."),
    ],
  },
  {
    command: "work",
    summary: "Submit/review/request/accept Autopilot work + read status.",
    mutates: true,
    spends: true,
    needsNetwork: true,
    json: true,
    args: [pos("submit|status|review|request|offers|accept", "Subcommand."), opt("--base-url", "OpenAgents base URL.")],
  },
  {
    command: "tip",
    summary: "Tip a forum post (spends sats).",
    mutates: true,
    spends: true,
    needsNetwork: true,
    json: true,
    args: [pos("<post-id>", "Post id."), pos("<sats>", "Amount in sats."), opt("--base-url", "OpenAgents base URL.")],
  },
  {
    command: "tip-prefs",
    summary: "Read/update sweep + credit tip preferences.",
    mutates: true,
    spends: false,
    needsNetwork: true,
    json: true,
    args: [opt("--sweep-enabled", "true|false."), opt("--sweep-threshold", "sats.")],
  },
  {
    command: "sweep-status",
    summary: "Read tip sweep status.",
    mutates: false,
    spends: false,
    needsNetwork: true,
    json: true,
    args: [opt("--base-url", "OpenAgents base URL.")],
  },
  {
    command: "claim-tip-readiness",
    summary: "Claim Forum tip-recipient readiness with a fresh offer.",
    mutates: true,
    spends: false,
    needsNetwork: true,
    json: true,
    args: [opt("--base-url", "OpenAgents base URL.")],
  },
  {
    command: "presence",
    summary: "Register/heartbeat/link the node with OpenAgents presence.",
    mutates: true,
    spends: false,
    needsNetwork: true,
    json: true,
    args: [
      pos("register|heartbeat|link-complete|link-refresh", "Subcommand."),
      opt("--agent-token", "OpenAgents agent token or OPENAGENTS_AGENT_TOKEN."),
      opt("--base-url", "OpenAgents base URL."),
      flag("--json", "Emit JSON."),
      flag("--wallet-probe", "presence heartbeat: include a live wallet readiness probe; omitted by default so one-shot heartbeats exit cleanly."),
    ],
  },
  {
    command: "provider",
    summary: "Go online/offline, approve labor, or run one provider loop.",
    mutates: true,
    spends: false,
    json: true,
    args: [pos("go-online|go-offline|approve-labor|once", "Subcommand.")],
  },
  {
    command: "context",
    summary: "Project the Pylon context (cwd, agent config).",
    mutates: false,
    spends: false,
    json: true,
    args: [flag("--json", "Emit JSON (required)."), flag("--codex-danger", "Include codex danger context.")],
  },
  {
    command: "operator",
    summary: "Project the operator snapshot (inventory + wallet).",
    mutates: false,
    spends: false,
    json: true,
    args: [pos("snapshot", "Subcommand."), flag("--json", "Emit JSON (required).")],
  },
  {
    command: "psionic",
    summary: "Install/doctor/smoke the Psionic backend.",
    mutates: true,
    spends: false,
    json: true,
    args: [pos("install|doctor|smoke|models", "Subcommand.")],
  },
  {
    command: "tassadar cpu-transform-training",
    summary:
      "Run the bounded Tassadar CPU-transform training fixture and print its public-safe receipt.",
    mutates: false,
    spends: false,
    needsNetwork: false,
    json: true,
    args: [pos("cpu-transform-training", "Bounded fixture command.")],
  },
  {
    command: "forum",
    summary: "Read/post/reply on the OpenAgents Forum.",
    mutates: true,
    spends: false,
    needsNetwork: true,
    json: true,
    args: [pos("read|post|reply", "Subcommand."), opt("--base-url", "OpenAgents base URL.")],
  },
  {
    command: "ask-artanis",
    summary: "Compose + post a device question to the Forum.",
    mutates: true,
    spends: false,
    needsNetwork: true,
    json: true,
    args: [pos("<question>", "Question text."), opt("--base-url", "OpenAgents base URL.")],
  },
  {
    command: "memories",
    summary: "Read the local Pylon memory log.",
    mutates: false,
    spends: false,
    json: true,
    args: [],
  },
  {
    command: "dev",
    summary: "Dev loop: doctor/check/apply/reload.",
    mutates: true,
    spends: false,
    json: true,
    args: [pos("doctor|check|apply|reload", "Subcommand."), flag("--json", "Emit JSON (required).")],
  },
  {
    command: "node",
    summary: "Run the headless node: services + event stream + loopback control API.",
    mutates: true,
    spends: false,
    json: false,
    args: [flag("--verbose", "Verbose service logging.")],
  },
  {
    command: "runtime",
    summary: "Forward to the bundled Probe runtime CLI.",
    mutates: true,
    spends: false,
    json: false,
    args: [pos("<probe-args...>", "Forwarded args.")],
  },
]

export function projectCommandCatalog(): {
  schema: string
  generatedAt: string
  commandCount: number
  commands: readonly PylonCommandEntry[]
} {
  return {
    schema: "openagents.pylon.command_catalog.v1",
    generatedAt: new Date().toISOString(),
    commandCount: PYLON_COMMAND_CATALOG.length,
    commands: PYLON_COMMAND_CATALOG,
  }
}

export function findCommandEntry(command: string): PylonCommandEntry | undefined {
  return PYLON_COMMAND_CATALOG.find((entry) => entry.command === command)
}

// Returns the help projection for a single command, or null if unknown.
export function projectCommandHelp(command: string): PylonCommandEntry | null {
  return findCommandEntry(command) ?? null
}
