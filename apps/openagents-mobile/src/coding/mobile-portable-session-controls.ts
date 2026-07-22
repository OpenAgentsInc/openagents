import type { ConfirmedPortableSessionSnapshot } from "@openagentsinc/khala-sync-client";
import {
  auditPortableSessionSnapshot,
  PortableSessionCommandSchema,
  type PortableAttachment,
  type PortableCodingSession,
  type PortableCommandProjection,
  type PortableSessionCommand,
  type PortableSessionCommandKind,
  type PortableSessionCommandOutcome,
  type PortableTargetDescriptor,
  type PortableTargetDirectoryProjection,
} from "@openagentsinc/portable-session-contract";
import { Schema } from "effect";

export type MobilePortableControlAction = Extract<
  PortableSessionCommandKind,
  "stop" | "checkpoint" | "move" | "resume" | "failback"
>;

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
  | "invalid_invocation";

export type MobilePortableActionAvailability = Readonly<{
  available: boolean;
  reason: MobilePortableUnavailableReason | null;
  destinations: ReadonlyArray<PortableTargetDescriptor>;
}>;

export type MobilePortableSessionControl =
  | Readonly<{
      state: "unavailable";
      sessionRef: string;
      reason: MobilePortableUnavailableReason;
    }>
  | Readonly<{
      state: "ready";
      sessionRef: string;
      ownerRef: string;
      sourceAttachment: PortableAttachment;
      sourceTarget: PortableTargetDescriptor;
      targets: ReadonlyArray<PortableTargetDescriptor>;
      pendingCommand: PortableSessionCommand | null;
      pendingLocalCommandCount: number;
      latestOutcome: PortableSessionCommandOutcome | null;
      actions: Readonly<Record<MobilePortableControlAction, MobilePortableActionAvailability>>;
    }>;

const unavailable = (
  sessionRef: string,
  reason: MobilePortableUnavailableReason,
): MobilePortableSessionControl => ({ state: "unavailable", sessionRef, reason });

const action = (
  available: boolean,
  reason: MobilePortableUnavailableReason | null,
  destinations: ReadonlyArray<PortableTargetDescriptor> = [],
): MobilePortableActionAvailability => ({ available, reason, destinations });

const portableRefPattern = /^[A-Za-z0-9][A-Za-z0-9._:-]{2,255}$/;
const privateMaterialPattern =
  /(?:^|[._:-])(?:pid|fd|handle|socket|process)[._:-]?\d+$|github_pat_|gh[pousr]_|sk-[A-Za-z0-9]|xox[baprs]-|AKIA[0-9A-Z]{16}|eyJ[A-Za-z0-9_-]{10,}[.]|(?:bearer|basic)[._:-]|(?:password|secret|authorization_value|api_key|apikey|mnemonic|private_key|access_token|refresh_token|session_token|provider_token|token_value|credential_value|auth_home|local_path|hostname)[._:-]/i;
const endpointOrPathPattern = /^(?:[A-Za-z]:[\\/]|[\\/]|~[\\/]|file:|https?:|ssh:|wss?:)/i;

const isOpaquePortableRef = (value: string): boolean =>
  portableRefPattern.test(value) &&
  !privateMaterialPattern.test(value) &&
  !endpointOrPathPattern.test(value);

const allOpaque = (values: ReadonlyArray<string | undefined>): boolean =>
  values.every((value) => value === undefined || isOpaquePortableRef(value));

const sessionRefsAreSafe = (session: PortableCodingSession): boolean =>
  allOpaque([
    session.sessionRef,
    session.ownerRef,
    session.workContextRef,
    session.eventLogRef,
    session.currentProjectionRef,
    session.volatileStreamRef,
    session.commandScopeRef,
    session.adoptionReceiptRef,
    session.graph.rootAgentRef,
    ...session.graph.nodes.flatMap((node) => [
      node.agentRef,
      node.parentAgentRef,
      node.threadRef,
      node.transcriptRef,
    ]),
  ]);

const directoryRefsAreSafe = (directory: PortableTargetDirectoryProjection): boolean =>
  allOpaque([
    directory.sessionRef,
    ...directory.targets.flatMap((target) => [
      target.targetRef,
      target.adapterRef,
      target.ownerRef,
      target.compatibilityRef,
    ]),
  ]);

