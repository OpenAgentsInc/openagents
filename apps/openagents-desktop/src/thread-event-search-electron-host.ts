import path from "node:path";

import { Effect, Schema } from "effect";

import { DesktopThreadEventSearchChannel } from "./thread-event-search-bridge-contract.ts";
import {
  openDesktopThreadEventSearchHostRuntime,
  type DesktopThreadEventSearchHostRuntimeDependencies,
} from "./thread-event-search-host-runtime.ts";
import type { DesktopThreadEventSearchMainHandler } from "./thread-event-search-main-handler.ts";

export type DesktopThreadEventSearchElectronHostDependencies = Readonly<{
  userDataDirectory: string;
  handle: (
    channel: typeof DesktopThreadEventSearchChannel,
    handler: DesktopThreadEventSearchMainHandler,
  ) => void;
  removeHandler: (channel: typeof DesktopThreadEventSearchChannel) => void;
  isTrustedSender: DesktopThreadEventSearchHostRuntimeDependencies["isTrustedSender"];
}>;

export class DesktopThreadEventSearchElectronHostUnavailable extends Schema.TaggedErrorClass<DesktopThreadEventSearchElectronHostUnavailable>()(
  "DesktopThreadEventSearchElectronHostUnavailable",
  { stage: Schema.Literal("user_data") },
) {}

const privateDirectories = (
  userDataDirectory: string,
): Readonly<{ artifactStoreDirectory: string; receiptCatalogDirectory: string }> | undefined => {
  if (!path.isAbsolute(userDataDirectory) || userDataDirectory.includes("\0")) return undefined;
  const resolved = path.resolve(userDataDirectory);
  if (resolved === path.parse(resolved).root) return undefined;
  const root = path.join(resolved, "thread-exports");
  return {
    artifactStoreDirectory: path.join(root, "artifacts"),
    receiptCatalogDirectory: path.join(root, "search-receipts"),
  };
};

const register =
  (
    dependencies: Pick<
      DesktopThreadEventSearchElectronHostDependencies,
      "handle" | "removeHandler"
    >,
  ): DesktopThreadEventSearchHostRuntimeDependencies["register"] =>
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
 * Acquire canonical-event search through narrow Electron host seams. The
 * caller supplies user-data, IPC, and sender authority; storage paths and
 * native registration detail remain inside the returned close-only lifetime.
 */
export const openDesktopThreadEventSearchElectronHost = Effect.fn(
  "DesktopThreadEventSearchElectronHost.open",
)(function* (dependencies: DesktopThreadEventSearchElectronHostDependencies) {
  const directories = privateDirectories(dependencies.userDataDirectory);
  if (directories === undefined) {
    return yield* Effect.fail(
      new DesktopThreadEventSearchElectronHostUnavailable({ stage: "user_data" }),
    );
  }

  return yield* openDesktopThreadEventSearchHostRuntime({
    ...directories,
    register: register(dependencies),
    isTrustedSender: dependencies.isTrustedSender,
  });
});
