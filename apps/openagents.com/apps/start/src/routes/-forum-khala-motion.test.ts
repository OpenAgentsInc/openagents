import { Effect, Exit, Scope } from "@effect-native/core/effect";
import { afterEach, describe, expect, test, vi } from "vitest";

import {
  forumBoardAssemblyDurationMillis,
  mountForumBoardAssembly,
  type ForumKhalaAssemblyReceipt,
  type ForumKhalaVisibility,
} from "./-forum-khala-motion";

type ControlledAnimation = Readonly<{
  animation: Animation;
  finish: () => void;
}>;

const board = (): Readonly<{
  container: HTMLElement;
  decoration: HTMLElement;
}> => {
  const container = document.createElement("div");
  container.innerHTML = `
    <main data-en-key="forum-index-panel">
      <div data-en-khala-decoration aria-hidden="true"></div>
      <h1>OpenAgents Forum</h1>
      <a href="/forum/f/product-promises">Product Promises</a>
    </main>
  `;
  document.body.appendChild(container);
  return {
    container,
    decoration: container.querySelector<HTMLElement>("[data-en-khala-decoration]")!,
  };
};

const controlledAnimate = (): Readonly<{
  active: () => number;
  animations: ReadonlyArray<ControlledAnimation>;
  install: () => void;
  maximumActive: () => number;
}> => {
  const animations: ControlledAnimation[] = [];
  let active = 0;
  let maximumActive = 0;
  return {
    active: () => active,
    animations,
    maximumActive: () => maximumActive,
    install: () => {
      const animate = vi.fn(() => {
        let resolveFinished: (() => void) | undefined;
        const finished = new Promise<void>((resolve) => {
          resolveFinished = resolve;
        });
        let live = true;
        const settle = () => {
          if (!live) return;
          live = false;
          active -= 1;
          resolveFinished?.();
        };
        active += 1;
        maximumActive = Math.max(maximumActive, active);
        const animation = {
          cancel: vi.fn(settle),
          finished,
        } as unknown as Animation;
        animations.push({ animation, finish: settle });
        return animation;
      });
      Object.defineProperty(HTMLElement.prototype, "animate", {
        configurable: true,
        value: animate,
        writable: true,
      });
    },
  };
};

const controlledVisibility = (
  initiallyHidden = false,
): Readonly<{
  activeListeners: () => number;
  hide: () => void;
  visibility: ForumKhalaVisibility;
}> => {
  let hidden = initiallyHidden;
  const listeners = new Set<() => void>();
  return {
    activeListeners: () => listeners.size,
    hide: () => {
      hidden = true;
      for (const listener of listeners) listener();
    },
    visibility: {
      hidden: () => hidden,
      subscribe: (listener) => {
        listeners.add(listener);
        return () => listeners.delete(listener);
      },
    },
  };
};

afterEach(() => {
  vi.restoreAllMocks();
  Reflect.deleteProperty(HTMLElement.prototype, "animate");
  document.body.replaceChildren();
});

