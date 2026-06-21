// VCODE-12 (#5929): Codex account/session sync loop for Verse code mode.
//
// These tests pin the one-snapshot contract: account registry mutations,
// node-state polls, event tails, artifacts, decisions, and readiness/quota
// projections converge into one public-safe model tick for code-mode panes.

import { describe, expect, test } from "bun:test"
import type { SessionSummary } from "@openagentsinc/autopilot-control-protocol"
import type {
  AccountRow,
  BuiltInAgentReadinessResponse,
  InferenceGatewayReadinessResponse,
  ManagedAccountsResponse,
  NodeStateMessage,
  SessionArtifactStats,
  SessionEventRow,
} from "../src/shared/rpc"
import { projectCodeModeSyncSnapshot } from "../src/ui/code-mode-sync"
import { initialModel, Model, modelCodeModeSync } from "../src/ui/model"
import { GotManagedAccounts, GotNodeState, SelectedSession } from "../src/ui/message"
import { update } from "../src/ui/update"
import { view } from "../src/ui/view"

type VNodeLike = Readonly<{
  sel?: string
  text?: string
  children?: ReadonlyArray<VNodeLike | string>
  data?: {
    attrs?: Record<string, unknown>
    props?: Record<string, unknown>
    class?: Record<string, boolean>
  }
}>

const isVNodeLike = (value: unknown): value is VNodeLike =>
  typeof value === "object" && value !== null

const renderHtml = (node: unknown): string => {
  if (!isVNodeLike(node)) return ""
  const attrs = node.data?.attrs ?? {}
  const props = node.data?.props ?? {}
  const classes = Object.entries(node.data?.class ?? {})
    .filter(([, on]) => on)
    .map(([c]) => c)
    .join(" ")
  const attrStr = [
    ...Object.entries(attrs),
    ...Object.entries(props),
    ...(classes ? [["class", classes] as const] : []),
  ]
    .filter(([, v]) => v !== false && v !== undefined && v !== null)
    .map(([k, v]) => (v === true ? ` ${k}` : ` ${k}="${String(v)}"`))
    .join("")
  const tag = node.sel ?? "node"
  const children = (node.children ?? [])
    .map((c) => (typeof c === "string" ? c : renderHtml(c)))
    .join("")
  return `<${tag}${attrStr}>${node.text ?? ""}${children}</${tag}>`
}

const workHash = "account.pylon.codex.work.abcdef0123456789abcdef0123456789"
const altHash = "account.pylon.codex.alt.11111111111111111111111111111111"
const sessionRef = "session.pylon.codex.sync"

const account = (input: Partial<AccountRow> & Pick<AccountRow, "accountRefHash">): AccountRow => ({
  provider: input.provider ?? "codex",
  homeState: input.homeState ?? "present",
  ready: input.ready ?? true,
  accountRef: input.accountRef ?? "work",
  accountRefHash: input.accountRefHash,
  selector: input.selector ?? "registry_ref",
  blockerRefs: input.blockerRefs ?? [],
  priority: input.priority ?? 1,
})

const managed = (refs: readonly string[]): ManagedAccountsResponse => ({
  ok: true,
  accounts: refs.map((ref, index) => ({
    ref,
    provider: "codex",
    homePresent: true,
    priority: index,
  })),
})

const session = (input: Partial<SessionSummary> = {}): SessionSummary => ({
  sessionRef: input.sessionRef ?? sessionRef,
  adapter: input.adapter ?? "codex",
  state: input.state ?? "running",
  objectiveRef: input.objectiveRef ?? "objective.sync.safe.abcdef0123456789",
  accountRefHash: input.accountRefHash ?? workHash,
  workspaceRef: input.workspaceRef ?? "workspace.openagents.desktop",
  latestActivity: input.latestActivity ?? "running sync test",
  updatedAt: input.updatedAt ?? "2026-06-21T22:00:00.000Z",
})

const event = (input: Partial<SessionEventRow> = {}): SessionEventRow => ({
  eventIndex: input.eventIndex ?? 0,
  phase: input.phase ?? "progress",
  state: input.state ?? "running",
  observedAt: input.observedAt ?? "2026-06-21T22:00:00.000Z",
  detail: input.detail ?? "thinking: synchronize code mode",
  ...(input.full !== undefined ? { full: input.full } : {}),
})

