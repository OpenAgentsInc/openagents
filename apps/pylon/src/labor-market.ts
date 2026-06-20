import { existsSync } from "node:fs"
import { mkdir, readFile, writeFile } from "node:fs/promises"
import { join } from "node:path"
import { createHash } from "node:crypto"
import {
  LBR_AGENTIC_CODING_REQUEST_KIND,
  LBR_FEEDBACK_KIND,
  decodeLbrAcceptanceEvent,
  decodeLbrAgenticCodingRequestEvent,
  lbrQuoteToDraft,
  lbrResultToDraft,
  makeLbrQuote,
  makeLbrResult,
  type LbrAcceptance,
  type LbrAgenticCodingRequest,
} from "@openagentsinc/nip90"
import {
  CLAUDE_AGENT_CAPABILITY_REF,
  probeClaudeAgentReadiness,
  type ClaudeAgentProbeOptions,
} from "./claude-agent.js"
import { runWithClaudeAgentSdk, type ClaudeAgentRunner } from "./claude-agent-executor.js"
import {
  assertLaborPublicSafe,
  hasLaborFirstRunApproval,
  laborPrompt,
  makeConfiguredLaborRuntime,
  type LaborLocalAgentKind,
  type LaborRuntime,
  type LaborWorkspace,
} from "./labor.js"
import {
  signNostrEvent,
  type NostrEvent,
  type ProviderRelayTransport,
} from "./provider-nip90.js"
import { ensureStateDirectories, type PylonLocalState } from "./state.js"
import type { PylonNostrPrivateIdentity } from "./nostr-identity.js"

/**
 * The Pylon labor-market provider loop (issue #4730, promise
 * labor.nostr_negotiation_market.v1): watch NIP-LBR agentic-coding jobs on
 * the owned relay, quote the ones this device is capability-true for under
 * contributor-configured pricing, execute on acceptance through the labor
 * runtime on the contributor's own agent, and deliver output-only results.
 *
 * Negotiation discipline: LBR requests are never auto-executed; the
 * provider quotes, the requester accepts exactly one quote (carrying the
 * escrow receipt ref), and only then does admission and execution run.
 * The provider never self-accepts, never sees requester funds, and never
 * ships failing work - the stated verification command must pass locally
 * before a result is published.
 */

export const LABOR_MARKET_STATE_SCHEMA = "openagents.pylon.labor_market_state.v0.3"

export const LABOR_MARKET_VERIFICATION_COMMANDS: Readonly<Record<string, ReadonlyArray<string>>> = {
  "command.public.pylon.labor.bun_test": ["bun", "test"],
  "command.public.pylon.labor.bun_test_sum": ["bun", "test", "sum.test.ts"],
}

export type LaborMarketPolicy = {
  autoQuote: boolean
  priceMsats: number
  maxConcurrentJobs: number
  allowedJobKinds: ReadonlyArray<number>
  agentKind?: LaborLocalAgentKind
}

export const DEFAULT_LABOR_MARKET_POLICY: LaborMarketPolicy = {
  autoQuote: false,
  priceMsats: 1_000_000,
  maxConcurrentJobs: 1,
  allowedJobKinds: [LBR_AGENTIC_CODING_REQUEST_KIND],
}

export type LaborMarketQuoteRecord = {
  requestEventId: string
  requesterPubkey: string
  quoteRef: string
  quoteEventId: string
  amountMsats: number
  requestEvent: NostrEvent
  status: "quoted" | "executing" | "delivered" | "refused"
  resultEventId?: string
  reasonRef?: string
  quotedAt: string
}

export type LaborMarketStore = {
  schema: typeof LABOR_MARKET_STATE_SCHEMA
  quotes: Record<string, LaborMarketQuoteRecord>
}

export type LaborMarketRefusalRef =
  | "refusal.labor_market.auto_quote_disabled"
  | "refusal.labor_market.job_kind_not_allowed"
  | "refusal.labor_market.capability_untrue"
  | "refusal.labor_market.price_above_bid"
  | "refusal.labor_market.request_expired"
  | "refusal.labor_market.already_quoted"
  | "refusal.labor_market.max_concurrent_jobs"

