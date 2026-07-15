/**
 * CUT-11 (#8691): desktop-local canonical live agent graph emission.
 *
 * The desktop already streams one typed local runtime event envelope
 * (`FableLocalEventEnvelope`) for BOTH local lanes:
 *
 * - fable-local Claude root turns, including their Codex delegate children
 *   (`child_started` / `child_activity` / `child_completed` / `child_failed`
 *   from the delegate MCP tool); and
 * - codex-local root turns (the direct Codex chat lane shares the envelope).
 *
 * This module unifies those two lanes into the canonical provider-neutral
 * `openagents.live_agent_graph.v1` contract. It NEVER builds a parallel graph
 * shape: every applied observation becomes one exact-cursor delta folded
 * through the shared reducer (`advanceLiveAgentGraphPostImage`), so all
 * canonical graph laws — stable identity, parent/edge agreement, terminal
 * monotonicity, cursor/timestamp monotonicity, orphan/cycle refusal — are
 * enforced here exactly as they are on the server projection path.
 *
 * Honesty rules:
 * - Provider identity is set ONLY from a terminal observation that names the
 *   final account (rotation means earlier account refs are candidates, not
 *   identity). Until then the provider fact is an explicit unknown.
 * - Facts the local stream does not carry (worktree identity, a child's
 *   unobserved start) stay loss-accounted unknowns — never fabricated.
 * - Events that cannot attach honestly (unknown turn, settled node, thread
 *   mismatch, malformed payload) are REFUSED as typed records, never silently
 *   dropped and never guessed into the graph.
 * - Exact token usage is attributed per node through typed attribution
 *   entries (`usageTruth: "exact"` only when the stream reported the split;
 *   `"unreported"` is the loss-accounted alternative).
 *
 * Runtime kind note: the local Codex lanes execute through `codex exec
 * --json` — the same codex-rs core the app server runs. The canonical v1
 * runtime set has exactly one Codex runtime kind (`codex_app_server`), so
 * that kind is used with a `runtimeRef` that names the exec transport
 * (`runtime.codex_exec.desktop_local`); the ref carries the transport truth
 * rather than this module widening the frozen v1 contract.
 *
 * Live wiring note: main-process wiring (feeding the real IPC stream into
 * this assembler) is deliberately NOT part of this module — `main.ts` is a
 * hot file owned by concurrent lanes. This module consumes the existing
 * envelope contract read-only so that wiring is a one-callback change when
 * that file is free.
 */
import {
  advanceLiveAgentGraphPostImage,
  emptyLiveAgentGraphEntity,
  projectLiveAgentGraphPostImage,
  type LiveAgentGraphEntity,
  type LiveAgentGraphPostImage,
} from "@openagentsinc/khala-sync"

import {
  decodeFableLocalEventEnvelope,
  type FableChildUsage,
  type FableLocalEvent,
  type FableLocalEventEnvelope,
} from "./fable-local-contract.ts"

type CanonicalDelta = Parameters<typeof advanceLiveAgentGraphPostImage>[1]
type CanonicalNode = LiveAgentGraphEntity["nodes"][number]
type CanonicalEdge = LiveAgentGraphEntity["edges"][number]
type CanonicalToolEdge = Extract<CanonicalEdge, { kind: "tool" }>

const SCHEMA = "openagents.live_agent_graph.v1" as const

/** The two desktop-local lanes that share the fable-local event envelope. */
export type LocalAgentGraphLane = "fable_claude" | "codex_local"

export type LocalAgentGraphTurnStart = Readonly<{
  turnRef: string
  threadRef: string
  lane: LocalAgentGraphLane
}>

export type LocalAgentGraphRefusalReason =
  | "invalid_event"
  | "thread_mismatch"
  | "duplicate_turn"
  | "unknown_turn"
  | "unknown_child"
  | "after_terminal"
  | "graph_law_refused"

export type LocalAgentGraphRefusal = Readonly<{
  reason: LocalAgentGraphRefusalReason
  detail: string
  at: string
}>

