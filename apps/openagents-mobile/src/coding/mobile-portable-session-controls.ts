import type { ConfirmedPortableSessionSnapshot } from "@openagentsinc/khala-sync-client"
import {
  PortableSessionCommandSchema,
  type PortableAttachment,
  type PortableSessionCommand,
  type PortableSessionCommandKind,
  type PortableSessionCommandOutcome,
  type PortableTargetDescriptor,
} from "@openagentsinc/portable-session-contract"
import { Schema } from "effect"

export type MobilePortableControlAction = Extract<
  PortableSessionCommandKind,
  "stop" | "checkpoint" | "move" | "resume" | "failback"
>

export type MobilePortableUnavailableReason =
  | "authority_unavailable"
  | "projection_invalid"
  | "session_not_portable"
  | "target_directory_missing"
  | "attachment_authority_ambiguous"
  | "source_target_missing"
  | "command_in_flight"
  | "action_requires_active_attachment"
  | "action_requires_suspended_attachment"
  | "destination_required"
  | "destination_not_ready"
  | "destination_is_source"
  | "failback_target_missing"
  | "invalid_invocation"

export type MobilePortableActionAvailability = Readonly<{
  available: boolean
  reason: MobilePortableUnavailableReason | null
  destinations: ReadonlyArray<PortableTargetDescriptor>
}>

export type MobilePortableSessionControl =
  | Readonly<{
      state: "unavailable"
      sessionRef: string
      reason: MobilePortableUnavailableReason
    }>
  | Readonly<{
      state: "ready"
      sessionRef: string
      ownerRef: string
      sourceAttachment: PortableAttachment
      sourceTarget: PortableTargetDescriptor
      targets: ReadonlyArray<PortableTargetDescriptor>
      pendingCommand: PortableSessionCommand | null
      pendingLocalCommandCount: number
      latestOutcome: PortableSessionCommandOutcome | null
      actions: Readonly<Record<MobilePortableControlAction, MobilePortableActionAvailability>>
    }>

const unavailable = (
  sessionRef: string,
  reason: MobilePortableUnavailableReason,
): MobilePortableSessionControl => ({ state: "unavailable", sessionRef, reason })

const action = (
  available: boolean,
  reason: MobilePortableUnavailableReason | null,
  destinations: ReadonlyArray<PortableTargetDescriptor> = [],
): MobilePortableActionAvailability => ({ available, reason, destinations })

const newestActionableAttachment = (
  attachments: ReadonlyArray<PortableAttachment>,
): PortableAttachment | null => {
  const active = attachments.filter(value => value.state === "active")
  if (active.length === 1) return active[0]!
  if (active.length > 1) return null
  const suspended = attachments
    .filter(value => value.state === "quiesced" || value.state === "detached" || value.state === "failed")
    .sort((left, right) => right.generation - left.generation)
  if (suspended.length === 0 || suspended[0]?.generation === suspended[1]?.generation) return null
  return suspended[0]!
}

/**
 * Join one coding-session ref to confirmed portable authority. No row from a
 * withheld scope or malformed snapshot can enable a control.
 */
