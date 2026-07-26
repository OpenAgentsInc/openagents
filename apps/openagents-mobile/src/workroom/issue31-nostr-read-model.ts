import {
  foldIssue31Grant,
  type Issue31GrantState,
  type Issue31PairingEvent,
} from "@openagentsinc/sarah/issue31-nostr";
import {
  SARAH_ENGRAM_KIND,
  SARAH_READ_STATE_KIND,
  SARAH_REMINDER_KIND,
} from "@openagentsinc/sarah/nostr-memory";
import {
  NIP_29_GROUP_CHAT_KIND,
  NIP_AP_MANAGED_INSTANCE_KIND,
  NIP_AP_PERSONA_KIND,
} from "@openagentsinc/sarah/community";
import {
  LBR_AGENTIC_CODING_REQUEST_KIND,
  LBR_AGENTIC_CODING_RESULT_KIND,
  LBR_FEEDBACK_KIND,
} from "@openagentsinc/sarah/lbr-request-quote";
import {
  XP_AWARD_KIND,
  XP_BADGE_AWARD_KIND,
  XP_BADGE_DEFINITION_KIND,
  XP_PROFILE_BADGES_KIND,
  XP_RANK_KIND,
} from "@openagentsinc/sarah/xp";

import type { Issue31ConfirmedEvent, Issue31NostrClientSnapshot } from "./issue31-nostr-client.ts";
import {
  EMPTY_ISSUE31_OWNER_PROJECTED_SOURCE_IDS,
  type Issue31OwnerProjectedSource,
  type Issue31OwnerProjectedSourceIds,
} from "./issue31-owner-private-read-model.ts";
import {
  ISSUE31_CAPABILITY_DESCRIPTORS,
  decodeIssue31SourceSnapshot,
  type Issue31CapabilityId,
  type Issue31SourceSnapshot,
} from "./issue31-workroom-read-model.ts";

const descriptorFor = (capabilityId: Issue31CapabilityId) => {
  const descriptor = ISSUE31_CAPABILITY_DESCRIPTORS.find((row) => row.id === capabilityId);
  if (descriptor === undefined) throw new Error(`Issue 31 descriptor ${capabilityId} is absent.`);
  return descriptor;
};

const observedAtFor = (events: ReadonlyArray<Issue31ConfirmedEvent>): string | null => {
  const latest = events.reduce(
    (timestamp, event) => Math.max(timestamp, event.event.created_at),
    -1,
  );
  return latest < 0 ? null : new Date(latest * 1_000).toISOString();
};

const signedSource = (
  input: Readonly<{
    capabilityId: Issue31CapabilityId;
    events: ReadonlyArray<Issue31ConfirmedEvent>;
    status: "ready" | "gap";
    reasonRef: string | null;
    role: "owner" | "member" | "read_only";
  }>,
): Issue31SourceSnapshot => {
  const descriptor = descriptorFor(input.capabilityId);
  const observedAt = observedAtFor(input.events);
  return decodeIssue31SourceSnapshot({
    capabilityId: input.capabilityId,
    authority: "signed_nostr_record",
    sourceRef: descriptor.sourceRef,
    status: input.status,
    freshness: observedAt === null ? "unknown" : "live",
    observedAt,
    recordRefs: input.events.map((event) => event.canonicalRecordId),
    reasonRef: input.reasonRef,
    role: input.role,
    roleStatus: "active",
    actionState: { kind: "idle" },
  });
};

const eventsWithKinds = (
  events: ReadonlyArray<Issue31ConfirmedEvent>,
  kinds: ReadonlyArray<number>,
): ReadonlyArray<Issue31ConfirmedEvent> =>
  events.filter((event) => kinds.includes(event.event.kind));

/**
 * A capability the device reads only through the host's owner projections.
 *
 * The raw engram, read-state, and reminder events are addressed to the owner
 * and encrypted to the owner–Sarah pair, so seeing one on the wire tells this
 * device nothing about whether it can read it. Status therefore comes from the
 * projections that arrived and decrypted, and the gap names the sources that
 * are visible on the relay and not yet readable here.
 *
 * Before this, both rows were a fixed `gap` emitted on the mere presence of a
 * source event. omega#49 read that on a paired device whose engram had been
 * projected, decrypted, and was renderable in the room directly below the row.
 */
