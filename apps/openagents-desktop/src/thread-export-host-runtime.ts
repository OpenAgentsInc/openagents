import { Effect } from "effect";

import { openDesktopThreadExportArtifactStore } from "./thread-export-artifact-store.ts";
import {
  openDesktopThreadExportCommandFromConfirmedTimeline,
  type DesktopThreadExportConfirmedTimelineCommandDependencies,
} from "./thread-export-confirmed-timeline-command.ts";
import {
  openDesktopThreadExportFileTransport,
  type DesktopThreadExportFileTransportDependencies,
} from "./thread-export-file-transport.ts";
import {
  openDesktopThreadExportMainComposition,
  type DesktopThreadExportMainCompositionDependencies,
} from "./thread-export-main-composition.ts";

export type DesktopThreadExportHostRuntimeDependencies = Readonly<{
  storeDirectory: string;
  snapshotForThread: DesktopThreadExportConfirmedTimelineCommandDependencies["snapshotForThread"];
  selectDestination: DesktopThreadExportFileTransportDependencies["selectDestination"];
  registerWrite: DesktopThreadExportMainCompositionDependencies["registerWrite"];
  registerCreate: DesktopThreadExportMainCompositionDependencies["registerCreate"];
  isTrustedSender: DesktopThreadExportMainCompositionDependencies["isTrustedSender"];
  makeReceiptRef: DesktopThreadExportConfirmedTimelineCommandDependencies["makeReceiptRef"];
  observedAt: DesktopThreadExportConfirmedTimelineCommandDependencies["observedAt"];
  sha256: DesktopThreadExportConfirmedTimelineCommandDependencies["sha256"];
}>;

/**
 * Acquire the complete owner-only canonical-export main-process graph behind
 * one close-only lifetime. Store bytes, destination paths, and host authority
 * remain inside the composed dependencies and never enter the returned value.
 */
export const openDesktopThreadExportHostRuntime = Effect.fn(
  "DesktopThreadExportHostRuntime.open",
)(function* (dependencies: DesktopThreadExportHostRuntimeDependencies) {
  const store = openDesktopThreadExportArtifactStore(dependencies.storeDirectory);
  const command = openDesktopThreadExportCommandFromConfirmedTimeline({
    snapshotForThread: dependencies.snapshotForThread,
    persist: store.persist,
    makeReceiptRef: dependencies.makeReceiptRef,
    observedAt: dependencies.observedAt,
    sha256: dependencies.sha256,
  });
  const transport = openDesktopThreadExportFileTransport({
    load: store.load,
    selectDestination: dependencies.selectDestination,
  });

  return yield* openDesktopThreadExportMainComposition({
    registerWrite: dependencies.registerWrite,
    registerCreate: dependencies.registerCreate,
    isTrustedSender: dependencies.isTrustedSender,
    write: transport.write,
    execute: command.execute,
  });
});