export type LaborMarketQuoteDecision =
  | { quote: true; amountMsats: number }
  | { quote: false; reasonRef: LaborMarketRefusalRef }

function stableRef(prefix: string, input: string) {
  return `${prefix}.${createHash("sha256").update(input).digest("hex").slice(0, 24)}`
}

export function laborMarketStatePath(state: PylonLocalState) {
  return join(state.paths.home, "labor-market-state.json")
}

export async function loadLaborMarketStore(state: PylonLocalState): Promise<LaborMarketStore> {
  await ensureStateDirectories(state.paths)
  const path = laborMarketStatePath(state)
  if (!existsSync(path)) {
    return { schema: LABOR_MARKET_STATE_SCHEMA, quotes: {} }
  }
  return JSON.parse(await readFile(path, "utf8")) as LaborMarketStore
}

export async function writeLaborMarketStore(state: PylonLocalState, store: LaborMarketStore) {
  assertLaborPublicSafe(store)
  await writeFile(laborMarketStatePath(state), `${JSON.stringify(store, null, 2)}\n`)
}

/**
 * Reads the laborMarket config section. Quoting is opt-in: autoQuote
 * defaults to false, pricing is the contributor's, never the platform's.
 */
export async function loadLaborMarketPolicy(
  state: PylonLocalState,
  env: Readonly<Record<string, string | undefined>> = process.env,
): Promise<LaborMarketPolicy> {
  let section: Record<string, unknown> = {}
  try {
    const raw = JSON.parse(await readFile(state.paths.config, "utf8")) as {
      laborMarket?: unknown
    }
    if (raw.laborMarket !== null && typeof raw.laborMarket === "object") {
      section = raw.laborMarket as Record<string, unknown>
    }
  } catch {
    // missing or malformed config means defaults
  }
  const envAutoQuote = env.PYLON_LABOR_MARKET_AUTO_QUOTE?.trim().toLowerCase()
  const envPrice = Number(env.PYLON_LABOR_MARKET_PRICE_MSATS ?? "")
  const agentKind = section.agentKind
  return {
    autoQuote:
      envAutoQuote === "1" || envAutoQuote === "true"
        ? true
        : envAutoQuote === "0" || envAutoQuote === "false"
          ? false
          : typeof section.autoQuote === "boolean"
            ? section.autoQuote
            : DEFAULT_LABOR_MARKET_POLICY.autoQuote,
    priceMsats:
      Number.isFinite(envPrice) && envPrice > 0
        ? Math.floor(envPrice)
        : typeof section.priceMsats === "number" && Number.isFinite(section.priceMsats) && section.priceMsats > 0
          ? Math.floor(section.priceMsats)
          : DEFAULT_LABOR_MARKET_POLICY.priceMsats,
    maxConcurrentJobs:
      typeof section.maxConcurrentJobs === "number" && Number.isFinite(section.maxConcurrentJobs) && section.maxConcurrentJobs > 0
        ? Math.floor(section.maxConcurrentJobs)
        : DEFAULT_LABOR_MARKET_POLICY.maxConcurrentJobs,
    allowedJobKinds: DEFAULT_LABOR_MARKET_POLICY.allowedJobKinds,
    ...(agentKind === "codex" || agentKind === "opencode" || agentKind === "claude_code" || agentKind === "test_fixture"
      ? { agentKind }
      : {}),
  }
}

/**
 * Pure quoting policy: quote only when opted in, capability-true, inside
 * concurrency bounds, and when the contributor's price fits the bid.
 */
