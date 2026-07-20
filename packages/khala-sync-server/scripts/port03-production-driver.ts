import type {
  CapabilityBrokerClock,
  CapabilitySecretVault,
  PortableAgentGraph,
  PortableSessionExecutionBinding,
} from "@openagentsinc/portable-session-contract"
import { join } from "node:path"

import type { PylonPortableControlSessionLifecycle } from "../../../apps/pylon/src/node/control-sessions.js"
import {
  PylonPortableCheckpointArtifactStore,
  type PylonPortableCheckpointCustodyKeyProvider,
  type PylonPortableCheckpointKmsEnvelopeAuthority,
} from "../../../apps/pylon/src/portable-session-checkpoint-artifact.js"
import type { PylonPortableDestinationAuthority } from "../../../apps/pylon/src/portable-session-destination.js"
import { createPylonOwnerLocalDestinationLifecycle } from "../../../apps/pylon/src/portable-session-destination.js"
import { createPylonPortableLocalRehydrator } from "../../../apps/pylon/src/portable-session-local-rehydrator.js"
import { PylonPortableSessionOperationLedger } from "../../../apps/pylon/src/portable-session-operation-ledger.js"
import { createPylonOwnerLocalExecutionTarget } from "../../../apps/pylon/src/portable-session-target.js"
import {
  PostgresManagedAgentComputerTarget,
} from "../src/portable-managed-agent-computer-target.js"
import {
  createOaCodexControlPortableManagedContinuation,
  PostgresPortableManagedContinuationAuthority,
  type PortableManagedContinuation,
  type PortableManagedContinuationAuthority,
  type PortableManagedContinuationPlan,
} from "../src/portable-managed-continuation.js"
import {
  createOaCodexControlPortableProvisioner,
  type OaCodexControlPortableProvisionerConfig,
} from "../src/portable-managed-agent-computer-provisioner.js"
import {
  HttpPortableCapabilityGrantVault,
  makePortableCapabilityTargetAdapter,
  type HttpPortableCapabilityGrantVaultConfig,
} from "../src/portable-capability-runtime-adapters.js"
import {
  OwnerLocalPortableCapabilityInstallationPort,
  createPostgresManagedPortableCapabilityInstallationPort,
  type ManagedPortableCapabilityInstallationConfig,
  type OwnerLocalPortableCapabilityInstallationConfig,
} from "../src/portable-capability-installation-ports.js"
import {
  PostgresPortableSessionMoveRuntime,
  type PortableSessionMoveRuntimeBrokerConfig,
  type PortableSessionMoveRuntimeConfig,
  type PortableSessionMoveRuntimeInput,
} from "../src/portable-session-move-runtime.js"
import type {
  PortableSessionExecutionTarget,
  PortableSessionMoveInput,
  PortableSessionMoveResult,
} from "../src/portable-session-move.js"
import { readPortableSessionAuthoritySnapshot } from "../src/portable-session-authority.js"

const SAFE_REF = /^[A-Za-z0-9][A-Za-z0-9._:-]{2,255}$/u

const attachmentEvidenceRefs = (value: unknown): ReadonlyArray<string> => {
  const decoded = typeof value === "string" ? JSON.parse(value) : value
  if (!Array.isArray(decoded) || decoded.some(item => typeof item !== "string" || !SAFE_REF.test(item))) {
    throw new PortableSessionProductionDriverError("unsafe_receipt", "attachment authority evidence is invalid")
  }
  return decoded
}

const createPostgresLocalDestinationAuthority = (input: Readonly<{
  sql: PortableSessionMoveRuntimeConfig["sql"]
  ownerRef: string
}>): PylonPortableDestinationAuthority => ({
  readCurrentAttachment: async sessionRef => {
    const snapshot = await readPortableSessionAuthoritySnapshot(input.sql, {
      sessionRef,
      ownerUserId: input.ownerRef,
    })
    if (snapshot === null) throw new PortableSessionProductionDriverError("identity_mismatch", "portable authority is absent")
    const attachmentRef = String(snapshot.session.current_attachment_ref)
    const generation = Number(snapshot.session.current_attachment_generation)
    const attachment = snapshot.attachments.find(row => row.attachment_ref === attachmentRef)
    const state = attachment?.state
    const evidenceRefs = attachmentEvidenceRefs(attachment?.evidence_refs_json)
    if (attachment === undefined || !SAFE_REF.test(attachmentRef) || !Number.isSafeInteger(generation) || generation < 0 ||
        (state !== "active" && state !== "quiesced" && state !== "reclaimed") ||
        typeof attachment.target_ref !== "string" || !SAFE_REF.test(attachment.target_ref) || evidenceRefs.length === 0) {
      throw new PortableSessionProductionDriverError("identity_mismatch", "portable current attachment is invalid")
    }
    return {
      sessionRef,
      targetRef: attachment.target_ref,
      attachmentRef,
      generation,
      state,
      ...(typeof attachment.checkpoint_ref === "string" ? { checkpointRef: attachment.checkpoint_ref } : {}),
      authorityEvidenceRef: evidenceRefs[0]!,
    }
  },
})

