import { Effect, Schema } from "effect";

import {
  registerDesktopThreadExportCreateMainHandler,
  type DesktopThreadExportCreateMainHandlerDependencies,
} from "./thread-export-create-main-handler.ts";
import {
  registerDesktopThreadExportMainHandler,
  type DesktopThreadExportMainHandlerDependencies,
} from "./thread-export-main-handler.ts";

export class DesktopThreadExportMainCompositionUnavailable extends Schema.TaggedErrorClass<DesktopThreadExportMainCompositionUnavailable>()(
  "DesktopThreadExportMainCompositionUnavailable",
  { stage: Schema.Literals(["write", "create"]) },
) {}

export type DesktopThreadExportMainCompositionDependencies = Readonly<{
  registerWrite: DesktopThreadExportMainHandlerDependencies["register"];
  registerCreate: DesktopThreadExportCreateMainHandlerDependencies["register"];
  isTrustedSender: (event: unknown) => boolean;
  write: DesktopThreadExportMainHandlerDependencies["write"];
  execute: DesktopThreadExportCreateMainHandlerDependencies["execute"];
}>;

export type DesktopThreadExportMainComposition = Readonly<{ close: () => void }>;

const closeWithoutProjection = (registration: DesktopThreadExportMainComposition): void => {
  try {
    registration.close();
  } catch {
    // Native cleanup details stay inside the host boundary.
  }
};

/**
 * Atomically acquires both fixed canonical-export main handlers. The caller
 * owns the returned lifetime resource; no handler or host authority escapes.
 */
export const openDesktopThreadExportMainComposition = Effect.fn(
  "DesktopThreadExportMainComposition.open",
)(function* (dependencies: DesktopThreadExportMainCompositionDependencies) {
  const writeRegistration = yield* Effect.try({
    try: () =>
      registerDesktopThreadExportMainHandler({
        register: dependencies.registerWrite,
        isTrustedSender: dependencies.isTrustedSender,
        write: dependencies.write,
      }),
    catch: () => new DesktopThreadExportMainCompositionUnavailable({ stage: "write" }),
  });

  const createRegistration = yield* Effect.try({
    try: () =>
      registerDesktopThreadExportCreateMainHandler({
        register: dependencies.registerCreate,
        isTrustedSender: dependencies.isTrustedSender,
        execute: dependencies.execute,
      }),
    catch: () => new DesktopThreadExportMainCompositionUnavailable({ stage: "create" }),
  }).pipe(
    Effect.catch((error) =>
      Effect.gen(function* () {
        yield* Effect.sync(() => closeWithoutProjection(writeRegistration));
        return yield* Effect.fail(error);
      }),
    ),
  );

  let closed = false;
  return {
    close: (): void => {
      if (closed) return;
      closed = true;
      closeWithoutProjection(createRegistration);
      closeWithoutProjection(writeRegistration);
    },
  } satisfies DesktopThreadExportMainComposition;
});