export const projectMobilePortableSessionControl = (
  snapshot: ConfirmedPortableSessionSnapshot,
  sessionRef: string,
): MobilePortableSessionControl => {
  if (snapshot.status.phase !== "live") return unavailable(sessionRef, "authority_unavailable")
  if (snapshot.issues.length > 0) return unavailable(sessionRef, "projection_invalid")
  const session = snapshot.sessions.find(value => value.sessionRef === sessionRef)
  if (session === undefined) return unavailable(sessionRef, "session_not_portable")
  const directory = snapshot.targetDirectories.find(value => value.sessionRef === sessionRef)
  if (directory === undefined) return unavailable(sessionRef, "target_directory_missing")
  const sourceAttachment = newestActionableAttachment(
    snapshot.attachments.filter(value => value.sessionRef === sessionRef),
  )
  if (sourceAttachment === null) return unavailable(sessionRef, "attachment_authority_ambiguous")
  const sourceTarget = directory.targets.find(value =>
    value.targetRef === sourceAttachment.targetRef && value.ownerRef === session.ownerRef)
  if (sourceTarget === undefined) return unavailable(sessionRef, "source_target_missing")

  const commands = snapshot.commands.filter(value => value.command.sessionRef === sessionRef)
  const pendingCommand = commands.find(value => "status" in value)?.command ?? null
  const latestOutcome = commands.find(value => "outcome" in value && value.outcome !== undefined)
  const outcome = latestOutcome !== undefined && "outcome" in latestOutcome
    ? latestOutcome.outcome
    : null
  const hasPending = pendingCommand !== null || snapshot.status.pendingCommandCount > 0
  const readyDestinations = directory.targets.filter(value =>
    value.ownerRef === session.ownerRef && value.health === "ready" &&
    value.targetRef !== sourceTarget.targetRef)
  const failbackDestinations = readyDestinations.filter(value => value.targetClass === "owner_local")
  const active = sourceAttachment.state === "active"
  const suspended = sourceAttachment.state === "quiesced" ||
    sourceAttachment.state === "detached" || sourceAttachment.state === "failed"

  return {
    state: "ready",
    sessionRef,
    ownerRef: session.ownerRef,
    sourceAttachment,
    sourceTarget,
    targets: directory.targets,
    pendingCommand,
    pendingLocalCommandCount: snapshot.status.pendingCommandCount,
    latestOutcome: outcome,
    actions: {
      stop: hasPending
        ? action(false, "command_in_flight")
        : action(active, active ? null : "action_requires_active_attachment"),
      checkpoint: hasPending
        ? action(false, "command_in_flight")
        : action(active, active ? null : "action_requires_active_attachment"),
      move: hasPending
        ? action(false, "command_in_flight", readyDestinations)
        : action(active && readyDestinations.length > 0,
            !active ? "action_requires_active_attachment" : readyDestinations.length === 0 ? "destination_not_ready" : null,
            readyDestinations),
      resume: hasPending
        ? action(false, "command_in_flight")
        : action(suspended, suspended ? null : "action_requires_suspended_attachment"),
      failback: hasPending
        ? action(false, "command_in_flight", failbackDestinations)
        : action(active && failbackDestinations.length > 0,
            !active ? "action_requires_active_attachment" : failbackDestinations.length === 0 ? "failback_target_missing" : null,
            failbackDestinations),
    },
  }
}

export type MobilePortableCommandBuildResult =
  | Readonly<{ state: "admitted"; command: PortableSessionCommand }>
  | Readonly<{ state: "rejected"; reason: MobilePortableUnavailableReason }>

const movementActions: ReadonlySet<MobilePortableControlAction> = new Set(["move", "failback"])
const checkpointActions: ReadonlySet<MobilePortableControlAction> = new Set(["checkpoint", "move", "failback"])

/** Build byte-stable command identity from an injected invocation and timestamp. */
export const buildMobilePortableSessionCommand = (input: Readonly<{
  control: MobilePortableSessionControl
  action: MobilePortableControlAction
  invocationRef: string
  issuedAt: string
  destinationTargetRef?: string
  ttlMillis?: number
}>): MobilePortableCommandBuildResult => {
  if (input.control.state !== "ready") return { state: "rejected", reason: input.control.reason }
  const availability = input.control.actions[input.action]
  if (!availability.available) return { state: "rejected", reason: availability.reason ?? "invalid_invocation" }
  const invocationRef = input.invocationRef.trim()
  const issuedAtMs = Date.parse(input.issuedAt)
  const ttlMillis = input.ttlMillis ?? 60_000
  if (!/^[A-Za-z0-9][A-Za-z0-9._:-]{2,120}$/.test(invocationRef) ||
      !Number.isFinite(issuedAtMs) || !Number.isInteger(ttlMillis) || ttlMillis < 1_000 || ttlMillis > 300_000) {
    return { state: "rejected", reason: "invalid_invocation" }
  }
  let destination: PortableTargetDescriptor | undefined
  if (movementActions.has(input.action)) {
    if (input.destinationTargetRef === undefined) return { state: "rejected", reason: "destination_required" }
    if (input.destinationTargetRef === input.control.sourceTarget.targetRef) {
      return { state: "rejected", reason: "destination_is_source" }
    }
    destination = availability.destinations.find(value => value.targetRef === input.destinationTargetRef)
    if (destination === undefined) return { state: "rejected", reason: "destination_not_ready" }
  }
  const suffix = `${input.action}.${invocationRef}`
  try {
    return {
      state: "admitted",
      command: Schema.decodeUnknownSync(PortableSessionCommandSchema)({
        schema: "openagents.portable_session_command.v1",
        commandRef: `command.mobile.${suffix}`,
        idempotencyKey: `idempotency.mobile.${suffix}`,
        ownerRef: input.control.ownerRef,
        sessionRef: input.control.sessionRef,
        kind: input.action,
        expectedAttachmentRef: input.control.sourceAttachment.attachmentRef,
        expectedGeneration: input.control.sourceAttachment.generation,
        ...(destination === undefined ? {} : { destinationTargetRef: destination.targetRef }),
        ...(checkpointActions.has(input.action) ? { checkpointRef: `checkpoint.mobile.${suffix}` } : {}),
        expiresAt: new Date(issuedAtMs + ttlMillis).toISOString(),
      }),
    }
  } catch {
    return { state: "rejected", reason: "invalid_invocation" }
  }
}
