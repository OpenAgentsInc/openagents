import { describe, expect, test } from "bun:test"
import {
  KIND_JOB_FEEDBACK,
  createJobFeedbackEvent,
  createJobResultEvent,
  makeJobFeedback,
  makeJobResult,
} from "@openagentsinc/nip90"
import { deriveNip06Identity } from "../src/nostr-identity"
import { signNostrEvent, type NostrEvent } from "../src/provider-nip90"
import {
  DEFAULT_STRANGER_PROBE_BID_MSATS,
  ENV_STRANGER_PROBE_ALLOW_SPEND,
  STRANGER_PROBE_SCHEMA,
  assertStrangerProbeArtifactPublicSafe,
  buildProbeJobRequestEvent,
  buildProbeResponseFilters,
  buildRegisteredProviderMap,
  classifyProbeResponse,
  evaluatePaidLegGate,
  makeThrowawayCustomerIdentity,
  runStrangerProbe,
  type ProbeCollectionResult,
  type RegisteredProviderMap,
} from "../src/stranger-probe"

const fixedNow = () => new Date("2026-06-13T00:00:00.000Z")

// Keys injected: deterministic test identities derived from the repo's
// standard NIP-06 test vector mnemonics; never used outside fake transports.
const registeredProvider = deriveNip06Identity(
  "leader monkey parrot ring guide accident before fence cannon height naive bean",
  "memory:stranger-probe-test-registered-provider",
)
const strangerProvider = deriveNip06Identity(
  "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about",
  "memory:stranger-probe-test-stranger-provider",
)
const customer = makeThrowawayCustomerIdentity()

function pylonsPayload() {
  return {
    pylons: [
      {
        pylonRef: "pylon.test.registered.alpha",
        displayName: "Registered Alpha",
        providerNostrPubkey: registeredProvider.publicKey,
        providerNostrNpub: registeredProvider.npub,
        providerMarketRelayRefs: ["wss://relay.openagents.com"],
        providerNip90LaneRefs: ["lane.public.nip90.5050.text_generation"],
        latestHeartbeatStatus: "online",
        capabilityRefs: ["capability.public.pylon.nip90.text_inference.v0.3"],
      },
      {
        pylonRef: "pylon.test.no-provider-pubkey",
        displayName: "Legacy Without Pubkey",
        providerNostrPubkey: null,
        providerMarketRelayRefs: [],
        providerNip90LaneRefs: [],
        capabilityRefs: [],
      },
    ],
  }
}

function requestAndMap() {
  const map = buildRegisteredProviderMap(pylonsPayload())
  const request = buildProbeJobRequestEvent({
    identity: customer,
    prompt: "Bounded stranger probe test prompt.",
    bidMsats: DEFAULT_STRANGER_PROBE_BID_MSATS,
    createdAtSeconds: 1_781_000_000,
  })
  return { map, request }
}

function paymentRequiredEvent(input: {
  provider: typeof registeredProvider
  requestEventId: string
  bolt11?: string
}) {
  return signNostrEvent(
    createJobFeedbackEvent(
      makeJobFeedback({
        status: "payment-required",
        requestId: input.requestEventId,
        customerPubkey: customer.publicKey,
        amount: 21_000,
        bolt11: input.bolt11 ?? "lnbc210n1strangerprobetestinvoice",
        content: "",
      }),
      1_781_000_010,
    ) as Omit<NostrEvent, "id" | "sig">,
    input.provider,
  )
}

function resultEvent(input: { provider: typeof registeredProvider; requestEventId: string }) {
  return signNostrEvent(
    createJobResultEvent(
      makeJobResult({
        requestKind: 5050,
        requestId: input.requestEventId,
        customerPubkey: customer.publicKey,
        content: "probe result",
        inputs: [],
      }),
      1_781_000_020,
    ) as Omit<NostrEvent, "id" | "sig">,
    input.provider,
  )
}

describe("stranger probe registered-capacity mapping", () => {
  test("builds the pubkey map from /api/pylons provider fields and skips entries without pubkeys", () => {
    const map = buildRegisteredProviderMap(pylonsPayload())
    expect(map.providers).toHaveLength(1)
    expect(map.skippedWithoutPubkey).toBe(1)
    const entry = map.byPubkey.get(registeredProvider.publicKey.toLowerCase())
    expect(entry).toHaveLength(1)
    expect(entry?.[0].pylonRef).toBe("pylon.test.registered.alpha")
    expect(entry?.[0].providerNip90LaneRefs).toContain("lane.public.nip90.5050.text_generation")
    expect(entry?.[0].providerMarketRelayRefs).toContain("wss://relay.openagents.com")
  })

  test("tolerates malformed payloads", () => {
    expect(buildRegisteredProviderMap(null).providers).toHaveLength(0)
    expect(buildRegisteredProviderMap({ pylons: "nope" }).providers).toHaveLength(0)
    expect(buildRegisteredProviderMap({ pylons: [42, null, {}] }).providers).toHaveLength(0)
  })
})

