import { mkdtemp, rm, writeFile } from "node:fs/promises"
import { join } from "node:path"
import { tmpdir } from "node:os"
import { describe, expect, test } from "bun:test"
import {
  LBR_AGENTIC_CODING_REQUEST_KIND,
  decodeLbrQuoteEvent,
  decodeLbrResultEvent,
  lbrAcceptanceToDraft,
  lbrAgenticCodingRequestToDraft,
  makeLbrAcceptance,
  makeLbrAgenticCodingRequest,
} from "@openagentsinc/nip90"
import { createBootstrapSummary, parseBootstrapArgs } from "../src/bootstrap"
import { approveLaborFirstRun } from "../src/labor"
import { CLAUDE_AGENT_CAPABILITY_REF, CLAUDE_AGENT_SDK_PACKAGE } from "../src/claude-agent"
import {
  DEFAULT_LABOR_MARKET_POLICY,
  buildLaborMarketReqFilters,
  evaluateLbrRequestForQuote,
  handleLaborMarketEventOnce,
  loadLaborMarketStore,
  makeClaudeAgentLaborRuntime,
  type LaborMarketPolicy,
  type LaborMarketStore,
} from "../src/labor-market"
import { deriveNip06Identity, loadOrCreateNostrIdentity } from "../src/nostr-identity"
import {
  runProviderJobOnce,
  signNostrEvent,
  type NostrEvent,
  type ProviderRelayTransport,
} from "../src/provider-nip90"
import { ensurePylonLocalState } from "../src/state"
import type { ClaudeAgentRunner } from "../src/claude-agent-executor"

const requester = deriveNip06Identity(
  "leader monkey parrot ring guide accident before fence cannon height naive bean",
  "/tmp/pylon-labor-market-test-requester.mnemonic",
)

const VERIFICATION_REF = "command.public.pylon.labor.bun_test_sum"

async function withTempHome<T>(fn: (home: string) => Promise<T>) {
  const home = await mkdtemp(join(tmpdir(), "pylon-labor-market-test-"))
  try {
    return await fn(home)
  } finally {
    await rm(home, { recursive: true, force: true })
  }
}

async function readyState(home: string) {
  const summary = createBootstrapSummary(
    parseBootstrapArgs(["--display-name", "Labor Market Test"]),
    { PYLON_HOME: home },
    "darwin",
  )
  const state = await ensurePylonLocalState(summary)
  state.runtime.lifecycle = "online"
  state.runtime.capabilityRefs = [CLAUDE_AGENT_CAPABILITY_REF]
  return { summary, state, identity: await loadOrCreateNostrIdentity(summary.paths) }
}

class FakeRelay implements ProviderRelayTransport {
  readonly relayUrl = "wss://fake.relay.test"
  readonly published: NostrEvent[] = []
  async publish(event: NostrEvent) {
    this.published.push(event)
    return { accepted: true, eventId: event.id }
  }
  async *subscribe(): AsyncGenerator<NostrEvent> {}
  async close() {}
}

function lbrRequest(overrides: { bidMsats?: number; requiredCapabilityRefs?: string[] } = {}) {
  return makeLbrAgenticCodingRequest({
    objectiveRef: "goal.public.labor_market.fixture_repair",
    repositoryRefs: ["repo.public.github.openagentsinc.fixture"],
    verificationCommandRef: VERIFICATION_REF,
    requiredCapabilityRefs: overrides.requiredCapabilityRefs ?? [CLAUDE_AGENT_CAPABILITY_REF],
    bidMsats: overrides.bidMsats ?? 2_000_000,
  })
}

function signedRequestEvent(input: { bidMsats?: number; requiredCapabilityRefs?: string[] } = {}) {
  const draft = lbrAgenticCodingRequestToDraft(lbrRequest(input))
  return signNostrEvent(
    {
      pubkey: requester.publicKey,
      created_at: Math.floor(Date.now() / 1000),
      kind: draft.kind,
      tags: draft.tags.map((tag) => [...tag]),
      content: draft.content,
    },
    requester,
  )
}

function signedAcceptanceEvent(input: { requestId: string; providerPubkey: string }) {
  const acceptance = makeLbrAcceptance({
    requestId: input.requestId,
    providerPubkey: input.providerPubkey,
    escrowReceiptRef: "escrow.public.labor_market.test_reserved",
    acceptanceRef: "acceptance.public.labor_market.test",
  })
  const draft = lbrAcceptanceToDraft(acceptance)
  return signNostrEvent(
    {
      pubkey: requester.publicKey,
      created_at: Math.floor(Date.now() / 1000),
      kind: draft.kind,
      tags: draft.tags.map((tag) => [...tag]),
      content: draft.content,
    },
    requester,
  )
}