const attachmentRefsAreSafe = (attachment: PortableAttachment): boolean =>
  allOpaque([
    attachment.attachmentRef,
    attachment.sessionRef,
    attachment.targetRef,
    attachment.checkpointRef,
    ...attachment.descendantAgentRefs,
    ...attachment.capabilityLeaseRefs,
    ...attachment.evidenceRefs,
  ]);

const commandRefsAreSafe = (projection: PortableCommandProjection): boolean => {
  const outcomeRefs =
    "outcome" in projection
      ? [
          projection.outcome.commandRef,
          projection.outcome.sessionRef,
          projection.outcome.sourceAttachmentRef,
          projection.outcome.destinationAttachmentRef,
          projection.outcome.checkpointRef,
          projection.outcome.reasonRef,
          ...projection.outcome.evidenceRefs,
        ]
      : [];
  return allOpaque([
    projection.command.commandRef,
    projection.command.idempotencyKey,
    projection.command.ownerRef,
    projection.command.sessionRef,
    projection.command.expectedAttachmentRef,
    projection.command.destinationTargetRef,
    projection.command.checkpointRef,
    ...outcomeRefs,
  ]);
};

const unique = (values: ReadonlyArray<string>): boolean => new Set(values).size === values.length;

const graphIsCoherent = (session: PortableCodingSession): boolean => {
  const nodes = new Map(session.graph.nodes.map((node) => [node.agentRef, node]));
  const root = nodes.get(session.graph.rootAgentRef);
  if (
    nodes.size !== session.graph.nodes.length ||
    root === undefined ||
    root.parentAgentRef !== undefined
  ) {
    return false;
  }
  for (const node of nodes.values()) {
    if (node.parentAgentRef !== undefined && !nodes.has(node.parentAgentRef)) return false;
    const visited = new Set([node.agentRef]);
    let cursor = node;
    while (cursor.parentAgentRef !== undefined) {
      if (visited.has(cursor.parentAgentRef)) return false;
      visited.add(cursor.parentAgentRef);
      const parent = nodes.get(cursor.parentAgentRef);
      if (parent === undefined) return false;
      cursor = parent;
    }
  }
  return true;
};

const newestActionableAttachment = (
  attachments: ReadonlyArray<PortableAttachment>,
): PortableAttachment | null => {
  const active = attachments.filter((value) => value.state === "active");
  if (active.length === 1) return active[0] ?? null;
  if (active.length > 1) return null;
  const suspended = attachments
    .filter(
      (value) =>
        value.state === "quiesced" || value.state === "detached" || value.state === "failed",
    )
    .toSorted((left, right) => right.generation - left.generation);
  if (suspended.length === 0 || suspended[0]?.generation === suspended[1]?.generation) return null;
  return suspended[0] ?? null;
};

/**
 * Join one coding-session ref to confirmed portable authority. No row from a
 * withheld scope or malformed snapshot can enable a control.
 */
