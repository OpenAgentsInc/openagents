import { projectThreadEventAuthority } from "@openagentsinc/agent-runtime-schema";
import { Effect } from "effect";

import { openDesktopThreadEventAuthorityRelationLedger } from "./thread-event-authority-relation-ledger.ts";
import {
  readDesktopThreadExportEvidenceFromConfirmedTimeline,
  type DesktopThreadExportConfirmedTimelineEvidenceDependencies,
} from "./thread-export-confirmed-timeline-evidence.ts";
import type { DesktopThreadExportEvidenceSnapshot } from "./thread-export-command.ts";

export type DesktopThreadExportTerminalAuthorityOverlayDependencies =
  DesktopThreadExportConfirmedTimelineEvidenceDependencies &
    Readonly<{ authorityLedgerDirectory: string }>;

const unavailable = (): DesktopThreadExportEvidenceSnapshot => ({ status: "unavailable" });

const field = (value: unknown, key: string): unknown =>
  typeof value === "object" && value !== null ? Reflect.get(value, key) : undefined;

const eventRefOf = (event: unknown): string | null => {
  const eventRef = field(event, "eventRef");
  return typeof eventRef === "string" ? eventRef : null;
};

/**
 * Overlay exact terminal authority facts already retained by the private
 * ledger onto target-owned confirmed accepted evidence. This reader neither
 * observes nor creates authority; incomplete or invalid reference histories
 * remain unavailable.
 */
export const readDesktopThreadExportEvidenceWithTerminalAuthority = Effect.fn(
  "DesktopThreadExportTerminalAuthorityOverlay.read",
)(function* (
  dependencies: DesktopThreadExportTerminalAuthorityOverlayDependencies,
  rawThreadRef: unknown,
) {
  const confirmed = yield* readDesktopThreadExportEvidenceFromConfirmedTimeline(
    { snapshotForThread: dependencies.snapshotForThread },
    rawThreadRef,
  );
  if (confirmed.status !== "available") return unavailable();

  const listed = openDesktopThreadEventAuthorityRelationLedger(
    dependencies.authorityLedgerDirectory,
  ).listForThread(confirmed.threadRef);
  if (listed.status !== "available") return unavailable();
  if (listed.relations.length === 0) return confirmed;

  const eventRefs = new Set<string>();
  for (const event of confirmed.events) {
    const eventRef = eventRefOf(event);
    if (eventRef === null || eventRefs.has(eventRef)) return unavailable();
    eventRefs.add(eventRef);
  }

  for (const relation of listed.relations) {
    if (!eventRefs.has(relation.eventRef)) return unavailable();
    const referenced =
      relation.kind === "superseded"
        ? [relation.supersededByEventRef]
        : [relation.revertedByEventRef, relation.restoredEventRef];
    if (referenced.some((eventRef) => !eventRefs.has(eventRef))) return unavailable();
  }

  const relations = [...confirmed.relations, ...listed.relations];
  for (const eventRef of eventRefs) {
    if (
      projectThreadEventAuthority({
        threadRef: confirmed.threadRef,
        eventRef,
        relations,
      }).status !== "resolved"
    ) {
      return unavailable();
    }
  }

  return { ...confirmed, relations };
});