type RuntimeMove = Pick<PostgresPortableSessionMoveRuntime, "move">

export type PortableProductionBroker = Readonly<{
  config: PortableSessionMoveRuntimeBrokerConfig
  prepare: (leg: PortableRoundTripMoveLeg) => void
}>

export type PortableRoundTripMoveLeg = Readonly<{
  moveRef: string
  command: PortableSessionMoveInput["command"]
  destinationAttachmentRef: string
  destinationRunnerSessionRef: string
  capabilityTransfers: PortableSessionMoveInput["capabilityTransfers"]
}>

export type PortableRoundTripReceipt = Readonly<{
  schema: "openagents.portable_session_round_trip_candidate.v1"
  proofClass: "deterministic" | "live_candidate"
  sessionRef: string
  runRef: string
  repositoryRef: string
  pinnedBaseRef: string
  localSourceAttachmentRef: string
  managedAttachmentRef: string
  localDestinationAttachmentRef: string
  finalGeneration: number
  localToManagedStatus: PortableSessionMoveResult["status"]
  managedToLocalStatus: PortableSessionMoveResult["status"]
  acceptedWorkRefs: ReadonlyArray<Readonly<{ agentRef: string; turnRef: string }>>
  evidenceRefs: ReadonlyArray<string>
  liveAcceptanceClaimed: false
}>

export class PortableSessionProductionDriverError extends Error {
  readonly _tag = "PortableSessionProductionDriverError"
  override readonly name = "PortableSessionProductionDriverError"

  constructor(
    readonly reason:
      | "continuation_mismatch"
      | "direction_mismatch"
      | "graph_mismatch"
      | "identity_mismatch"
      | "non_terminal_leg"
      | "unsafe_receipt",
    message: string,
  ) {
    super(message)
  }
}

const refs = (values: ReadonlyArray<string>, field: string): ReadonlyArray<string> => {
  if (values.some(value => !SAFE_REF.test(value)) || new Set(values).size !== values.length) {
    throw new PortableSessionProductionDriverError("unsafe_receipt", `${field} is not unique refs-only data`)
  }
  return values
}

const uniqueRefs = (values: ReadonlyArray<string>, field: string): ReadonlyArray<string> => {
  if (values.some(value => !SAFE_REF.test(value))) {
    throw new PortableSessionProductionDriverError("unsafe_receipt", `${field} contains unsafe data`)
  }
  return [...new Set(values)]
}

const sameRefs = (left: ReadonlyArray<string>, right: ReadonlyArray<string>): boolean => {
  const a = [...left].sort()
  const b = [...right].sort()
  return a.length === b.length && a.every((value, index) => value === b[index])
}

const completed = (result: PortableSessionMoveResult): boolean =>
  result.status === "completed" || result.status === "replayed"

const runtimeInput = (
  leg: PortableRoundTripMoveLeg,
  source: PortableSessionExecutionTarget,
  destination: PortableSessionExecutionTarget,
  broker: PortableSessionMoveRuntimeBrokerConfig,
): PortableSessionMoveRuntimeInput => ({
  moveRef: leg.moveRef,
  broker,
  move: {
    command: leg.command,
    destinationAttachmentRef: leg.destinationAttachmentRef,
    capabilityTransfers: leg.capabilityTransfers,
    source,
    destination,
  },
})

export class PortableSessionProductionDriver {
  constructor(private readonly config: Readonly<{
    runtime: RuntimeMove
    broker: PortableSessionMoveRuntimeBrokerConfig
    prepareBroker?: ((leg: PortableRoundTripMoveLeg) => void) | undefined
    local: PortableSessionExecutionTarget
    managed: PortableSessionExecutionTarget
    continuation: PortableManagedContinuation
    continuationAuthority: PortableManagedContinuationAuthority
  }>) {}

