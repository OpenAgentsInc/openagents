import {
  ManagedSandboxCommandSchema,
  type ManagedSandboxCommand,
  type ManagedSandboxResource,
} from "@openagentsinc/managed-sandbox-contract"
import { Context, Effect, Layer, Ref, Schema } from "effect"

import type { IdeAgentAttachment, IdeAgentCodeSnapshot } from "./agent-code-contract.ts"
import {
  IdeManagedSandboxAdmissionSchema,
  IdeManagedSandboxCommandSchema,
  IdeManagedSandboxGatewayResultSchema,
  IdeManagedSandboxSnapshotSchema,
  emptyIdeManagedSandboxSnapshot,
  type IdeManagedSandboxAdmission,
  type IdeManagedSandboxBinding,
  type IdeManagedSandboxCommand,
  type IdeManagedSandboxGatewayResult,
  type IdeManagedSandboxReceiptProjection,
  type IdeManagedSandboxResourceProjection,
  type IdeManagedSandboxSnapshot,
  type IdeManagedSandboxTurnProjection,
} from "./managed-sandbox-contract.ts"
import {
  IdeCapabilityRefSchema,
  IdePlacementRefSchema,
  IdeServiceGenerationSchema,
  type IdeCapabilitySnapshot,
} from "./project-contract.ts"

export type IdeManagedSandboxPrincipal = Readonly<{
  ownerRef: string
  tenantRef: string
  requestedByRef: string
}>

export type IdeManagedSandboxGateway = Readonly<{
  admission: (
    input: Readonly<{
      principal: IdeManagedSandboxPrincipal
      attachment: IdeAgentAttachment
    }>,
  ) => Effect.Effect<IdeManagedSandboxAdmission, Error>
  execute: (command: ManagedSandboxCommand) => Effect.Effect<IdeManagedSandboxGatewayResult, Error>
}>

export class IdeManagedSandboxRefused extends Schema.TaggedErrorClass<IdeManagedSandboxRefused>()(
  "IdeManagedSandbox.Refused",
  {
    reason: Schema.Literals([
      "invalid_input",
      "signed_out",
      "unattached",
      "stale_attachment",
      "not_configured",
      "wrong_sandbox",
      "stale_resource",
      "capability_denied",
      "gateway_unavailable",
      "invalid_response",
      "invariant_violation",
    ]),
    message: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(800)),
  },
) {}

export type IdeManagedSandboxServiceError = IdeManagedSandboxRefused

export type IdeManagedSandboxServiceShape = Readonly<{
  snapshot: () => Effect.Effect<IdeManagedSandboxSnapshot>
  command: (
    command: IdeManagedSandboxCommand,
  ) => Effect.Effect<IdeManagedSandboxSnapshot, IdeManagedSandboxServiceError>
}>

export class IdeManagedSandboxService extends Context.Service<
  IdeManagedSandboxService,
  IdeManagedSandboxServiceShape
>()("@openagentsinc/openagents-desktop/IdeManagedSandboxService") {}

const refusal = (reason: IdeManagedSandboxRefused["reason"], message: string): IdeManagedSandboxRefused =>
  new IdeManagedSandboxRefused({ reason, message })

const decode = <S extends Schema.ConstraintDecoder<unknown, never>>(
  schema: S,
  value: unknown,
  reason: IdeManagedSandboxRefused["reason"],
  message: string,
): Effect.Effect<S["Type"], IdeManagedSandboxRefused> =>
  Schema.decodeUnknownEffect(schema)(value).pipe(Effect.mapError(() => refusal(reason, message)))

const sameAttachment = (left: IdeAgentAttachment, right: IdeAgentAttachment): boolean =>
  left.projectRef === right.projectRef &&
  left.rootRef === right.rootRef &&
  left.worktreeRef === right.worktreeRef &&
  left.sessionRef === right.sessionRef &&
  left.agentAttachmentRef === right.agentAttachmentRef &&
  left.attachmentGeneration === right.attachmentGeneration &&
  left.placementGeneration === right.placementGeneration &&
  left.grantRef === right.grantRef

const safeSuffix = (value: string): string => value.replaceAll(/[^A-Za-z0-9._-]/gu, "-").slice(0, 120)

const placementRefFor = (sandboxRef: string) =>
  IdePlacementRefSchema.make(`ide.placement.openagents-managed.${safeSuffix(sandboxRef)}`)