describe("stranger probe request and filters", () => {
  test("publishes an untargeted bounded kind-5050 request with a bid", () => {
    const { request } = requestAndMap()
    expect(request.kind).toBe(5050)
    expect(request.pubkey).toBe(customer.publicKey)
    expect(request.tags).toContainEqual(["bid", String(DEFAULT_STRANGER_PROBE_BID_MSATS)])
    expect(request.tags.some((tag) => tag[0] === "p")).toBe(false)
  })

  test("response filters cover feedback and results by request id and customer pubkey", () => {
    const filters = buildProbeResponseFilters({
      requestEventId: "11".repeat(32),
      customerPubkey: customer.publicKey,
      sinceSeconds: 1_781_000_000,
    })
    expect(filters[0]["#e"]).toEqual(["11".repeat(32)])
    expect(filters[0].kinds).toEqual([KIND_JOB_FEEDBACK, 6050])
    expect(filters[1]["#p"]).toEqual([customer.publicKey])
  })
})

describe("stranger probe responder classification", () => {
  test("maps a registered responder via the #4864 provider pubkey and redacts the invoice to presence", () => {
    const { map, request } = requestAndMap()
    const record = classifyProbeResponse({
      event: paymentRequiredEvent({ provider: registeredProvider, requestEventId: request.id }),
      map,
      requestEventId: request.id,
      customerPubkey: customer.publicKey,
      observedAt: fixedNow(),
    })
    expect(record?.classification).toBe("registered")
    expect(record?.registeredPylonRefs).toEqual(["pylon.test.registered.alpha"])
    expect(record?.status).toBe("payment-required")
    expect(record?.amountMsats).toBe(21_000)
    expect(record?.hasBolt11Invoice).toBe(true)
    expect(record?.observedAt).toBe("2026-06-13T00:00:00.000Z")
    expect(JSON.stringify(record)).not.toContain("lnbc")
  })

  test("classifies an unmapped responder as unregistered", () => {
    const { map, request } = requestAndMap()
    const record = classifyProbeResponse({
      event: paymentRequiredEvent({ provider: strangerProvider, requestEventId: request.id }),
      map,
      requestEventId: request.id,
      customerPubkey: customer.publicKey,
      observedAt: fixedNow(),
    })
    expect(record?.classification).toBe("unregistered")
    expect(record?.registeredPylonRefs).toEqual([])
  })

  test("ignores events that reference neither the request nor the customer", () => {
    const { map, request } = requestAndMap()
    const unrelated = paymentRequiredEvent({ provider: strangerProvider, requestEventId: "22".repeat(32) })
    const stripped = { ...unrelated, tags: unrelated.tags.filter((tag) => tag[0] !== "p") }
    const record = classifyProbeResponse({
      event: stripped,
      map,
      requestEventId: request.id,
      customerPubkey: customer.publicKey,
      observedAt: fixedNow(),
    })
    expect(record).toBeNull()
  })

  test("ignores the customer's own published events", () => {
    const { map, request } = requestAndMap()
    const record = classifyProbeResponse({
      event: { ...request, tags: [...request.tags, ["e", request.id]] },
      map,
      requestEventId: request.id,
      customerPubkey: customer.publicKey,
      observedAt: fixedNow(),
    })
    expect(record).toBeNull()
  })
})

describe("stranger probe paid-leg gate", () => {
  test("refuses by default", () => {
    const gate = evaluatePaidLegGate({ paidFlag: false, env: {} })
    expect(gate.authorized).toBe(false)
    if (!gate.authorized) expect(gate.reasonRef).toBe("blocker.pylon.stranger_probe.paid_leg_not_requested")
  })

  test("refuses --paid without the spend env guard", () => {
    const gate = evaluatePaidLegGate({ paidFlag: true, env: {} })
    expect(gate.authorized).toBe(false)
    if (!gate.authorized) expect(gate.reasonRef).toBe("blocker.pylon.stranger_probe.spend_env_guard_missing")
  })

  test("refuses the env guard without the explicit --paid flag", () => {
    const gate = evaluatePaidLegGate({ paidFlag: false, env: { [ENV_STRANGER_PROBE_ALLOW_SPEND]: "1" } })
    expect(gate.authorized).toBe(false)
    if (!gate.authorized) expect(gate.reasonRef).toBe("blocker.pylon.stranger_probe.paid_flag_missing")
  })

  test("authorizes only with both the flag and the exact env guard value", () => {
    expect(evaluatePaidLegGate({ paidFlag: true, env: { [ENV_STRANGER_PROBE_ALLOW_SPEND]: "1" } }).authorized).toBe(true)
    expect(evaluatePaidLegGate({ paidFlag: true, env: { [ENV_STRANGER_PROBE_ALLOW_SPEND]: "true" } }).authorized).toBe(false)
  })
})