  async runRoundTrip(input: Readonly<{
    proofClass: PortableRoundTripReceipt["proofClass"]
    executionBinding: PortableSessionExecutionBinding
    expectedGraph: PortableAgentGraph
    managedContinuation: PortableManagedContinuationPlan
    localToManaged: PortableRoundTripMoveLeg
    managedToLocal: PortableRoundTripMoveLeg
  }>): Promise<PortableRoundTripReceipt> {
    refs([
      input.localToManaged.moveRef,
      input.localToManaged.destinationAttachmentRef,
      input.localToManaged.destinationRunnerSessionRef,
      input.managedToLocal.moveRef,
      input.managedToLocal.destinationAttachmentRef,
      input.managedToLocal.destinationRunnerSessionRef,
    ], "round-trip leg refs")
    const expectedAgents = refs(input.expectedGraph.nodes.map(node => node.agentRef), "expected agents")
    const expectedAgentSet = new Set(expectedAgents)
    if (expectedAgents.length < 2 ||
        !expectedAgentSet.has(input.expectedGraph.rootAgentRef) ||
        !input.expectedGraph.nodes.some(node => node.parentAgentRef !== undefined) ||
        input.expectedGraph.nodes.some(node => !["running", "waiting"].includes(node.lifecycle)) ||
        input.expectedGraph.nodes.some(node =>
          node.parentAgentRef !== undefined && !expectedAgentSet.has(node.parentAgentRef))) {
      throw new PortableSessionProductionDriverError("graph_mismatch", "round trip requires one canonical child-bearing agent graph")
    }
    if (
        input.localToManaged.command.kind !== "move" ||
        input.localToManaged.command.destinationTargetRef !== this.config.managed.targetRef ||
        input.managedToLocal.command.kind !== "failback" ||
        input.managedToLocal.command.destinationTargetRef !== this.config.local.targetRef ||
        input.localToManaged.command.sessionRef !== input.managedToLocal.command.sessionRef ||
        input.localToManaged.command.sessionRef !== input.executionBinding.sessionRef ||
        input.localToManaged.command.ownerRef !== input.executionBinding.ownerRef ||
        input.managedToLocal.command.ownerRef !== input.executionBinding.ownerRef) {
      throw new PortableSessionProductionDriverError("direction_mismatch", "round trip legs do not bind local → managed → local")
    }
    this.config.prepareBroker?.(input.localToManaged)
    const moved = await this.config.runtime.move(runtimeInput(
      input.localToManaged,
      this.config.local,
      this.config.managed,
      this.config.broker,
    ))
    if (!completed(moved) || moved.destinationAttachmentRef === undefined || moved.destinationGeneration === undefined ||
        moved.destinationAttachmentRef !== input.localToManaged.destinationAttachmentRef) {
      throw new PortableSessionProductionDriverError("non_terminal_leg", "local → managed move did not complete")
    }
    if (input.managedToLocal.command.expectedAttachmentRef !== moved.destinationAttachmentRef ||
        input.managedToLocal.command.expectedGeneration !== moved.destinationGeneration) {
      throw new PortableSessionProductionDriverError("direction_mismatch", "failback does not start from the completed managed generation")
    }
    const expectedThreadCursors = await this.config.continuationAuthority.readExpectedCursors({
      ownerRef: input.executionBinding.ownerRef,
      sessionRef: moved.sessionRef,
      attachmentRef: moved.destinationAttachmentRef,
      generation: moved.destinationGeneration,
      expectedGraph: input.expectedGraph,
    })
    const continuation = await this.config.continuation.run({
      sessionRef: moved.sessionRef,
      attachmentRef: moved.destinationAttachmentRef,
      generation: moved.destinationGeneration,
      expectedGraph: input.expectedGraph,
      expectedThreadCursors,
      plan: input.managedContinuation,
    })
    const acceptedAgents = continuation.acceptedWorkRefs.map(row => row.agentRef)
    const acceptedPairs = continuation.acceptedWorkRefs.map(row => `${row.agentRef}:${row.turnRef}`)
    refs(continuation.evidenceRefs, "continuation evidence")
    if (!sameRefs(acceptedAgents, expectedAgents) ||
        continuation.acceptedWorkRefs.length !== expectedAgents.length ||
        new Set(acceptedPairs).size !== acceptedPairs.length ||
        continuation.acceptedWorkRefs.some(row => !SAFE_REF.test(row.agentRef) || !SAFE_REF.test(row.turnRef))) {
      throw new PortableSessionProductionDriverError("continuation_mismatch", "managed continuation did not accept exactly one turn per canonical agent")
    }
    await this.config.continuationAuthority.commit({
      ownerRef: input.executionBinding.ownerRef,
      sessionRef: moved.sessionRef,
      attachmentRef: moved.destinationAttachmentRef,
      generation: moved.destinationGeneration,
      expectedGraph: input.expectedGraph,
      expectedThreadCursors,
      plan: input.managedContinuation,
      receipt: continuation,
    })
    this.config.prepareBroker?.(input.managedToLocal)
    const failedBack = await this.config.runtime.move(runtimeInput(
      input.managedToLocal,
      this.config.managed,
      this.config.local,
      this.config.broker,
    ))
    if (!completed(failedBack) || failedBack.destinationAttachmentRef === undefined ||
        failedBack.destinationGeneration !== moved.destinationGeneration + 1 ||
        failedBack.destinationAttachmentRef !== input.managedToLocal.destinationAttachmentRef) {
      throw new PortableSessionProductionDriverError("non_terminal_leg", "managed → local failback did not complete")
    }
    const identity = [moved, failedBack].every(result =>
      result.sessionRef === input.executionBinding.sessionRef &&
      result.runRef === input.executionBinding.runRef &&
      result.repositoryRef === input.executionBinding.repositoryRef &&
      result.pinnedBaseRef === input.executionBinding.pinnedBaseRef)
    if (!identity) {
      throw new PortableSessionProductionDriverError("identity_mismatch", "canonical execution identity changed across the round trip")
    }
    const evidenceRefs = uniqueRefs([
      ...moved.evidenceRefs,
      ...continuation.evidenceRefs,
      ...failedBack.evidenceRefs,
    ], "round-trip evidence")
    return {
      schema: "openagents.portable_session_round_trip_candidate.v1",
      proofClass: input.proofClass,
      sessionRef: input.executionBinding.sessionRef,
      runRef: input.executionBinding.runRef,
      repositoryRef: input.executionBinding.repositoryRef,
      pinnedBaseRef: input.executionBinding.pinnedBaseRef,
      localSourceAttachmentRef: input.localToManaged.command.expectedAttachmentRef,
      managedAttachmentRef: moved.destinationAttachmentRef,
      localDestinationAttachmentRef: failedBack.destinationAttachmentRef,
      finalGeneration: failedBack.destinationGeneration,
      localToManagedStatus: moved.status,
      managedToLocalStatus: failedBack.status,
      acceptedWorkRefs: continuation.acceptedWorkRefs,
      evidenceRefs,
      liveAcceptanceClaimed: false,
    }
  }
}