const capabilityFor = (binding: IdeManagedSandboxBinding, resource: ManagedSandboxResource): IdeCapabilitySnapshot => {
  const capabilityRef = IdeCapabilityRefSchema.make(`ide.capability.managed-sandbox.${safeSuffix(resource.sandboxRef)}`)
  const serviceGeneration = IdeServiceGenerationSchema.make(Math.max(1, resource.version + 1))
  const base = {
    capabilityRef,
    kind: "agent" as const,
    attachmentGeneration: binding.attachmentGeneration,
    placementGeneration: binding.placementGeneration,
  }
  switch (resource.facts.lifecycle) {
    case "provisioning":
    case "resuming":
      return {
        ...base,
        state: { _tag: "Starting", since: resource.updatedAt, serviceGeneration },
      }
    case "ready":
    case "idle":
    case "running":
      return {
        ...base,
        state: {
          _tag: "Ready",
          serviceGeneration,
          placementRef: binding.placementRef,
          evidenceTier: "managed",
          observedAt: resource.updatedAt,
        },
      }
    case "stopping":
    case "deleting":
      return {
        ...base,
        state: {
          _tag: "Degraded",
          serviceGeneration,
          placementRef: binding.placementRef,
          evidenceTier: "managed",
          reason: `${resource.facts.lifecycle} is in progress`,
          observedAt: resource.updatedAt,
        },
      }
    case "stopped":
    case "deleted":
      return {
        ...base,
        state: {
          _tag: "Stopped",
          reason: resource.facts.lifecycle,
          stoppedAt: resource.updatedAt,
        },
      }
    case "failed":
    case "recovery_required":
      return {
        ...base,
        state: {
          _tag: "Failed",
          serviceGeneration,
          reason: resource.facts.lifecycle,
          retry: "manual",
          observedAt: resource.updatedAt,
        },
      }
  }
}

const projectResource = (resource: ManagedSandboxResource): IdeManagedSandboxResourceProjection => ({
  sandboxRef: resource.sandboxRef,
  workUnitRef: resource.workUnitRef,
  attachmentRef: resource.attachmentRef,
  attachmentGeneration: resource.attachmentGeneration,
  resourceGeneration: resource.resourceGeneration,
  version: resource.version,
  lastEventSequence: resource.lastEventSequence,
  target: resource.target,
  imageDigest: resource.imageDigest,
  profileRef: resource.profileRef,
  lease: resource.lease,
  budget: resource.budget,
  capabilities: resource.capabilities,
  facts: resource.facts,
  createdAt: resource.createdAt,
  updatedAt: resource.updatedAt,
})

const projectReceipt = (result: IdeManagedSandboxGatewayResult): IdeManagedSandboxReceiptProjection => ({
  receiptRef: result.receipt.receiptRef,
  commandRef: result.receipt.commandRef,
  sandboxRef: result.receipt.sandboxRef,
  resourceGeneration: result.receipt.resourceGeneration,
  version: result.receipt.version,
  outcome: result.receipt.outcome,
  lifecycle: result.receipt.lifecycle,
  eventRefs: result.receipt.eventRefs,
  artifactRefs: result.receipt.artifactRefs,
  errorCode: result.receipt.errorCode ?? null,
  observedAt: result.receipt.observedAt,
})

const projectTurn = (result: IdeManagedSandboxGatewayResult): IdeManagedSandboxTurnProjection | null =>
  result.turn === null
    ? null
    : {
        turnRef: result.turn.turnRef,
        commandRef: result.turn.commandRef,
        capabilityRef: result.turn.capabilityRef,
        turnSequence: result.turn.turnSequence,
        lastEventSequence: result.turn.lastEventSequence,
        runtime: result.turn.runtime,
        status: result.turn.status,
        usage: result.turn.usage ?? null,
        createdAt: result.turn.createdAt,
        startedAt: result.turn.startedAt ?? null,
        settledAt: result.turn.settledAt ?? null,
      }

const exactAdmissionResource = (
  admission: Extract<IdeManagedSandboxAdmission, { readonly _tag: "Available" }>,
  resource: ManagedSandboxResource,
): boolean =>
  resource.target.targetRef === admission.target.targetRef &&
  resource.target.targetClass === "openagents_managed" &&
  resource.target.provider === "google_cloud" &&
  resource.target.adapterRef === admission.target.adapterRef &&
  resource.target.region === admission.target.region &&
  resource.target.isolation === admission.target.isolation &&
  resource.imageDigest === admission.imageDigest &&
  resource.profileRef === admission.profileRef

