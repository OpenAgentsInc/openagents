import {
  CanonicalThreadExportEvent,
  decodeThreadEventAuthorityRelation,
} from "@openagentsinc/agent-runtime-schema";
import {
  ConfirmedAgentRunSchema,
  ConfirmedAgentTimelineEventSchema,
  MAX_CONFIRMED_AGENT_TIMELINE_EVENTS,
} from "@openagentsinc/khala-sync-client";
import { createHash } from "node:crypto";
import { Effect, Schema as S } from "effect";

import type { DesktopThreadExportEvidenceSnapshot } from "./thread-export-command.ts";

export type DesktopThreadExportConfirmedTimelineEvidenceDependencies = Readonly<{
  snapshotForThread: (threadRef: string) => unknown;
}>;

const Ref = S.String.check(
  S.isMinLength(1),
  S.isMaxLength(256),
  S.isPattern(/^[A-Za-z0-9][A-Za-z0-9._:-]*$/),
);

const NonNegativeInt = S.Number.check(S.isInt(), S.isGreaterThanOrEqualTo(0));

const ConfirmedTimelineSnapshot = S.Struct({
  status: S.Struct({
    phase: S.Literals([
      "idle",
      "bootstrapping",
      "catching_up",
      "live",
      "must_refetch",
      "denied",
    ]),
    cursor: S.NullOr(NonNegativeInt),
    pendingMutationCount: NonNegativeInt,
  }),
  run: S.NullOr(ConfirmedAgentRunSchema),
  events: S.Array(ConfirmedAgentTimelineEventSchema).check(
    S.isMaxLength(MAX_CONFIRMED_AGENT_TIMELINE_EVENTS),
  ),
});

const decodeRef = S.decodeUnknownSync(Ref);
const decodeSnapshot = S.decodeUnknownSync(ConfirmedTimelineSnapshot);
const decodeCanonicalEvent = S.decodeUnknownSync(CanonicalThreadExportEvent);

const unavailable = (): DesktopThreadExportEvidenceSnapshot => ({ status: "unavailable" });

const relationRefFor = (
  threadRef: string,
  event: typeof ConfirmedAgentTimelineEventSchema.Type,
): string =>
  `relation.confirmed.${createHash("sha256")
    .update(JSON.stringify([threadRef, event.runRef, event.eventRef, event.version]))
    .digest("hex")}`;

const projectAvailableSnapshot = (
  threadRef: string,
  snapshot: typeof ConfirmedTimelineSnapshot.Type,
): DesktopThreadExportEvidenceSnapshot => {
  if (
    snapshot.status.phase !== "live" ||
    snapshot.status.cursor === null ||
    snapshot.status.pendingMutationCount !== 0 ||
    snapshot.run === null
  ) {
    return unavailable();
  }

  const eventRefs = new Set<string>();
  const sequences = new Set<number>();
  const events = [];
  const relations = [];
  try {
    for (const event of snapshot.events) {
      if (
        event.runRef !== snapshot.run.runRef ||
        eventRefs.has(event.eventRef) ||
        sequences.has(event.sequence)
      ) {
        return unavailable();
      }
      eventRefs.add(event.eventRef);
      sequences.add(event.sequence);

      events.push(
        decodeCanonicalEvent({
          eventRef: event.eventRef,
          threadRef,
          sequence: event.sequence,
          data: {
            runRef: event.runRef,
            eventType: event.eventType,
            summary: event.summary,
            status: event.status,
            artifactRefs: event.artifactRefs,
            ...(event.item === undefined ? {} : { item: event.item }),
            createdAt: event.createdAt,
            version: event.version,
          },
        }),
      );
      relations.push(
        decodeThreadEventAuthorityRelation({
          schema: "openagents.thread_event_authority.v1",
          relationRef: relationRefFor(threadRef, event),
          threadRef,
          eventRef: event.eventRef,
          observedAt: event.createdAt,
          kind: "accepted",
        }),
      );
    }
  } catch {
    return unavailable();
  }

  return { status: "available", threadRef, events, relations };
};

/**
 * Read canonical-export evidence only from the target-owned, server-confirmed
 * timeline. A confirmed event proves its accepted fact; this adapter never
 * invents supersession or reversion facts absent from that source.
 */
export const readDesktopThreadExportEvidenceFromConfirmedTimeline = Effect.fn(
  "DesktopThreadExportConfirmedTimelineEvidence.read",
)(function* (
  dependencies: DesktopThreadExportConfirmedTimelineEvidenceDependencies,
  rawThreadRef: unknown,
) {
  let threadRef: string;
  try {
    threadRef = decodeRef(rawThreadRef);
  } catch {
    return unavailable();
  }

  const snapshot = yield* Effect.try({
    try: () => decodeSnapshot(dependencies.snapshotForThread(threadRef)),
    catch: () => null,
  }).pipe(Effect.match({ onFailure: () => null, onSuccess: (value) => value }));
  return snapshot === null ? unavailable() : projectAvailableSnapshot(threadRef, snapshot);
});