export type LocalAgentGraphResult =
  | Readonly<{ applied: true; postImage: LiveAgentGraphPostImage }>
  | Readonly<{ applied: false; refusal: LocalAgentGraphRefusal }>

/**
 * One exact usage attribution tying a canonical graph node to the session
 * usage ledger's (provider, accountRef) key. `usageTruth: "exact"` iff the
 * stream reported the split; a terminal without reported usage is recorded
 * as `"unreported"` (loss-accounted), never synthesized.
 */
export type LocalAgentGraphUsageAttribution = Readonly<{
  attributionRef: string
  agentRef: string
  turnRef: string
  childRef: string | null
  provider: "claude_agent" | "codex"
  accountRef: string | null
  usageTruth: "exact" | "unreported"
  usage: FableChildUsage | null
  recordedAt: string
}>

export type LocalAgentGraphAssembler = Readonly<{
  /** Register a root turn before its stream events arrive. */
  startTurn: (start: LocalAgentGraphTurnStart, at: string) => LocalAgentGraphResult
  /** Fold one typed local runtime event envelope into the graph. */
  applyEvent: (envelope: FableLocalEventEnvelope, at: string) => LocalAgentGraphResult
  /** Decode an untyped envelope (raw stream payload) and fold it. */
  applyEnvelopeValue: (value: unknown, at: string) => LocalAgentGraphResult
  /** Current validated canonical post-image. */
  postImage: () => LiveAgentGraphPostImage
  /** Current validated canonical snapshot value. */
  snapshot: () => LiveAgentGraphEntity
  /** Exact/loss-accounted usage attributions recorded so far. */
  usageAttributions: () => ReadonlyArray<LocalAgentGraphUsageAttribution>
  /** Typed refusal ledger (bounded; newest last). */
  refusals: () => ReadonlyArray<LocalAgentGraphRefusal>
}>

const REFUSAL_LIMIT = 200
const ATTRIBUTION_LIMIT = 4_000
const REF_SEGMENT_PATTERN = /^[A-Za-z0-9][A-Za-z0-9._:-]*$/

const fnv1a = (value: string): string => {
  let hash = 0x811c9dc5
  for (let index = 0; index < value.length; index += 1) {
    hash ^= value.charCodeAt(index)
    hash = Math.imul(hash, 0x01000193) >>> 0
  }
  return hash.toString(16).padStart(8, "0")
}

/**
 * Deterministic ref-segment sanitizer. A value already inside the canonical
 * ref charset passes through unchanged; anything else is normalized and
 * suffixed with a short content hash so distinct inputs cannot collide after
 * normalization ("a b" and "a-b" stay distinct refs).
 */
export const sanitizeLocalRefSegment = (value: string): string => {
  const bounded = value.slice(0, 80)
  if (bounded === value && REF_SEGMENT_PATTERN.test(bounded)) return bounded
  const replaced = bounded.replace(/[^A-Za-z0-9._:-]/g, "-")
  const anchored = /^[A-Za-z0-9]/.test(replaced) ? replaced : `r${replaced}`
  const base = anchored === "" ? "r" : anchored
  return `${base}.${fnv1a(value)}`
}

/** Exact canonical child ref shared by graph assembly and card navigation. */
export const localDelegateAgentRef = (turnRef: string, childRef: string): string =>
  `agent.local.${sanitizeLocalRefSegment(turnRef)}.child.${sanitizeLocalRefSegment(childRef)}`

const terminalRootStatuses: ReadonlySet<CanonicalNode["status"]> = new Set([
  "completed",
  "failed",
  "canceled",
  "interrupted",
])

type TurnState = {
  readonly rootAgentRef: string
  readonly lane: LocalAgentGraphLane
  /** local childRef -> canonical child agentRef */
  readonly children: Map<string, string>
  toolSequence: number
  openToolEdgeRef: string | null
}

