import { Effect } from "effect";

import {
  openDesktopThreadExportCommand,
  type DesktopThreadExportCommandDependencies,
} from "./thread-export-command.ts";
import {
  readDesktopThreadExportEvidenceWithTerminalAuthority,
  type DesktopThreadExportTerminalAuthorityOverlayDependencies,
} from "./thread-export-terminal-authority-overlay.ts";

export type DesktopThreadExportConfirmedTimelineCommandDependencies =
  DesktopThreadExportTerminalAuthorityOverlayDependencies &
    Omit<DesktopThreadExportCommandDependencies, "readEvidence">;

/**
 * Bind the target-owned confirmed timeline to the existing owner-only export
 * command. Runtime acquisition, persistence, metadata, and handler lifetime
 * stay explicit host dependencies for the eventual main-process call site.
 */
export const openDesktopThreadExportCommandFromConfirmedTimeline = (
  dependencies: DesktopThreadExportConfirmedTimelineCommandDependencies,
) => {
  const readEvidence = Effect.fn("DesktopThreadExportConfirmedTimelineCommand.readEvidence")(
    function* (threadRef: string) {
      return yield* readDesktopThreadExportEvidenceWithTerminalAuthority(dependencies, threadRef);
    },
  );

  return openDesktopThreadExportCommand({
    readEvidence: (threadRef) => Effect.runPromise(readEvidence(threadRef)),
    persist: dependencies.persist,
    makeReceiptRef: dependencies.makeReceiptRef,
    observedAt: dependencies.observedAt,
    sha256: dependencies.sha256,
  });
};
