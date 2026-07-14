import { Effect } from "effect"

import type {
  ExecutionEnvironmentRef,
  PortableCapabilityKind,
  PortableCapabilityLease,
  PortableTargetClass,
} from "./index.js"

export const PORTABLE_CAPABILITY_BROKER_VERSION =
  "openagents.portable_capability_broker.v1" as const

export type CapabilityBrokerOperation =
  | "issue"
  | "redeem"
  | "renew"
  | "revoke"
  | "reissue"
  | "release"
  | "wipe"

export type CapabilityBrokerOutcomeStatus =
  | "completed"
  | "replayed"
  | "rejected"
  | "failed"

export type CapabilityBrokerReason =
  | "broker_unavailable"
  | "cleanup_failed"
  | "conflicting_replay"
  | "expired"
  | "invalid_scope"
  | "lease_not_active"
  | "proof_invalid"
  | "proof_required"
  | "source_revoked"
  | "target_denied"
  | "target_mismatch"

export type CapabilityBrokerOutcome = {
  readonly schema: typeof PORTABLE_CAPABILITY_BROKER_VERSION
  readonly operationRef: string
  readonly operation: CapabilityBrokerOperation
  readonly status: CapabilityBrokerOutcomeStatus
  readonly leaseRef: string
  readonly resultingLeaseRef?: string
  readonly reason?: CapabilityBrokerReason
  readonly evidenceRefs: ReadonlyArray<string>
}

export type CapabilityBrokerEvidence = {
  readonly schema: typeof PORTABLE_CAPABILITY_BROKER_VERSION
  readonly evidenceRef: string
  readonly operationRef: string
  readonly operation: CapabilityBrokerOperation
  readonly status: Exclude<CapabilityBrokerOutcomeStatus, "replayed">
  readonly leaseRef: string
  readonly resultingLeaseRef?: string
  readonly ownerRef: string
  readonly sessionRef: string
  readonly attachmentRef: string
  readonly attachmentGeneration: number
  readonly targetRef: ExecutionEnvironmentRef
  readonly capability: PortableCapabilityKind
  readonly accountRef?: string
  readonly toolRef?: string
  readonly reason?: CapabilityBrokerReason
  readonly occurredAt: string
  readonly material: "excluded"
}

export type CapabilityLeaseRecord = {
  readonly lease: PortableCapabilityLease
  readonly sourceGrantRef: string
  readonly permissions: ReadonlyArray<string>
  /**
   * ENV-2 (openagents #8780): optional RFC 7638 thumbprint of the client key
   * that must prove possession (RFC 9449 DPoP) to redeem this lease. Public
   * key-hash material only — never a secret. Absent = the lease keeps the
   * pre-ENV-2 redemption path unchanged.
   */
  readonly clientKeyThumbprint?: string
  readonly issuedAt: string
  readonly redeemedAt?: string
  readonly renewedAt?: string
  readonly revokedAt?: string
  readonly revocationConfirmedAt?: string
  readonly releasedAt?: string
  readonly wipedAt?: string
  readonly renewalCount: number
  readonly targetInstallationRef?: string
}

export type CapabilityBrokerSnapshot = {
  readonly schema: typeof PORTABLE_CAPABILITY_BROKER_VERSION
  readonly leases: ReadonlyArray<{
    readonly lease: PortableCapabilityLease
    readonly permissions: ReadonlyArray<string>
    readonly clientKeyThumbprint?: string
    readonly issuedAt: string
    readonly redeemedAt?: string
    readonly renewedAt?: string
    readonly revokedAt?: string
    readonly revocationConfirmedAt?: string
    readonly releasedAt?: string
    readonly wipedAt?: string
    readonly renewalCount: number
    readonly targetInstallationRef?: string
  }>
  readonly outcomes: ReadonlyArray<CapabilityBrokerOutcome>
  readonly evidence: ReadonlyArray<CapabilityBrokerEvidence>
}

export type SecretMaterial = Uint8Array & { readonly __secretMaterial: unique symbol }

export type CapabilitySecretVault = {
  readonly withSourceGrantMaterial: <A>(input: {
    readonly sourceGrantRef: string
    readonly leaseRef: string
    readonly use: (material: SecretMaterial) => Promise<A>
  }) => Promise<A>
  readonly revokeSourceGrant: (input: {
    readonly sourceGrantRef: string
    readonly leaseRef: string
  }) => Promise<void>
}

export type CapabilityTargetAdapter = {
  readonly adapterRef: string
  readonly targetClass: PortableTargetClass
  readonly redeem: (input: {
    readonly lease: PortableCapabilityLease
    readonly permissions: ReadonlyArray<string>
    readonly material: SecretMaterial
  }) => Promise<{ readonly installationRef: string }>
  readonly wipe: (input: {
    readonly leaseRef: string
    readonly targetRef: ExecutionEnvironmentRef
    readonly attachmentRef: string
    readonly attachmentGeneration: number
    readonly installationRef?: string
  }) => Promise<{ readonly wipeReceiptRef: string }>
}

