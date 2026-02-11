import { Context, Effect, Exit, Layer, Scope } from "effect";
import { mountPaneSystemDom } from "@openagentsinc/effuse-panes";

import type { PaneSystemConfig, PaneSystemDom } from "@openagentsinc/effuse-panes";

export type PaneSystemMountInput = Readonly<{
  readonly root: HTMLElement;
  readonly config?: Partial<PaneSystemConfig>;
  readonly mountFn?: (root: HTMLElement, config?: Partial<PaneSystemConfig>) => PaneSystemDom;
}>;

export type MountedPaneSystem = Readonly<{
  readonly paneSystem: PaneSystemDom;
  readonly release: Effect.Effect<void>;
}>;

export type PaneSystemServiceApi = {
  readonly mount: (input: PaneSystemMountInput) => Effect.Effect<MountedPaneSystem>;
};

export class PaneSystemService extends Context.Tag("@openagents/web/PaneSystemService")<
  PaneSystemService,
  PaneSystemServiceApi
>() {}

const mount = Effect.fn("PaneSystemService.mount")(function* (input: PaneSystemMountInput) {
  const scope = yield* Scope.make();
  const mountFn = input.mountFn ?? mountPaneSystemDom;

  const paneSystem = yield* Effect.acquireRelease(
    Effect.sync(() => mountFn(input.root, input.config)),
    (mounted) => Effect.sync(() => mounted.destroy()),
  ).pipe(Scope.extend(scope));

  let released = false;
  const release = Effect.suspend(() => {
    if (released) return Effect.void;
    released = true;
    return Scope.close(scope, Exit.void);
  });

  return { paneSystem, release } satisfies MountedPaneSystem;
});

export const PaneSystemLive = Layer.succeed(
  PaneSystemService,
  PaneSystemService.of({ mount }),
);