const projectedSource = (
  input: Readonly<{
    capabilityId: Issue31CapabilityId;
    events: ReadonlyArray<Issue31ConfirmedEvent>;
    projected: ReadonlyArray<Issue31OwnerProjectedSource>;
    reasonRef: string;
  }>,
): Issue31SourceSnapshot | null => {
  const projectedIds = new Set(input.projected.map((row) => row.sourceEventId));
  if (projectedIds.size === 0 && input.events.length === 0) return null;
  const unprojected = input.events.filter(
    (event) => !projectedIds.has(event.canonicalRecordId),
  );
  const ready = projectedIds.size > 0 && unprojected.length === 0;
  const descriptor = descriptorFor(input.capabilityId);
  // A projection this device holds is evidence on its own: the relay serves
  // the owner-addressed wrap, so the raw source event need never arrive here.
  const observedAtUnix = Math.max(
    ...input.events.map((event) => event.event.created_at),
    ...input.projected.map((row) => row.sourceCreatedAt),
    -1,
  );
  const observedAt = observedAtUnix < 0 ? null : new Date(observedAtUnix * 1_000).toISOString();
  return decodeIssue31SourceSnapshot({
    capabilityId: input.capabilityId,
    authority: "signed_nostr_record",
    sourceRef: descriptor.sourceRef,
    status: ready ? "ready" : "gap",
    freshness: observedAt === null ? "unknown" : "live",
    observedAt,
    recordRefs: [
      ...new Set([
        ...input.events.map((event) => event.canonicalRecordId),
        ...projectedIds,
      ]),
    ],
    reasonRef: ready ? null : input.reasonRef,
    role: "owner",
    roleStatus: "active",
    actionState: { kind: "idle" },
  });
};