export type CapabilityAdapterRuntime = {
  readonly install: CapabilityTargetAdapter["redeem"]
  readonly wipe: CapabilityTargetAdapter["wipe"]
}

export type CapabilityTargetBinding = {
  readonly targetRef: ExecutionEnvironmentRef
  readonly targetClass: PortableTargetClass
  readonly adapterRef: string
  readonly ready: boolean
}

export type CapabilityBrokerClock = {
  readonly now: () => Date
}

/**
 * ENV-2 (openagents #8780) opt-in proof-of-possession for lease redemption.
 * `htm`/`htu` MUST be populated by the transport host from the request it
 * actually received (method + target URI), never from client-supplied
 * values — otherwise the proof binds nothing.
 */
export type CapabilityRedemptionProof = {
  readonly scheme: "dpop"
  /** Compact JWS DPoP proof presented by the redeeming client. */
  readonly proof: string
  readonly htm: string
  readonly htu: string
}

/**
 * Verifier seam for key-bound redemption. The reference implementation is
 * `@openagentsinc/environment-auth`'s `makeDpopCapabilityProofVerifier`
 * (RFC 9449 semantics: signature, thumbprint binding, htm/htu, bounded
 * clock skew, single-use jti). The broker treats it as opaque: any thrown
 * error or `ok: false` fails the redemption closed.
 */
export type CapabilityProofVerifier = {
  readonly verify: (input: {
    readonly proof: string
    readonly htm: string
    readonly htu: string
    readonly expectedThumbprint: string
  }) => Promise<
    | { readonly ok: true; readonly thumbprint: string }
    | { readonly ok: false; readonly reason: string }
  >
}

export type CapabilityBrokerConfig = {
  readonly vault: CapabilitySecretVault
  readonly targets: ReadonlyArray<CapabilityTargetBinding>
  readonly adapters: ReadonlyArray<CapabilityTargetAdapter>
  readonly clock?: CapabilityBrokerClock
  /** Legacy split sink. Production portable moves use atomicStateStore. */
  readonly evidenceSink?: {
    readonly append: (evidence: CapabilityBrokerEvidence) => Promise<void>
  }
  readonly maxTtlMs?: number
  /** Private refs-only persistence; never expose this state as a public snapshot. */
  readonly stateStore?: CapabilityBrokerStateStore
  /**
   * Production persistence seam. One CAS transaction stores the complete
   * refs-only broker state and its operation evidence while verifying the
   * exact active move claim bound by the concrete store.
   */
  readonly atomicStateStore?: CapabilityBrokerAtomicStateStore
  /**
   * ENV-2 (openagents #8780) opt-in proof-of-possession seam. Only consulted
   * for leases issued with a `clientKeyThumbprint`; such leases fail closed
   * when this verifier is absent, and unbound leases never touch it.
   */
  readonly proofVerifier?: CapabilityProofVerifier
}

export type CapabilityBrokerPrivateDurableState = {
  readonly schema: typeof PORTABLE_CAPABILITY_BROKER_VERSION
  readonly records: ReadonlyArray<CapabilityLeaseRecord>
  readonly operations: ReadonlyArray<Readonly<{
    operationRef: string
    fingerprint: string
    outcome: CapabilityBrokerOutcome
  }>>
  readonly evidence: ReadonlyArray<CapabilityBrokerEvidence>
  readonly material: "excluded"
}

export type CapabilityBrokerStateStore = {
  readonly load: () => Promise<CapabilityBrokerPrivateDurableState | null>
  readonly save: (state: CapabilityBrokerPrivateDurableState) => Promise<void>
}

export type CapabilityBrokerAtomicLoad = {
  readonly revision: number
  readonly state: CapabilityBrokerPrivateDurableState | null
}

export type CapabilityBrokerAtomicCommit = {
  readonly expectedRevision: number
  readonly state: CapabilityBrokerPrivateDurableState
  readonly evidence: CapabilityBrokerEvidence
}

export type CapabilityBrokerAtomicStateStore = {
  readonly load: () => Promise<CapabilityBrokerAtomicLoad>
  readonly commit: (
    input: CapabilityBrokerAtomicCommit,
  ) => Promise<{ readonly revision: number }>
}

export type IssueCapabilityInput = {
  readonly operationRef: string
  readonly leaseRef: string
  readonly ownerRef: string
  readonly sessionRef: string
  readonly attachmentRef: string
  readonly attachmentGeneration: number
  readonly targetRef: ExecutionEnvironmentRef
  readonly capability: PortableCapabilityKind
  readonly sourceGrantRef: string
  readonly accountRef?: string
  readonly toolRef?: string
  readonly permissions: ReadonlyArray<string>
  readonly expiresAt: string
  /**
   * ENV-2 (openagents #8780): opt into DPoP-bound redemption by binding the
   * lease to the RFC 7638 thumbprint of the redeeming client's key. Once
   * bound, redemption requires a valid possession proof and the binding can
   * never be silently dropped (see reissue).
   */
  readonly clientKeyThumbprint?: string
}