const validateResult = (
  result: IdeManagedSandboxGatewayResult,
  command: ManagedSandboxCommand,
  principal: IdeManagedSandboxPrincipal,
  attachment: IdeAgentAttachment,
  admission: IdeManagedSandboxAdmission,
  expectedWorkUnitRef: string,
): Effect.Effect<IdeManagedSandboxGatewayResult, IdeManagedSandboxRefused> => {
  const resource = result.resource
  if (result.command.commandRef !== command.commandRef || result.command._tag !== command._tag) {
    return Effect.fail(refusal("invalid_response", "The gateway changed the command identity."))
  }
  if (resource.ownerRef !== principal.ownerRef || resource.tenantRef !== principal.tenantRef) {
    return Effect.fail(refusal("invalid_response", "The gateway returned another owner or tenant scope."))
  }
  if (
    resource.workUnitRef !== expectedWorkUnitRef ||
    resource.attachmentRef !== attachment.agentAttachmentRef ||
    resource.attachmentGeneration !== attachment.attachmentGeneration
  ) {
    return Effect.fail(refusal("stale_attachment", "The gateway changed the project attachment scope."))
  }
  if (admission._tag !== "Available" || !exactAdmissionResource(admission, resource)) {
    return Effect.fail(refusal("invalid_response", "The gateway substituted the admitted managed target."))
  }
  if (
    result.receipt.commandRef !== command.commandRef ||
    result.receipt.sandboxRef !== resource.sandboxRef ||
    result.receipt.ownerRef !== principal.ownerRef ||
    result.receipt.tenantRef !== principal.tenantRef ||
    result.receipt.resourceGeneration !== resource.resourceGeneration ||
    result.receipt.version !== resource.version ||
    result.receipt.lifecycle !== resource.facts.lifecycle
  ) {
    return Effect.fail(refusal("invalid_response", "The gateway receipt does not bind the exact resource."))
  }
  if (
    result.turn !== null &&
    (result.turn.sandboxRef !== resource.sandboxRef ||
      result.turn.ownerRef !== principal.ownerRef ||
      result.turn.tenantRef !== principal.tenantRef ||
      result.turn.workUnitRef !== resource.workUnitRef ||
      result.turn.attachmentRef !== resource.attachmentRef ||
      result.turn.attachmentGeneration !== resource.attachmentGeneration ||
      result.turn.resourceGeneration !== resource.resourceGeneration)
  ) {
    return Effect.fail(refusal("invalid_response", "The gateway turn does not bind the exact IDE attachment."))
  }
  let priorEventSequence = 0
  for (const event of result.events) {
    if (
      event.sandboxRef !== resource.sandboxRef ||
      event.resourceGeneration !== resource.resourceGeneration ||
      event.sequence <= priorEventSequence
    ) {
      return Effect.fail(refusal("invalid_response", "The gateway mixed or reordered sandbox events."))
    }
    priorEventSequence = event.sequence
  }
  return Effect.succeed(result)
}

const bindingFrom = (attachment: IdeAgentAttachment, resource: ManagedSandboxResource): IdeManagedSandboxBinding => ({
  projectRef: attachment.projectRef,
  rootRef: attachment.rootRef,
  worktreeRef: attachment.worktreeRef,
  sessionRef: attachment.sessionRef,
  agentAttachmentRef: attachment.agentAttachmentRef,
  attachmentGeneration: attachment.attachmentGeneration,
  placementGeneration: attachment.placementGeneration,
  placementRef: placementRefFor(resource.sandboxRef),
  workUnitRef: resource.workUnitRef,
  sandboxRef: resource.sandboxRef,
})

const nextSnapshot = (
  current: IdeManagedSandboxSnapshot,
  attachment: IdeAgentAttachment,
  result: IdeManagedSandboxGatewayResult,
): IdeManagedSandboxSnapshot => {
  const binding = current.binding ?? bindingFrom(attachment, result.resource)
  const receipt = projectReceipt(result)
  const receipts = [
    ...current.receipts.filter((candidate) => candidate.receiptRef !== receipt.receiptRef),
    receipt,
  ].slice(-64)
  return IdeManagedSandboxSnapshotSchema.make({
    ...current,
    revision: current.revision + 1,
    binding,
    resource: projectResource(result.resource),
    projectCapability: capabilityFor(binding, result.resource),
    turn: projectTurn(result) ?? current.turn,
    events: result.events.slice(-256),
    receipts,
    freshness: "live",
    latencyClass: result.resource.facts.runtimeState === "running" ? "remote_interactive" : "remote_background",
    lastError: null,
  })
}