const readyProbe = {
  env: { ANTHROPIC_API_KEY: "test-key-shape" },
  platform: "darwin",
  importer: async (specifier: string) => {
    if (specifier !== CLAUDE_AGENT_SDK_PACKAGE) throw new Error("unexpected import")
    return {}
  },
}

const fixingRunner: ClaudeAgentRunner = async (input) => {
  await writeFile(
    join(input.cwd, "sum.ts"),
    "export const sum = (left: number, right: number) => left + right\n",
  )
  await writeFile(
    join(input.cwd, "sum.test.ts"),
    [
      'import { expect, test } from "bun:test"',
      'import { sum } from "./sum"',
      'test("adds", () => { expect(sum(2, 3)).toBe(5) })',
      "",
    ].join("\n"),
  )
  return { outcome: "completed", turnCount: 2, editedFileCount: 2, commandCount: 0, sessionRef: null }
}

const quotingPolicy: LaborMarketPolicy = {
  ...DEFAULT_LABOR_MARKET_POLICY,
  autoQuote: true,
  priceMsats: 1_500_000,
  agentKind: "claude_code",
}

const emptyStore = (): LaborMarketStore => ({
  schema: "openagents.pylon.labor_market_state.v0.3",
  quotes: {},
})

describe("labor market quoting policy", () => {
  const request = lbrRequest()
  const base = {
    request,
    requestEventId: "event.test.request",
    declaredCapabilityRefs: [CLAUDE_AGENT_CAPABILITY_REF],
    policy: quotingPolicy,
    store: emptyStore(),
    now: new Date("2026-06-10T23:00:00.000Z"),
  }

  test("quoting is opt-in: default policy refuses", () => {
    const decision = evaluateLbrRequestForQuote({ ...base, policy: DEFAULT_LABOR_MARKET_POLICY })
    expect(decision).toEqual({ quote: false, reasonRef: "refusal.labor_market.auto_quote_disabled" })
  })

  test("capability-untrue requests are never quoted", () => {
    const decision = evaluateLbrRequestForQuote({ ...base, declaredCapabilityRefs: [] })
    expect(decision).toEqual({ quote: false, reasonRef: "refusal.labor_market.capability_untrue" })
  })

  test("the contributor's price must fit the bid", () => {
    const decision = evaluateLbrRequestForQuote({
      ...base,
      request: lbrRequest({ bidMsats: 1_000_000 }),
    })
    expect(decision).toEqual({ quote: false, reasonRef: "refusal.labor_market.price_above_bid" })
  })

  test("quote-once: an existing record refuses a second quote", () => {
    const store = emptyStore()
    store.quotes["event.test.request"] = {
      requestEventId: "event.test.request",
      requesterPubkey: requester.publicKey,
      quoteRef: "quote.public.test",
      quoteEventId: "event.test.quote",
      amountMsats: 1_500_000,
      requestEvent: {} as NostrEvent,
      status: "quoted",
      quotedAt: "2026-06-10T22:59:00.000Z",
    }
    const decision = evaluateLbrRequestForQuote({ ...base, store })
    expect(decision).toEqual({ quote: false, reasonRef: "refusal.labor_market.already_quoted" })
  })

  test("concurrency bound refuses when inflight jobs fill the policy", () => {
    const store = emptyStore()
    store.quotes["event.other"] = {
      requestEventId: "event.other",
      requesterPubkey: requester.publicKey,
      quoteRef: "quote.public.other",
      quoteEventId: "event.other.quote",
      amountMsats: 1_500_000,
      requestEvent: {} as NostrEvent,
      status: "executing",
      quotedAt: "2026-06-10T22:59:00.000Z",
    }
    const decision = evaluateLbrRequestForQuote({ ...base, store })
    expect(decision).toEqual({ quote: false, reasonRef: "refusal.labor_market.max_concurrent_jobs" })
  })

  test("a capability-true, price-fitting request gets a quote", () => {
    const decision = evaluateLbrRequestForQuote(base)
    expect(decision).toEqual({ quote: true, amountMsats: 1_500_000 })
  })
})

