#!/usr/bin/env bun

import { mkdtemp, rm } from "node:fs/promises"
import { join } from "node:path"
import { tmpdir } from "node:os"
import {
  KIND_JOB_FEEDBACK,
  KIND_JOB_TEXT_GENERATION,
  createJobRequestEvent,
  jobInput,
  makeJobRequest,
} from "@openagentsinc/nip90"
import { createBootstrapSummary, parseBootstrapArgs } from "../src/bootstrap"
import { deriveNip06Identity, loadOrCreateNostrIdentity } from "../src/nostr-identity"
import {
  OPENAGENTS_MARKET_RELAY_URL,
  PYLON_NIP90_PROVIDER_CAPABILITY_REF,
  WebSocketRelayTransport,
  loadProviderAdmissionStore,
  signNostrEvent,
  startNip90ProviderLoop,
} from "../src/provider-nip90"
import { ensurePylonLocalState, writeRuntimeState } from "../src/state"

const relayUrl = Bun.env.PYLON_NIP90_SMOKE_RELAY ?? OPENAGENTS_MARKET_RELAY_URL
const home = await mkdtemp(join(tmpdir(), "pylon-nip90-provider-smoke-"))
const buyer = deriveNip06Identity(
  "leader monkey parrot ring guide accident before fence cannon height naive bean",
  `${home}/buyer.mnemonic`,
)

try {
  const summary = createBootstrapSummary(
    parseBootstrapArgs(["--display-name", "NIP-90 Provider Smoke", "--capability-ref", PYLON_NIP90_PROVIDER_CAPABILITY_REF]),
    { PYLON_HOME: home },
    "darwin",
  )
  const state = await ensurePylonLocalState(summary)
  await writeRuntimeState(state.paths, {
    ...state.runtime,
    lifecycle: "online",
    capabilityRefs: [...new Set([...state.runtime.capabilityRefs, PYLON_NIP90_PROVIDER_CAPABILITY_REF])],
    blockerRefs: [],
  })
  const identity = await loadOrCreateNostrIdentity(summary.paths)
  const logs: string[] = []
  const loop = startNip90ProviderLoop(summary, {
    relays: [relayUrl],
    once: true,
    runtime: {
      async complete(prompt: string) {
        return {
          text: `public smoke result: ${prompt.slice(0, 32)}`,
          model: "smoke-runtime",
          receiptRefs: ["receipt.public.pylon.nip90_provider_smoke.runtime"],
        }
      },
    },
    walletRunner: async (args) => ({
      exitCode: 0,
      stdout: JSON.stringify({
        invoice: `lnbc10n1pylonsmoke${args.join("").replace(/[^a-z0-9]/gi, "").toLowerCase()}`,
      }),
      stderr: "",
    }),
    log: (message) => logs.push(message),
  })

  await new Promise((resolve) => setTimeout(resolve, 1_000))

  const relay = new WebSocketRelayTransport(relayUrl)
  const request = makeJobRequest({
    kind: KIND_JOB_TEXT_GENERATION,
    inputs: [jobInput.text("Summarize the OpenAgents market relay smoke in one sentence.")],
    bid: 1_000,
    output: "text/plain",
    serviceProviders: [identity.publicKey],
  })
  const event = signNostrEvent(createJobRequestEvent(request), buyer)
  const requestPublish = await relay.publish(event)
  const result = await Promise.race([
    loop,
    new Promise<never>((_, reject) => setTimeout(() => reject(new Error("provider loop smoke timed out")), 20_000)),
  ])
  const store = await loadProviderAdmissionStore(state.paths)
  const summaryOut = {
    schema: "openagents.pylon.nip90_provider_loop_smoke.v0.3",
    status: result.started && result.handled > 0 ? "passed" : "blocked",
    relayUrl,
    requestEventId: event.id,
    requestAcceptedByRelay: requestPublish.accepted,
    handled: result.started ? result.handled : 0,
    handlerPublished: logs.some((line) => line.includes("Published handler info")),
    feedbackKinds: [KIND_JOB_FEEDBACK],
    resultKind: 6050,
    earningRefs: store.earnings.map((earning) => earning.receiptRef),
    blockerRefs: result.started ? [] : [result.reasonRef],
  }
  const serialized = JSON.stringify(summaryOut)
  if (/lnbc|lntb|lnbcrt|mnemonic|preimage|payment_hash|Bearer\s+/i.test(serialized)) {
    throw new Error("smoke summary contains private payment or auth material")
  }
  process.stdout.write(`${JSON.stringify(summaryOut, null, 2)}\n`)
  process.exitCode = summaryOut.status === "passed" ? 0 : 2
} catch (error) {
  process.stderr.write(`Pylon NIP-90 provider smoke failed: ${error instanceof Error ? error.message : String(error)}\n`)
  process.exitCode = 1
} finally {
  await rm(home, { recursive: true, force: true })
}