class MovingPortableCapabilityVault implements CapabilitySecretVault {
  private transfers = new Map<string, Readonly<{
    destinationGrantRef: string
    runnerSessionId: string
  }>>()

  constructor(private readonly authority: HttpPortableCapabilityGrantVault) {}

  prepare(leg: PortableRoundTripMoveLeg): void {
    if (!SAFE_REF.test(leg.destinationRunnerSessionRef)) {
      throw new PortableSessionProductionDriverError("unsafe_receipt", "destination runner session ref is unsafe")
    }
    const next = new Map<string, Readonly<{
      destinationGrantRef: string
      runnerSessionId: string
    }>>()
    for (const transfer of leg.capabilityTransfers) {
      if (next.has(transfer.sourceLeaseRef)) {
        throw new PortableSessionProductionDriverError("direction_mismatch", "capability source lease is duplicated")
      }
      next.set(transfer.sourceLeaseRef, {
        destinationGrantRef: transfer.destinationSourceGrantRef,
        runnerSessionId: leg.destinationRunnerSessionRef,
      })
    }
    this.transfers = next
  }

  withSourceGrantMaterial: CapabilitySecretVault["withSourceGrantMaterial"] = input =>
    this.authority.withSourceGrantMaterial(input)

  revokeSourceGrant: CapabilitySecretVault["revokeSourceGrant"] = async input => {
    await this.authority.revokeSourceGrant(input)
    const transfer = this.transfers.get(input.leaseRef)
    if (transfer === undefined) return
    await this.authority.reissue({
      sourceGrantRef: input.sourceGrantRef,
      destinationGrantRef: transfer.destinationGrantRef,
      runnerSessionId: transfer.runnerSessionId,
      requestedAction: "portable_session_resume",
    })
  }
}

