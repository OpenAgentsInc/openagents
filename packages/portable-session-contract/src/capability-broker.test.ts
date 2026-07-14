import { describe, expect, test } from "vite-plus/test"
import { Effect } from "effect"

import {
  CapabilityBrokerError,
  makeOpenAgentsManagedCapabilityAdapter,
  makeOwnerLocalCapabilityAdapter,
  PortableCapabilityBroker,
  type CapabilityBrokerEvidence,
  type CapabilityBrokerConfig,
  type CapabilityBrokerPrivateDurableState,
  type CapabilityBrokerStateStore,
  type CapabilityProofVerifier,
  type CapabilityRedemptionProof,
  type CapabilitySecretVault,
  type IssueCapabilityInput,
  type SecretMaterial,
} from "./capability-broker.js"

const RAW_SECRET = "port02_fixture_secret_do_not_project"
const at = (minute: number) => `2026-07-13T04:${String(minute).padStart(2, "0")}:00.000Z`

class FixtureClock {
  value = new Date(at(0))
  now = () => this.value
  set(minute: number) {
    this.value = new Date(at(minute))
  }
}

class MemoryBrokerStateStore implements CapabilityBrokerStateStore {
  state: CapabilityBrokerPrivateDurableState | null = null
  load = async () => this.state === null ? null : structuredClone(this.state)
  save = async (state: CapabilityBrokerPrivateDurableState) => {
    this.state = structuredClone(state)
  }
}

function fixture(
  options: {
    vaultOutage?: boolean
    outage?: { value: boolean }
    managedDenial?: boolean
    managedWipeFailure?: boolean
    clock?: FixtureClock
    stateStore?: CapabilityBrokerStateStore
    proofVerifier?: CapabilityProofVerifier
  } = {},
) {
  const clock = options.clock ?? new FixtureClock()
  const installed = new Map<string, { target: string; materialDigest: number }>()
  const revoked = new Set<string>()
  const wiped = new Set<string>()
  const evidence: CapabilityBrokerEvidence[] = []
  const vault: CapabilitySecretVault = {
    withSourceGrantMaterial: async ({ sourceGrantRef, use }) => {
      if (options.vaultOutage || options.outage?.value) throw new Error("broker outage")
      if (revoked.has(sourceGrantRef)) throw new CapabilityBrokerError("source_revoked", "vault.redeem", "revoked")
      const bytes = new TextEncoder().encode(RAW_SECRET) as SecretMaterial
      try {
        return await use(bytes)
      } finally {
        bytes.fill(0)
      }
    },
    revokeSourceGrant: async ({ sourceGrantRef }) => {
      if (options.vaultOutage || options.outage?.value) throw new Error("broker outage")
      revoked.add(sourceGrantRef)
    },
  }
  const localAdapter = makeOwnerLocalCapabilityAdapter("adapter.owner-local.v1", {
    install: async ({ lease, material }) => {
      installed.set(lease.leaseRef, { target: lease.targetRef, materialDigest: material.reduce((sum, byte) => sum + byte, 0) })
      return { installationRef: `installation.${lease.leaseRef}` }
    },
    wipe: async ({ leaseRef }) => {
      installed.delete(leaseRef)
      wiped.add(leaseRef)
      return { wipeReceiptRef: `receipt.wipe.${leaseRef}` }
    },
  })
  const managedAdapter = makeOpenAgentsManagedCapabilityAdapter("adapter.openagents-managed.v1", {
    install: async ({ lease, material }) => {
      if (options.managedDenial) throw new Error("target_denied")
      installed.set(lease.leaseRef, { target: lease.targetRef, materialDigest: material.reduce((sum, byte) => sum + byte, 0) })
      return { installationRef: `installation.${lease.leaseRef}` }
    },
    wipe: async ({ leaseRef }) => {
      if (options.managedWipeFailure) throw new Error("wipe failed")
      installed.delete(leaseRef)
      wiped.add(leaseRef)
      return { wipeReceiptRef: `receipt.wipe.${leaseRef}` }
    },
  })
  const brokerConfig: CapabilityBrokerConfig = {
    vault,
    clock,
    targets: [
      { targetRef: "target.owner-local", targetClass: "owner_local", adapterRef: localAdapter.adapterRef, ready: true },
      { targetRef: "target.agent-computer", targetClass: "openagents_managed", adapterRef: managedAdapter.adapterRef, ready: true },
      { targetRef: "target.denied", targetClass: "openagents_managed", adapterRef: managedAdapter.adapterRef, ready: false },
      { targetRef: "target.mismatch", targetClass: "openagents_managed", adapterRef: localAdapter.adapterRef, ready: true },
    ],
    adapters: [localAdapter, managedAdapter],
    evidenceSink: { append: async item => { evidence.push(item) } },
    ...(options.stateStore ? { stateStore: options.stateStore } : {}),
    ...(options.proofVerifier ? { proofVerifier: options.proofVerifier } : {}),
  }
  const broker = new PortableCapabilityBroker(brokerConfig)
  return { broker, brokerConfig, clock, evidence, installed, revoked, wiped }
}