export type LeaseOperationInput = {
  readonly operationRef: string
  readonly leaseRef: string
}

export type RedeemCapabilityInput = LeaseOperationInput & {
  /** Required iff the lease was issued with a `clientKeyThumbprint`. */
  readonly redemptionProof?: CapabilityRedemptionProof
}

export type RenewCapabilityInput = LeaseOperationInput & {
  readonly expiresAt: string
}

export type ReissueCapabilityInput = LeaseOperationInput & {
  readonly newLeaseRef: string
  readonly destinationSourceGrantRef: string
  readonly destinationAttachmentRef: string
  readonly destinationAttachmentGeneration: number
  readonly destinationTargetRef: string
  readonly expiresAt: string
  /**
   * ENV-2 (openagents #8780): key binding for the destination lease. When the
   * source lease is key-bound this is REQUIRED — a bound lease can never be
   * laundered into an unbound one through reissue. When the source lease is
   * unbound this may add a binding (narrowing is always allowed).
   */
  readonly destinationClientKeyThumbprint?: string
}

export class CapabilityBrokerError extends Error {
  readonly _tag = "CapabilityBrokerError"

  constructor(
    readonly reason: CapabilityBrokerReason,
    readonly operationRef: string,
    message: string,
  ) {
    super(message)
  }
}

type StoredOutcome = {
  readonly fingerprint: string
  readonly outcome: CapabilityBrokerOutcome
}

const DEFAULT_MAX_TTL_MS = 15 * 60 * 1000

function canonical(value: unknown): string {
  if (Array.isArray(value)) return `[${value.map(canonical).join(",")}]`
  if (value !== null && typeof value === "object") {
    return `{${Object.entries(value as Record<string, unknown>)
      .sort(([left], [right]) => left.localeCompare(right))
      .map(([key, item]) => `${JSON.stringify(key)}:${canonical(item)}`)
      .join(",")}}`
  }
  return JSON.stringify(value)
}

function safeRef(value: string): boolean {
  return /^[a-zA-Z0-9][a-zA-Z0-9._:-]{2,255}$/.test(value)
}

// Base64url SHA-256 (RFC 7638 JWK thumbprint) — exactly 43 base64url chars.
function safeThumbprint(value: string): boolean {
  return /^[a-zA-Z0-9_-]{43}$/.test(value)
}

function cloneLease(lease: PortableCapabilityLease): PortableCapabilityLease {
  return { ...lease }
}

export class PortableCapabilityBroker {
  private readonly clock: CapabilityBrokerClock
  private readonly maxTtlMs: number
  private readonly targets: Map<string, CapabilityTargetBinding>
  private readonly adapters: Map<string, CapabilityTargetAdapter>
  private readonly records = new Map<string, CapabilityLeaseRecord>()
  private readonly operations = new Map<string, StoredOutcome>()
  private readonly evidence: CapabilityBrokerEvidence[] = []
  private durableRevision = 0

  constructor(private readonly config: CapabilityBrokerConfig) {
    if (config.atomicStateStore && (config.stateStore || config.evidenceSink)) {
      throw new CapabilityBrokerError(
        "invalid_scope",
        "operation.broker.configure",
        "atomic broker persistence cannot be combined with split persistence",
      )
    }
    if (!config.atomicStateStore && !config.evidenceSink) {
      throw new CapabilityBrokerError(
        "invalid_scope",
        "operation.broker.configure",
        "broker evidence persistence is required",
      )
    }
    this.clock = config.clock ?? { now: () => new Date() }
    this.maxTtlMs = config.maxTtlMs ?? DEFAULT_MAX_TTL_MS
    this.targets = new Map(config.targets.map((target) => [target.targetRef, target]))
    this.adapters = new Map(config.adapters.map((adapter) => [adapter.adapterRef, adapter]))
  }

  static async restore(config: CapabilityBrokerConfig): Promise<PortableCapabilityBroker> {
    const broker = new PortableCapabilityBroker(config)
    const atomic = await config.atomicStateStore?.load()
    if (atomic) broker.durableRevision = atomic.revision
    const state = atomic?.state ?? await config.stateStore?.load()
    if (state === undefined || state === null) return broker
    if (state.schema !== PORTABLE_CAPABILITY_BROKER_VERSION || state.material !== "excluded") {
      throw new CapabilityBrokerError("invalid_scope", "operation.broker.restore", "durable broker state is invalid")
    }
    for (const record of state.records) {
      broker.records.set(record.lease.leaseRef, {
        ...record,
        lease: cloneLease(record.lease),
        permissions: [...record.permissions],
      })
    }
    for (const operation of state.operations) {
      broker.operations.set(operation.operationRef, {
        fingerprint: operation.fingerprint,
        outcome: { ...operation.outcome, evidenceRefs: [...operation.outcome.evidenceRefs] },
      })
    }
    broker.evidence.push(...state.evidence.map(item => ({ ...item })))
    return broker
  }