const proofStats = (): SessionArtifactStats => ({
  kind: "proof",
  outcome: "completed",
  editedFileCount: 1,
  commandCount: 2,
  totalTokens: 900,
  detail: {
    schema: "schema.pylon.proof.v1",
    objectiveDigestRef: "digest.objective.sync",
    verifyRef: "verify.sync.safe",
    responseDigestRef: "digest.response.sync",
    externalSessionRef: null,
    executionPathRef: "control_session.composer",
    executionMode: "local_bounded",
    sandboxMode: "workspace-write",
    permissionMode: "on-request",
    devCheckState: "passed",
    deviationRefs: [],
    redactionState: "clean",
    errorClass: null,
    errorDigestRef: null,
    workspaceRef: "workspace.openagents.desktop",
  },
})

const nodeState = (input: Partial<NodeStateMessage> = {}): NodeStateMessage => ({
  ok: input.ok ?? true,
  schema: input.schema ?? "openagents.pylon.control.v0.3",
  sessions: input.sessions ?? [session()],
  events: input.events ?? {
    [sessionRef]: [
      event({ eventIndex: 0, detail: "thinking: synchronize code mode" }),
      event({ eventIndex: 1, detail: "edited src/sync.ts (+4 -1)" }),
      event({ eventIndex: 1, observedAt: "2026-06-21T22:00:05.000Z", detail: "edited src/sync.ts (+5 -1)" }),
      event({ eventIndex: 2, phase: "decision_requested", detail: "approval required: run tests" }),
    ],
  },
  accounts: input.accounts ?? [
    account({ accountRefHash: workHash, accountRef: "work", priority: 0 }),
  ],
  artifacts: input.artifacts ?? { [sessionRef]: proofStats() },
  approvals: input.approvals ?? [
    {
      approvalRef: "approval.sync.run-tests",
      kind: "shell",
      prompt: "Run tests for sync projection",
      createdAt: "2026-06-21T22:00:00.000Z",
      sessionRef,
      workspaceRef: "workspace.openagents.desktop",
      commandClass: "test",
      accountRefHash: workHash,
      expiresAt: "2026-06-21T23:00:00.000Z",
      lane: "local",
      persistentApprovalSupported: true,
    },
  ],
})

const gatewayReadiness = (over: Partial<InferenceGatewayReadinessResponse> = {}): InferenceGatewayReadinessResponse => ({
  ok: over.ok ?? true,
  fetchedAt: over.fetchedAt ?? "2026-06-21T22:00:00.000Z",
  sourceUrl: over.sourceUrl ?? "desktop:inference-gateway-readiness",
  enabled: over.enabled ?? true,
  apiKeyPresent: over.apiKeyPresent ?? true,
  model: over.model ?? "codex-test",
  creditBalance: over.creditBalance ?? 0,
  lowBalanceThreshold: over.lowBalanceThreshold ?? 1,
  blockerRefs: over.blockerRefs ?? [],
  ...(over.error !== undefined ? { error: over.error } : {}),
})

const builtInReadiness = (over: Partial<BuiltInAgentReadinessResponse> = {}): BuiltInAgentReadinessResponse => ({
  ok: over.ok ?? true,
  fetchedAt: over.fetchedAt ?? "2026-06-21T22:00:00.000Z",
  sourceUrl: over.sourceUrl ?? "desktop:builtin-agent-readiness",
  enabled: over.enabled ?? true,
  localPylonReady: over.localPylonReady ?? true,
  hostedComputeConfigured: over.hostedComputeConfigured ?? true,
  userApiKeyRequired: false,
  lane: over.lane ?? "cloud-gcp",
  modelSet: over.modelSet ?? "codex-test",
  maxSessionSeconds: over.maxSessionSeconds ?? 600,
  dailySessionCap: over.dailySessionCap ?? 2,
  dailySessionsUsed: over.dailySessionsUsed ?? 2,
  meteringLabel: over.meteringLabel ?? "2 sessions/day",
  worktreePathPresent: over.worktreePathPresent ?? true,
  blockerRefs: over.blockerRefs ?? [],
  ...(over.error !== undefined ? { error: over.error } : {}),
})