type IssueOverrides = { [K in keyof IssueCapabilityInput]?: IssueCapabilityInput[K] | undefined }

function issueInput(overrides: IssueOverrides = {}): IssueCapabilityInput {
  const merged: Record<string, unknown> = {
    operationRef: "operation.issue.local",
    leaseRef: "lease.local.provider",
    ownerRef: "owner.port02",
    sessionRef: "session.port02",
    attachmentRef: "attachment.port02.local",
    attachmentGeneration: 1,
    targetRef: "target.owner-local",
    capability: "provider",
    sourceGrantRef: "grant.source.provider",
    accountRef: "account.codex-2",
    permissions: ["provider.turn.execute"],
    expiresAt: at(10),
    ...overrides,
  }
  if (merged.accountRef === undefined) delete merged.accountRef
  if (merged.toolRef === undefined) delete merged.toolRef
  return merged as IssueCapabilityInput
}

const run = <A>(effect: Effect.Effect<A, CapabilityBrokerError>) => Effect.runPromise(effect)

async function expectReason(effect: Effect.Effect<unknown, CapabilityBrokerError>, reason: CapabilityBrokerError["reason"]) {
  try {
    await run(effect)
    throw new Error("expected broker operation to fail")
  } catch (error) {
    expect(error).toBeInstanceOf(CapabilityBrokerError)
    expect((error as CapabilityBrokerError).reason).toBe(reason)
  }
}