type FakeProbeOptions = {
  responders?: (requestEventId: string) => NostrEvent[]
  paidFlag?: boolean
  env?: NodeJS.ProcessEnv
  walletRunner?: (args: string[]) => Promise<{ exitCode: number; stdout: string; stderr: string }>
}

async function runFakeProbe(options: FakeProbeOptions, map?: RegisteredProviderMap) {
  const providerMap = map ?? buildRegisteredProviderMap(pylonsPayload())
  const published: NostrEvent[] = []
  const artifact = await runStrangerProbe({
    relayUrl: "wss://relay.test/",
    baseUrl: "https://openagents.test",
    paidFlag: options.paidFlag ?? false,
    env: options.env ?? {},
    identity: customer,
    now: fixedNow,
    fetchProviders: async () => providerMap,
    publishRequest: async (_relayUrl, event) => {
      published.push(event)
      return { relayUrl: "wss://relay.test/", accepted: true, message: "" }
    },
    collectResponses: async (): Promise<ProbeCollectionResult> => ({
      events: options.responders === undefined ? [] : options.responders(published[0].id),
      relayClosedEarly: false,
    }),
    walletRunner: options.walletRunner,
  })
  return { artifact, published }
}

describe("stranger probe artifact", () => {
  test("records the honest zero-responder baseline as a passed run", async () => {
    const { artifact, published } = await runFakeProbe({})
    expect(artifact.schema).toBe(STRANGER_PROBE_SCHEMA)
    expect(artifact.mode).toBe("no_spend")
    expect(artifact.request.eventId).toBe(published[0].id)
    expect(artifact.request.acceptedByRelay).toBe(true)
    expect(artifact.registeredCapacity.providerCount).toBe(1)
    expect(artifact.collection.responderCount).toBe(0)
    expect(artifact.verdict.status).toBe("passed")
    expect(artifact.verdict.zeroRegisteredResponders).toBe(true)
    expect(artifact.paidLeg.attempted).toBe(false)
    if (!artifact.paidLeg.attempted) {
      expect(artifact.paidLeg.reasonRef).toBe("blocker.pylon.stranger_probe.paid_leg_not_requested")
    }
    expect(artifact.provenance.forumPostRef).toBe("forum.post.7be6aa0a-c64a-466f-b90e-45e1d24ef93f")
  })

  test("classifies registered and unregistered responders and stays redaction-safe", async () => {
    const { artifact } = await runFakeProbe({
      responders: (requestEventId) => [
        paymentRequiredEvent({ provider: registeredProvider, requestEventId }),
        paymentRequiredEvent({ provider: strangerProvider, requestEventId }),
      ],
    })
    expect(artifact.collection.responderCount).toBe(2)
    expect(artifact.collection.registeredResponderCount).toBe(1)
    expect(artifact.collection.unregisteredResponderCount).toBe(1)
    expect(artifact.verdict.zeroRegisteredResponders).toBe(false)
    const registered = artifact.collection.responses.find((record) => record.classification === "registered")
    expect(registered?.registeredPylonRefs).toEqual(["pylon.test.registered.alpha"])
    expect(registered?.hasBolt11Invoice).toBe(true)
    const serialized = JSON.stringify(artifact)
    expect(serialized).not.toContain("lnbc")
    expect(serialized).not.toContain(customer.nsec)
    expect(serialized).not.toContain(customer.privateKeyHex)
    expect(serialized.toLowerCase()).not.toContain("mnemonic")
    assertStrangerProbeArtifactPublicSafe(artifact)
  })

  test("never invokes the wallet runner when the paid leg is not authorized", async () => {
    let walletCalls = 0
    const { artifact } = await runFakeProbe({
      paidFlag: true,
      env: {},
      responders: (requestEventId) => [paymentRequiredEvent({ provider: registeredProvider, requestEventId })],
      walletRunner: async () => {
        walletCalls += 1
        return { exitCode: 0, stdout: "{}", stderr: "" }
      },
    })
    expect(walletCalls).toBe(0)
    expect(artifact.mode).toBe("no_spend")
    expect(artifact.paidLeg.attempted).toBe(false)
    if (!artifact.paidLeg.attempted) {
      expect(artifact.paidLeg.authorized).toBe(false)
      expect(artifact.paidLeg.reasonRef).toBe("blocker.pylon.stranger_probe.spend_env_guard_missing")
    }
  })

  test("authorized paid leg settles only a registered payment-required quote and keeps the invoice out of the artifact", async () => {
    const walletArgs: string[][] = []
    const { artifact } = await runFakeProbe({
      paidFlag: true,
      env: { [ENV_STRANGER_PROBE_ALLOW_SPEND]: "1" },
      responders: (requestEventId) => [
        paymentRequiredEvent({ provider: strangerProvider, requestEventId }),
        paymentRequiredEvent({ provider: registeredProvider, requestEventId }),
        resultEvent({ provider: registeredProvider, requestEventId }),
      ],
      walletRunner: async (args) => {
        walletArgs.push(args)
        return { exitCode: 0, stdout: JSON.stringify({ status: "settled" }), stderr: "" }
      },
    })
    expect(walletArgs).toHaveLength(1)
    expect(walletArgs[0][0]).toBe("send")
    expect(walletArgs[0][1]).toStartWith("lnbc")
    expect(artifact.mode).toBe("paid")
    expect(artifact.paidLeg.attempted).toBe(true)
    if (artifact.paidLeg.attempted) {
      expect(artifact.paidLeg.settled).toBe(true)
      expect(artifact.paidLeg.amountMsats).toBe(21_000)
      expect(artifact.paidLeg.providerPubkey).toBe(registeredProvider.publicKey)
      expect(artifact.paidLeg.registeredPylonRefs).toEqual(["pylon.test.registered.alpha"])
      expect(artifact.paidLeg.settlementReceiptRef).toStartWith("receipt.public.pylon.stranger_probe.settlement.")
      expect(artifact.paidLeg.resultEventId).toBeDefined()
    }
    expect(JSON.stringify(artifact)).not.toContain("lnbc")
    assertStrangerProbeArtifactPublicSafe(artifact)
  })

  test("authorized paid leg refuses when only unregistered responders quoted", async () => {
    let walletCalls = 0
    const { artifact } = await runFakeProbe({
      paidFlag: true,
      env: { [ENV_STRANGER_PROBE_ALLOW_SPEND]: "1" },
      responders: (requestEventId) => [paymentRequiredEvent({ provider: strangerProvider, requestEventId })],
      walletRunner: async () => {
        walletCalls += 1
        return { exitCode: 0, stdout: "{}", stderr: "" }
      },
    })
    expect(walletCalls).toBe(0)
    expect(artifact.mode).toBe("no_spend")
    expect(artifact.paidLeg.attempted).toBe(false)
    if (!artifact.paidLeg.attempted) {
      expect(artifact.paidLeg.authorized).toBe(true)
      expect(artifact.paidLeg.reasonRef).toBe("blocker.pylon.stranger_probe.no_registered_payment_required_quote")
    }
  })

  test("a relay-rejected request blocks the verdict", async () => {
    const map = buildRegisteredProviderMap(pylonsPayload())
    const artifact = await runStrangerProbe({
      relayUrl: "wss://relay.test/",
      baseUrl: "https://openagents.test",
      paidFlag: false,
      env: {},
      identity: customer,
      now: fixedNow,
      fetchProviders: async () => map,
      publishRequest: async () => ({ relayUrl: "wss://relay.test/", accepted: false, message: "blocked: rate limit" }),
      collectResponses: async () => {
        throw new Error("collection must not run when the relay rejected the request")
      },
    })
    expect(artifact.verdict.status).toBe("blocked")
    expect(artifact.verdict.blockerRefs).toContain("blocker.pylon.stranger_probe.request_rejected_by_relay")
    expect(artifact.collection.collectedEventCount).toBe(0)
  })
})

describe("stranger probe public-safety assertion", () => {
  test("rejects artifacts carrying invoice-shaped or key material", async () => {
    const { artifact } = await runFakeProbe({})
    const poisoned = JSON.parse(JSON.stringify(artifact)) as typeof artifact
    poisoned.request.relayMessage = "lnbc210n1leakedinvoice"
    expect(() => assertStrangerProbeArtifactPublicSafe(poisoned)).toThrow(
      /private-data-shaped text|private payment, key, or auth material/,
    )

    const keyLeak = JSON.parse(JSON.stringify(artifact)) as typeof artifact & { extra?: string }
    keyLeak.extra = `nsec1${"q".repeat(58)}`
    expect(() => assertStrangerProbeArtifactPublicSafe(keyLeak)).toThrow(
      "stranger probe artifact contains private payment, key, or auth material",
    )
  })
})
