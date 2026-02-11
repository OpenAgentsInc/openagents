import type { PaneSystemConfig, PaneSystemDom } from "@openagentsinc/effuse-panes";
import { Effect } from "effect";
import { describe, expect, it, vi } from "vitest";

import { PaneSystemLive, PaneSystemService } from "../../src/effect/paneSystem";

const makeStubPaneSystem = (destroy: () => void): PaneSystemDom => ({
  store: {} as PaneSystemDom["store"],
  hotbar: {} as PaneSystemDom["hotbar"],
  setHotbarItems: () => {},
  destroy,
  render: () => {},
});

describe("apps/web PaneSystemService", () => {
  it("mounts via service and releases via scoped finalizer", async () => {
    const destroy = vi.fn();
    const root = {} as HTMLElement;
    const config: Partial<PaneSystemConfig> = { enableHotbar: false };
    const mountFn = vi.fn((inputRoot: HTMLElement, inputConfig?: Partial<PaneSystemConfig>) => {
      expect(inputRoot).toBe(root);
      expect(inputConfig).toEqual(config);
      return makeStubPaneSystem(destroy);
    });

    const mounted = await Effect.gen(function* () {
      const panes = yield* PaneSystemService;
      return yield* panes.mount({ root, config, mountFn });
    }).pipe(Effect.provide(PaneSystemLive), Effect.runPromise);

    expect(mountFn).toHaveBeenCalledTimes(1);
    expect(destroy).toHaveBeenCalledTimes(0);

    await Effect.runPromise(mounted.release);
    expect(destroy).toHaveBeenCalledTimes(1);
  });

  it("release is idempotent", async () => {
    const destroy = vi.fn();
    const root = {} as HTMLElement;
    const mountFn = vi.fn(() => makeStubPaneSystem(destroy));

    const mounted = await Effect.gen(function* () {
      const panes = yield* PaneSystemService;
      return yield* panes.mount({ root, mountFn });
    }).pipe(Effect.provide(PaneSystemLive), Effect.runPromise);

    await Effect.runPromise(mounted.release);
    await Effect.runPromise(mounted.release);

    expect(destroy).toHaveBeenCalledTimes(1);
  });
});