describe("portable target-scoped capability broker", () => {
  test("binds and redeems provider, SCM writeback, tool, and API leases at least privilege", async () => {
    const { broker, installed } = fixture()
    const cases: ReadonlyArray<IssueOverrides> = [
      {},
      { operationRef: "operation.issue.scm", leaseRef: "lease.local.scm", capability: "scm_write", accountRef: "account.github-primary", permissions: ["scm.branch.push"] },
      { operationRef: "operation.issue.tool", leaseRef: "lease.local.tool", capability: "tool", accountRef: undefined, toolRef: "tool.mcp.github.issue.read", permissions: ["tool.invoke"] },
      { operationRef: "operation.issue.api", leaseRef: "lease.local.api", capability: "api", accountRef: undefined, toolRef: "api.preview.bounded", permissions: ["api.preview.read"] },
    ]
    for (const item of cases) {
      const issued = issueInput(item)
      await run(broker.issue(issued))
      await run(broker.redeem({ operationRef: `${issued.operationRef}.redeem`, leaseRef: issued.leaseRef }))
      expect(installed.get(issued.leaseRef)?.target).toBe("target.owner-local")
    }
    const snapshot = broker.snapshot()
    expect(snapshot.leases.map(item => item.lease.capability)).toEqual(["provider", "scm_write", "tool", "api"])
    expect(snapshot.leases.every(item => item.permissions.length === 1 && item.lease.state === "redeemed")).toBe(true)
  })

  test("reissues from owner-local to accepted managed only after source revoke and wipe", async () => {
    const { broker, installed, revoked, wiped } = fixture()
    await run(broker.issue(issueInput()))
    await run(broker.redeem({ operationRef: "operation.redeem.local", leaseRef: "lease.local.provider" }))
    const moved = await run(broker.reissue({
      operationRef: "operation.reissue.managed",
      leaseRef: "lease.local.provider",
      newLeaseRef: "lease.managed.provider",
      destinationSourceGrantRef: "grant.destination.provider",
      destinationAttachmentRef: "attachment.port02.managed",
      destinationAttachmentGeneration: 2,
      destinationTargetRef: "target.agent-computer",
      expiresAt: at(12),
    }))
    expect(moved.resultingLeaseRef).toBe("lease.managed.provider")
    expect(revoked).toContain("grant.source.provider")
    expect(wiped).toContain("lease.local.provider")
    expect(installed.has("lease.local.provider")).toBe(false)
    await run(broker.redeem({ operationRef: "operation.redeem.managed", leaseRef: "lease.managed.provider" }))
    expect(installed.get("lease.managed.provider")?.target).toBe("target.agent-computer")
    expect(broker.snapshot().leases.find(item => item.lease.leaseRef === "lease.local.provider")?.lease.state).toBe("revoked")
    expect(broker.snapshot().leases.find(item => item.lease.leaseRef === "lease.managed.provider")?.lease.attachmentGeneration).toBe(2)
  })

  test("lost acknowledgement replay is idempotent and conflicting bytes are rejected", async () => {
    const { broker } = fixture()
    const first = await run(broker.issue(issueInput()))
    const retry = await run(broker.issue(issueInput()))
    expect(first.status).toBe("completed")
    expect(retry.status).toBe("replayed")
    expect(broker.snapshot().leases).toHaveLength(1)
    expect(broker.snapshot().evidence).toHaveLength(1)
    await expectReason(broker.issue(issueInput({ targetRef: "target.agent-computer" })), "conflicting_replay")
    expect(broker.snapshot().leases).toHaveLength(1)
  })

  test("restores leases and operation replay identity into a truly fresh broker instance", async () => {
    const stateStore = new MemoryBrokerStateStore()
    const first = fixture({ stateStore })
    await run(first.broker.issue(issueInput()))
    await run(first.broker.redeem({ operationRef: "operation.redeem.before-restart", leaseRef: "lease.local.provider" }))
    const reissueInput = {
      operationRef: "operation.reissue.before-restart",
      leaseRef: "lease.local.provider",
      newLeaseRef: "lease.managed.after-restart",
      destinationSourceGrantRef: "grant.destination.after-restart",
      destinationAttachmentRef: "attachment.managed.after-restart",
      destinationAttachmentGeneration: 2,
      destinationTargetRef: "target.agent-computer",
      expiresAt: at(12),
    }
    await run(first.broker.reissue(reissueInput))

    const restored = await PortableCapabilityBroker.restore(first.brokerConfig)
    expect(restored).not.toBe(first.broker)
    expect((await run(restored.reissue(reissueInput))).status).toBe("replayed")
    await run(restored.redeem({
      operationRef: "operation.redeem.after-restart",
      leaseRef: "lease.managed.after-restart",
    }))
    expect(restored.snapshot().leases.find(row =>
      row.lease.leaseRef === "lease.managed.after-restart")?.lease.state).toBe("redeemed")
    expect(JSON.stringify(restored.snapshot())).not.toContain("grant.destination.after-restart")
  })

  test("renew is bounded and expired grants revoke and wipe before refusing", async () => {
    const clock = new FixtureClock()
    const { broker, revoked, wiped } = fixture({ clock })
    await run(broker.issue(issueInput({ expiresAt: at(2) })))
    await run(broker.redeem({ operationRef: "operation.redeem.expiring", leaseRef: "lease.local.provider" }))
    await run(broker.renew({ operationRef: "operation.renew", leaseRef: "lease.local.provider", expiresAt: at(4) }))
    expect(broker.snapshot().leases[0]?.renewalCount).toBe(1)
    clock.set(5)
    await expectReason(broker.redeem({ operationRef: "operation.redeem.after-expiry", leaseRef: "lease.local.provider" }), "expired")
    expect(revoked).toContain("grant.source.provider")
    expect(wiped).toContain("lease.local.provider")
    expect(broker.snapshot().leases[0]?.lease.state).toBe("expired")
  })

  test("expiry sweep emits durable redacted evidence and performs cleanup", async () => {
    const clock = new FixtureClock()
    const { broker, revoked, wiped } = fixture({ clock })
    await run(broker.issue(issueInput({ expiresAt: at(2) })))
    await run(broker.redeem({ operationRef: "operation.redeem.sweep", leaseRef: "lease.local.provider" }))
    clock.set(3)
    const outcomes = await run(broker.expireLeases("operation.expire"))
    expect(outcomes).toHaveLength(1)
    expect(outcomes[0]?.reason).toBe("expired")
    expect(outcomes[0]?.evidenceRefs).toEqual(["evidence.capability.operation.expire.lease.local.provider"])
    expect(revoked).toContain("grant.source.provider")
    expect(wiped).toContain("lease.local.provider")
    expect(broker.snapshot().evidence.at(-1)?.material).toBe("excluded")
  })

  test("mid-move revocation cannot replay source or mint destination on cleanup failure", async () => {
    const { broker, installed } = fixture({ managedWipeFailure: true })
    await run(broker.issue(issueInput({ targetRef: "target.agent-computer", operationRef: "operation.issue.source-managed" })))
    await run(broker.redeem({ operationRef: "operation.redeem.source-managed", leaseRef: "lease.local.provider" }))
    await expectReason(broker.reissue({
      operationRef: "operation.reissue.cleanup-fails",
      leaseRef: "lease.local.provider",
      newLeaseRef: "lease.destination.denied",
      destinationSourceGrantRef: "grant.destination.provider",
      destinationAttachmentRef: "attachment.destination",
      destinationAttachmentGeneration: 2,
      destinationTargetRef: "target.owner-local",
      expiresAt: at(10),
    }), "cleanup_failed")
    expect(broker.snapshot().leases.some(item => item.lease.leaseRef === "lease.destination.denied")).toBe(false)
    expect(broker.snapshot().leases[0]?.lease.state).toBe("revoked")
    await expectReason(broker.redeem({ operationRef: "operation.replay.revoked", leaseRef: "lease.local.provider" }), "lease_not_active")
    expect(installed.has("lease.destination.denied")).toBe(false)
  })

  test("target denial and broker outage fail closed with durable redacted evidence", async () => {
    const denied = fixture({ managedDenial: true })
    await run(denied.broker.issue(issueInput({ targetRef: "target.agent-computer" })))
    await expectReason(denied.broker.redeem({ operationRef: "operation.redeem.denied", leaseRef: "lease.local.provider" }), "target_denied")
    expect(denied.broker.snapshot().leases[0]?.lease.state).toBe("issued")
    expect(denied.broker.snapshot().evidence.at(-1)?.reason).toBe("target_denied")

    const unavailable = fixture({ vaultOutage: true })
    await run(unavailable.broker.issue(issueInput()))
    await expectReason(unavailable.broker.redeem({ operationRef: "operation.redeem.outage", leaseRef: "lease.local.provider" }), "broker_unavailable")
    expect(unavailable.broker.snapshot().leases[0]?.lease.state).toBe("issued")
    expect(unavailable.broker.snapshot().evidence.at(-1)?.reason).toBe("broker_unavailable")
  })

  test("a failed external revocation stays locally denied and a new operation can finish revocation", async () => {
    const outage = { value: false }
    const { broker, revoked } = fixture({ outage })
    await run(broker.issue(issueInput()))
    await run(broker.redeem({ operationRef: "operation.redeem.before-revoke-outage", leaseRef: "lease.local.provider" }))
    outage.value = true
    await expectReason(broker.revoke({ operationRef: "operation.revoke.outage", leaseRef: "lease.local.provider" }), "broker_unavailable")
    expect(broker.snapshot().leases[0]?.lease.state).toBe("revoked")
    expect(broker.snapshot().leases[0]?.revocationConfirmedAt).toBeUndefined()
    await expectReason(broker.redeem({ operationRef: "operation.redeem.during-revoke-outage", leaseRef: "lease.local.provider" }), "lease_not_active")
    outage.value = false
    await run(broker.revoke({ operationRef: "operation.revoke.reconcile", leaseRef: "lease.local.provider" }))
    expect(revoked).toContain("grant.source.provider")
    expect(broker.snapshot().leases[0]?.revocationConfirmedAt).toBe(at(0))
  })

  test("release revokes source, wipes target, and cannot be redeemed again", async () => {
    const { broker, installed, revoked, wiped } = fixture()
    await run(broker.issue(issueInput()))
    await run(broker.redeem({ operationRef: "operation.redeem.release", leaseRef: "lease.local.provider" }))
    await run(broker.release({ operationRef: "operation.release", leaseRef: "lease.local.provider" }))
    expect(installed.has("lease.local.provider")).toBe(false)
    expect(revoked).toContain("grant.source.provider")
    expect(wiped).toContain("lease.local.provider")
    expect(broker.snapshot().leases[0]?.lease.state).toBe("released")
    await expectReason(broker.redeem({ operationRef: "operation.redeem.released", leaseRef: "lease.local.provider" }), "lease_not_active")
  })

  test("revoke and wipe expose separate durable outcomes for operator reconciliation", async () => {
    const { broker, revoked, wiped } = fixture()
    await run(broker.issue(issueInput()))
    await run(broker.redeem({ operationRef: "operation.redeem.explicit", leaseRef: "lease.local.provider" }))
    const revokedOutcome = await run(broker.revoke({ operationRef: "operation.revoke.explicit", leaseRef: "lease.local.provider" }))
    const wipedOutcome = await run(broker.wipe({ operationRef: "operation.wipe.explicit", leaseRef: "lease.local.provider" }))
    expect(revokedOutcome.operation).toBe("revoke")
    expect(wipedOutcome.operation).toBe("wipe")
    expect(revokedOutcome.evidenceRefs).toEqual(["evidence.capability.operation.revoke.explicit"])
    expect(wipedOutcome.evidenceRefs).toEqual(["evidence.capability.operation.wipe.explicit"])
    expect(revoked).toContain("grant.source.provider")
    expect(wiped).toContain("lease.local.provider")
  })

  test("snapshots, evidence, diagnostics, checkpoint-like state, artifacts, prompts, and logs never project material", async () => {
    const { broker, evidence } = fixture()
    await run(broker.issue(issueInput()))
    await run(broker.redeem({ operationRef: "operation.redeem.scan", leaseRef: "lease.local.provider" }))
    const exportedSurfaces = {
      brokerSnapshot: broker.snapshot(),
      syncRows: broker.snapshot().leases.map(item => item.lease),
      checkpoint: { capabilityLeaseRefs: ["lease.local.provider"], secretMaterial: "excluded" },
      prompt: { capabilityLeaseRef: "lease.local.provider" },
      logs: evidence,
      diagnostics: broker.snapshot().outcomes,
      publicReceipt: evidence.at(-1),
      artifact: { evidenceRefs: broker.snapshot().evidence.map(item => item.evidenceRef) },
    }
    const serialized = JSON.stringify(exportedSurfaces)
    expect(serialized).not.toContain(RAW_SECRET)
    expect(serialized).not.toContain("grant.source.provider")
    expect(serialized).not.toMatch(/access[_-]?token|authorization|bearer|client[_-]?secret|private[_-]?key|refresh[_-]?token/i)
    expect(broker.snapshot().evidence.every(item => item.material === "excluded")).toBe(true)
  })

  test("scope validation denies unready targets, missing account/tool scope, empty permissions, and overlong TTL", async () => {
    const { broker } = fixture()
    await expectReason(broker.issue(issueInput({ targetRef: "target.denied" })), "target_denied")
    await run(broker.issue(issueInput({ operationRef: "operation.issue.mismatch", leaseRef: "lease.mismatch", targetRef: "target.mismatch" })))
    await expectReason(broker.redeem({ operationRef: "operation.redeem.mismatch", leaseRef: "lease.mismatch" }), "target_mismatch")
    await expectReason(broker.issue(issueInput({ operationRef: "operation.issue.no-account", accountRef: undefined })), "invalid_scope")
    await expectReason(broker.issue(issueInput({ operationRef: "operation.issue.no-permissions", permissions: [] })), "invalid_scope")
    await expectReason(broker.issue(issueInput({ operationRef: "operation.issue.no-tool", capability: "tool", accountRef: undefined, toolRef: undefined })), "invalid_scope")
    await expectReason(broker.issue(issueInput({ operationRef: "operation.issue.long", expiresAt: at(30) })), "invalid_scope")
    expect(broker.snapshot().leases).toHaveLength(1)
    expect(broker.snapshot().leases[0]?.lease.state).toBe("issued")
  })
})

