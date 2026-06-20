// W-1 (#5377, EPIC #5376): a headless run-to-completion task primitive for the
// Pylon CLI. `pylon sessions exec` is the blocking one-shot that replaces a
// Claude Code subagent call in a script: spawn a coding session, drive its turn
// loop to a terminal state by polling the EXISTING control verbs
// (session.spawn / session.list / session.events / session.artifact), and
// return a structured, projection-safe JSON result with the final state, the
// result/summary text, the changeset, the verify outcome, and the artifact
// refs. Exit 0 on a success-terminal, non-zero on failure/timeout.
//
// This is a THIN composition over the loopback control API the headless node
// already owns. It introduces no new wire verb, no new authority, and no new
// money/spend surface — it only spawns + observes. Bounded AUTO-approve is left
// to W-3 (#5379); here we surface pending approvals and support an
// `--on-approval=manual|deny` policy via an injectable callback so W-3 can plug
// a real policy in without changing this driver. W-2 (#5378, reply/steer) plugs
// in the same way: an additional control verb invoked between polls.

import type {
  ControlSessionEvent,
  ControlSessionProjection,
  ControlSessionSpawnCommand,
  ControlSessionState,
} from "./control-sessions.js"

// The bounded subset of the control surface this driver composes over. Each is
// the SAME function the running node exposes (and the desktop GUI drives), so
// tests can pass `createControlSessionActions(...)` directly and the CLI can
// pass thin `runControlCommand` wrappers.
export type SessionsExecControl = {
  spawn: (command: {
    type: "session.spawn"
    adapter: "codex" | "claude_agent"
    lane?: ControlSessionSpawnCommand["lane"]
    objective: string
    repoRef?: ControlSessionSpawnCommand["repoRef"]
    verify: string[]
    worktreePath?: string
    timeoutSeconds?: number
  }) => Promise<{ sessionRef: string; state: ControlSessionState }>
  list: () => Promise<ControlSessionProjection[]>
  events: (sessionRef: string) => Promise<{
    sessionRef: string
    eventsPath: string
    state: ControlSessionState
    recentEvents: ControlSessionEvent[]
  }>
  artifact: (sessionRef: string) => Promise<{
    sessionRef: string
    kind: "proof" | "failure" | "none"
    artifact: unknown | null
  }>
  // Optional: surfacing pending operator approvals so the driver can pause /
  // report instead of silently blocking forever (no node-side queue means an
  // empty list). W-3 (#5379) replaces the policy below with bounded
  // auto-approve; it does NOT belong here.
  approvalsList?: () => Promise<{ approvals: PendingApprovalSummary[] }>
  // Optional exactly-once approval resolver owned by the running node. The
  // driver may decide approve/deny, but the node still owns applying that
  // decision to its approval queue.
  approvalsResolve?: (
    approvalRef: string,
    decision: Extract<ApprovalDecision, "approve" | "deny">,
  ) => Promise<unknown>
}

export type PendingApprovalSummary = {
  approvalRef: string
  kind: string
  [key: string]: unknown
}

// W-3 (#5379) plug point. The driver consults this policy whenever it observes
// a pending approval. Default `manual` pauses and reports; `deny` records a
// blocked outcome; `auto` plugs the bounded auto-approve policy
// (`createBoundedAutoApprovalPolicy`) into the `approvalPolicy` callback below,
// which returns `approve` ONLY for allow-listed, in-scope, in-bounds approvals
// and escalates/denies everything else — WITHOUT this driver gaining any
// blanket bypass. The `auto` lane is owner-local / headless-OA dogfood only.
export type ApprovalPolicy = "manual" | "deny" | "auto"
export type ApprovalDecision = "pause" | "deny" | "approve"
export type ApprovalPolicyCallback = (
  approval: PendingApprovalSummary,
) => ApprovalDecision | Promise<ApprovalDecision>