export function evaluateLbrRequestForQuote(input: {
  request: LbrAgenticCodingRequest
  requestEventId: string
  declaredCapabilityRefs: ReadonlyArray<string>
  policy: LaborMarketPolicy
  store: LaborMarketStore
  now: Date
}): LaborMarketQuoteDecision {
  if (!input.policy.autoQuote) {
    return { quote: false, reasonRef: "refusal.labor_market.auto_quote_disabled" }
  }
  if (!input.policy.allowedJobKinds.includes(input.request.kind)) {
    return { quote: false, reasonRef: "refusal.labor_market.job_kind_not_allowed" }
  }
  if (input.store.quotes[input.requestEventId] !== undefined) {
    return { quote: false, reasonRef: "refusal.labor_market.already_quoted" }
  }
  const declared = new Set(input.declaredCapabilityRefs)
  if (!input.request.requiredCapabilityRefs.every((ref) => declared.has(ref))) {
    return { quote: false, reasonRef: "refusal.labor_market.capability_untrue" }
  }
  if (input.request.deadline !== undefined && new Date(input.request.deadline).getTime() <= input.now.getTime()) {
    return { quote: false, reasonRef: "refusal.labor_market.request_expired" }
  }
  const inflight = Object.values(input.store.quotes).filter(
    (record) => record.status === "quoted" || record.status === "executing",
  ).length
  if (inflight >= input.policy.maxConcurrentJobs) {
    return { quote: false, reasonRef: "refusal.labor_market.max_concurrent_jobs" }
  }
  if (input.policy.priceMsats > input.request.bidMsats) {
    return { quote: false, reasonRef: "refusal.labor_market.price_above_bid" }
  }
  return { quote: true, amountMsats: input.policy.priceMsats }
}

export type LaborMarketHandleResult =
  | { handled: false }
  | { handled: true; action: "quoted"; quoteEventId: string; amountMsats: number }
  | { handled: true; action: "refused"; reasonRef: string }
  | { handled: true; action: "deferred"; reasonRef: string }
  | { handled: true; action: "delivered"; resultEventId: string; closeoutRef: string }
  | { handled: true; action: "verification_failed"; reasonRef: string }

export type LaborMarketOptions = {
  policy?: LaborMarketPolicy
  laborRuntime?: LaborRuntime
  claudeAgentRunner?: ClaudeAgentRunner
  claudeAgentProbe?: ClaudeAgentProbeOptions
  verificationCommands?: Readonly<Record<string, ReadonlyArray<string>>>
  now?: () => Date
  // CL-16: invoked when a job is deferred for first-run operator approval, so
  // the node can enqueue a pending approval for the clients to resolve.
  onDeferredForApproval?: (input: { approvalRef: string; jobType: string; policyRef: string }) => void
  // Resolves the opaque, content-addressed objectiveRef to public-safe,
  // actionable task detail for the local agent. The kind-5934 request is
  // strictly ref-only, so the provider fetches the public objective out-of-band
  // (e.g. the openagents.com work-request API) right before execution. Returns
  // null when no detail is available (the agent then sees refs only).
  resolveObjectiveDetail?: (input: {
    objectiveRef: string
    requestEventId: string
    requesterPubkey: string
  }) => Promise<string | null>
}

function decodeLbrRequest(event: NostrEvent): LbrAgenticCodingRequest | null {
  if (event.kind !== LBR_AGENTIC_CODING_REQUEST_KIND) return null
  try {
    return decodeLbrAgenticCodingRequestEvent(event)
  } catch {
    return null
  }
}

function decodeAcceptanceFor(
  event: NostrEvent,
  providerPubkey: string,
): LbrAcceptance | null {
  if (event.kind !== LBR_FEEDBACK_KIND) return null
  try {
    const acceptance = decodeLbrAcceptanceEvent(event)
    return acceptance.providerPubkey === providerPubkey ? acceptance : null
  } catch {
    return null
  }
}

/**
 * Returns true when the event belongs to the labor-market negotiation
 * lane (an LBR request or an LBR acceptance), so the generic provider
 * loop can route it here instead of auto-executing it.
 */
export function isLaborMarketEvent(event: NostrEvent, providerPubkey: string): boolean {
  return decodeLbrRequest(event) !== null || decodeAcceptanceFor(event, providerPubkey) !== null
}