  issue(input: IssueCapabilityInput): Effect.Effect<CapabilityBrokerOutcome, CapabilityBrokerError> {
    return this.execute("issue", input.operationRef, input.leaseRef, input, async () => {
      this.validateIssue(input)
      if (this.records.has(input.leaseRef)) {
        throw new CapabilityBrokerError("conflicting_replay", input.operationRef, "lease ref already exists")
      }
      const now = this.clock.now().toISOString()
      const lease: PortableCapabilityLease = {
        leaseRef: input.leaseRef,
        ownerRef: input.ownerRef,
        sessionRef: input.sessionRef,
        attachmentRef: input.attachmentRef,
        attachmentGeneration: input.attachmentGeneration,
        targetRef: input.targetRef,
        capability: input.capability,
        ...(input.accountRef ? { accountRef: input.accountRef } : {}),
        ...(input.toolRef ? { toolRef: input.toolRef } : {}),
        expiresAt: input.expiresAt,
        state: "issued",
      }
      this.records.set(input.leaseRef, {
        lease,
        sourceGrantRef: input.sourceGrantRef,
        permissions: [...new Set(input.permissions)].sort(),
        ...(input.clientKeyThumbprint === undefined
          ? {}
          : { clientKeyThumbprint: input.clientKeyThumbprint }),
        issuedAt: now,
        renewalCount: 0,
      })
      return this.success("issue", input.operationRef, input.leaseRef)
    })
  }

  redeem(input: RedeemCapabilityInput): Effect.Effect<CapabilityBrokerOutcome, CapabilityBrokerError> {
    return this.execute("redeem", input.operationRef, input.leaseRef, input, async () => {
      const record = await this.requireActive(input, true)
      // Proof-of-possession gate (ENV-2 #8780) runs before any target or
      // vault access, so a key-bound lease fails closed without ever letting
      // secret material near an unproven client.
      await this.requireRedemptionProof(record, input)
      const target = this.requireTarget(record, input.operationRef)
      const adapter = this.requireAdapter(target, input.operationRef)
      try {
        const installed = await this.config.vault.withSourceGrantMaterial({
          sourceGrantRef: record.sourceGrantRef,
          leaseRef: input.leaseRef,
          use: material => adapter.redeem({
            lease: cloneLease(record.lease),
            permissions: record.permissions,
            material,
          }),
        })
        this.records.set(input.leaseRef, {
          ...record,
          lease: { ...record.lease, state: "redeemed" },
          redeemedAt: this.clock.now().toISOString(),
          targetInstallationRef: installed.installationRef,
        })
        return this.success("redeem", input.operationRef, input.leaseRef)
      } catch (error) {
        const reason = error instanceof CapabilityBrokerError
          ? error.reason
          : String(error).includes("target_denied") ? "target_denied" : "broker_unavailable"
        throw new CapabilityBrokerError(reason, input.operationRef, "target-scoped redemption failed closed")
      }
    })
  }

  renew(input: RenewCapabilityInput): Effect.Effect<CapabilityBrokerOutcome, CapabilityBrokerError> {
    return this.execute("renew", input.operationRef, input.leaseRef, input, async () => {
      const record = await this.requireActive(input, false)
      this.validateExpiry(input.expiresAt, input.operationRef)
      this.records.set(input.leaseRef, {
        ...record,
        lease: { ...record.lease, expiresAt: input.expiresAt },
        renewedAt: this.clock.now().toISOString(),
        renewalCount: record.renewalCount + 1,
      })
      return this.success("renew", input.operationRef, input.leaseRef)
    })
  }

  revoke(input: LeaseOperationInput): Effect.Effect<CapabilityBrokerOutcome, CapabilityBrokerError> {
    return this.execute("revoke", input.operationRef, input.leaseRef, input, () => this.revokeInternal(input, "revoke"))
  }

  wipe(input: LeaseOperationInput): Effect.Effect<CapabilityBrokerOutcome, CapabilityBrokerError> {
    return this.execute("wipe", input.operationRef, input.leaseRef, input, () => this.wipeInternal(input, "wipe"))
  }

  release(input: LeaseOperationInput): Effect.Effect<CapabilityBrokerOutcome, CapabilityBrokerError> {
    return this.execute("release", input.operationRef, input.leaseRef, input, async () => {
      await this.revokeInternal({ ...input, operationRef: `${input.operationRef}.revoke` }, "release")
      await this.wipeInternal({ ...input, operationRef: `${input.operationRef}.wipe` }, "release")
      const record = this.requireRecord(input.leaseRef, input.operationRef)
      this.records.set(input.leaseRef, {
        ...record,
        lease: { ...record.lease, state: "released" },
        releasedAt: this.clock.now().toISOString(),
      })
      return this.success("release", input.operationRef, input.leaseRef)
    })
  }