export const createLocalAgentGraphAssembler = (input: Readonly<{
  sessionRef: string
  threadRef: string
  graphRef?: string
  createdAt: string
}>): LocalAgentGraphAssembler => {
  const sessionRef = sanitizeLocalRefSegment(input.sessionRef)
  const threadRef = sanitizeLocalRefSegment(input.threadRef)
  const graphRef = input.graphRef === undefined
    ? `graph.local.${threadRef}`
    : sanitizeLocalRefSegment(input.graphRef)

  let post: LiveAgentGraphPostImage = projectLiveAgentGraphPostImage(
    emptyLiveAgentGraphEntity({
      graphRef,
      sessionRef,
      threadRef,
      attachmentGeneration: 1,
      updatedAt: input.createdAt,
    }),
  )
  let lastAt = input.createdAt
  const turns = new Map<string, TurnState>()
  const refusals: Array<LocalAgentGraphRefusal> = []
  const attributions: Array<LocalAgentGraphUsageAttribution> = []

  const clamp = (at: string): string => {
    if (at > lastAt) lastAt = at
    return lastAt
  }

  const refuse = (
    reason: LocalAgentGraphRefusalReason,
    detail: string,
    at: string,
  ): LocalAgentGraphResult => {
    const refusal: LocalAgentGraphRefusal = { reason, detail, at }
    refusals.push(refusal)
    if (refusals.length > REFUSAL_LIMIT) refusals.shift()
    return { applied: false, refusal }
  }

  const nodeOf = (agentRef: string): CanonicalNode | undefined =>
    post.value.nodes.find(node => node.agentRef === agentRef)

  const edgeOf = (edgeRef: string): CanonicalEdge | undefined =>
    post.value.edges.find(edge => edge.edgeRef === edgeRef)

  const commit = (
    at: string,
    upsertNodes: ReadonlyArray<CanonicalNode>,
    upsertEdges: ReadonlyArray<CanonicalEdge> = [],
  ): LocalAgentGraphResult => {
    const cursor = post.value.cursor + 1
    const delta: CanonicalDelta = {
      schema: SCHEMA,
      deltaRef: `delta.${graphRef}.${String(cursor)}`,
      graphRef,
      sessionRef,
      threadRef,
      attachmentGeneration: 1,
      previousCursor: post.value.cursor,
      cursor,
      upsertNodes: [...upsertNodes],
      removeAgentRefs: [],
      upsertEdges: [...upsertEdges],
      removeEdgeRefs: [],
      committedAt: at,
    }
    try {
      post = advanceLiveAgentGraphPostImage(post, delta)
      return { applied: true, postImage: post }
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error)
      return refuse("graph_law_refused", message.slice(0, 400), at)
    }
  }

  const laneRuntime = (lane: LocalAgentGraphLane): CanonicalNode["runtime"] =>
    lane === "fable_claude"
      ? {
          state: "known",
          kind: "claude_agent_sdk",
          runtimeRef: "runtime.claude_agent_sdk.desktop_local",
        }
      : {
          state: "known",
          kind: "codex_app_server",
          runtimeRef: "runtime.codex_exec.desktop_local",
        }

  const makeNode = (agentRef: string, base: Readonly<{
    parent: CanonicalNode["parent"]
    runtime: CanonicalNode["runtime"]
    status: CanonicalNode["status"]
    at: string
    startedAt: string | null
    runRef: string
  }>): CanonicalNode => ({
    agentRef,
    sessionRef,
    threadRef,
    transcriptRef: `transcript.${threadRef}`,
    runRef: base.runRef,
    parent: base.parent,
    provider: { state: "unknown", reason: "not_observed" },
    runtime: base.runtime,
    // The local stream carries no worktree identity fact — explicit unknown.
    worktree: { state: "unknown", reason: "not_observed" },
    status: base.status,
    attention: { state: "none" },
    terminal: { state: "active" },
    currentTool: { state: "none" },
    attachmentGeneration: 1,
    activityCursor: 0,
    createdAt: base.at,
    updatedAt: base.at,
    startedAt: base.startedAt,
    endedAt: null,
    version: 1,
  })

  /** New node object advancing per-node version/cursor/timestamp laws. */
  const advanceNode = (
    node: CanonicalNode,
    at: string,
    changes: Partial<CanonicalNode>,
  ): CanonicalNode => ({
    ...node,
    ...changes,
    activityCursor: node.activityCursor + 1,
    updatedAt: at,
    version: node.version + 1,
  })

  const terminalNode = (
    node: CanonicalNode,
    at: string,
    status: "completed" | "failed" | "canceled" | "interrupted",
    terminalReason: "completed" | "failed" | "canceled" | "interrupted" | "provider_lost" | "revoked" | "unknown",
    extra: Partial<CanonicalNode> = {},
  ): CanonicalNode =>
    advanceNode(node, at, {
      ...extra,
      status,
      terminal: { state: "terminal", reason: terminalReason, at },
      endedAt: at,
      attention: { state: "none" },
      currentTool: { state: "none" },
    })

  const recordAttribution = (entry: Omit<LocalAgentGraphUsageAttribution, "attributionRef">): void => {
    if (attributions.length >= ATTRIBUTION_LIMIT) return
    attributions.push({
      attributionRef: `usage.${entry.agentRef}.${String(attributions.length + 1)}`,
      ...entry,
    })
  }

  const knownProvider = (
    kind: "codex" | "claude",
    accountRef: string,
  ): CanonicalNode["provider"] => ({
    state: "known",
    kind,
    providerRef: `account.${kind}.${sanitizeLocalRefSegment(accountRef)}`,
  })

  /** Still-active children of a settled root become honest interruptions. */
  const settleChildren = (turn: TurnState, at: string): Array<CanonicalNode> => {
    const settled: Array<CanonicalNode> = []
    for (const childAgentRef of turn.children.values()) {
      const child = nodeOf(childAgentRef)
      if (child === undefined || terminalRootStatuses.has(child.status)) continue
      settled.push(
        terminalNode(child, at, "interrupted", "interrupted", {
          statusReasonRef: "reason.parent_settled",
        }),
      )
    }
    return settled
  }

  /** Settle a dangling running tool edge as loss-accounted unknown. */
  const settleOpenToolEdge = (turn: TurnState): Array<CanonicalEdge> => {
    if (turn.openToolEdgeRef === null) return []
    const edge = edgeOf(turn.openToolEdgeRef)
    turn.openToolEdgeRef = null
    if (edge === undefined || edge.kind !== "tool") return []
    if (edge.status === "completed" || edge.status === "failed") return []
    return [{ ...edge, status: "unknown", version: edge.version + 1 }]
  }

  const startTurn = (start: LocalAgentGraphTurnStart, atInput: string): LocalAgentGraphResult => {
    const at = clamp(atInput)
    const startThreadRef = sanitizeLocalRefSegment(start.threadRef)
    if (startThreadRef !== threadRef) {
      return refuse(
        "thread_mismatch",
        `turn targets thread ${startThreadRef}, graph owns ${threadRef}`,
        at,
      )
    }
    if (turns.has(start.turnRef)) {
      return refuse("duplicate_turn", `turn ${start.turnRef} already started`, at)
    }
    const turnSegment = sanitizeLocalRefSegment(start.turnRef)
    const rootAgentRef = `agent.local.${turnSegment}`
    if (nodeOf(rootAgentRef) !== undefined) {
      return refuse("duplicate_turn", `agent ${rootAgentRef} already exists`, at)
    }
    const result = commit(at, [
      makeNode(rootAgentRef, {
        parent: { kind: "root" },
        runtime: laneRuntime(start.lane),
        status: "queued",
        at,
        startedAt: null,
        runRef: `run.${turnSegment}`,
      }),
    ])
    if (result.applied) {
      turns.set(start.turnRef, {
        rootAgentRef,
        lane: start.lane,
        children: new Map(),
        toolSequence: 0,
        openToolEdgeRef: null,
      })
    }
    return result
  }

  const ensureChild = (
    turn: TurnState,
    turnRef: string,
    childRef: string,
    at: string,
    startedAt: string | null,
    parentChildRef?: string,
  ): Readonly<{ agentRef: string; created: CanonicalNode | null; edge: CanonicalEdge | null }> => {
    const existing = turn.children.get(childRef)
    if (existing !== undefined) return { agentRef: existing, created: null, edge: null }
    const childSegment = sanitizeLocalRefSegment(childRef)
    const agentRef = localDelegateAgentRef(turnRef, childRef)
    turn.children.set(childRef, agentRef)
    const parentAgentRef = parentChildRef === undefined
      ? turn.rootAgentRef
      : turn.children.get(parentChildRef) ?? turn.rootAgentRef
    const node = makeNode(agentRef, {
      parent: { kind: "agent", agentRef: parentAgentRef },
      // Delegate children are always Codex exec children, on either lane.
      runtime: laneRuntime("codex_local"),
      status: "running",
      at,
      startedAt,
      runRef: `run.${sanitizeLocalRefSegment(turnRef)}.child.${childSegment}`,
    })
    const edge: CanonicalEdge = {
      edgeRef: `edge.parent.${agentRef}`,
      kind: "parent",
      fromAgentRef: parentAgentRef,
      toAgentRef: agentRef,
      version: 1,
    }
    return { agentRef, created: node, edge }
  }

  const applyEvent = (envelope: FableLocalEventEnvelope, atInput: string): LocalAgentGraphResult => {
    const at = clamp(atInput)
    const turn = turns.get(envelope.turnRef)
    if (turn === undefined) {
      return refuse(
        "unknown_turn",
        `event ${envelope.event.kind} for unstarted turn ${envelope.turnRef}`,
        at,
      )
    }
    const root = nodeOf(turn.rootAgentRef)
    if (root === undefined) {
      return refuse("unknown_turn", `root node ${turn.rootAgentRef} missing`, at)
    }
    const event = envelope.event
    const rootTerminal = terminalRootStatuses.has(root.status)

    const childEventRef = event.kind === "child_started" ||
        event.kind === "child_activity" ||
        event.kind === "child_completed" ||
        event.kind === "child_failed" ||
        event.kind === "child_steered"
      ? event.childRef
      : null

    if (rootTerminal && childEventRef === null) {
      return refuse("after_terminal", `event ${event.kind} after root settled`, at)
    }

    switch (event.kind) {
      case "turn_started": {
        if (root.status !== "queued") {
          // Loss-tolerant: a repeated start marker is plain activity.
          return commit(at, [advanceNode(root, at, {})])
        }
        return commit(at, [advanceNode(root, at, { status: "running", startedAt: at })])
      }
      case "text_delta":
      case "reasoning":
      case "lane_notice":
      case "model_effective":
      case "composer_admission":
      case "plan_updated":
      case "mcp_server_unavailable":
      case "followup_queued":
      case "followup_promoted": {
        return commit(at, [advanceNode(root, at, {})])
      }
      case "tool_use": {
        turn.toolSequence += 1
        const toolCallRef =
          `tool.${sanitizeLocalRefSegment(envelope.turnRef)}.${String(turn.toolSequence)}`
        const edgeRef = `edge.tool.${toolCallRef}`
        const settledEdges = settleOpenToolEdge(turn)
        turn.openToolEdgeRef = edgeRef
        const toolEdge: CanonicalToolEdge = {
          edgeRef,
          kind: "tool",
          agentRef: turn.rootAgentRef,
          toolCallRef,
          status: "running",
          version: 1,
        }
        return commit(
          at,
          [advanceNode(root, at, {
            currentTool: {
              state: "known",
              toolCallRef,
              toolName: event.toolName.slice(0, 256),
              status: "running",
            },
          })],
          [...settledEdges, toolEdge],
        )
      }
      case "tool_result": {
        const edgeRef = turn.openToolEdgeRef
        turn.openToolEdgeRef = null
        const edge = edgeRef === null ? undefined : edgeOf(edgeRef)
        const upsertEdges: Array<CanonicalEdge> = edge !== undefined && edge.kind === "tool"
          ? [{ ...edge, status: event.ok ? "completed" : "failed", version: edge.version + 1 }]
          : []
        return commit(
          at,
          [advanceNode(root, at, { currentTool: { state: "none" } })],
          upsertEdges,
        )
      }
      case "question_pending": {
        return commit(at, [advanceNode(root, at, {
          status: "waiting_for_input",
          attention: {
            state: "question",
            attentionRef: `question.${sanitizeLocalRefSegment(event.questionRef)}`,
            since: at,
          },
        })])
      }
      case "question_resolved": {
        return commit(at, [advanceNode(root, at, {
          status: root.status === "waiting_for_input" ? "running" : root.status,
          attention: { state: "none" },
        })])
      }
      case "child_started": {
        if (event.parentChildRef !== undefined && !turn.children.has(event.parentChildRef)) {
          return refuse("unknown_child", `parent child ${event.parentChildRef} untracked`, at)
        }
        const ensured = ensureChild(turn, envelope.turnRef, event.childRef, at, at, event.parentChildRef)
        if (ensured.created === null) {
          const child = nodeOf(ensured.agentRef)
          if (child === undefined) return refuse("unknown_child", `child ${event.childRef} untracked`, at)
          if (terminalRootStatuses.has(child.status)) {
            return refuse("after_terminal", `child ${event.childRef} already settled`, at)
          }
          return commit(at, [advanceNode(child, at, {})])
        }
        const created = commit(at, [ensured.created], ensured.edge === null ? [] : [ensured.edge])
        if (!created.applied) turn.children.delete(event.childRef)
        return created
      }
      case "child_activity": {
        if (event.parentChildRef !== undefined && !turn.children.has(event.parentChildRef)) {
          return refuse("unknown_child", `parent child ${event.parentChildRef} untracked`, at)
        }
        const ensured = ensureChild(turn, envelope.turnRef, event.childRef, at, null, event.parentChildRef)
        if (ensured.created !== null) {
          // Loss-tolerant creation: the start was not observed (startedAt
          // stays null) but parentage is known from the envelope.
          const created = commit(at, [ensured.created], ensured.edge === null ? [] : [ensured.edge])
          if (!created.applied) turn.children.delete(event.childRef)
          return created
        }
        const child = nodeOf(ensured.agentRef)
        if (child === undefined) return refuse("unknown_child", `child ${event.childRef} untracked`, at)
        if (terminalRootStatuses.has(child.status)) {
          return refuse("after_terminal", `activity for settled child ${event.childRef}`, at)
        }
        return commit(at, [advanceNode(child, at, {
          ...(event.accountRef === undefined
            ? {}
            : { provider: knownProvider("codex", event.accountRef) }),
        })])
      }
      case "child_completed":
      case "child_failed": {
        if (event.parentChildRef !== undefined && !turn.children.has(event.parentChildRef)) {
          return refuse("unknown_child", `parent child ${event.parentChildRef} untracked`, at)
        }
        const ensured = ensureChild(turn, envelope.turnRef, event.childRef, at, null, event.parentChildRef)
        const child = ensured.created ?? nodeOf(ensured.agentRef)
        if (child === undefined) return refuse("unknown_child", `child ${event.childRef} untracked`, at)
        if (ensured.created === null && terminalRootStatuses.has(child.status)) {
          return refuse("after_terminal", `child ${event.childRef} already settled`, at)
        }
        const accountRef = event.accountRef
        // Provider identity is stable once observed. A later terminal account
        // can differ after rotation, but exact terminal account attribution is
        // carried separately by the usage ledger and must not rewrite the
        // already-rendered live identity.
        const provider = child.provider.state === "known" || accountRef === null || accountRef === undefined
          ? child.provider
          : knownProvider("codex", accountRef)
        const settled = event.kind === "child_completed"
          ? terminalNode(child, at, "completed", "completed", { provider })
          : terminalNode(
              child,
              at,
              "failed",
              event.reason === "account_reconnect_required" ? "revoked" : "failed",
              { provider, statusReasonRef: `reason.${event.reason}` },
            )
        const result = commit(
          at,
          [settled],
          ensured.edge === null ? [] : [ensured.edge],
        )
        if (!result.applied && ensured.created !== null) turn.children.delete(event.childRef)
        if (result.applied) {
          const usage = event.kind === "child_completed" ? event.usage : null
          recordAttribution({
            agentRef: ensured.agentRef,
            turnRef: envelope.turnRef,
            childRef: event.childRef,
            provider: "codex",
            accountRef: accountRef ?? null,
            usageTruth: usage === null ? "unreported" : "exact",
            usage,
            recordedAt: at,
          })
        }
        return result
      }
      case "child_steered": {
        const childAgentRef = turn.children.get(event.childRef)
        const child = childAgentRef === undefined ? undefined : nodeOf(childAgentRef)
        if (childAgentRef === undefined || child === undefined) {
          return refuse("unknown_child", `steer for untracked child ${event.childRef}`, at)
        }
        if (terminalRootStatuses.has(child.status)) {
          return refuse("after_terminal", `steer for settled child ${event.childRef}`, at)
        }
        if (event.outcome === "interrupted") {
          return commit(at, [
            terminalNode(child, at, "interrupted", "interrupted", {
              statusReasonRef: "reason.steer_interrupt",
            }),
          ])
        }
        return commit(at, [advanceNode(child, at, {})])
      }
      case "turn_completed": {
        const provider = event.accountRef === undefined
          ? root.provider
          : knownProvider(turn.lane === "fable_claude" ? "claude" : "codex", event.accountRef)
        const settledChildren = settleChildren(turn, at)
        const settledEdges = settleOpenToolEdge(turn)
        const result = commit(
          at,
          [terminalNode(root, at, "completed", "completed", { provider }), ...settledChildren],
          settledEdges,
        )
        if (result.applied) {
          recordAttribution({
            agentRef: turn.rootAgentRef,
            turnRef: envelope.turnRef,
            childRef: null,
            provider: turn.lane === "fable_claude" ? "claude_agent" : "codex",
            accountRef: event.accountRef ?? null,
            usageTruth: event.usage === undefined ? "unreported" : "exact",
            usage: event.usage ?? null,
            recordedAt: at,
          })
        }
        return result
      }
      case "turn_failed": {
        const interrupted = event.reason === "interrupted"
        const settledChildren = settleChildren(turn, at)
        const settledEdges = settleOpenToolEdge(turn)
        const settledRoot = interrupted
          ? terminalNode(root, at, "interrupted", "interrupted", {
              statusReasonRef: `reason.${event.reason}`,
            })
          : terminalNode(
              root,
              at,
              "failed",
              event.reason === "account_reconnect_required" ? "revoked" : "failed",
              { statusReasonRef: `reason.${event.reason}` },
            )
        const result = commit(at, [settledRoot, ...settledChildren], settledEdges)
        if (result.applied) {
          recordAttribution({
            agentRef: turn.rootAgentRef,
            turnRef: envelope.turnRef,
            childRef: null,
            provider: turn.lane === "fable_claude" ? "claude_agent" : "codex",
            accountRef: null,
            usageTruth: "unreported",
            usage: null,
            recordedAt: at,
          })
        }
        return result
      }
    }
  }

  const applyEnvelopeValue = (value: unknown, at: string): LocalAgentGraphResult => {
    const envelope = decodeFableLocalEventEnvelope(value)
    if (envelope === null) {
      return refuse("invalid_event", "envelope failed contract decode", clamp(at))
    }
    return applyEvent(envelope, at)
  }

  return {
    startTurn,
    applyEvent,
    applyEnvelopeValue,
    postImage: () => post,
    snapshot: () => post.value,
    usageAttributions: () => [...attributions],
    refusals: () => [...refusals],
  }
}

/** Convenience type guard for tests and future wiring. */
export const isLocalAgentGraphApplied = (
  result: LocalAgentGraphResult,
): result is Extract<LocalAgentGraphResult, { applied: true }> => result.applied

export type { FableLocalEvent as LocalAgentGraphSourceEvent }