export const projectMobilePortableSessionControl = (
  snapshot: ConfirmedPortableSessionSnapshot,
  sessionRef: string,
): MobilePortableSessionControl => {
  if (!isOpaquePortableRef(sessionRef)) return unavailable("session.invalid", "projection_invalid");
  if (snapshot.status.phase !== "live") return unavailable(sessionRef, "authority_unavailable");
  if (
    snapshot.status.cursor === null ||
    !Number.isSafeInteger(snapshot.status.cursor) ||
    snapshot.status.cursor < 0 ||
    !Number.isSafeInteger(snapshot.status.pendingCommandCount) ||
    snapshot.status.pendingCommandCount < 0
  ) {
    return unavailable(sessionRef, "projection_invalid");
  }
  if (snapshot.issues.length > 0) return unavailable(sessionRef, "projection_invalid");
  const sessions = snapshot.sessions.filter((value) => value.sessionRef === sessionRef);
  const session = sessions[0];
  if (session === undefined) return unavailable(sessionRef, "session_not_portable");
  if (
    sessions.length !== 1 ||
    snapshot.sessions.some(
      (value) => value.sessionRef !== sessionRef && value.workContextRef === session.workContextRef,
    )
  ) {
    return unavailable(sessionRef, "projection_invalid");
  }
  const directories = snapshot.targetDirectories.filter((value) => value.sessionRef === sessionRef);
  const directory = directories[0];
  if (directory === undefined) return unavailable(sessionRef, "target_directory_missing");
  if (directories.length !== 1) return unavailable(sessionRef, "projection_invalid");
  const attachments = snapshot.attachments.filter((value) => value.sessionRef === sessionRef);
  const commands = snapshot.commands.filter((value) => value.command.sessionRef === sessionRef);
  if (
    !sessionRefsAreSafe(session) ||
    !directoryRefsAreSafe(directory) ||
    attachments.some((attachment) => !attachmentRefsAreSafe(attachment)) ||
    commands.some((command) => !commandRefsAreSafe(command))
  ) {
    return unavailable(sessionRef, "projection_invalid");
  }
  if (!graphIsCoherent(session)) {
    return unavailable(sessionRef, "projection_invalid");
  }
  const targetRefs = new Set(directory.targets.map((value) => value.targetRef));
  if (
    targetRefs.size !== directory.targets.length ||
    directory.targets.some((value) => value.ownerRef !== session.ownerRef) ||
    attachments.some((value) => !targetRefs.has(value.targetRef))
  ) {
    return unavailable(sessionRef, "projection_invalid");
  }
  const generations = new Set(attachments.map((value) => value.generation));
  if (
    generations.size !== attachments.length ||
    !unique(attachments.map((value) => value.attachmentRef))
  ) {
    return unavailable(sessionRef, "attachment_authority_ambiguous");
  }
  const sourceAttachment = newestActionableAttachment(attachments);
  if (sourceAttachment === null) return unavailable(sessionRef, "attachment_authority_ambiguous");
  const highestGeneration = Math.max(...attachments.map((value) => value.generation));
  const acceptingAttachments = attachments.filter(
    (value) =>
      value.state === "preparing" || value.state === "active" || value.state === "quiescing",
  );
  const expectedDescendants = new Set(session.graph.nodes.map((value) => value.agentRef));
  const sourceDescendants = new Set(sourceAttachment.descendantAgentRefs);
  const invariantViolations = auditPortableSessionSnapshot({
    session,
    targets: directory.targets,
    attachments,
    checkpoints: [],
    leases: [],
    pendingCommands: [],
    topLevelCatalogSessionRefs: [
      session.graph.nodes.find((value) => value.agentRef === session.graph.rootAgentRef)
        ?.threadRef ?? "missing.root",
    ],
  });
  if (
    sourceAttachment.generation !== highestGeneration ||
    acceptingAttachments.some((value) => value.attachmentRef !== sourceAttachment.attachmentRef) ||
    (sourceAttachment.state === "active"
      ? acceptingAttachments.length !== 1
      : acceptingAttachments.length !== 0) ||
    session.graph.nodes.some(
      (value) => value.attachmentGeneration !== sourceAttachment.generation,
    ) ||
    sourceDescendants.size !== sourceAttachment.descendantAgentRefs.length ||
    sourceDescendants.size !== expectedDescendants.size ||
    [...expectedDescendants].some((value) => !sourceDescendants.has(value)) ||
    invariantViolations.length > 0
  ) {
    return unavailable(sessionRef, "attachment_authority_ambiguous");
  }
  const sourceTarget = directory.targets.find(
    (value) =>
      value.targetRef === sourceAttachment.targetRef && value.ownerRef === session.ownerRef,
  );
  if (sourceTarget === undefined) return unavailable(sessionRef, "source_target_missing");

  const acceptedCommands = commands.filter((value) => "status" in value);
  const commandRefs = commands.map((value) => value.command.commandRef);
  const commandIdempotencyKeys = commands.map((value) => value.command.idempotencyKey);
  const attachmentsByRef = new Map(attachments.map((value) => [value.attachmentRef, value]));
  if (
    !unique(commandRefs) ||
    !unique(commandIdempotencyKeys) ||
    commands.some(
      (value) =>
        value.command.ownerRef !== session.ownerRef ||
        ("outcome" in value &&
          (() => {
            const source = attachmentsByRef.get(value.outcome.sourceAttachmentRef);
            const destination =
              value.outcome.destinationAttachmentRef === undefined
                ? undefined
                : attachmentsByRef.get(value.outcome.destinationAttachmentRef);
            return (
              value.outcome.commandRef !== value.command.commandRef ||
              value.outcome.sessionRef !== sessionRef ||
              value.outcome.sourceAttachmentRef !== value.command.expectedAttachmentRef ||
              value.outcome.sourceGeneration !== value.command.expectedGeneration ||
              source?.generation !== value.outcome.sourceGeneration ||
              (value.outcome.destinationAttachmentRef === undefined) !==
                (value.outcome.destinationGeneration === undefined) ||
              (value.outcome.destinationAttachmentRef !== undefined && destination === undefined) ||
              (destination !== undefined &&
                destination.generation !== value.outcome.destinationGeneration)
            );
          })()),
    ) ||
    acceptedCommands.length > 1 ||
    acceptedCommands.some(
      (value) =>
        value.command.ownerRef !== session.ownerRef ||
        value.command.expectedAttachmentRef !== sourceAttachment.attachmentRef ||
        value.command.expectedGeneration !== sourceAttachment.generation ||
        (() => {
          const isMovement =
            value.command.kind === "move" ||
            value.command.kind === "attach" ||
            value.command.kind === "failback";
          if (!isMovement) return value.command.destinationTargetRef !== undefined;
          const destination = directory.targets.find(
            (target) => target.targetRef === value.command.destinationTargetRef,
          );
          return (
            destination === undefined ||
            destination.targetRef === sourceTarget.targetRef ||
            destination.ownerRef !== session.ownerRef ||
            destination.health !== "ready"
          );
        })(),
    )
  ) {
    return unavailable(sessionRef, "projection_invalid");
  }
  const pendingCommand = acceptedCommands[0]?.command ?? null;
  const latestOutcome = commands.find((value) => "outcome" in value && value.outcome !== undefined);
  const outcome =
    latestOutcome !== undefined && "outcome" in latestOutcome ? latestOutcome.outcome : null;
  const hasPending = pendingCommand !== null || snapshot.status.pendingCommandCount > 0;
  const readyDestinations = directory.targets.filter(
    (value) =>
      value.ownerRef === session.ownerRef &&
      value.health === "ready" &&
      value.targetRef !== sourceTarget.targetRef,
  );
  const failbackDestinations = readyDestinations.filter(
    (value) => value.targetClass === "owner_local",
  );
  const active = sourceAttachment.state === "active";
  const suspended =
    sourceAttachment.state === "quiesced" ||
    sourceAttachment.state === "detached" ||
    sourceAttachment.state === "failed";

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
        : action(
            active && readyDestinations.length > 0,
            !active
              ? "action_requires_active_attachment"
              : readyDestinations.length === 0
                ? "destination_not_ready"
                : null,
            readyDestinations,
          ),
      resume: hasPending
        ? action(false, "command_in_flight")
        : action(suspended, suspended ? null : "action_requires_suspended_attachment"),
      failback: hasPending
        ? action(false, "command_in_flight", failbackDestinations)
        : action(
            active && failbackDestinations.length > 0,
            !active
              ? "action_requires_active_attachment"
              : failbackDestinations.length === 0
                ? "failback_target_missing"
                : null,
            failbackDestinations,
          ),
    },
  };
};