  reissue(input: ReissueCapabilityInput): Effect.Effect<CapabilityBrokerOutcome, CapabilityBrokerError> {
    return this.execute("reissue", input.operationRef, input.leaseRef, input, async () => {
      const source = this.requireRecord(input.leaseRef, input.operationRef)
      // ENV-2 (#8780): a key-bound lease can never be laundered into an
      // unbound one, and a malformed destination binding must not revoke the
      // source — both checks run before any revocation side effect.
      if (source.clientKeyThumbprint !== undefined && input.destinationClientKeyThumbprint === undefined) {
        throw new CapabilityBrokerError(
          "invalid_scope",
          input.operationRef,
          "key-bound lease cannot be reissued without a destination key binding",
        )
      }
      if (input.destinationClientKeyThumbprint !== undefined && !safeThumbprint(input.destinationClientKeyThumbprint)) {
        throw new CapabilityBrokerError(
          "invalid_scope",
          input.operationRef,
          "destination client key thumbprint is malformed",
        )
      }
      await this.revokeInternal({ operationRef: `${input.operationRef}.revoke`, leaseRef: input.leaseRef }, "reissue")
      await this.wipeInternal({ operationRef: `${input.operationRef}.wipe`, leaseRef: input.leaseRef }, "reissue")
      const issueInput: IssueCapabilityInput = {
        operationRef: `${input.operationRef}.issue`,
        leaseRef: input.newLeaseRef,
        ownerRef: source.lease.ownerRef,
        sessionRef: source.lease.sessionRef,
        attachmentRef: input.destinationAttachmentRef,
        attachmentGeneration: input.destinationAttachmentGeneration,
        targetRef: input.destinationTargetRef,
        capability: source.lease.capability,
        sourceGrantRef: input.destinationSourceGrantRef,
        ...(source.lease.accountRef ? { accountRef: source.lease.accountRef } : {}),
        ...(source.lease.toolRef ? { toolRef: source.lease.toolRef } : {}),
        permissions: source.permissions,
        expiresAt: input.expiresAt,
      }
      this.validateIssue(issueInput)
      if (input.destinationAttachmentGeneration <= source.lease.attachmentGeneration) {
        throw new CapabilityBrokerError("invalid_scope", input.operationRef, "destination generation must advance")
      }
      if (this.records.has(input.newLeaseRef)) {
        throw new CapabilityBrokerError("conflicting_replay", input.operationRef, "destination lease ref already exists")
      }
      this.records.set(input.newLeaseRef, {
        lease: {
          leaseRef: input.newLeaseRef,
          ownerRef: source.lease.ownerRef,
          sessionRef: source.lease.sessionRef,
          attachmentRef: input.destinationAttachmentRef,
          attachmentGeneration: input.destinationAttachmentGeneration,
          targetRef: input.destinationTargetRef,
          capability: source.lease.capability,
          ...(source.lease.accountRef ? { accountRef: source.lease.accountRef } : {}),
          ...(source.lease.toolRef ? { toolRef: source.lease.toolRef } : {}),
          expiresAt: input.expiresAt,
          state: "issued",
        },
        sourceGrantRef: input.destinationSourceGrantRef,
        permissions: source.permissions,
        ...(input.destinationClientKeyThumbprint === undefined
          ? {}
          : { clientKeyThumbprint: input.destinationClientKeyThumbprint }),
        issuedAt: this.clock.now().toISOString(),
        renewalCount: 0,
      })
      return this.success("reissue", input.operationRef, input.leaseRef, input.newLeaseRef)
    })
  }

  expireLeases(operationPrefix: string): Effect.Effect<ReadonlyArray<CapabilityBrokerOutcome>, CapabilityBrokerError> {
    return Effect.tryPromise({
      try: async () => {
        const now = this.clock.now().getTime()
        const outcomes: CapabilityBrokerOutcome[] = []
        for (const record of this.records.values()) {
          if (["issued", "redeemed"].includes(record.lease.state) && Date.parse(record.lease.expiresAt) <= now) {
            const operationRef = `${operationPrefix}.${record.lease.leaseRef}`
            const outcome = await Effect.runPromise(this.execute(
              "revoke",
              operationRef,
              record.lease.leaseRef,
              { operationRef, leaseRef: record.lease.leaseRef, reason: "expired" },
              async () => {
                await this.revokeInternal({ operationRef, leaseRef: record.lease.leaseRef }, "revoke", "expired")
                await this.wipeInternal({ operationRef: `${operationRef}.wipe`, leaseRef: record.lease.leaseRef }, "wipe")
                return this.success("revoke", operationRef, record.lease.leaseRef, undefined, "expired")
              },
            ))
            outcomes.push(outcome)
          }
        }
        return outcomes
      },
      catch: error => this.asBrokerError(error, operationPrefix),
    })
  }

  snapshot(): CapabilityBrokerSnapshot {
    return {
      schema: PORTABLE_CAPABILITY_BROKER_VERSION,
      leases: [...this.records.values()].map(({ sourceGrantRef: _excluded, ...record }) => ({
        ...record,
        lease: cloneLease(record.lease),
        permissions: [...record.permissions],
      })),
      outcomes: [...this.operations.values()].map(item => ({ ...item.outcome, evidenceRefs: [...item.outcome.evidenceRefs] })),
      evidence: this.evidence.map(item => ({ ...item })),
    }
  }