export type SessionsExecOptions = {
  adapter: "codex" | "claude_agent"
  lane?: ControlSessionSpawnCommand["lane"]
  objective: string
  repoRef?: ControlSessionSpawnCommand["repoRef"]
  verify: string[]
  worktreePath?: string
  timeoutSeconds?: number
  onApproval?: ApprovalPolicy
  // W-3 plug point: an explicit callback overrides `onApproval`. W-3 wires a
  // bounded auto-approve callback through this field.
  approvalPolicy?: ApprovalPolicyCallback
  // W-3 audit accessor: when the auto policy is in use, the driver reads this
  // after the loop to attach the full per-approval audit trail
  // (`autoApprovals[]`) to the result. The records are refs + stable reasons
  // only — projection-safe, no raw command/path/prompt text.
  approvalAudit?: () => SessionsExecResult["autoApprovals"]
  // Polling cadence + wall-clock bound for the whole exec. `timeoutSeconds` is
  // forwarded to the session itself (the executor's own timeout); `deadlineMs`
  // bounds the DRIVER's polling so a wedged node cannot hang the CLI forever.
  pollIntervalMs?: number
  deadlineMs?: number
  now?: () => number
  sleep?: (ms: number) => Promise<void>
}

export type SessionsExecVerifyCommand = {
  commandRef: string
  exitCode: number | null
  status: string
}

export type SessionsExecResult = {
  schema: "openagents.pylon.sessions_exec_result.v0.1"
  ok: boolean
  // The terminal-or-timeout outcome. `timeout` is distinct from `failed` so a
  // caller can retry vs. treat as a real verification/execution failure.
  outcome: "completed" | "failed" | "cancelled" | "timeout" | "approval_required"
  sessionRef: string
  adapter: "codex" | "claude_agent" | string
  state: ControlSessionState
  // Bounded, redaction-scanned human-readable summary of the last agent action
  // (from the session's own event tail). Never raw secrets/seeds — the control
  // session emits already redaction-scanned `messageText`.
  resultSummary: string | null
  resultRef: string | null
  artifactRef: string | null
  // The changeset/diff the session produced, taken from the retained proof
  // artifact's dev-check change summary (refs only — no raw file contents).
  changeset: {
    state: string
    changedFileRefs: unknown[]
    areaRefs: unknown[]
  } | null
  // The verify outcome: whether the dev-check passed and the per-command result
  // refs (exit code + status), reused from the same artifact.
  verify: {
    passed: boolean
    state: string
    commands: SessionsExecVerifyCommand[]
  } | null
  errorClass: string | null
  errorDigestRef: string | null
  // Pending approvals observed while driving the loop, plus the policy decision
  // taken. Empty unless the node surfaced an approval. W-3's `auto` policy turns
  // selected (allow-listed, in-scope, in-bounds) entries into `approve`.
  pendingApprovals: Array<{ approvalRef: string; kind: string; decision: ApprovalDecision }>
  // W-3 (#5379) audit trail for the BOUNDED auto-approve policy. One entry per
  // approval the auto policy decided on, carrying the approval ref, kind, the
  // resolved bounded category (`allow`/`escalate`/`deny`), the decision, and a
  // stable reason ref. Empty unless `--on-approval auto` (or an equivalent
  // `--approval-policy`) is selected. Receipt-first: the autonomous run leaves a
  // dereferenceable approval trail here and in the session record. Refs + reason
  // enums only — never raw command/path/prompt text.
  autoApprovals: Array<{
    approvalRef: string
    kind: string
    category: "allow" | "escalate" | "deny"
    decision: ApprovalDecision
    reason: string
  }>
  startedAt: string | null
  completedAt: string | null
  // Diagnostics: how long the driver waited and how many polls it took.
  driver: { elapsedMs: number; polls: number; timedOut: boolean }
}

const RESULT_SCHEMA = "openagents.pylon.sessions_exec_result.v0.1" as const

const TERMINAL_STATES: ReadonlyArray<ControlSessionState> = ["completed", "failed", "cancelled"]

function isTerminal(state: ControlSessionState): boolean {
  return TERMINAL_STATES.includes(state)
}

function defaultSleep(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms))
}

// Pull the latest human-readable activity from the session's event tail. Each
// event's `messageText` was already redaction-scanned by the control session
// before it was published, so this is projection-safe.
function summaryFromEvents(events: ControlSessionEvent[]): string | null {
  for (let i = events.length - 1; i >= 0; i -= 1) {
    const text = events[i]?.messageText
    if (typeof text === "string" && text.length > 0) return text.slice(0, 2000)
  }
  return null
}