async function handleLbrRequest(input: {
  state: PylonLocalState
  event: NostrEvent
  request: LbrAgenticCodingRequest
  identity: PylonNostrPrivateIdentity
  relay: ProviderRelayTransport
  policy: LaborMarketPolicy
  store: LaborMarketStore
  now: Date
}): Promise<LaborMarketHandleResult> {
  const decision = evaluateLbrRequestForQuote({
    request: input.request,
    requestEventId: input.event.id,
    declaredCapabilityRefs: input.state.runtime.capabilityRefs,
    policy: input.policy,
    store: input.store,
    now: input.now,
  })
  if (!decision.quote) {
    return { handled: true, action: "refused", reasonRef: decision.reasonRef }
  }

  const quoteRef = stableRef("quote.public.pylon.labor_market", `${input.event.id}:${decision.amountMsats}`)
  const quote = makeLbrQuote({
    requestId: input.event.id,
    requesterPubkey: input.event.pubkey,
    amountMsats: decision.amountMsats,
    providerRef: stableRef("provider.public.pylon", input.identity.publicKey),
    capabilityRefs: input.request.requiredCapabilityRefs,
    quoteRef,
    requestRelay: input.relay.relayUrl,
    ...(input.request.deadline === undefined ? {} : { expiresAt: input.request.deadline }),
  })
  const draft = lbrQuoteToDraft(quote)
  const quoteEvent = signNostrEvent(
    {
      pubkey: input.identity.publicKey,
      created_at: Math.floor(input.now.getTime() / 1000),
      kind: draft.kind,
      tags: draft.tags.map((tag) => [...tag]),
      content: draft.content,
    },
    input.identity,
  )
  await input.relay.publish(quoteEvent)

  input.store.quotes[input.event.id] = {
    requestEventId: input.event.id,
    requesterPubkey: input.event.pubkey,
    quoteRef,
    quoteEventId: quoteEvent.id,
    amountMsats: decision.amountMsats,
    requestEvent: input.event,
    status: "quoted",
    quotedAt: input.now.toISOString(),
  }
  await writeLaborMarketStore(input.state, input.store)
  return { handled: true, action: "quoted", quoteEventId: quoteEvent.id, amountMsats: decision.amountMsats }
}

/**
 * The Claude Agent SDK-backed labor runtime: the claude_code lane runs a
 * bounded sandboxed session (escape denial, settings isolation, budgets)
 * instead of shelling out to a CLI. Refuses typed when the device probe
 * is not ready.
 */
export function makeClaudeAgentLaborRuntime(options: {
  runner?: ClaudeAgentRunner
  probe?: ClaudeAgentProbeOptions
  maxTurns?: number
  timeoutMs?: number
} = {}): LaborRuntime {
  return {
    async runLabor(run) {
      const probed = await probeClaudeAgentReadiness(options.probe ?? {})
      if (probed.state !== "ready") {
        throw new Error(`claude agent lane unavailable: ${probed.state}`)
      }
      const runner = options.runner ?? runWithClaudeAgentSdk
      const result = await runner({
        cwd: run.workspace.absolutePath,
        instructions: laborPrompt(run.request),
        allowedTools: ["Read", "Edit", "Write", "Bash", "Glob", "Grep"],
        maxTurns: options.maxTurns ?? 16,
        timeoutMs: options.timeoutMs ?? 300_000,
      })
      if (result.outcome !== "completed") {
        throw new Error(`claude agent labor session ended without completion: ${result.outcome}`)
      }
      const artifactRef = stableRef(
        "artifact.public.pylon.labor_market.patch",
        `${run.requestEventId}:${result.editedFileCount}`,
      )
      const receiptRef = stableRef(
        "receipt.public.pylon.labor_market.claude_agent",
        `${run.requestEventId}:${result.turnCount}:${result.commandCount}`,
      )
      return {
        artifactRefs: [artifactRef],
        content: `Local Claude Agent completed the labor job: ${result.editedFileCount} file edit(s), ${result.commandCount} command(s), ${result.turnCount} turn(s).`,
        model: "claude_agent_sdk",
        receiptRefs: [receiptRef],
      }
    },
  }
}