// ENV-2 (openagents #8780): opt-in DPoP-bound redemption through the
// `proofVerifier` seam. These are broker-contract tests with a scripted
// verifier; the real RFC 9449 verifier and its adversarial crypto suite live
// in @openagentsinc/environment-auth, which also runs an end-to-end
// integration against this broker.
const BOUND_THUMBPRINT = `tp_${"A".repeat(40)}`
const FOREIGN_THUMBPRINT = `tp_${"B".repeat(40)}`

const redemptionProof = (
  overrides: Partial<CapabilityRedemptionProof> = {},
): CapabilityRedemptionProof => ({
  scheme: "dpop",
  proof: "header.payload.signature",
  htm: "POST",
  htu: "http://127.0.0.1:4310/broker/redeem",
  ...overrides,
})

function scriptedVerifier(
  behavior:
    | { kind: "echo-expected" }
    | { kind: "foreign-key"; thumbprint: string }
    | { kind: "reject"; reason: string }
    | { kind: "throw" },
) {
  const calls: Array<{ proof: string; htm: string; htu: string; expectedThumbprint: string }> = []
  const verifier: CapabilityProofVerifier = {
    verify: async (input) => {
      calls.push(input)
      switch (behavior.kind) {
        case "echo-expected":
          return { ok: true, thumbprint: input.expectedThumbprint }
        case "foreign-key":
          return { ok: true, thumbprint: behavior.thumbprint }
        case "reject":
          return { ok: false, reason: behavior.reason }
        case "throw":
          throw new Error("verifier dependency failed")
      }
    },
  }
  return { calls, verifier }
}