describe("labor market negotiation flow", () => {
  test("an LBR request is quoted, never auto-executed", async () => {
    await withTempHome(async (home) => {
      const { state, identity } = await readyState(home)
      const relay = new FakeRelay()
      const event = signedRequestEvent()
      const result = await handleLaborMarketEventOnce({
        state,
        event,
        identity,
        relay,
        options: { policy: quotingPolicy },
      })
      expect(result).toMatchObject({ handled: true, action: "quoted", amountMsats: 1_500_000 })
      expect(relay.published).toHaveLength(1)
      const quote = decodeLbrQuoteEvent(relay.published[0])
      expect(quote.requestId).toBe(event.id)
      expect(quote.amountMsats).toBe(1_500_000)
      const store = await loadLaborMarketStore(state)
      expect(store.quotes[event.id]?.status).toBe("quoted")
    })
  })

  test("default policy means a request is refused without any relay traffic", async () => {
    await withTempHome(async (home) => {
      const { state, identity } = await readyState(home)
      const relay = new FakeRelay()
      const result = await handleLaborMarketEventOnce({
        state,
        event: signedRequestEvent(),
        identity,
        relay,
      })
      expect(result).toMatchObject({
        handled: true,
        action: "refused",
        reasonRef: "refusal.labor_market.auto_quote_disabled",
      })
      expect(relay.published).toEqual([])
    })
  })

  test("acceptance without a prior quote is refused", async () => {
    await withTempHome(async (home) => {
      const { state, identity } = await readyState(home)
      const relay = new FakeRelay()
      const result = await handleLaborMarketEventOnce({
        state,
        event: signedAcceptanceEvent({ requestId: "ab".repeat(32), providerPubkey: identity.publicKey }),
        identity,
        relay,
        options: { policy: quotingPolicy },
      })
      expect(result).toMatchObject({
        handled: true,
        action: "refused",
        reasonRef: "refusal.labor_market.acceptance_without_quote",
      })
    })
  })

  test("acceptance addressed to a different provider is not labor-market traffic for us", async () => {
    await withTempHome(async (home) => {
      const { state, identity } = await readyState(home)
      const relay = new FakeRelay()
      const result = await handleLaborMarketEventOnce({
        state,
        event: signedAcceptanceEvent({ requestId: "cd".repeat(32), providerPubkey: requester.publicKey }),
        identity,
        relay,
        options: { policy: quotingPolicy },
      })
      expect(result).toEqual({ handled: false })
    })
  })

  test("quoted + accepted + approved executes on the local agent and delivers output-only", async () => {
    await withTempHome(async (home) => {
      const { state, identity } = await readyState(home)
      await approveLaborFirstRun({ paths: state.paths, approvedByRef: "operator.test", jobType: "code_task" })
      const relay = new FakeRelay()
      const requestEvent = signedRequestEvent()

      const quoted = await handleLaborMarketEventOnce({
        state,
        event: requestEvent,
        identity,
        relay,
        options: { policy: quotingPolicy, claudeAgentRunner: fixingRunner, claudeAgentProbe: readyProbe },
      })
      expect(quoted).toMatchObject({ action: "quoted" })

      const delivered = await handleLaborMarketEventOnce({
        state,
        event: signedAcceptanceEvent({ requestId: requestEvent.id, providerPubkey: identity.publicKey }),
        identity,
        relay,
        options: { policy: quotingPolicy, claudeAgentRunner: fixingRunner, claudeAgentProbe: readyProbe },
      })
      expect(delivered).toMatchObject({ handled: true, action: "delivered" })
      expect(relay.published).toHaveLength(2)
      const result = decodeLbrResultEvent(relay.published[1])
      expect(result.labor.result.requestId).toBe(requestEvent.id)
      expect(result.artifactRefs.length).toBeGreaterThan(0)
      expect(result.platformCloseoutRef).toStartWith("closeout.public.pylon.labor_market.")
      const serialized = JSON.stringify(relay.published)
      expect(serialized).not.toContain(state.paths.cache)
      expect(serialized).not.toContain("ANTHROPIC")
      const store = await loadLaborMarketStore(state)
      expect(store.quotes[requestEvent.id]?.status).toBe("delivered")
    })
  })

  test("missing first-run approval defers instead of executing", async () => {
    await withTempHome(async (home) => {
      const { state, identity } = await readyState(home)
      const relay = new FakeRelay()
      const requestEvent = signedRequestEvent()
      await handleLaborMarketEventOnce({
        state,
        event: requestEvent,
        identity,
        relay,
        options: { policy: quotingPolicy, claudeAgentRunner: fixingRunner, claudeAgentProbe: readyProbe },
      })
      const deferred = await handleLaborMarketEventOnce({
        state,
        event: signedAcceptanceEvent({ requestId: requestEvent.id, providerPubkey: identity.publicKey }),
        identity,
        relay,
        options: { policy: quotingPolicy, claudeAgentRunner: fixingRunner, claudeAgentProbe: readyProbe },
      })
      expect(deferred).toMatchObject({
        handled: true,
        action: "deferred",
        reasonRef: "labor_first_run_approval_required",
      })
      expect(relay.published).toHaveLength(1)
    })
  })

  test("a failing verification command means no result is published", async () => {
    await withTempHome(async (home) => {
      const { state, identity } = await readyState(home)
      await approveLaborFirstRun({ paths: state.paths, approvedByRef: "operator.test", jobType: "code_task" })
      const relay = new FakeRelay()
      const requestEvent = signedRequestEvent()
      const brokenRunner: ClaudeAgentRunner = async (input) => {
        await writeFile(join(input.cwd, "sum.ts"), "export const sum = () => 0\n")
        await writeFile(
          join(input.cwd, "sum.test.ts"),
          [
            'import { expect, test } from "bun:test"',
            'import { sum } from "./sum"',
            'test("adds", () => { expect(sum(2, 3)).toBe(5) })',
            "",
          ].join("\n"),
        )
        return { outcome: "completed", turnCount: 1, editedFileCount: 2, commandCount: 0, sessionRef: null }
      }
      await handleLaborMarketEventOnce({
        state,
        event: requestEvent,
        identity,
        relay,
        options: { policy: quotingPolicy, claudeAgentRunner: brokenRunner, claudeAgentProbe: readyProbe },
      })
      const failed = await handleLaborMarketEventOnce({
        state,
        event: signedAcceptanceEvent({ requestId: requestEvent.id, providerPubkey: identity.publicKey }),
        identity,
        relay,
        options: { policy: quotingPolicy, claudeAgentRunner: brokenRunner, claudeAgentProbe: readyProbe },
      })
      expect(failed).toMatchObject({
        handled: true,
        action: "verification_failed",
        reasonRef: "refusal.labor_market.verification_failed",
      })
      expect(relay.published).toHaveLength(1)
      const store = await loadLaborMarketStore(state)
      expect(store.quotes[requestEvent.id]?.status).toBe("refused")
    })
  })

  test("claude agent runtime refuses typed when the probe is not ready", async () => {
    const runtime = makeClaudeAgentLaborRuntime({
      probe: { env: {}, platform: "darwin", importer: async () => { throw new Error("missing") } },
    })
    await expect(
      runtime.runLabor({
        agentKind: "claude_code",
        request: lbrRequest().labor,
        requestEventId: "event.test",
        workspace: { absolutePath: "/tmp/never-used", relativePath: "never-used", root: "/tmp" },
      }),
    ).rejects.toThrow("claude agent lane unavailable")
  })
})

describe("provider loop integration", () => {
  test("runProviderJobOnce routes LBR requests to the market lane (no invoice, no execution)", async () => {
    await withTempHome(async (home) => {
      const { state, identity } = await readyState(home)
      const relay = new FakeRelay()
      const event = signedRequestEvent()
      const result = await runProviderJobOnce({
        state,
        event,
        identity,
        relay,
        laborMarket: { policy: quotingPolicy, claudeAgentRunner: fixingRunner, claudeAgentProbe: readyProbe },
        online: true,
      })
      expect(result.status).toBe("accepted")
      expect(result.laborMarket).toMatchObject({ action: "quoted" })
      expect(relay.published).toHaveLength(1)
      expect(relay.published[0]?.kind).toBe(7000)
    })
  })

  test("the REQ filter set includes the acceptance lane", () => {
    const filters = buildLaborMarketReqFilters({ providerPubkey: "ab".repeat(32), since: 0 })
    expect(filters[0]?.kinds).toEqual([LBR_AGENTIC_CODING_REQUEST_KIND])
    expect(filters[1]?.kinds).toEqual([7000])
    expect(filters[1]?.["#p"]).toEqual(["ab".repeat(32)])
  })
})