export const createPortableSessionProductionBroker = (input: Readonly<{
  grantAuthority: HttpPortableCapabilityGrantVaultConfig
  sql: PortableSessionMoveRuntimeConfig["sql"]
  ownerRef: string
  sessionRef: string
  clock?: CapabilityBrokerClock | undefined
  maxTtlMs?: number | undefined
  local: Readonly<{
    targetRef: string
    adapterRef: string
    installation: Omit<OwnerLocalPortableCapabilityInstallationConfig, "ownerRef" | "targetRef">
  }>
  managed: Readonly<{
    targetRef: string
    adapterRef: string
    installation: Omit<
      ManagedPortableCapabilityInstallationConfig,
      "ownerRef" | "targetRef" | "sessionRef" | "resolveResource"
    >
  }>
}>): PortableProductionBroker => {
  const vault = new MovingPortableCapabilityVault(
    new HttpPortableCapabilityGrantVault(input.grantAuthority),
  )
  const localAdapter = makePortableCapabilityTargetAdapter({
    adapterRef: input.local.adapterRef,
    targetClass: "owner_local",
    port: new OwnerLocalPortableCapabilityInstallationPort({
      ...input.local.installation,
      ownerRef: input.ownerRef,
      targetRef: input.local.targetRef,
    }),
  })
  const managedAdapter = makePortableCapabilityTargetAdapter({
    adapterRef: input.managed.adapterRef,
    targetClass: "openagents_managed",
    port: createPostgresManagedPortableCapabilityInstallationPort({
      ...input.managed.installation,
      sql: input.sql,
      ownerRef: input.ownerRef,
      targetRef: input.managed.targetRef,
      sessionRef: input.sessionRef,
    }),
  })
  return {
    config: {
      vault,
      targets: [
        { targetRef: input.local.targetRef, targetClass: "owner_local", adapterRef: localAdapter.adapterRef, ready: true },
        { targetRef: input.managed.targetRef, targetClass: "openagents_managed", adapterRef: managedAdapter.adapterRef, ready: true },
      ],
      adapters: [localAdapter, managedAdapter],
      ...(input.clock === undefined ? {} : { clock: input.clock }),
      ...(input.maxTtlMs === undefined ? {} : { maxTtlMs: input.maxTtlMs }),
    },
    prepare: leg => vault.prepare(leg),
  }
}