describe("opt-in DPoP-bound capability redemption (ENV-2 #8780)", () => {
  test("an unbound lease redeems unchanged and never consults the verifier", async () => {
    const { calls, verifier } = scriptedVerifier({ kind: "echo-expected" })
    const { broker, installed } = fixture({ proofVerifier: verifier })
    await run(broker.issue(issueInput()))
    await run(broker.redeem({ operationRef: "operation.redeem.unbound", leaseRef: "lease.local.provider" }))
    expect(installed.has("lease.local.provider")).toBe(true)
    expect(calls).toHaveLength(0)
  })

  test("a key-bound lease redeems only with a verified proof for the bound key", async () => {
    const { calls, verifier } = scriptedVerifier({ kind: "echo-expected" })
    const { broker, installed } = fixture({ proofVerifier: verifier })
    await run(broker.issue(issueInput({ clientKeyThumbprint: BOUND_THUMBPRINT })))
    const outcome = await run(broker.redeem({
      operationRef: "operation.redeem.bound",
      leaseRef: "lease.local.provider",
      redemptionProof: redemptionProof(),
    }))
    expect(outcome.status).toBe("completed")
    expect(installed.has("lease.local.provider")).toBe(true)
    expect(calls).toEqual([{
      proof: "header.payload.signature",
      htm: "POST",
      htu: "http://127.0.0.1:4310/broker/redeem",
      expectedThumbprint: BOUND_THUMBPRINT,
    }])
  })

  test("a key-bound lease without a proof fails closed before vault or target access", async () => {
    const { verifier } = scriptedVerifier({ kind: "echo-expected" })
    const { broker, installed } = fixture({ proofVerifier: verifier })
    await run(broker.issue(issueInput({ clientKeyThumbprint: BOUND_THUMBPRINT })))
    await expectReason(
      broker.redeem({ operationRef: "operation.redeem.no-proof", leaseRef: "lease.local.provider" }),
      "proof_required",
    )
    expect(installed.has("lease.local.provider")).toBe(false)
    expect(broker.snapshot().leases[0]?.lease.state).toBe("issued")
    expect(broker.snapshot().outcomes.at(-1)?.status).toBe("rejected")
  })

  test("a key-bound lease fails closed when the broker has no verifier configured", async () => {
    const { broker, installed } = fixture()
    await run(broker.issue(issueInput({ clientKeyThumbprint: BOUND_THUMBPRINT })))
    await expectReason(
      broker.redeem({
        operationRef: "operation.redeem.no-verifier",
        leaseRef: "lease.local.provider",
        redemptionProof: redemptionProof(),
      }),
      "proof_required",
    )
    expect(installed.has("lease.local.provider")).toBe(false)
  })

  test("a proof proving a foreign key is rejected even when the verifier reports ok", async () => {
    const { verifier } = scriptedVerifier({ kind: "foreign-key", thumbprint: FOREIGN_THUMBPRINT })
    const { broker, installed } = fixture({ proofVerifier: verifier })
    await run(broker.issue(issueInput({ clientKeyThumbprint: BOUND_THUMBPRINT })))
    await expectReason(
      broker.redeem({
        operationRef: "operation.redeem.foreign-key",
        leaseRef: "lease.local.provider",
        redemptionProof: redemptionProof(),
      }),
      "proof_invalid",
    )
    expect(installed.has("lease.local.provider")).toBe(false)
  })

  test("verifier rejection and verifier outage both fail closed as proof_invalid", async () => {
    const rejected = fixture({ proofVerifier: scriptedVerifier({ kind: "reject", reason: "jti_replayed" }).verifier })
    await run(rejected.broker.issue(issueInput({ clientKeyThumbprint: BOUND_THUMBPRINT })))
    await expectReason(
      rejected.broker.redeem({
        operationRef: "operation.redeem.replayed",
        leaseRef: "lease.local.provider",
        redemptionProof: redemptionProof(),
      }),
      "proof_invalid",
    )
    expect(rejected.installed.has("lease.local.provider")).toBe(false)

    const throwing = fixture({ proofVerifier: scriptedVerifier({ kind: "throw" }).verifier })
    await run(throwing.broker.issue(issueInput({ clientKeyThumbprint: BOUND_THUMBPRINT })))
    await expectReason(
      throwing.broker.redeem({
        operationRef: "operation.redeem.verifier-outage",
        leaseRef: "lease.local.provider",
        redemptionProof: redemptionProof(),
      }),
      "proof_invalid",
    )
    expect(throwing.installed.has("lease.local.provider")).toBe(false)
  })

  test("a malformed client key thumbprint is rejected at issue time", async () => {
    const { broker } = fixture()
    await expectReason(
      broker.issue(issueInput({ clientKeyThumbprint: "not-a-thumbprint" })),
      "invalid_scope",
    )
    expect(broker.snapshot().leases).toHaveLength(0)
  })

  test("reissue can never launder a key-bound lease into an unbound one", async () => {
    const { verifier } = scriptedVerifier({ kind: "echo-expected" })
    const { broker, revoked } = fixture({ proofVerifier: verifier })
    await run(broker.issue(issueInput({ clientKeyThumbprint: BOUND_THUMBPRINT })))
    await expectReason(
      broker.reissue({
        operationRef: "operation.reissue.drop-binding",
        leaseRef: "lease.local.provider",
        newLeaseRef: "lease.managed.unbound",
        destinationSourceGrantRef: "grant.destination.provider",
        destinationAttachmentRef: "attachment.port02.managed",
        destinationAttachmentGeneration: 2,
        destinationTargetRef: "target.agent-computer",
        expiresAt: at(12),
      }),
      "invalid_scope",
    )
    // The malformed move must not have revoked the source grant.
    expect(revoked.size).toBe(0)
    expect(broker.snapshot().leases[0]?.lease.state).toBe("issued")
  })

  test("reissue carries an explicit destination binding that gates the destination redeem", async () => {
    const { verifier } = scriptedVerifier({ kind: "echo-expected" })
    const { broker, installed } = fixture({ proofVerifier: verifier })
    await run(broker.issue(issueInput({ clientKeyThumbprint: BOUND_THUMBPRINT })))
    const moved = await run(broker.reissue({
      operationRef: "operation.reissue.rebind",
      leaseRef: "lease.local.provider",
      newLeaseRef: "lease.managed.rebound",
      destinationSourceGrantRef: "grant.destination.provider",
      destinationAttachmentRef: "attachment.port02.managed",
      destinationAttachmentGeneration: 2,
      destinationTargetRef: "target.agent-computer",
      expiresAt: at(12),
      destinationClientKeyThumbprint: FOREIGN_THUMBPRINT,
    }))
    expect(moved.resultingLeaseRef).toBe("lease.managed.rebound")
    await expectReason(
      broker.redeem({ operationRef: "operation.redeem.rebound.no-proof", leaseRef: "lease.managed.rebound" }),
      "proof_required",
    )
    const outcome = await run(broker.redeem({
      operationRef: "operation.redeem.rebound",
      leaseRef: "lease.managed.rebound",
      redemptionProof: redemptionProof(),
    }))
    expect(outcome.status).toBe("completed")
    expect(installed.get("lease.managed.rebound")?.target).toBe("target.agent-computer")
    expect(broker.snapshot().leases.find(item => item.lease.leaseRef === "lease.managed.rebound")?.clientKeyThumbprint)
      .toBe(FOREIGN_THUMBPRINT)
  })

  test("key bindings survive durable restore into a fresh broker instance", async () => {
    const stateStore = new MemoryBrokerStateStore()
    const { verifier } = scriptedVerifier({ kind: "echo-expected" })
    const first = fixture({ stateStore, proofVerifier: verifier })
    await run(first.broker.issue(issueInput({ clientKeyThumbprint: BOUND_THUMBPRINT })))
    const restored = await PortableCapabilityBroker.restore(first.brokerConfig)
    await expectReason(
      restored.redeem({ operationRef: "operation.redeem.restored.no-proof", leaseRef: "lease.local.provider" }),
      "proof_required",
    )
    const outcome = await run(restored.redeem({
      operationRef: "operation.redeem.restored",
      leaseRef: "lease.local.provider",
      redemptionProof: redemptionProof(),
    }))
    expect(outcome.status).toBe("completed")
  })
})