const currentAttachment = (
  currentAgentSnapshot: () => Effect.Effect<IdeAgentCodeSnapshot, Error>,
  expected?: IdeAgentAttachment,
): Effect.Effect<IdeAgentAttachment, IdeManagedSandboxRefused> =>
  currentAgentSnapshot().pipe(
    Effect.mapError(() => refusal("unattached", "The canonical agent graph is unavailable.")),
    Effect.flatMap((snapshot) =>
      snapshot.attachment === null
        ? Effect.fail(refusal("unattached", "Attach the project to the canonical agent graph first."))
        : expected !== undefined && !sameAttachment(snapshot.attachment, expected)
          ? Effect.fail(refusal("stale_attachment", "The project or agent attachment generation changed."))
          : Effect.succeed(snapshot.attachment),
    ),
  )

const canonicalCommand = (
  command: Exclude<IdeManagedSandboxCommand, { readonly _tag: "RefreshAdmission" }>,
  principal: IdeManagedSandboxPrincipal,
  admission: Extract<IdeManagedSandboxAdmission, { readonly _tag: "Available" }>,
  resource: IdeManagedSandboxResourceProjection | null,
): Effect.Effect<ManagedSandboxCommand, IdeManagedSandboxRefused> => {
  const base = {
    schema: "openagents.managed_sandbox_command.v1" as const,
    commandRef: command.requestRef,
    requestedByRef: principal.requestedByRef,
    ownerRef: principal.ownerRef,
    tenantRef: principal.tenantRef,
    idempotencyRef: command.idempotencyRef,
    requestedAt: command.requestedAt,
  }
  switch (command._tag) {
    case "Create":
      return decode(
        ManagedSandboxCommandSchema,
        {
          ...base,
          _tag: "Create",
          workUnitRef: command.workUnitRef,
          attachmentRef: command.expectedAttachment.agentAttachmentRef,
          target: admission.target,
          imageDigest: admission.imageDigest,
          profileRef: admission.profileRef,
          lease: admission.lease,
          budget: admission.budget,
          requestedCapabilities: admission.requestedCapabilities,
        },
        "invalid_input",
        "The create command is invalid.",
      )
    case "Inspect":
      return decode(
        ManagedSandboxCommandSchema,
        {
          ...base,
          _tag: "Inspect",
          sandboxRef: command.sandboxRef,
        },
        "invalid_input",
        "The inspect command is invalid.",
      )
    case "Dispatch":
      if (resource === null) return Effect.fail(refusal("wrong_sandbox", "No managed sandbox is attached."))
      return decode(
        ManagedSandboxCommandSchema,
        {
          ...base,
          _tag: "Dispatch",
          sandboxRef: command.sandboxRef,
          expectedVersion: resource.version,
          turnRef: command.turnRef,
          capabilityRef: command.capabilityRef,
          promptDigest: command.promptDigest,
          runtime: command.runtime,
        },
        "invalid_input",
        "The dispatch command is invalid.",
      )
    case "Interrupt":
      if (resource === null) return Effect.fail(refusal("wrong_sandbox", "No managed sandbox is attached."))
      return decode(
        ManagedSandboxCommandSchema,
        {
          ...base,
          _tag: "Interrupt",
          sandboxRef: command.sandboxRef,
          expectedVersion: resource.version,
          turnRef: command.turnRef,
          reasonRef: command.reasonRef,
        },
        "invalid_input",
        "The interrupt command is invalid.",
      )
    case "Stop":
      if (resource === null) return Effect.fail(refusal("wrong_sandbox", "No managed sandbox is attached."))
      return decode(
        ManagedSandboxCommandSchema,
        {
          ...base,
          _tag: "Stop",
          sandboxRef: command.sandboxRef,
          expectedVersion: resource.version,
          reasonRef: command.reasonRef,
        },
        "invalid_input",
        "The stop command is invalid.",
      )
    case "Resume":
      if (resource === null) return Effect.fail(refusal("wrong_sandbox", "No managed sandbox is attached."))
      return decode(
        ManagedSandboxCommandSchema,
        {
          ...base,
          _tag: "Resume",
          sandboxRef: command.sandboxRef,
          expectedVersion: resource.version,
        },
        "invalid_input",
        "The resume command is invalid.",
      )
    case "Delete":
      if (resource === null) return Effect.fail(refusal("wrong_sandbox", "No managed sandbox is attached."))
      return decode(
        ManagedSandboxCommandSchema,
        {
          ...base,
          _tag: "Delete",
          sandboxRef: command.sandboxRef,
          expectedVersion: resource.version,
          reasonRef: command.reasonRef,
        },
        "invalid_input",
        "The delete command is invalid.",
      )
  }
}