export type MobilePortableCommandBuildResult =
  | Readonly<{ state: "admitted"; command: PortableSessionCommand }>
  | Readonly<{ state: "rejected"; reason: MobilePortableUnavailableReason }>;

const movementActions: ReadonlySet<MobilePortableControlAction> = new Set(["move", "failback"]);
const checkpointActions: ReadonlySet<MobilePortableControlAction> = new Set([
  "checkpoint",
  "move",
  "failback",
]);
const decodePortableSessionCommand = Schema.decodeUnknownSync(PortableSessionCommandSchema);

/** Build byte-stable command identity from an injected invocation and timestamp. */
export const buildMobilePortableSessionCommand = (
  input: Readonly<{
    control: MobilePortableSessionControl;
    action: MobilePortableControlAction;
    invocationRef: string;
    issuedAt: string;
    destinationTargetRef?: string;
    ttlMillis?: number;
  }>,
): MobilePortableCommandBuildResult => {
  if (input.control.state !== "ready") return { state: "rejected", reason: input.control.reason };
  const availability = input.control.actions[input.action];
  if (
    !allOpaque([
      input.control.sessionRef,
      input.control.ownerRef,
      input.control.sourceAttachment.attachmentRef,
      input.control.sourceAttachment.sessionRef,
      input.control.sourceAttachment.targetRef,
      input.control.sourceTarget.targetRef,
      input.control.sourceTarget.ownerRef,
      ...availability.destinations.flatMap((value) => [
        value.targetRef,
        value.adapterRef,
        value.ownerRef,
        value.compatibilityRef,
      ]),
    ])
  )
    return { state: "rejected", reason: "invalid_invocation" };
  if (!availability.available)
    return { state: "rejected", reason: availability.reason ?? "invalid_invocation" };
  if (
    input.control.pendingCommand !== null ||
    input.control.pendingLocalCommandCount !== 0 ||
    input.control.sourceAttachment.sessionRef !== input.control.sessionRef ||
    input.control.sourceAttachment.targetRef !== input.control.sourceTarget.targetRef ||
    input.control.sourceTarget.ownerRef !== input.control.ownerRef ||
    !Number.isSafeInteger(input.control.sourceAttachment.generation) ||
    input.control.sourceAttachment.generation < 0 ||
    (input.action === "resume"
      ? input.control.sourceAttachment.state !== "quiesced" &&
        input.control.sourceAttachment.state !== "detached" &&
        input.control.sourceAttachment.state !== "failed"
      : input.control.sourceAttachment.state !== "active")
  ) {
    return { state: "rejected", reason: "invalid_invocation" };
  }
  const invocationRef = input.invocationRef.trim();
  const issuedAtMs = Date.parse(input.issuedAt);
  const ttlMillis = input.ttlMillis ?? 60_000;
  if (
    !isOpaquePortableRef(invocationRef) ||
    invocationRef !== input.invocationRef ||
    invocationRef.length > 121 ||
    !Number.isFinite(issuedAtMs) ||
    new Date(issuedAtMs).toISOString() !== input.issuedAt ||
    !Number.isInteger(ttlMillis) ||
    ttlMillis < 1_000 ||
    ttlMillis > 300_000
  ) {
    return { state: "rejected", reason: "invalid_invocation" };
  }
  let destination: PortableTargetDescriptor | undefined;
  if (movementActions.has(input.action)) {
    if (input.destinationTargetRef === undefined)
      return { state: "rejected", reason: "destination_required" };
    if (!isOpaquePortableRef(input.destinationTargetRef)) {
      return { state: "rejected", reason: "invalid_invocation" };
    }
    if (input.destinationTargetRef === input.control.sourceTarget.targetRef) {
      return { state: "rejected", reason: "destination_is_source" };
    }
    destination = availability.destinations.find(
      (value) => value.targetRef === input.destinationTargetRef,
    );
    if (
      destination === undefined ||
      destination.health !== "ready" ||
      destination.ownerRef !== input.control.ownerRef
    ) {
      return { state: "rejected", reason: "destination_not_ready" };
    }
  } else if (input.destinationTargetRef !== undefined) {
    return { state: "rejected", reason: "invalid_invocation" };
  }
  const suffix = `${input.action}.${invocationRef}`;
  try {
    return {
      state: "admitted",
      command: decodePortableSessionCommand({
        schema: "openagents.portable_session_command.v1",
        commandRef: `command.mobile.${suffix}`,
        idempotencyKey: `idempotency.mobile.${suffix}`,
        ownerRef: input.control.ownerRef,
        sessionRef: input.control.sessionRef,
        kind: input.action,
        expectedAttachmentRef: input.control.sourceAttachment.attachmentRef,
        expectedGeneration: input.control.sourceAttachment.generation,
        ...(destination === undefined ? {} : { destinationTargetRef: destination.targetRef }),
        ...(checkpointActions.has(input.action)
          ? { checkpointRef: `checkpoint.mobile.${suffix}` }
          : {}),
        expiresAt: new Date(issuedAtMs + ttlMillis).toISOString(),
      }),
    };
  } catch {
    return { state: "rejected", reason: "invalid_invocation" };
  }
};