  private execute<T extends object>(
    operation: CapabilityBrokerOperation,
    operationRef: string,
    leaseRef: string,
    input: T,
    run: () => Promise<CapabilityBrokerOutcome>,
  ): Effect.Effect<CapabilityBrokerOutcome, CapabilityBrokerError> {
    const fingerprint = canonical({ operation, input })
    const prior = this.operations.get(operationRef)
    if (prior) {
      if (prior.fingerprint !== fingerprint) {
        return Effect.fail(new CapabilityBrokerError("conflicting_replay", operationRef, "operation ref was replayed with different bytes"))
      }
      return Effect.succeed({ ...prior.outcome, status: "replayed" })
    }
    return Effect.tryPromise({
      try: async () => {
        try {
          const outcome = await run()
          await this.recordOutcome(fingerprint, outcome)
          return this.operations.get(operationRef)?.outcome ?? outcome
        } catch (error) {
          const brokerError = this.asBrokerError(error, operationRef)
          const record = this.records.get(leaseRef)
          const outcome: CapabilityBrokerOutcome = {
            schema: PORTABLE_CAPABILITY_BROKER_VERSION,
            operationRef,
            operation,
            status: brokerError.reason === "invalid_scope" || brokerError.reason === "lease_not_active" || brokerError.reason === "target_mismatch" || brokerError.reason === "conflicting_replay" || brokerError.reason === "proof_required" || brokerError.reason === "proof_invalid" ? "rejected" : "failed",
            leaseRef,
            reason: brokerError.reason,
            evidenceRefs: [],
          }
          await this.recordOutcome(fingerprint, outcome, record)
          throw brokerError
        }
      },
      catch: error => this.asBrokerError(error, operationRef),
    })
  }

  private async recordOutcome(fingerprint: string, outcome: CapabilityBrokerOutcome, record = this.records.get(outcome.leaseRef)): Promise<void> {
    const evidenceRef = `evidence.capability.${outcome.operationRef}`
    const evidence = this.makeEvidence(evidenceRef, outcome, record)
    const withEvidence = { ...outcome, evidenceRefs: [evidenceRef] }
    if (this.config.atomicStateStore) {
      const state = this.privateState(
        [...this.operations.entries(), [outcome.operationRef, { fingerprint, outcome: withEvidence }]],
        [...this.evidence, evidence],
      )
      const committed = await this.config.atomicStateStore.commit({
        expectedRevision: this.durableRevision,
        state,
        evidence,
      })
      this.durableRevision = committed.revision
      this.operations.set(outcome.operationRef, { fingerprint, outcome: withEvidence })
      this.evidence.push(evidence)
      return
    }
    await this.config.evidenceSink!.append(evidence)
    this.operations.set(outcome.operationRef, { fingerprint, outcome: withEvidence })
    this.evidence.push(evidence)
    await this.persistState()
  }

  private privateState(
    operations: ReadonlyArray<readonly [string, StoredOutcome]> = [...this.operations.entries()],
    evidence: ReadonlyArray<CapabilityBrokerEvidence> = this.evidence,
  ): CapabilityBrokerPrivateDurableState {
    return {
      schema: PORTABLE_CAPABILITY_BROKER_VERSION,
      records: [...this.records.values()].map(record => ({
        ...record,
        lease: cloneLease(record.lease),
        permissions: [...record.permissions],
      })),
      operations: operations.map(([operationRef, operation]) => ({
        operationRef,
        fingerprint: operation.fingerprint,
        outcome: { ...operation.outcome, evidenceRefs: [...operation.outcome.evidenceRefs] },
      })),
      evidence: evidence.map(item => ({ ...item })),
      material: "excluded",
    }
  }

  private async persistState(): Promise<void> {
    if (!this.config.stateStore) return
    await this.config.stateStore.save(this.privateState())
  }

  private makeEvidence(evidenceRef: string, outcome: CapabilityBrokerOutcome, record?: CapabilityLeaseRecord): CapabilityBrokerEvidence {
    const lease = record?.lease
    return {
      schema: PORTABLE_CAPABILITY_BROKER_VERSION,
      evidenceRef,
      operationRef: outcome.operationRef,
      operation: outcome.operation,
      status: outcome.status === "replayed" ? "completed" : outcome.status,
      leaseRef: outcome.leaseRef,
      ...(outcome.resultingLeaseRef ? { resultingLeaseRef: outcome.resultingLeaseRef } : {}),
      ownerRef: lease?.ownerRef ?? "owner.unknown",
      sessionRef: lease?.sessionRef ?? "session.unknown",
      attachmentRef: lease?.attachmentRef ?? "attachment.unknown",
      attachmentGeneration: lease?.attachmentGeneration ?? 0,
      targetRef: lease?.targetRef ?? "target.unknown",
      capability: lease?.capability ?? "api",
      ...(lease?.accountRef ? { accountRef: lease.accountRef } : {}),
      ...(lease?.toolRef ? { toolRef: lease.toolRef } : {}),
      ...(outcome.reason ? { reason: outcome.reason } : {}),
      occurredAt: this.clock.now().toISOString(),
      material: "excluded",
    }
  }

