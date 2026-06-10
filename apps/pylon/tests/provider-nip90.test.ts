import { mkdir, mkdtemp, rm, writeFile } from "node:fs/promises"
import { join } from "node:path"
import { tmpdir } from "node:os"
import { describe, expect, test } from "bun:test"
import {
  KIND_JOB_FEEDBACK,
  KIND_JOB_LABOR_CODE_TASK,
  KIND_JOB_TEXT_GENERATION,
  createJobRequestEvent,
  jobInput,
  jobParam,
  makeJobRequest,
  makeLaborJobRequest,
  parseJobFeedbackEvent,
  parseJobResultEvent,
  parseLaborJobResultEvent,
} from "@openagents/nip90"
import { createBootstrapSummary, parseBootstrapArgs } from "../src/bootstrap"
import { approveLaborFirstRun, laborResultContent } from "../src/labor"
import { deriveNip06Identity, loadOrCreateNostrIdentity } from "../src/nostr-identity"
import {
  buildNip89HandlerInfoEvent,
  buildProviderReqFilters,
  classifyProviderRequestEvent,
  defaultProviderAdmissionPolicy,
  emptyProviderAdmissionStore,
  evaluateProviderAdmission,
  loadProviderAdmissionStore,
  runProviderJobOnce,
  signNostrEvent,
  type NostrEvent,
  type ProviderRelayTransport,
} from "../src/provider-nip90"
import { ensurePylonLocalState, type PylonLocalState } from "../src/state"
import type { WalletCommandRunner } from "../src/wallet"

const buyer = deriveNip06Identity(
  "leader monkey parrot ring guide accident before fence cannon height naive bean",
  "/tmp/pylon-provider-test-buyer.mnemonic",
)

async function withTempHome<T>(fn: (home: string) => Promise<T>) {
  const home = await mkdtemp(join(tmpdir(), "pylon-provider-nip90-test-"))
  try {
    return await fn(home)
  } finally {
    await rm(home, { recursive: true, force: true })
  }
}

async function readyState(home: string) {
  const summary = createBootstrapSummary(
    parseBootstrapArgs(["--display-name", "Provider Test", "--capability-ref", "capability.public.pylon.nip90.text_inference.v0.3"]),
    { PYLON_HOME: home },
    "darwin",
  )
  const state = await ensurePylonLocalState(summary)
  state.runtime.lifecycle = "online"
  state.runtime.capabilityRefs = ["capability.public.pylon.nip90.text_inference.v0.3"]
  return { summary, state, identity: await loadOrCreateNostrIdentity(summary.paths) }
}

function requestEvent(input: {
  providerPubkey?: string
  content?: string
  bid?: number
  output?: string
  kind?: number
  createdAt?: number
}) {
  const request = makeJobRequest({
    kind: input.kind ?? KIND_JOB_TEXT_GENERATION,
    inputs: [jobInput.text(input.content ?? "Summarize the public OpenAgents market relay.")],
    bid: input.bid,
    output: input.output,
    serviceProviders: input.providerPubkey ? [input.providerPubkey] : [],
  })
  return signNostrEvent(
    createJobRequestEvent(request, input.createdAt ?? 1_781_000_000),
    buyer,
  )
}

function laborRequestEvent(input: {
  providerPubkey?: string
  content?: string
  bid?: number
  workspace?: string
}) {
  const request = makeLaborJobRequest({
    jobType: "code_task",
    inputRefs: ["fixture.public.failing-test"],
    acceptanceCriteria: ["bounded fixture test passes"],
    bid: input.bid,
    serviceProviders: input.providerPubkey ? [input.providerPubkey] : [],
    content: input.content ?? "Fix the failing fixture test and return a public-safe artifact ref.",
    params: input.workspace === undefined ? [] : [jobParam("workspace", input.workspace)],
  })
  return signNostrEvent(
    createJobRequestEvent(request.request, 1_781_000_000),
    buyer,
  )
}

class FakeRelay implements ProviderRelayTransport {
  readonly relayUrl = "wss://relay.test/"
  readonly published: NostrEvent[] = []

  async publish(event: NostrEvent) {
    this.published.push(event)
    return { relayUrl: this.relayUrl, accepted: true, message: "" }
  }

  async *subscribe() {
    return
  }
}