export const issue31SourceSnapshotsFromNostr = (
  snapshot: Issue31NostrClientSnapshot,
  nowUnixSeconds: number,
  /**
   * What the owner-private projection actually read on this device.
   *
   * Omitted means nothing was projected, which is the honest default for a
   * caller that has not run the owner-private projection.
   */
  projected: Issue31OwnerProjectedSourceIds = EMPTY_ISSUE31_OWNER_PROJECTED_SOURCE_IDS,
): ReadonlyArray<Issue31SourceSnapshot> => {
  const admittedHostPublicKeys = new Set(snapshot.admittedHostPublicKeys);
  const selectedHostPublicKeys = new Set(snapshot.selectedHostPublicKeys);
  const ownerPrivateAuthors = new Set(snapshot.ownerPrivateAuthors);
  const ownerRecipientPublicKeys = new Set(snapshot.ownerRecipientPublicKeys);
  const events = snapshot.confirmedEvents.filter((event) => {
    if (event.hostAnnouncement !== null) {
      return admittedHostPublicKeys.has(event.hostAnnouncement.hostPublicKeyHex);
    }
    if (event.privateRecord !== null) {
      return (
        admittedHostPublicKeys.has(event.privateRecord.hostPublicKeyHex) &&
        selectedHostPublicKeys.has(event.privateRecord.hostPublicKeyHex) &&
        event.privateRecord.devicePublicKeyHex === snapshot.devicePublicKeyHex
      );
    }
    if (event.privateRumorId !== null) return true;
    if (event.room !== "owner_private") return true;
    const ownerRecipientTags = event.event.tags.filter((tag) => tag[0] === "p");
    return (
      ownerPrivateAuthors.has(event.event.pubkey) &&
      ownerRecipientTags.length === 1 &&
      ownerRecipientPublicKeys.has(ownerRecipientTags[0]?.[1] ?? "")
    );
  });
  const pairingEvents = events.flatMap(
    (event): ReadonlyArray<Issue31PairingEvent> =>
      event.privateRecord?.schema === "openagents.omega.issue31.pairing.v1"
        ? [{ eventId: event.canonicalRecordId, record: event.privateRecord }]
        : [],
  );
  const grantRefs = new Set(
    pairingEvents.flatMap(({ record }) =>
      record.recordType === "scoped_grant" ||
      record.recordType === "grant_renewal" ||
      record.recordType === "grant_revocation"
        ? [record.grantRef]
        : [],
    ),
  );
  const grantProjectionConflictEvents: Issue31ConfirmedEvent[] = [];
  const activeGrant = [...grantRefs]
    .flatMap((grantRef) => {
      try {
        const grant = foldIssue31Grant(pairingEvents, grantRef);
        return grant === null ? [] : [grant];
      } catch {
        grantProjectionConflictEvents.push(
          ...events.filter(
            (event) =>
              event.privateRecord?.schema === "openagents.omega.issue31.pairing.v1" &&
              "grantRef" in event.privateRecord &&
              event.privateRecord.grantRef === grantRef,
          ),
        );
        return [];
      }
    })
    .filter(
      (grant): grant is Issue31GrantState =>
        grant !== null &&
        grant.status === "active" &&
        grant.expiresAt !== null &&
        grant.expiresAt > nowUnixSeconds,
    )
    .filter((grant) => grant.devicePublicKeyHex === snapshot.devicePublicKeyHex)
    .sort(
      (left, right) =>
        right.issuedAt - left.issuedAt || right.sourceEventId.localeCompare(left.sourceEventId),
    )[0];
  const sources: Issue31SourceSnapshot[] = [];
  if (grantProjectionConflictEvents.length > 0) {
    sources.push(
      signedSource({
        capabilityId: "connection_and_identity",
        events: grantProjectionConflictEvents,
        status: "gap",
        reasonRef: "reason.issue31.grant_projection_fork",
        role: "owner",
      }),
    );
  } else if (activeGrant !== undefined) {
    const grantEvent = events.find(
      (event) => event.canonicalRecordId === activeGrant.sourceEventId,
    );
    const announcement = events
      .filter(
        (event) =>
          event.hostAnnouncement?.hostRef === activeGrant.hostRef &&
          event.hostAnnouncement.hostPublicKeyHex === activeGrant.hostPublicKeyHex &&
          event.hostAnnouncement.sarahPublicKeyHex === activeGrant.sarahPublicKeyHex &&
          ownerPrivateAuthors.has(activeGrant.sarahPublicKeyHex) &&
          event.event.pubkey === activeGrant.hostPublicKeyHex &&
          event.hostAnnouncement.issuedAt <= nowUnixSeconds &&
          event.hostAnnouncement.expiresAt > nowUnixSeconds,
      )
      .sort(
        (left, right) =>
          (right.hostAnnouncement?.generation ?? 0) - (left.hostAnnouncement?.generation ?? 0) ||
          right.event.id.localeCompare(left.event.id),
      )[0];
    const sourceEvents = events.filter(
      (event) =>
        event.canonicalRecordId === grantEvent?.canonicalRecordId ||
        event.event.id === announcement?.event.id,
    );
    if (grantEvent !== undefined) {
      sources.push(
        signedSource({
          capabilityId: "connection_and_identity",
          events: sourceEvents,
          status: announcement === undefined ? "gap" : "ready",
          reasonRef:
            announcement === undefined
              ? "reason.issue31.host_discovery_missing_or_mismatched"
              : null,
          role: "owner",
        }),
      );
    }
  }
  if (
    !sources.some((source) => source.capabilityId === "connection_and_identity") &&
    snapshot.relays.length > 0
  ) {
    const degraded = snapshot.relays.some(
      (relay) => relay.gapReason !== null || relay.state !== "live",
    );
    sources.push(
      decodeIssue31SourceSnapshot({
        capabilityId: "connection_and_identity",
        authority: "signed_nostr_record",
        sourceRef: descriptorFor("connection_and_identity").sourceRef,
        status: degraded ? "gap" : "unavailable",
        freshness: "unknown",
        observedAt: null,
        recordRefs: [],
        reasonRef: degraded
          ? "reason.issue31.nostr_relay_degraded"
          : "reason.issue31.pairing_required",
        role: "none",
        roleStatus: "unknown",
        actionState: { kind: "idle" },
      }),
    );
  }

  const memoryEvents = eventsWithKinds(events, [SARAH_ENGRAM_KIND]);
  const memory = projectedSource({
    capabilityId: "memory",
    events: memoryEvents,
    projected: projected.memory,
    reasonRef: "reason.issue31.device_projection_missing:memory",
  });
  if (memory !== null) sources.push(memory);
  const stateEvents = eventsWithKinds(events, [SARAH_READ_STATE_KIND, SARAH_REMINDER_KIND]);
  const readStateAndReminders = projectedSource({
    capabilityId: "read_state_and_reminders",
    events: stateEvents,
    projected: projected.readStateAndReminders,
    reasonRef: "reason.issue31.device_projection_missing:read_state_and_reminders",
  });
  if (readStateAndReminders !== null) sources.push(readStateAndReminders);

  const membershipEvents = eventsWithKinds(events, [
    NIP_29_GROUP_CHAT_KIND,
    NIP_AP_PERSONA_KIND,
    NIP_AP_MANAGED_INSTANCE_KIND,
  ]);
  if (membershipEvents.length > 0) {
    sources.push(
      signedSource({
        capabilityId: "community_membership",
        events: membershipEvents,
        status: "gap",
        reasonRef: "reason.issue31.community_projection_missing:community_membership",
        role: "read_only",
      }),
    );
  }
  const workEvents = eventsWithKinds(events, [
    LBR_AGENTIC_CODING_REQUEST_KIND,
    LBR_AGENTIC_CODING_RESULT_KIND,
    LBR_FEEDBACK_KIND,
  ]);
  if (workEvents.length > 0) {
    sources.push(
      signedSource({
        capabilityId: "community_work",
        events: workEvents,
        status: "gap",
        reasonRef: "reason.issue31.community_projection_missing:community_work",
        role: "read_only",
      }),
    );
  }
  const experienceEvents = eventsWithKinds(events, [
    XP_AWARD_KIND,
    XP_RANK_KIND,
    XP_BADGE_DEFINITION_KIND,
    XP_BADGE_AWARD_KIND,
    XP_PROFILE_BADGES_KIND,
  ]);
  if (experienceEvents.length > 0) {
    sources.push(
      signedSource({
        capabilityId: "experience",
        events: experienceEvents,
        status: "gap",
        reasonRef: "reason.issue31.community_projection_missing:experience",
        role: "read_only",
      }),
    );
  }
  return sources;
};