async function runVerificationCommand(input: {
  args: ReadonlyArray<string>
  cwd: string
}): Promise<{ exitCode: number }> {
  const proc = Bun.spawn([...input.args], { cwd: input.cwd, stderr: "pipe", stdout: "pipe" })
  const [, , exitCode] = await Promise.all([
    new Response(proc.stdout).arrayBuffer(),
    new Response(proc.stderr).arrayBuffer(),
    proc.exited,
  ])
  return { exitCode }
}

async function executeAcceptedLbrJob(input: {
  state: PylonLocalState
  acceptance: LbrAcceptance
  identity: PylonNostrPrivateIdentity
  relay: ProviderRelayTransport
  record: LaborMarketQuoteRecord
  store: LaborMarketStore
  options: LaborMarketOptions
  now: Date
}): Promise<LaborMarketHandleResult> {
  const request = decodeLbrRequest(input.record.requestEvent)
  if (request === null) {
    return { handled: true, action: "refused", reasonRef: "refusal.labor_market.stored_request_invalid" }
  }

  const verificationArgs =
    (input.options.verificationCommands ?? LABOR_MARKET_VERIFICATION_COMMANDS)[request.verificationCommandRef]
  if (verificationArgs === undefined) {
    return { handled: true, action: "refused", reasonRef: "refusal.labor_market.verification_command_unknown" }
  }

  const approved = await hasLaborFirstRunApproval(input.state.paths, request.labor)
  if (!approved) {
    // CL-16: surface a pending operator approval for the clients to resolve.
    input.options.onDeferredForApproval?.({
      approvalRef: stableRef("approval.public.pylon.labor.first_run", `${request.labor.jobType}:${request.labor.policyRef}`),
      jobType: request.labor.jobType,
      policyRef: request.labor.policyRef,
    })
    return { handled: true, action: "deferred", reasonRef: "labor_first_run_approval_required" }
  }

  const workspaceRef = stableRef("workspace.pylon.labor_market", input.record.requestEventId)
  const absolutePath = join(input.state.paths.cache, "labor-market", workspaceRef)
  await mkdir(absolutePath, { recursive: true })
  const workspace: LaborWorkspace = {
    absolutePath,
    relativePath: workspaceRef,
    root: join(input.state.paths.cache, "labor-market"),
  }

  input.record.status = "executing"
  await writeLaborMarketStore(input.state, input.store)

  const policy = input.options.policy ?? DEFAULT_LABOR_MARKET_POLICY
  const agentKind = policy.agentKind ?? "claude_code"
  const runtime =
    input.options.laborRuntime ??
    (agentKind === "claude_code"
      ? makeClaudeAgentLaborRuntime({
          ...(input.options.claudeAgentRunner === undefined ? {} : { runner: input.options.claudeAgentRunner }),
          ...(input.options.claudeAgentProbe === undefined ? {} : { probe: input.options.claudeAgentProbe }),
        })
      : makeConfiguredLaborRuntime())

  let objectiveDetail: string | null = null
  if (input.options.resolveObjectiveDetail !== undefined) {
    try {
      objectiveDetail = await input.options.resolveObjectiveDetail({
        objectiveRef: request.labor.inputRefs[0] ?? "",
        requestEventId: input.record.requestEventId,
        requesterPubkey: input.record.requesterPubkey,
      })
    } catch {
      objectiveDetail = null
    }
  }

  let completion
  try {
    completion = await runtime.runLabor({
      agentKind,
      request: request.labor,
      requestEventId: input.record.requestEventId,
      workspace,
      ...(objectiveDetail !== null && objectiveDetail.trim() !== ""
        ? { objectiveDetail }
        : {}),
    })
    assertLaborPublicSafe(completion)
  } catch {
    input.record.status = "refused"
    input.record.reasonRef = "refusal.labor_market.execution_refused"
    await writeLaborMarketStore(input.state, input.store)
    return { handled: true, action: "refused", reasonRef: "refusal.labor_market.execution_refused" }
  }

  const verification = await runVerificationCommand({ args: verificationArgs, cwd: absolutePath })
  if (verification.exitCode !== 0) {
    input.record.status = "refused"
    input.record.reasonRef = "refusal.labor_market.verification_failed"
    await writeLaborMarketStore(input.state, input.store)
    return { handled: true, action: "verification_failed", reasonRef: "refusal.labor_market.verification_failed" }
  }

  const testRef = stableRef(
    "test.public.pylon.labor_market.verification",
    `${input.record.requestEventId}:${request.verificationCommandRef}:${verification.exitCode}`,
  )
  const closeoutRef = stableRef(
    "closeout.public.pylon.labor_market",
    `${input.record.requestEventId}:${input.acceptance.acceptanceRef}:${completion.artifactRefs.join(",")}:${testRef}`,
  )
  const summaryRef = stableRef(
    "summary.public.pylon.labor_market",
    `${input.record.requestEventId}:${completion.model}`,
  )
  const result = makeLbrResult({
    requestId: input.record.requestEventId,
    requesterPubkey: input.record.requesterPubkey,
    artifactRefs: completion.artifactRefs,
    platformCloseoutRef: closeoutRef,
    summaryRef,
    testRef,
    requestRelay: input.relay.relayUrl,
  })
  const draft = lbrResultToDraft(result)
  const resultEvent = signNostrEvent(
    {
      pubkey: input.identity.publicKey,
      created_at: Math.floor(input.now.getTime() / 1000),
      kind: draft.kind,
      tags: draft.tags.map((tag) => [...tag]),
      content: draft.content,
    },
    input.identity,
  )
  await input.relay.publish(resultEvent)

  input.record.status = "delivered"
  input.record.resultEventId = resultEvent.id
  await writeLaborMarketStore(input.state, input.store)
  return { handled: true, action: "delivered", resultEventId: resultEvent.id, closeoutRef }
}