// Extract the changeset + verify outcome from the retained proof/failure
// artifact. The artifact was assembled + redaction-scanned at write time, so we
// only re-shape its existing fields here.
function changesetAndVerifyFromArtifact(artifact: unknown): {
  changeset: SessionsExecResult["changeset"]
  verify: SessionsExecResult["verify"]
} {
  if (artifact === null || typeof artifact !== "object") return { changeset: null, verify: null }
  const record = artifact as Record<string, unknown>
  const devCheck = record.devCheck
  if (devCheck === null || typeof devCheck !== "object") return { changeset: null, verify: null }
  const dev = devCheck as Record<string, unknown>

  const changeSummary = dev.changeSummary as Record<string, unknown> | undefined
  const changeset: SessionsExecResult["changeset"] =
    changeSummary === undefined
      ? null
      : {
          state:
            typeof changeSummary.dirty === "object" && changeSummary.dirty !== null
              ? String((changeSummary.dirty as Record<string, unknown>).state ?? "unknown")
              : "unknown",
          changedFileRefs: Array.isArray(changeSummary.changedFileRefs) ? changeSummary.changedFileRefs : [],
          areaRefs: Array.isArray(changeSummary.areaRefs) ? changeSummary.areaRefs : [],
        }

  const commandResults = Array.isArray(dev.commandResults) ? dev.commandResults : []
  const verify: SessionsExecResult["verify"] = {
    passed: dev.state === "passed",
    state: String(dev.state ?? "unknown"),
    commands: commandResults.map((entry) => {
      const cmd = (entry ?? {}) as Record<string, unknown>
      return {
        commandRef: typeof cmd.commandRef === "string" ? cmd.commandRef : "command.unknown",
        exitCode: typeof cmd.exitCode === "number" ? cmd.exitCode : null,
        status: typeof cmd.status === "string" ? cmd.status : "unknown",
      }
    }),
  }
  return { changeset, verify }
}

// Resolve the approval policy into a callback. An explicit `approvalPolicy`
// callback (W-3's plug point) wins; otherwise we map the `onApproval` flag.
function resolveApprovalPolicy(options: SessionsExecOptions): ApprovalPolicyCallback {
  if (options.approvalPolicy) return options.approvalPolicy
  const mode: ApprovalPolicy = options.onApproval ?? "manual"
  return () => (mode === "deny" ? "deny" : "pause")
}

/**
 * W-1 blocking one-shot. Spawn a coding session, drive its turn loop to a
 * terminal state (or the driver deadline), and return a structured result.
 *
 * - Reuses the existing `session.spawn` args (incl. the tokenized verify argv).
 * - Polls `session.list` (+ `session.events` for the live tail) until terminal.
 * - On terminal, reads the retained artifact for the changeset + verify outcome.
 * - Surfaces pending approvals and applies the (W-3-pluggable) approval policy:
 *   default `manual` pauses/reports; `deny` records a blocked outcome. No
 *   blanket auto-approve here.
 */