  private success(
    operation: CapabilityBrokerOperation,
    operationRef: string,
    leaseRef: string,
    resultingLeaseRef?: string,
    reason?: CapabilityBrokerReason,
  ): CapabilityBrokerOutcome {
    return {
      schema: PORTABLE_CAPABILITY_BROKER_VERSION,
      operationRef,
      operation,
      status: "completed",
      leaseRef,
      ...(resultingLeaseRef ? { resultingLeaseRef } : {}),
      ...(reason ? { reason } : {}),
      evidenceRefs: [],
    }
  }

  private validateIssue(input: IssueCapabilityInput): void {
    for (const value of [input.operationRef, input.leaseRef, input.ownerRef, input.sessionRef, input.attachmentRef, input.targetRef, input.sourceGrantRef]) {
      if (!safeRef(value)) throw new CapabilityBrokerError("invalid_scope", input.operationRef, "scope contains an invalid ref")
    }
    if (input.attachmentGeneration < 0 || !Number.isInteger(input.attachmentGeneration)) {
      throw new CapabilityBrokerError("invalid_scope", input.operationRef, "attachment generation must be a non-negative integer")
    }
    if (input.permissions.length === 0 || input.permissions.some(permission => !safeRef(permission))) {
      throw new CapabilityBrokerError("invalid_scope", input.operationRef, "least-privilege permissions are required")
    }
    if (input.clientKeyThumbprint !== undefined && !safeThumbprint(input.clientKeyThumbprint)) {
      throw new CapabilityBrokerError("invalid_scope", input.operationRef, "client key thumbprint is malformed")
    }
    if (["provider", "scm_read", "scm_write"].includes(input.capability) && !input.accountRef) {
      throw new CapabilityBrokerError("invalid_scope", input.operationRef, "provider and SCM leases require an account ref")
    }
    if ((input.capability === "tool" || input.capability === "api") && !input.toolRef) {
      throw new CapabilityBrokerError("invalid_scope", input.operationRef, "tool and API leases require a tool ref")
    }
    this.validateExpiry(input.expiresAt, input.operationRef)
    const target = this.targets.get(input.targetRef)
    if (!target || !target.ready) {
      throw new CapabilityBrokerError("target_denied", input.operationRef, "target is absent, revoked, or not ready")
    }
  }

  private validateExpiry(expiresAt: string, operationRef: string): void {
    const expiry = Date.parse(expiresAt)
    const now = this.clock.now().getTime()
    if (!Number.isFinite(expiry) || expiry <= now) {
      throw new CapabilityBrokerError("expired", operationRef, "lease expiry is not in the future")
    }
    if (expiry - now > this.maxTtlMs) {
      throw new CapabilityBrokerError("invalid_scope", operationRef, "lease exceeds maximum TTL")
    }
  }

  private requireRecord(leaseRef: string, operationRef: string): CapabilityLeaseRecord {
    const record = this.records.get(leaseRef)
    if (!record) throw new CapabilityBrokerError("lease_not_active", operationRef, "lease does not exist")
    return record
  }

  private async requireActive(input: LeaseOperationInput, expireAndWipe: boolean): Promise<CapabilityLeaseRecord> {
    const record = this.requireRecord(input.leaseRef, input.operationRef)
    if (!["issued", "redeemed"].includes(record.lease.state)) {
      throw new CapabilityBrokerError("lease_not_active", input.operationRef, "lease is not active")
    }
    if (Date.parse(record.lease.expiresAt) <= this.clock.now().getTime()) {
      this.records.set(input.leaseRef, {
        ...record,
        lease: { ...record.lease, state: "expired" },
        revokedAt: this.clock.now().toISOString(),
      })
      if (expireAndWipe) {
        try {
          await this.revokeInternal(input, "revoke", "expired")
          await this.wipeInternal({ ...input, operationRef: `${input.operationRef}.wipe` }, "wipe")
        } catch {
          throw new CapabilityBrokerError("cleanup_failed", input.operationRef, "expired lease cleanup failed closed")
        }
      }
      throw new CapabilityBrokerError("expired", input.operationRef, "lease expired")
    }
    return record
  }

  /**
   * ENV-2 (#8780) opt-in fail-closed possession gate. Leases without a key
   * binding return immediately (existing behavior, proof ignored). Key-bound
   * leases require a configured verifier, a DPoP-scheme proof, and a verified
   * thumbprint equal to the bound one — anything else fails closed before the
   * vault or target adapter is touched.
   */
  private async requireRedemptionProof(
    record: CapabilityLeaseRecord,
    input: RedeemCapabilityInput,
  ): Promise<void> {
    const boundThumbprint = record.clientKeyThumbprint
    if (boundThumbprint === undefined) return
    const verifier = this.config.proofVerifier
    if (!verifier) {
      throw new CapabilityBrokerError(
        "proof_required",
        input.operationRef,
        "lease is key-bound but this broker has no proof verifier configured",
      )
    }
    const redemptionProof = input.redemptionProof
    if (!redemptionProof || redemptionProof.scheme !== "dpop") {
      throw new CapabilityBrokerError(
        "proof_required",
        input.operationRef,
        "key-bound lease redemption requires a DPoP possession proof",
      )
    }
    let verification: Awaited<ReturnType<CapabilityProofVerifier["verify"]>>
    try {
      verification = await verifier.verify({
        proof: redemptionProof.proof,
        htm: redemptionProof.htm,
        htu: redemptionProof.htu,
        expectedThumbprint: boundThumbprint,
      })
    } catch {
      throw new CapabilityBrokerError(
        "proof_invalid",
        input.operationRef,
        "possession proof verification failed closed",
      )
    }
    if (!verification.ok || verification.thumbprint !== boundThumbprint) {
      throw new CapabilityBrokerError(
        "proof_invalid",
        input.operationRef,
        "possession proof does not prove the bound client key",
      )
    }
  }

