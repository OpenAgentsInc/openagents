/**
 * ENV-2 (openagents #8780) pilot-seam integration: the real RFC 9449 DPoP
 * verifier plugged into the real portable-session capability broker through
 * its opt-in `proofVerifier` seam. Proves end to end that a key-bound lease
 * redeems only for the client holding the bound private key, that every
 * proof is single-use, and that unbound leases keep the pre-ENV-2 path.
 */
import { describe, expect, test } from "vite-plus/test"
import { Effect } from "effect"
import {
  CapabilityBrokerError,
  PortableCapabilityBroker,
  makeOwnerLocalCapabilityAdapter,
  type CapabilityBrokerConfig,
  type CapabilityRedemptionProof,
  type CapabilitySecretVault,
  type IssueCapabilityInput,
  type SecretMaterial,
} from "@openagentsinc/portable-session-contract"

import { makeDpopCapabilityProofVerifier } from "./broker-verifier.js"
import { generateDpopKeyPair, mintDpopProof, type DpopKeyPair } from "./dpop.js"

const HTM = "POST"
const HTU = "http://127.0.0.1:4310/broker/redeem"
const NOW = new Date("2026-07-14T05:00:00.000Z")

function makeFixture(input: { client: DpopKeyPair }) {
  const installed = new Set<string>()
  const vault: CapabilitySecretVault = {
    withSourceGrantMaterial: async ({ use }) => {
      const bytes = new TextEncoder().encode("env2_fixture_secret") as SecretMaterial
      try {
        return await use(bytes)
      } finally {
        bytes.fill(0)
      }
    },
    revokeSourceGrant: async () => {},
  }
  const adapter = makeOwnerLocalCapabilityAdapter("adapter.owner-local.v1", {
    install: async ({ lease }) => {
      installed.add(lease.leaseRef)
      return { installationRef: `installation.${lease.leaseRef}` }
    },
    wipe: async ({ leaseRef }) => {
      installed.delete(leaseRef)
      return { wipeReceiptRef: `receipt.wipe.${leaseRef}` }
    },
  })
  const config: CapabilityBrokerConfig = {
    vault,
    clock: { now: () => NOW },
    targets: [{
      targetRef: "environment.pylon.m4-desktop",
      targetClass: "owner_local",
      adapterRef: adapter.adapterRef,
      ready: true,
    }],
    adapters: [adapter],
    evidenceSink: { append: async () => {} },
    proofVerifier: makeDpopCapabilityProofVerifier({ now: () => NOW }),
  }
  const broker = new PortableCapabilityBroker(config)
  const issue = (leaseRef: string, clientKeyThumbprint?: string): IssueCapabilityInput => ({
    operationRef: `operation.issue.${leaseRef}`,
    leaseRef,
    ownerRef: "owner.chris",
    sessionRef: "session.env2",
    attachmentRef: "attachment.env2.local",
    attachmentGeneration: 1,
    targetRef: "environment.pylon.m4-desktop",
    capability: "tool",
    toolRef: "tool.local.control-socket",
    sourceGrantRef: `grant.source.${leaseRef}`,
    permissions: ["coding_session_control"],
    expiresAt: "2026-07-14T05:10:00.000Z",
    ...(clientKeyThumbprint === undefined ? {} : { clientKeyThumbprint }),
  })
  const proofFor = async (
    keys: DpopKeyPair,
    overrides: Partial<CapabilityRedemptionProof> = {},
    jti?: string,
  ): Promise<CapabilityRedemptionProof> => ({
    scheme: "dpop",
    proof: await mintDpopProof({
      privateKey: keys.privateKey,
      publicJwk: keys.publicJwk,
      htm: HTM,
      htu: HTU,
      nowEpochSeconds: Math.floor(NOW.getTime() / 1000),
      ...(jti === undefined ? {} : { jti }),
    }),
    htm: HTM,
    htu: HTU,
    ...overrides,
  })
  return { broker, installed, issue, proofFor, client: input.client }
}

const run = <A>(effect: Effect.Effect<A, CapabilityBrokerError>) => Effect.runPromise(effect)

async function expectReason(
  effect: Effect.Effect<unknown, CapabilityBrokerError>,
  reason: CapabilityBrokerError["reason"],
) {
  try {
    await run(effect)
    throw new Error("expected broker operation to fail")
  } catch (error) {
    expect(error).toBeInstanceOf(CapabilityBrokerError)
    expect((error as CapabilityBrokerError).reason).toBe(reason)
  }
}

describe("real DPoP verifier through the capability broker pilot seam", () => {
  test("the bound client redeems with a real proof; a second use of the same proof is refused", async () => {
    const client = await generateDpopKeyPair()
    const { broker, installed, issue, proofFor } = makeFixture({ client })
    await run(broker.issue(issue("lease.bound", client.thumbprint)))
    const proof = await proofFor(client, {}, "jti-once")
    const outcome = await run(broker.redeem({
      operationRef: "operation.redeem.bound",
      leaseRef: "lease.bound",
      redemptionProof: proof,
    }))
    expect(outcome.status).toBe("completed")
    expect(installed.has("lease.bound")).toBe(true)

    // Adversarial replay: same signed proof bytes under a NEW operation ref
    // (so broker idempotency does not absorb it) hits the jti replay window.
    await run(broker.issue(issue("lease.bound2", client.thumbprint)))
    await expectReason(
      broker.redeem({
        operationRef: "operation.redeem.replayed-proof",
        leaseRef: "lease.bound2",
        redemptionProof: proof,
      }),
      "proof_invalid",
    )
    expect(installed.has("lease.bound2")).toBe(false)
  })

  test("adversarial: a foreign key's proof cannot redeem a lease bound to another client", async () => {
    const client = await generateDpopKeyPair()
    const foreign = await generateDpopKeyPair()
    const { broker, installed, issue, proofFor } = makeFixture({ client })
    await run(broker.issue(issue("lease.bound", client.thumbprint)))
    await expectReason(
      broker.redeem({
        operationRef: "operation.redeem.foreign",
        leaseRef: "lease.bound",
        redemptionProof: await proofFor(foreign),
      }),
      "proof_invalid",
    )
    expect(installed.has("lease.bound")).toBe(false)
  })

  test("adversarial: a proof minted for another endpoint or method fails the htu/htm binding", async () => {
    const client = await generateDpopKeyPair()
    const { broker, issue, proofFor } = makeFixture({ client })
    await run(broker.issue(issue("lease.bound", client.thumbprint)))
    // Transport host observed a different URL than the proof was minted for.
    await expectReason(
      broker.redeem({
        operationRef: "operation.redeem.wrong-htu",
        leaseRef: "lease.bound",
        redemptionProof: await proofFor(client, { htu: "http://127.0.0.1:4310/broker/revoke" }),
      }),
      "proof_invalid",
    )
    await expectReason(
      broker.redeem({
        operationRef: "operation.redeem.wrong-htm",
        leaseRef: "lease.bound",
        redemptionProof: await proofFor(client, { htm: "DELETE" }),
      }),
      "proof_invalid",
    )
  })

  test("an unbound lease still redeems with no proof at all (opt-in stays opt-in)", async () => {
    const client = await generateDpopKeyPair()
    const { broker, installed, issue } = makeFixture({ client })
    await run(broker.issue(issue("lease.unbound")))
    const outcome = await run(broker.redeem({
      operationRef: "operation.redeem.unbound",
      leaseRef: "lease.unbound",
    }))
    expect(outcome.status).toBe("completed")
    expect(installed.has("lease.unbound")).toBe(true)
  })
})