describe("code-mode sync projection (#5929)", () => {
  test("de-dupes accounts, sessions, events and emits repair diagnostics", () => {
    const snapshot = projectCodeModeSyncSnapshot({
      source: "node_state",
      node: nodeState({
        sessions: [
          session({ updatedAt: "2026-06-21T21:59:00.000Z", latestActivity: "old" }),
          session({ latestActivity: "new" }),
        ],
      }),
      managedAccounts: managed(["work", "alt"]),
      inferenceGatewayReadiness: gatewayReadiness(),
      builtInAgentReadiness: builtInReadiness(),
      appleFmReadiness: null,
      selectedSessionRef: sessionRef,
      composerAccountRef: "alt",
    })

    expect(snapshot.sessions).toHaveLength(1)
    expect(snapshot.sessions[0]?.latestActivity).toBe("new")
    expect(snapshot.events[sessionRef]).toHaveLength(3)
    expect(snapshot.accounts.map((row) => `${row.accountRef}:${row.source}`)).toEqual([
      "work:managed_live",
      "alt:managed_only",
    ])
    expect(snapshot.counts.events).toBe(3)
    expect(snapshot.diagnostics.map((row) => row.key)).toContain("account.codex.alt.missing_live")
    expect(snapshot.diagnostics.map((row) => row.key)).toContain("readiness.gateway.low_balance")
    expect(snapshot.diagnostics.map((row) => row.key)).toContain("readiness.builtin.quota")
    expect(snapshot.syncRef).toEqual(
      projectCodeModeSyncSnapshot({
        source: "node_state",
        node: nodeState({
          sessions: [
            session({ updatedAt: "2026-06-21T21:59:00.000Z", latestActivity: "old" }),
            session({ latestActivity: "new" }),
          ],
        }),
        managedAccounts: managed(["work", "alt"]),
        inferenceGatewayReadiness: gatewayReadiness(),
        builtInAgentReadiness: builtInReadiness(),
        appleFmReadiness: null,
        selectedSessionRef: sessionRef,
        composerAccountRef: "alt",
      }).syncRef,
    )
  })

  test("one node-state tick updates Sessions, Agent Stream, Decisions, and Diff/Artifacts", () => {
    let model = Model.make({
      ...initialModel,
      pane: "sessions",
      verseMode: "code",
      selectedSessionRef: sessionRef,
      sessionDetailView: "diff-artifacts",
    })
    ;[model] = update(model, GotManagedAccounts({ projection: managed(["work"]) }))
    ;[model] = update(model, GotNodeState({ node: nodeState() }))

    const sync = modelCodeModeSync(model)
    expect(sync?.counts.sessions).toBe(1)
    expect(sync?.counts.events).toBe(3)
    expect(sync?.counts.approvals).toBe(1)
    expect(sync?.counts.artifacts).toBe(1)

    const sessionsHtml = renderHtml(view(Model.make({ ...model, pane: "sessions" })).body)
    expect(sessionsHtml).toContain("running sync test")
    expect(sessionsHtml).toContain('data-session-list-filtered-count="1"')

    const streamHtml = renderHtml(view(Model.make({ ...model, pane: "agent-stream" })).body)
    expect(streamHtml).toContain("Files")
    expect(streamHtml).toContain("src/sync.ts")

    const decisionsHtml = renderHtml(view(Model.make({ ...model, pane: "decisions" })).body)
    expect(decisionsHtml).toContain("Run tests for sync projection")
    expect(decisionsHtml).toContain("workspace.openagents.desktop")

    const diffHtml = renderHtml(view(Model.make({ ...model, pane: "diff-artifacts" })).body)
    expect(diffHtml).toContain('data-autopilot-diff-artifacts-panel="session.pylon.codex.sync"')
    expect(diffHtml).toContain("src/sync.ts")
  })

  test("account registry changes update picker rows and diagnostics without reload", () => {
    let model = Model.make({
      ...initialModel,
      pane: "composer",
      verseMode: "code",
      spawnAdapter: "codex",
    })
    ;[model] = update(model, GotManagedAccounts({ projection: managed(["work"]) }))
    ;[model] = update(model, GotNodeState({ node: nodeState() }))

    let rendered = renderHtml(view(model).body)
    expect(rendered).toContain('data-autopilot-composer-account-ref="work"')
    expect(rendered).not.toContain("account.codex.alt.missing_live")

    ;[model] = update(model, GotManagedAccounts({ projection: managed(["work", "alt"]) }))
    rendered = renderHtml(view(model).body)
    expect(rendered).toContain('data-autopilot-composer-account-ref="alt"')
    expect(rendered).toContain("Account waiting for node projection")

    ;[model] = update(
      model,
      GotNodeState({
        node: nodeState({
          accounts: [
            account({ accountRefHash: workHash, accountRef: "work", priority: 0 }),
            account({ accountRefHash: altHash, accountRef: "alt", priority: 1 }),
          ],
          sessions: [
            session({ sessionRef: "session.pylon.codex.alt", accountRefHash: altHash }),
          ],
          events: {},
          artifacts: {},
          approvals: [],
        }),
      }),
    )
    const sessionsHtml = renderHtml(view(Model.make({ ...model, pane: "sessions" })).body)
    expect(sessionsHtml).toContain("codex alt")
    expect(sessionsHtml).not.toContain("Account waiting for node projection")
  })
})