  private requireTarget(record: CapabilityLeaseRecord, operationRef: string): CapabilityTargetBinding {
    const target = this.targets.get(record.lease.targetRef)
    if (!target || !target.ready) throw new CapabilityBrokerError("target_denied", operationRef, "target is not ready")
    return target
  }

  private requireAdapter(target: CapabilityTargetBinding, operationRef: string): CapabilityTargetAdapter {
    const adapter = this.adapters.get(target.adapterRef)
    if (!adapter || adapter.targetClass !== target.targetClass) {
      throw new CapabilityBrokerError("target_mismatch", operationRef, "target adapter does not match target class")
    }
    return adapter
  }

  private async revokeInternal(input: LeaseOperationInput, operation: CapabilityBrokerOperation, reason?: CapabilityBrokerReason): Promise<CapabilityBrokerOutcome> {
    const record = this.requireRecord(input.leaseRef, input.operationRef)
    if (["revoked", "expired", "released"].includes(record.lease.state) && record.revocationConfirmedAt) {
      return this.success(operation, input.operationRef, input.leaseRef, undefined, reason)
    }
    this.records.set(input.leaseRef, {
      ...record,
      lease: { ...record.lease, state: reason === "expired" ? "expired" : "revoked" },
      revokedAt: this.clock.now().toISOString(),
    })
    try {
      await this.config.vault.revokeSourceGrant({ sourceGrantRef: record.sourceGrantRef, leaseRef: input.leaseRef })
      const current = this.requireRecord(input.leaseRef, input.operationRef)
      this.records.set(input.leaseRef, {
        ...current,
        revocationConfirmedAt: this.clock.now().toISOString(),
      })
    } catch {
      throw new CapabilityBrokerError("broker_unavailable", input.operationRef, "source grant revocation failed closed")
    }
    return this.success(operation, input.operationRef, input.leaseRef, undefined, reason)
  }

  private async wipeInternal(input: LeaseOperationInput, operation: CapabilityBrokerOperation): Promise<CapabilityBrokerOutcome> {
    const record = this.requireRecord(input.leaseRef, input.operationRef)
    if (record.wipedAt) return this.success(operation, input.operationRef, input.leaseRef)
    await this.wipeAdapter(record, input.operationRef)
    this.records.set(input.leaseRef, { ...this.requireRecord(input.leaseRef, input.operationRef), wipedAt: this.clock.now().toISOString() })
    return this.success(operation, input.operationRef, input.leaseRef)
  }

  private async wipeAdapter(record: CapabilityLeaseRecord, operationRef: string): Promise<void> {
    const target = this.targets.get(record.lease.targetRef)
    if (!target) throw new CapabilityBrokerError("target_mismatch", operationRef, "target binding is absent")
    const adapter = this.requireAdapter(target, operationRef)
    try {
      await adapter.wipe({
        leaseRef: record.lease.leaseRef,
        targetRef: record.lease.targetRef,
        attachmentRef: record.lease.attachmentRef,
        attachmentGeneration: record.lease.attachmentGeneration,
        ...(record.targetInstallationRef ? { installationRef: record.targetInstallationRef } : {}),
      })
    } catch {
      throw new CapabilityBrokerError("cleanup_failed", operationRef, "target wipe failed closed")
    }
  }

  private asBrokerError(error: unknown, operationRef: string): CapabilityBrokerError {
    return error instanceof CapabilityBrokerError
      ? error
      : new CapabilityBrokerError("broker_unavailable", operationRef, "capability broker dependency failed")
  }
}

export function makeCapabilityTargetAdapter(input: CapabilityTargetAdapter): CapabilityTargetAdapter {
  return input
}

export function makeOwnerLocalCapabilityAdapter(
  adapterRef: string,
  runtime: CapabilityAdapterRuntime,
): CapabilityTargetAdapter {
  return { adapterRef, targetClass: "owner_local", redeem: runtime.install, wipe: runtime.wipe }
}

export function makeOpenAgentsManagedCapabilityAdapter(
  adapterRef: string,
  runtime: CapabilityAdapterRuntime,
): CapabilityTargetAdapter {
  return { adapterRef, targetClass: "openagents_managed", redeem: runtime.install, wipe: runtime.wipe }
}