export const makeIdeManagedSandboxLayer = (
  input: Readonly<{
    principal: IdeManagedSandboxPrincipal
    gateway: IdeManagedSandboxGateway
    currentAgentSnapshot: () => Effect.Effect<IdeAgentCodeSnapshot, Error>
    initialSnapshot?: IdeManagedSandboxSnapshot
  }>,
): Layer.Layer<IdeManagedSandboxService> =>
  Layer.effect(
    IdeManagedSandboxService,
    Effect.gen(function* () {
      const state = yield* Ref.make(input.initialSnapshot ?? emptyIdeManagedSandboxSnapshot())
      yield* Effect.addFinalizer(() =>
        Ref.update(state, (current) =>
          IdeManagedSandboxSnapshotSchema.make({
            ...current,
            freshness: current.resource === null ? "unavailable" : "stale",
            lastError: "The Desktop managed-sandbox service stopped.",
          }),
        ),
      )

      const snapshot = Effect.fn("IdeManagedSandboxService.snapshot")(function* () {
        return yield* Ref.get(state)
      })

      const command = Effect.fn("IdeManagedSandboxService.command")(function* (value: IdeManagedSandboxCommand) {
        const decoded = yield* decode(
          IdeManagedSandboxCommandSchema,
          value,
          "invalid_input",
          "The managed-sandbox command is invalid.",
        )
        if (decoded._tag === "RefreshAdmission") {
          const attachment = yield* currentAttachment(input.currentAgentSnapshot)
          const admission = yield* input.gateway
            .admission({
              principal: input.principal,
              attachment,
            })
            .pipe(
              Effect.mapError(() =>
                refusal("gateway_unavailable", "The managed-sandbox admission service is unavailable."),
              ),
              Effect.flatMap((result) =>
                decode(
                  IdeManagedSandboxAdmissionSchema,
                  result,
                  "invalid_response",
                  "The managed-sandbox admission response is invalid.",
                ),
              ),
            )
          yield* Ref.update(state, (current) =>
            IdeManagedSandboxSnapshotSchema.make({
              ...current,
              revision: current.revision + 1,
              admission,
              freshness: admission._tag === "Available" ? "live" : "unavailable",
              latencyClass: admission._tag === "Available" ? "remote_background" : "unavailable",
              lastError: admission._tag === "Available" ? null : admission.reason,
            }),
          )
          return yield* Ref.get(state)
        }

        const current = yield* Ref.get(state)
        if (current.admission._tag !== "Available") {
          return yield* Effect.fail(
            refusal("not_configured", "Refresh and admit the OpenAgents-managed target before mutation."),
          )
        }
        const attachment = yield* currentAttachment(input.currentAgentSnapshot, decoded.expectedAttachment)
        if (current.binding !== null) {
          if (
            decoded._tag === "Create" ||
            current.binding.sandboxRef !== decoded.sandboxRef ||
            current.binding.agentAttachmentRef !== attachment.agentAttachmentRef ||
            current.binding.attachmentGeneration !== attachment.attachmentGeneration ||
            current.binding.placementGeneration !== attachment.placementGeneration
          ) {
            return yield* Effect.fail(
              refusal(
                "wrong_sandbox",
                "The command does not target the exact attached sandbox and project generation.",
              ),
            )
          }
        }
        const native = yield* canonicalCommand(decoded, input.principal, current.admission, current.resource)
        const rawResult = yield* input.gateway
          .execute(native)
          .pipe(
            Effect.mapError(() =>
              refusal("gateway_unavailable", "The managed-sandbox command service is unavailable."),
            ),
          )
        const result = yield* decode(
          IdeManagedSandboxGatewayResultSchema,
          rawResult,
          "invalid_response",
          "The managed-sandbox command response is invalid.",
        )
        const expectedWorkUnitRef =
          decoded._tag === "Create"
            ? decoded.workUnitRef
            : (current.binding?.workUnitRef ?? result.resource.workUnitRef)
        const validated = yield* validateResult(
          result,
          native,
          input.principal,
          attachment,
          current.admission,
          expectedWorkUnitRef,
        )
        if (
          decoded._tag !== "Create" &&
          current.resource !== null &&
          (validated.resource.sandboxRef !== current.resource.sandboxRef ||
            validated.resource.version < current.resource.version)
        ) {
          return yield* Effect.fail(
            refusal("stale_resource", "The gateway returned a stale or substituted sandbox resource."),
          )
        }
        const next = nextSnapshot(current, attachment, validated)
        yield* Ref.set(state, next)
        return next
      })

      return IdeManagedSandboxService.of({ snapshot, command })
    }),
  )