/**
 * Routes one relay event through the labor-market negotiation lane.
 * Returns { handled: false } when the event is not labor-market traffic,
 * so the generic provider loop keeps its existing behavior for everything
 * else.
 */
export async function handleLaborMarketEventOnce(input: {
  state: PylonLocalState
  event: NostrEvent
  identity: PylonNostrPrivateIdentity
  relay: ProviderRelayTransport
  options?: LaborMarketOptions
}): Promise<LaborMarketHandleResult> {
  const options = input.options ?? {}
  const now = options.now?.() ?? new Date()
  const store = await loadLaborMarketStore(input.state)

  const request = decodeLbrRequest(input.event)
  if (request !== null) {
    const policy = options.policy ?? await loadLaborMarketPolicy(input.state)
    return handleLbrRequest({
      state: input.state,
      event: input.event,
      request,
      identity: input.identity,
      relay: input.relay,
      policy,
      store,
      now,
    })
  }

  const acceptance = decodeAcceptanceFor(input.event, input.identity.publicKey)
  if (acceptance !== null) {
    const record = store.quotes[acceptance.requestId]
    if (record === undefined || record.status !== "quoted") {
      return { handled: true, action: "refused", reasonRef: "refusal.labor_market.acceptance_without_quote" }
    }
    if (acceptance.escrowReceiptRef.trim().length === 0) {
      return { handled: true, action: "refused", reasonRef: "refusal.labor_market.escrow_receipt_missing" }
    }
    return executeAcceptedLbrJob({
      state: input.state,
      acceptance,
      identity: input.identity,
      relay: input.relay,
      record,
      store,
      options: { ...options, policy: options.policy ?? await loadLaborMarketPolicy(input.state) },
      now,
    })
  }

  return { handled: false }
}

/**
 * REQ filters for the negotiation lane: open LBR requests plus feedback
 * addressed to this provider (acceptances).
 */
export function buildLaborMarketReqFilters(input: {
  providerPubkey: string
  since: number
  limit?: number
}) {
  return [
    {
      kinds: [LBR_AGENTIC_CODING_REQUEST_KIND],
      since: input.since,
      limit: input.limit ?? 64,
    },
    {
      kinds: [LBR_FEEDBACK_KIND],
      "#p": [input.providerPubkey],
      since: input.since,
      limit: input.limit ?? 64,
    },
  ]
}