describe("Forum Khala board assembly (#8849)", () => {
  test("animates only the inert decoration after semantic content exists", async () => {
    const { container, decoration } = board();
    const driver = controlledAnimate();
    const visibility = controlledVisibility();
    const receipts: ForumKhalaAssemblyReceipt[] = [];
    driver.install();
    const scope = await Effect.runPromise(Scope.make());
    try {
      await Effect.runPromise(
        Scope.provide(scope)(
          mountForumBoardAssembly(container, {
            onReceipt: (receipt) => receipts.push(receipt),
            visibility: visibility.visibility,
          }),
        ),
      );
      expect(container.textContent).toContain("OpenAgents Forum");
      expect(container.querySelector("a")?.getAttribute("href")).toBe("/forum/f/product-promises");
      expect(decoration.getAttribute("aria-hidden")).toBe("true");
      await vi.waitFor(() => expect(driver.animations).toHaveLength(1));
      expect(driver.maximumActive()).toBe(1);
      expect(visibility.activeListeners()).toBe(1);
      expect(receipts[0]).toEqual({
        activeDrivers: 1,
        phase: "started",
        scheduledWork: 1,
        state: "entering",
      });
      driver.animations[0]?.finish();
      await vi.waitFor(() => {
        expect(decoration.dataset.khalaMotion).toBe("settled");
      });
      expect(decoration.style.opacity).toBe("1");
      expect(visibility.activeListeners()).toBe(0);
      expect(receipts.at(-1)).toEqual({
        activeDrivers: 0,
        phase: "settled",
        scheduledWork: 0,
        state: "entered",
      });
    } finally {
      await Effect.runPromise(Scope.close(scope, Exit.void));
    }
  });

  test.each([
    ["reduced", false, true, "reduced-static"],
    ["hidden", true, false, "hidden-static"],
  ] as const)(
    "%s mode allocates no animation, listener, or scheduled transition work",
    async (_name, initiallyHidden, reducedMotion, phase) => {
      const { container, decoration } = board();
      const driver = controlledAnimate();
      const visibility = controlledVisibility(initiallyHidden);
      const receipts: ForumKhalaAssemblyReceipt[] = [];
      driver.install();
      const scope = await Effect.runPromise(Scope.make());
      try {
        await Effect.runPromise(
          Scope.provide(scope)(
            mountForumBoardAssembly(container, {
              onReceipt: (receipt) => receipts.push(receipt),
              reducedMotion,
              visibility: visibility.visibility,
            }),
          ),
        );
        expect(driver.animations).toHaveLength(0);
        expect(visibility.activeListeners()).toBe(0);
        expect(decoration.style.opacity).toBe("1");
        expect(receipts).toEqual([
          {
            activeDrivers: 0,
            phase,
            scheduledWork: 0,
            state: "entered",
          },
        ]);
      } finally {
        await Effect.runPromise(Scope.close(scope, Exit.void));
      }
    },
  );

  test("visibility interruption cancels work, settles the frame, and removes its listener", async () => {
    const { container, decoration } = board();
    const driver = controlledAnimate();
    const visibility = controlledVisibility();
    const receipts: ForumKhalaAssemblyReceipt[] = [];
    driver.install();
    const scope = await Effect.runPromise(Scope.make());
    try {
      await Effect.runPromise(
        Scope.provide(scope)(
          mountForumBoardAssembly(container, {
            onReceipt: (receipt) => receipts.push(receipt),
            visibility: visibility.visibility,
          }),
        ),
      );
      visibility.hide();
      await vi.waitFor(() => {
        expect(decoration.dataset.khalaMotion).toBe("settled");
      });
      expect(driver.active()).toBe(0);
      expect(visibility.activeListeners()).toBe(0);
      expect(decoration.style.opacity).toBe("1");
      expect(receipts.at(-1)?.scheduledWork).toBe(0);
    } finally {
      await Effect.runPromise(Scope.close(scope, Exit.void));
    }
  });

  test("scope disposal cancels an interrupted driver with no post-disposal work", async () => {
    const first = board();
    const driver = controlledAnimate();
    const visibility = controlledVisibility();
    driver.install();
    const firstScope = await Effect.runPromise(Scope.make());
    await Effect.runPromise(
      Scope.provide(firstScope)(
        mountForumBoardAssembly(first.container, {
          visibility: visibility.visibility,
        }),
      ),
    );
    await vi.waitFor(() => expect(driver.active()).toBe(1));
    await Effect.runPromise(Scope.close(firstScope, Exit.void));
    expect(driver.active()).toBe(0);
    expect(visibility.activeListeners()).toBe(0);

    const second = board();
    const secondScope = await Effect.runPromise(Scope.make());
    try {
      await Effect.runPromise(
        Scope.provide(secondScope)(
          mountForumBoardAssembly(second.container, {
            visibility: visibility.visibility,
          }),
        ),
      );
      expect(driver.maximumActive()).toBe(1);
      await vi.waitFor(() => expect(driver.active()).toBe(1));
      expect(forumBoardAssemblyDurationMillis).toBeGreaterThanOrEqual(150);
      expect(forumBoardAssemblyDurationMillis).toBeLessThanOrEqual(350);
    } finally {
      await Effect.runPromise(Scope.close(secondScope, Exit.void));
    }
    expect(driver.active()).toBe(0);
    expect(visibility.activeListeners()).toBe(0);
  });

  test("uses a zero-work stable frame when Web Animations is unavailable", async () => {
    const { container, decoration } = board();
    const receipts: ForumKhalaAssemblyReceipt[] = [];
    await Effect.runPromise(
      mountForumBoardAssembly(container, {
        onReceipt: (receipt) => receipts.push(receipt),
      }).pipe(Effect.scoped),
    );
    expect(decoration.style.opacity).toBe("1");
    expect(receipts).toEqual([
      {
        activeDrivers: 0,
        phase: "unsupported-static",
        scheduledWork: 0,
        state: "entered",
      },
    ]);
  });
});
