import { createHash, randomUUID } from "node:crypto";
import { existsSync } from "node:fs";
import path from "node:path";

import { Effect, Schema } from "effect";

import type { DesktopThreadExportConfirmedTimelineCommandDependencies } from "./thread-export-confirmed-timeline-command.ts";
import type { DesktopThreadExportCreateMainHandler } from "./thread-export-create-main-handler.ts";
import {
  openDesktopThreadExportHostRuntime,
  type DesktopThreadExportHostRuntimeDependencies,
} from "./thread-export-host-runtime.ts";
import type { DesktopThreadExportMainHandler } from "./thread-export-main-handler.ts";

export type DesktopThreadExportElectronHandler =
  | DesktopThreadExportCreateMainHandler
  | DesktopThreadExportMainHandler;

export type DesktopThreadExportSaveDialogOptions = Readonly<{
  title: string;
  buttonLabel: string;
  defaultPath: string;
  filters: ReadonlyArray<Readonly<{ name: string; extensions: ReadonlyArray<string> }>>;
  properties: ReadonlyArray<"createDirectory" | "showOverwriteConfirmation">;
}>;

export type DesktopThreadExportElectronHostDependencies = Readonly<{
  userDataDirectory: string;
  snapshotForThread: DesktopThreadExportConfirmedTimelineCommandDependencies["snapshotForThread"];
  showSaveDialog: (options: DesktopThreadExportSaveDialogOptions) => Promise<unknown>;
  handle: (channel: string, handler: DesktopThreadExportElectronHandler) => void;
  removeHandler: (channel: string) => void;
  isTrustedSender: DesktopThreadExportHostRuntimeDependencies["isTrustedSender"];
}>;

export class DesktopThreadExportElectronHostUnavailable extends Schema.TaggedErrorClass<DesktopThreadExportElectronHostUnavailable>()(
  "DesktopThreadExportElectronHostUnavailable",
  { stage: Schema.Literal("user_data") },
) {}

const field = (value: unknown, key: string): unknown =>
  typeof value === "object" && value !== null ? Reflect.get(value, key) : undefined;

const artifactDirectory = (userDataDirectory: string): string | undefined => {
  if (!path.isAbsolute(userDataDirectory) || userDataDirectory.includes("\0")) return undefined;
  const resolved = path.resolve(userDataDirectory);
  if (resolved === path.parse(resolved).root) return undefined;
  return path.join(resolved, "thread-exports", "artifacts");
};

const selectDestination = (
  dependencies: Pick<DesktopThreadExportElectronHostDependencies, "showSaveDialog">,
): DesktopThreadExportHostRuntimeDependencies["selectDestination"] =>
  async ({ suggestedName }) => {
    const raw = await dependencies.showSaveDialog({
      title: "Export canonical thread events",
      buttonLabel: "Export",
      defaultPath: suggestedName,
      filters: [{ name: "JSON", extensions: ["json"] }],
      properties: ["createDirectory", "showOverwriteConfirmation"],
    });
    if (field(raw, "canceled") === true) return { status: "cancelled" };
    const filePath = field(raw, "filePath");
    if (field(raw, "canceled") !== false || typeof filePath !== "string") return undefined;
    return {
      status: "selected",
      filePath,
      replaceExisting: existsSync(filePath),
    };
  };

type RegisterElectronHandler = (
  channel: string,
  handler: DesktopThreadExportElectronHandler,
) => () => void;

const register = (
  dependencies: Pick<DesktopThreadExportElectronHostDependencies, "handle" | "removeHandler">,
): RegisterElectronHandler =>
  (channel, handler) => {
    dependencies.handle(channel, handler);
    let removed = false;
    return (): void => {
      if (removed) return;
      removed = true;
      dependencies.removeHandler(channel);
    };
  };

/**
 * Bind the canonical-export resource graph to the narrow Electron host seams.
 * The later main-process call site supplies only trusted renderer, confirmed
 * timeline, IPC, save-dialog, and userData authority.
 */
export const openDesktopThreadExportElectronHost = Effect.fn(
  "DesktopThreadExportElectronHost.open",
)(function* (dependencies: DesktopThreadExportElectronHostDependencies) {
  const storeDirectory = artifactDirectory(dependencies.userDataDirectory);
  if (storeDirectory === undefined) {
    return yield* Effect.fail(
      new DesktopThreadExportElectronHostUnavailable({ stage: "user_data" }),
    );
  }

  const registerHandler = register(dependencies);
  return yield* openDesktopThreadExportHostRuntime({
    storeDirectory,
    snapshotForThread: dependencies.snapshotForThread,
    selectDestination: selectDestination(dependencies),
    registerWrite: registerHandler,
    registerCreate: registerHandler,
    isTrustedSender: dependencies.isTrustedSender,
    makeReceiptRef: () => `receipt.thread_export.${randomUUID()}`,
    observedAt: () => new Date().toISOString(),
    sha256: (bytes) => createHash("sha256").update(bytes).digest("hex"),
  });
});