export const createPortableSessionProductionDriver = async (input: Readonly<{
  runtime: PortableSessionMoveRuntimeConfig
  capabilities: Readonly<{
    grantAuthority: HttpPortableCapabilityGrantVaultConfig
    clock?: CapabilityBrokerClock | undefined
    maxTtlMs?: number | undefined
    local: Readonly<{
      adapterRef: string
      installation: Omit<OwnerLocalPortableCapabilityInstallationConfig, "ownerRef" | "targetRef">
    }>
    managed: Readonly<{
      adapterRef: string
      installation: Omit<
        ManagedPortableCapabilityInstallationConfig,
        "ownerRef" | "targetRef" | "sessionRef" | "resolveResource"
      >
    }>
  }>
  local: Readonly<{
    targetRef: string
    ledger: PylonPortableSessionOperationLedger
    lifecycle: PylonPortableControlSessionLifecycle
    binding: Readonly<{
      sessionRef: string
      attachmentRef: string
      generation: number
      agents: ReadonlyArray<Readonly<{ agentRef: string; controlSessionRef: string }>>
    }>
  }>
  managed: Readonly<{
    ownerRef: string
    targetRef: string
    provisioner: Omit<OaCodexControlPortableProvisionerConfig, "checkpointArtifacts">
  }>
  checkpointCustody: Readonly<{
    policy: "owner_managed"
    keyRef: string
    keyProvider: PylonPortableCheckpointCustodyKeyProvider
    maxArtifactBytes?: number
  }> | Readonly<{
    policy: "openagents_managed"
    keyRef: string
    kmsAuthority: PylonPortableCheckpointKmsEnvelopeAuthority
    maxArtifactBytes?: number
  }>
}>): Promise<PortableSessionProductionDriver> => {
  const pylonHome = input.capabilities.local.installation.pylonHome
  const checkpointArtifacts = new PylonPortableCheckpointArtifactStore({
    custodyDirectory: join(pylonHome, "runtime", "portable-checkpoints", "artifacts"),
    ...input.checkpointCustody,
  })
  const broker = createPortableSessionProductionBroker({
    grantAuthority: input.capabilities.grantAuthority,
    sql: input.runtime.sql,
    ownerRef: input.managed.ownerRef,
    sessionRef: input.local.binding.sessionRef,
    ...(input.capabilities.clock === undefined ? {} : { clock: input.capabilities.clock }),
    ...(input.capabilities.maxTtlMs === undefined ? {} : { maxTtlMs: input.capabilities.maxTtlMs }),
    local: {
      targetRef: input.local.targetRef,
      adapterRef: input.capabilities.local.adapterRef,
      installation: input.capabilities.local.installation,
    },
    managed: {
      targetRef: input.managed.targetRef,
      adapterRef: input.capabilities.managed.adapterRef,
      installation: input.capabilities.managed.installation,
    },
  })
  const destination = createPylonOwnerLocalDestinationLifecycle({
    targetRef: input.local.targetRef,
    ledger: input.local.ledger,
    authority: createPostgresLocalDestinationAuthority({
      sql: input.runtime.sql,
      ownerRef: input.managed.ownerRef,
    }),
    rehydrator: createPylonPortableLocalRehydrator({
      targetRef: input.local.targetRef,
      custodyRoot: join(pylonHome, "runtime", "portable-checkpoints", "rehydrated"),
      artifacts: checkpointArtifacts,
      lifecycle: input.local.lifecycle,
    }),
  })
  const local = await createPylonOwnerLocalExecutionTarget({
    targetRef: input.local.targetRef,
    ledger: input.local.ledger,
    lifecycle: input.local.lifecycle,
    binding: input.local.binding,
    destination,
    checkpointArtifacts,
  })
  const managed = new PostgresManagedAgentComputerTarget({
    sql: input.runtime.sql,
    ownerRef: input.managed.ownerRef,
    targetRef: input.managed.targetRef,
    provisioner: createOaCodexControlPortableProvisioner({
      ...input.managed.provisioner,
      checkpointArtifacts,
    }),
  })
  const continuation = createOaCodexControlPortableManagedContinuation({
    baseUrl: input.managed.provisioner.baseUrl,
    bearerToken: input.managed.provisioner.bearerToken,
    ownerRef: input.managed.ownerRef,
    targetRef: input.managed.targetRef,
    ...(input.managed.provisioner.fetch === undefined ? {} : { fetch: input.managed.provisioner.fetch }),
    ...(input.managed.provisioner.timeoutMs === undefined ? {} : { timeoutMs: input.managed.provisioner.timeoutMs }),
  })
  const continuationAuthority = new PostgresPortableManagedContinuationAuthority({
    sql: input.runtime.sql,
    transaction: input.runtime.transaction,
  })
  return new PortableSessionProductionDriver({
    runtime: new PostgresPortableSessionMoveRuntime(input.runtime),
    broker: broker.config,
    prepareBroker: broker.prepare,
    local,
    managed,
    continuation,
    continuationAuthority,
  })
}

export type PortableSessionLiveCandidateInput = Omit<
  Parameters<PortableSessionProductionDriver["runRoundTrip"]>[0],
  "proofClass"
>

/**
 * Runnable live-candidate entrypoint.
 *
 * The caller constructs `production` from environment/secret-manager values;
 * this function never reads secrets from argv and never serializes its
 * production configuration. A successful return remains only a candidate
 * receipt until the root-coordinated live observer accepts the direct journey.
 */
export const runPortableSessionLiveCandidate = async (input: Readonly<{
  production: Parameters<typeof createPortableSessionProductionDriver>[0]
  journey: PortableSessionLiveCandidateInput
}>): Promise<PortableRoundTripReceipt> => {
  const driver = await createPortableSessionProductionDriver(input.production)
  return driver.runRoundTrip({
    ...input.journey,
    proofClass: "live_candidate",
  })
}