const walletRunner: WalletCommandRunner = async (args) => {
  expect(args).toEqual(["receive", "1"])
  return {
    exitCode: 0,
    stdout: JSON.stringify({ invoice: "lnbc10n1providerinvoice" }),
    stderr: "",
  }
}

describe("Pylon NIP-90 provider loop", () => {
  test("advertises and subscribes to text inference and labor job kinds", async () => {
    await withTempHome(async (home) => {
      const { identity } = await readyState(home)
      const handler = buildNip89HandlerInfoEvent({
        identity,
        relayUrls: ["wss://relay.test/"],
        priceMsats: 1_000,
        now: new Date("2026-06-10T00:00:00.000Z"),
      })
      const filters = buildProviderReqFilters({
        providerPubkey: identity.publicKey,
        since: 1_781_000_000,
      })

      expect(handler.tags).toContainEqual(["k", String(KIND_JOB_TEXT_GENERATION)])
      expect(handler.tags).toContainEqual(["k", String(KIND_JOB_LABOR_CODE_TASK)])
      expect(filters[0].kinds).toContain(KIND_JOB_TEXT_GENERATION)
      expect(filters[0].kinds).toContain(KIND_JOB_LABOR_CODE_TASK)
    })
  })

  test("classifies only local, public kind 5050 text inference requests", async () => {
    await withTempHome(async (home) => {
      const { identity } = await readyState(home)
      const matched = classifyProviderRequestEvent({
        event: requestEvent({ providerPubkey: identity.publicKey, bid: 1_000 }),
        providerPubkey: identity.publicKey,
        relayUrl: "wss://relay.test/",
      })

      expect(matched.decision).toBe("match")
      expect(matched.targeted).toBe(true)
      expect(matched.promptPreview).toContain("Summarize")
      expect(matched.bidMsats).toBe(1_000)

      const targetMismatch = classifyProviderRequestEvent({
        event: requestEvent({ providerPubkey: "11".repeat(32), bid: 1_000 }),
        providerPubkey: identity.publicKey,
      })
      expect(targetMismatch.decision).toBe("drop")
      expect(targetMismatch.dropReason).toBe("target_mismatch")

      const unsupportedOutput = classifyProviderRequestEvent({
        event: requestEvent({ providerPubkey: identity.publicKey, bid: 1_000, output: "application/json" }),
        providerPubkey: identity.publicKey,
      })
      expect(unsupportedOutput.decision).toBe("drop")
      expect(unsupportedOutput.dropReason).toBe("unsupported_output")
    })
  })

  test("defers labor jobs until first-run operator approval is recorded", async () => {
    await withTempHome(async (home) => {
      const { state, identity } = await readyState(home)
      const relay = new FakeRelay()
      const event = laborRequestEvent({ providerPubkey: identity.publicKey, bid: 0 })

      const result = await runProviderJobOnce({
        state,
        event,
        identity,
        relay,
        policy: { priceMsats: 0 },
        laborWorkspaceRoot: home,
        laborAgentKind: "test_fixture",
        laborRuntime: {
          async runLabor() {
            throw new Error("labor runtime should not execute before approval")
          },
        },
        now: () => new Date("2026-06-10T00:00:00.000Z"),
      })

      expect(result.status).toBe("deferred")
      expect(result.reasonRef).toBe("labor_first_run_approval_required")
      expect(relay.published).toEqual([])
      const store = await loadProviderAdmissionStore(state.paths)
      expect(store.handledRequests[event.id]).toBeUndefined()
    })
  })

  test("blocks provider-auth-shaped labor requests before runtime execution", async () => {
    await withTempHome(async (home) => {
      const { state, identity } = await readyState(home)
      await approveLaborFirstRun({
        paths: state.paths,
        approvedByRef: "operator.public.test",
        now: new Date("2026-06-10T00:00:00.000Z"),
      })
      const relay = new FakeRelay()
      const event = laborRequestEvent({
        providerPubkey: identity.publicKey,
        bid: 0,
        content: "Use OPENAI_API_KEY=sk-testtesttesttesttesttesttest to run this.",
      })

      const result = await runProviderJobOnce({
        state,
        event,
        identity,
        relay,
        policy: { priceMsats: 0 },
        laborWorkspaceRoot: home,
        laborAgentKind: "test_fixture",
        laborRuntime: {
          async runLabor() {
            throw new Error("unsafe labor runtime should not execute")
          },
        },
        now: () => new Date("2026-06-10T00:00:00.000Z"),
      })

      expect(result.status).toBe("dropped")
      expect(result.reasonRef).toBe("labor_auth_exfiltration_blocked")
      expect(relay.published).toEqual([])
      const store = await loadProviderAdmissionStore(state.paths)
      expect(store.handledRequests[event.id]?.status).toBe("rejected")
    })
  })

  test("blocks labor workspaces outside the configured root", async () => {
    await withTempHome(async (home) => {
      const { state, identity } = await readyState(home)
      await approveLaborFirstRun({
        paths: state.paths,
        approvedByRef: "operator.public.test",
        now: new Date("2026-06-10T00:00:00.000Z"),
      })
      const relay = new FakeRelay()
      const event = laborRequestEvent({
        providerPubkey: identity.publicKey,
        bid: 0,
        workspace: "../outside",
      })

      const result = await runProviderJobOnce({
        state,
        event,
        identity,
        relay,
        policy: { priceMsats: 0 },
        laborWorkspaceRoot: join(home, "allowed"),
        laborAgentKind: "test_fixture",
        laborRuntime: {
          async runLabor() {
            throw new Error("out-of-bounds runtime should not execute")
          },
        },
        now: () => new Date("2026-06-10T00:00:00.000Z"),
      })

      expect(result.status).toBe("dropped")
      expect(result.reasonRef).toBe("labor_workspace_out_of_bounds")
      expect(relay.published).toEqual([])
    })
  })

  test("runs an approved labor fixture repair in a bounded local workspace and publishes a result", async () => {
    await withTempHome(async (home) => {
      const { state, identity } = await readyState(home)
      const root = join(home, "labor-root")
      const fixture = join(root, "fixture")
      await mkdir(fixture, { recursive: true })
      await writeFile(join(fixture, "sum.ts"), "export const sum = (left: number, right: number) => left - right\n")
      await writeFile(
        join(fixture, "sum.test.ts"),
        [
          'import { describe, expect, test } from "bun:test"',
          'import { sum } from "./sum"',
          "",
          'describe("sum", () => {',
          '  test("adds numbers", () => {',
          "    expect(sum(2, 2)).toBe(4)",
          "  })",
          "})",
          "",
        ].join("\n"),
      )
      await approveLaborFirstRun({
        paths: state.paths,
        approvedByRef: "operator.public.test",
        now: new Date("2026-06-10T00:00:00.000Z"),
      })
      const relay = new FakeRelay()
      const event = laborRequestEvent({
        providerPubkey: identity.publicKey,
        bid: 0,
        workspace: "fixture",
      })

      const result = await runProviderJobOnce({
        state,
        event,
        identity,
        relay,
        policy: { priceMsats: 0 },
        laborWorkspaceRoot: root,
        laborAgentKind: "test_fixture",
        laborRuntime: {
          async runLabor(input) {
            expect(input.workspace.relativePath).toBe("fixture")
            await writeFile(join(input.workspace.absolutePath, "sum.ts"), "export const sum = (left: number, right: number) => left + right\n")
            const proc = Bun.spawn(["bun", "test", "sum.test.ts"], {
              cwd: input.workspace.absolutePath,
              stdout: "pipe",
              stderr: "pipe",
            })
            const [stdout, stderr, exitCode] = await Promise.all([
              new Response(proc.stdout).text(),
              new Response(proc.stderr).text(),
              proc.exited,
            ])
            expect(exitCode, `${stdout}\n${stderr}`).toBe(0)
            const artifactRefs = ["artifact.public.pylon.labor.fixture_repair"]
            return {
              artifactRefs,
              content: laborResultContent({
                agentKind: "test_fixture",
                request: input.request,
                artifactRefs,
                receiptRefs: ["receipt.public.pylon.labor.fixture_test_passed"],
                summary: "Fixed the fixture sum implementation and verified bun test passes.",
                workspace: input.workspace,
              }),
              model: "test-fixture-agent",
              receiptRefs: ["receipt.public.pylon.labor.fixture_test_passed"],
            }
          },
        },
        now: () => new Date("2026-06-10T00:00:00.000Z"),
      })

      expect(result.status).toBe("completed")
      expect(relay.published.map((event) => event.kind)).toEqual([
        KIND_JOB_FEEDBACK,
        6934,
        KIND_JOB_FEEDBACK,
      ])
      expect(parseJobFeedbackEvent(relay.published[0]).status).toBe("processing")
      const laborResult = parseLaborJobResultEvent(relay.published[1])
      expect(laborResult.jobType).toBe("code_task")
      expect(laborResult.artifactRefs).toEqual(["artifact.public.pylon.labor.fixture_repair"])
      expect(JSON.parse(laborResult.result.content).summary).toContain("verified bun test passes")
      expect(parseJobFeedbackEvent(relay.published[2]).status).toBe("success")
      const store = await loadProviderAdmissionStore(state.paths)
      expect(store.handledRequests[event.id]?.status).toBe("completed")
      expect(JSON.stringify(store)).not.toContain("OPENAI_API_KEY")
      expect(JSON.stringify(store)).not.toContain(fixture)
    })
  })

  test("ports provider admission drops and defers before runtime execution", async () => {
    await withTempHome(async (home) => {
      const { identity } = await readyState(home)
      const event = requestEvent({ providerPubkey: identity.publicKey })
      const entry = classifyProviderRequestEvent({ event, providerPubkey: identity.publicKey })
      const store = emptyProviderAdmissionStore()
      const now = new Date("2026-06-10T00:00:00.000Z")

      expect(evaluateProviderAdmission({ entry, eventCreatedAtSeconds: event.created_at, store, now }).reasonRef).toBe("missing_bid")

      const underbid = classifyProviderRequestEvent({
        event: requestEvent({ providerPubkey: identity.publicKey, bid: 999 }),
        providerPubkey: identity.publicKey,
      })
      expect(evaluateProviderAdmission({ entry: underbid, eventCreatedAtSeconds: event.created_at, store, now }).reasonRef).toBe(
        "bid_below_price_floor",
      )

      const priced = classifyProviderRequestEvent({
        event: requestEvent({ providerPubkey: identity.publicKey, bid: 1_000 }),
        providerPubkey: identity.publicKey,
      })
      expect(
        evaluateProviderAdmission({
          entry: priced,
          eventCreatedAtSeconds: 0,
          store,
          now,
          policy: defaultProviderAdmissionPolicy({ requestTtlSeconds: 1 }),
        }).reasonRef,
      ).toBe("stale_request")

      store.admissionLeases["request.active"] = {
        requestEventId: "request.active",
        requesterPubkey: "buyer-a",
        status: "processing",
        expiresAtMs: now.getTime() + 60_000,
      }
      expect(
        evaluateProviderAdmission({
          entry: priced,
          eventCreatedAtSeconds: event.created_at,
          store,
          now,
        }),
      ).toEqual({ admitted: false, action: "defer", reasonRef: "max_inflight" })

      store.admissionLeases = {
        "request.active": {
          requestEventId: "request.active",
          requesterPubkey: priced.requesterPubkey,
          status: "processing",
          expiresAtMs: now.getTime() + 60_000,
        },
      }
      expect(
        evaluateProviderAdmission({
          entry: priced,
          eventCreatedAtSeconds: event.created_at,
          store,
          now,
          policy: { maxInflight: 2 },
        }),
      ).toEqual({ admitted: false, action: "defer", reasonRef: "buyer_limit" })
    })
  })

  test("rejects malformed jobs without executing the local runtime", async () => {
    await withTempHome(async (home) => {
      const { state, identity } = await readyState(home)
      const relay = new FakeRelay()
      let runtimeCalls = 0
      const malformed = {
        ...requestEvent({ providerPubkey: identity.publicKey, bid: 1_000 }),
        kind: 5050,
        tags: [["output", "application/json"]],
      }

      const result = await runProviderJobOnce({
        state,
        event: malformed,
        identity,
        relay,
        runtime: {
          async complete() {
            runtimeCalls += 1
            return { text: "should not execute", model: "test", receiptRefs: [] }
          },
        },
        walletRunner,
        now: () => new Date("2026-06-10T00:00:00.000Z"),
      })

      expect(result.status).toBe("dropped")
      expect(result.reasonRef).toBe("unsupported_output")
      expect(runtimeCalls).toBe(0)
      expect(relay.published).toEqual([])
    })
  })

  test("defers busy provider jobs without permanently handling them", async () => {
    await withTempHome(async (home) => {
      const { state, identity } = await readyState(home)
      const relay = new FakeRelay()
      const event = requestEvent({ providerPubkey: identity.publicKey, bid: 1_000 })
      const store = emptyProviderAdmissionStore()
      store.admissionLeases["request.active"] = {
        requestEventId: "request.active",
        requesterPubkey: "buyer-a",
        status: "processing",
        expiresAtMs: new Date("2026-06-10T00:00:00.000Z").getTime() + 60_000,
      }

      const result = await runProviderJobOnce({
        state,
        event,
        identity,
        relay,
        store,
        runtime: {
          async complete() {
            throw new Error("runtime should not execute for deferred admission")
          },
        },
        walletRunner,
        now: () => new Date("2026-06-10T00:00:00.000Z"),
      })

      expect(result.status).toBe("deferred")
      expect(result.reasonRef).toBe("max_inflight")
      expect(relay.published).toEqual([])
      const persisted = await loadProviderAdmissionStore(state.paths)
      expect(persisted.handledRequests[event.id]).toBeUndefined()
      expect(store.admissionLeases["request.active"]).toBeDefined()
    })
  })

  test("publishes error feedback when the local runtime fails", async () => {
    await withTempHome(async (home) => {
      const { state, identity } = await readyState(home)
      const relay = new FakeRelay()
      const event = requestEvent({ providerPubkey: identity.publicKey })

      const result = await runProviderJobOnce({
        state,
        event,
        identity,
        relay,
        policy: { priceMsats: 0 },
        runtime: {
          async complete() {
            throw new Error("local model unavailable")
          },
        },
        walletRunner,
        now: () => new Date("2026-06-10T00:00:00.000Z"),
      })

      expect(result.status).toBe("error")
      expect(result.reasonRef).toBe("runtime_error")
      expect(relay.published.map((event) => event.kind)).toEqual([KIND_JOB_FEEDBACK, KIND_JOB_FEEDBACK])
      expect(parseJobFeedbackEvent(relay.published[0]).status).toBe("processing")
      expect(parseJobFeedbackEvent(relay.published[1]).status).toBe("error")
      const store = await loadProviderAdmissionStore(state.paths)
      expect(store.handledRequests[event.id]?.status).toBe("error")
      expect(store.handledRequests[event.id]?.reasonRef).toBe("runtime_error")
    })
  })

  test("publishes payment, processing, success result events and records redacted earnings", async () => {
    await withTempHome(async (home) => {
      const { state, identity } = await readyState(home)
      const relay = new FakeRelay()
      const event = requestEvent({ providerPubkey: identity.publicKey, bid: 2_000 })

      const result = await runProviderJobOnce({
        state,
        event,
        identity,
        relay,
        policy: { priceMsats: 1_000 },
        runtime: {
          async complete(prompt: string) {
            expect(prompt).toContain("Summarize")
            return { text: "public inference result", model: "test-runtime", receiptRefs: ["receipt.public.runtime.test"] }
          },
        },
        walletRunner,
        now: () => new Date("2026-06-10T00:00:00.000Z"),
      })

      expect(result.status).toBe("completed")
      expect(result.earning?.amountMsats).toBe(1_000)
      expect(relay.published.map((event) => event.kind)).toEqual([
        KIND_JOB_FEEDBACK,
        KIND_JOB_FEEDBACK,
        6050,
        KIND_JOB_FEEDBACK,
      ])

      const payment = parseJobFeedbackEvent(relay.published[0])
      expect(payment.status).toBe("payment-required")
      expect(payment.amount).toBe(1_000)
      expect(payment.bolt11).toBe("lnbc10n1providerinvoice")

      const processing = parseJobFeedbackEvent(relay.published[1])
      expect(processing.status).toBe("processing")

      const jobResult = parseJobResultEvent(relay.published[2])
      expect(jobResult.content).toBe("public inference result")
      expect(jobResult.amount).toBe(1_000)
      expect(jobResult.bolt11).toBe("lnbc10n1providerinvoice")

      const success = parseJobFeedbackEvent(relay.published[3])
      expect(success.status).toBe("success")

      const store = await loadProviderAdmissionStore(state.paths)
      const serialized = JSON.stringify(store)
      expect(store.earnings).toHaveLength(1)
      expect(serialized).toContain("receipt.public.pylon.nip90.result")
      expect(serialized).not.toContain("lnbc10n1providerinvoice")
      expect(serialized).not.toContain(home)
    })
  })
})