export async function runSessionsExec(
  control: SessionsExecControl,
  options: SessionsExecOptions,
): Promise<SessionsExecResult> {
  const now = options.now ?? (() => Date.now())
  const sleep = options.sleep ?? defaultSleep
  const pollIntervalMs = Math.max(25, options.pollIntervalMs ?? 250)
  // The driver deadline defaults to the session timeout + a small grace, so the
  // CLI does not hang forever if the node wedges. `timeoutSeconds` defaults to
  // the node's own 600s session timeout.
  const sessionTimeoutMs = (options.timeoutSeconds ?? 600) * 1000
  const deadlineMs = options.deadlineMs ?? sessionTimeoutMs + 30_000
  const policy = resolveApprovalPolicy(options)

  const startedWall = now()
  const spawned = await control.spawn({
    type: "session.spawn",
    adapter: options.adapter,
    ...(options.lane === undefined ? {} : { lane: options.lane }),
    objective: options.objective,
    ...(options.repoRef ? { repoRef: options.repoRef } : {}),
    verify: options.verify,
    ...(options.repoRef === undefined && options.worktreePath ? { worktreePath: options.worktreePath } : {}),
    ...(options.timeoutSeconds ? { timeoutSeconds: options.timeoutSeconds } : {}),
  })
  const sessionRef = spawned.sessionRef

  const pendingApprovals: SessionsExecResult["pendingApprovals"] = []
  const seenApprovalRefs = new Set<string>()
  let polls = 0
  let timedOut = false
  let approvalDenied = false

  // Drive the loop: poll list for THIS session's projection until terminal or
  // the driver deadline. Between polls, observe any pending approvals and apply
  // the policy.
  let projection: ControlSessionProjection | undefined
  for (;;) {
    polls += 1
    const list = await control.list()
    projection = list.find((entry) => entry.sessionRef === sessionRef)

    // Surface + decide on any pending approvals not yet handled.
    if (control.approvalsList) {
      const { approvals } = await control.approvalsList()
      for (const approval of approvals) {
        if (seenApprovalRefs.has(approval.approvalRef)) continue
        seenApprovalRefs.add(approval.approvalRef)
        const decision = await policy(approval)
        pendingApprovals.push({ approvalRef: approval.approvalRef, kind: approval.kind, decision })
        if ((decision === "approve" || decision === "deny") && control.approvalsResolve) {
          await control.approvalsResolve(approval.approvalRef, decision)
        }
        if (decision === "deny") approvalDenied = true
        // `pause` is recorded without mutation; `approve`/`deny` are applied by
        // the node's approval queue when that resolver is available.
      }
    }

    if (projection && isTerminal(projection.state)) break

    // A manual approval that the policy chose to PAUSE on stops the driver: it
    // reports the pending approval rather than blocking until timeout. A `deny`
    // also stops (the session will be blocked node-side under W-3); we report it.
    const hasPausing = pendingApprovals.some((entry) => entry.decision === "pause")
    if (hasPausing || approvalDenied) break

    if (now() - startedWall >= deadlineMs) {
      timedOut = true
      break
    }
    await sleep(pollIntervalMs)
  }

  // Gather the event tail (result summary) + artifact (changeset/verify) for the
  // structured result. Best-effort: a missing artifact yields nulls, not a throw.
  let resultSummary: string | null = null
  try {
    const events = await control.events(sessionRef)
    resultSummary = summaryFromEvents(events.recentEvents)
  } catch {
    resultSummary = null
  }

  let changeset: SessionsExecResult["changeset"] = null
  let verify: SessionsExecResult["verify"] = null
  try {
    const artifact = await control.artifact(sessionRef)
    const parsed = changesetAndVerifyFromArtifact(artifact.artifact)
    changeset = parsed.changeset
    verify = parsed.verify
  } catch {
    changeset = null
    verify = null
  }

  const state: ControlSessionState = projection?.state ?? "queued"
  const pausingApproval = pendingApprovals.some((entry) => entry.decision === "pause")

  const outcome: SessionsExecResult["outcome"] = timedOut
    ? "timeout"
    : pausingApproval && !isTerminal(state)
      ? "approval_required"
      : state === "completed"
        ? "completed"
        : state === "cancelled"
          ? "cancelled"
          : isTerminal(state)
            ? "failed"
            : // non-terminal + not timed out + not pausing => denied-approval path
              "failed"

  const ok = outcome === "completed"

  return {
    schema: RESULT_SCHEMA,
    ok,
    outcome,
    sessionRef,
    adapter: projection?.adapter ?? options.adapter,
    state,
    resultSummary,
    resultRef: projection?.resultRef ?? null,
    artifactRef: projection?.artifactRef ?? null,
    changeset,
    verify,
    errorClass: projection?.errorClass ?? null,
    errorDigestRef: projection?.errorDigestRef ?? null,
    pendingApprovals,
    autoApprovals: options.approvalAudit ? options.approvalAudit() : [],
    startedAt: projection?.startedAt ?? null,
    completedAt: projection?.completedAt ?? null,
    driver: { elapsedMs: now() - startedWall, polls, timedOut },
  }
}
