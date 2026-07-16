import { Effect, Exit, Scope } from "@effect-native/core/effect";
import { afterEach, describe, expect, test, vi } from "vitest";

import {
  makeBoundedSplashCanvasScheduler,
  mountSplashHeroCanvas,
  splashCanvasDurationMillis,
  splashCanvasFrameCostBudgetMillis,
  type SplashCanvasSchedulerReceipt,
} from "./-splash-khala-canvas";

const controlledScheduler = (): Readonly<{
  flush: (timestamp: number) => void;
  maximumPending: () => number;
  pending: () => number;
  scheduler: Readonly<{
    cancel: (handle: number) => void;
    request: (callback: FrameRequestCallback) => number;
  }>;
}> => {
  const callbacks = new Map<number, FrameRequestCallback>();
  const state = { maximumPending: 0, nextHandle: 1 };
  return {
    flush: (timestamp) => {
      const callbacksToRun = [...callbacks.values()];
      callbacks.clear();
      callbacksToRun.forEach((callback) => callback(timestamp));
    },
    maximumPending: () => state.maximumPending,
    pending: () => callbacks.size,
    scheduler: {
      cancel: (handle) => {
        callbacks.delete(handle);
      },
      request: (callback) => {
        const handle = state.nextHandle;
        state.nextHandle += 1;
        callbacks.set(handle, callback);
        state.maximumPending = Math.max(state.maximumPending, callbacks.size);
        return handle;
      },
    },
  };
};

const canvasContext = (): CanvasRenderingContext2D =>
  ({
    arc: vi.fn(),
    beginPath: vi.fn(),
    clearRect: vi.fn(),
    createRadialGradient: vi.fn(() => ({ addColorStop: vi.fn() })),
    fill: vi.fn(),
    fillRect: vi.fn(),
    lineTo: vi.fn(),
    moveTo: vi.fn(),
    setLineDash: vi.fn(),
    setTransform: vi.fn(),
    stroke: vi.fn(),
  }) as unknown as CanvasRenderingContext2D;

const canvas = (context: CanvasRenderingContext2D | null = canvasContext()): HTMLCanvasElement => {
  const element = document.createElement("canvas");
  vi.spyOn(element, "getContext").mockReturnValue(context);
  document.body.appendChild(element);
  return element;
};

afterEach(() => {
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
  document.body.replaceChildren();
});

describe("Splash bounded Khala Canvas pilot (#8850)", () => {
  test("runs one bounded reveal and leaves no scheduled frame after its duration budget", async () => {
    const element = canvas();
    const frames = controlledScheduler();
    const clock = { now: 0 };
    const size = { dpr: 2, height: 480, width: 1_440 };
    const receipts: SplashCanvasSchedulerReceipt[] = [];
    const scope = await Effect.runPromise(Scope.make());
    try {
      await Effect.runPromise(
        Scope.provide(scope)(
          mountSplashHeroCanvas(element, {
            getSize: () => size,
            now: () => clock.now,
            onSchedulerReceipt: (receipt) => receipts.push(receipt),
            policy: {
              focused: true,
              offscreen: false,
              power: "normal",
              quality: "balanced",
              reducedMotion: false,
              visible: true,
            },
            scheduler: frames.scheduler,
          }),
        ),
      );
      expect(element.dataset.khalaCanvas).toBe("active");
      expect(element.width).toBe(2_160);
      expect(element.height).toBe(720);
      expect(frames.pending()).toBe(1);

      size.height = 400;
      size.width = 1_000;
      window.dispatchEvent(new Event("resize"));
      expect(element.width).toBe(1_500);
      expect(element.height).toBe(600);
      expect(frames.pending()).toBe(1);

      [0, 600, splashCanvasDurationMillis].forEach((timestamp) => {
        clock.now = timestamp;
        frames.flush(timestamp);
      });

      expect(frames.maximumPending()).toBe(1);
      expect(frames.pending()).toBe(0);
      expect(element.dataset.khalaCanvas).toBe("duration-settled");
      expect(receipts.at(-1)?.stopReason).toBe("duration-budget");
      expect(receipts.at(-1)?.frameCostViolations).toBe(0);
    } finally {
      await Effect.runPromise(Scope.close(scope, Exit.void));
    }
    expect(frames.pending()).toBe(0);
  });

  test.each([
    [1, 300],
    [1.5, 450],
    [2, 450],
  ])("resolves DPR %s through the balanced 1.5 cap", async (dpr, expectedWidth) => {
    const element = canvas();
    const frames = controlledScheduler();
    await Effect.runPromise(
      mountSplashHeroCanvas(element, {
        getSize: () => ({ dpr, height: 120, width: 300 }),
        policy: { quality: "balanced", reducedMotion: true },
        scheduler: frames.scheduler,
      }).pipe(Effect.scoped),
    );
    expect(element.width).toBe(expectedWidth);
    expect(element.height).toBe(expectedWidth * 0.4);
    expect(frames.pending()).toBe(0);
  });

  test.each([
    ["reduced", { reducedMotion: true, quality: "balanced" as const }, "reduced-static"],
    [
      "constrained",
      { reducedMotion: false, quality: "constrained" as const },
      "constrained-static",
    ],
    [
      "low-power",
      { reducedMotion: false, quality: "balanced" as const, power: "low" as const },
      "constrained-static",
    ],
  ])("%s policy renders a stable frame with no scheduler", async (_name, policy, state) => {
    const element = canvas();
    const frames = controlledScheduler();
    await Effect.runPromise(
      mountSplashHeroCanvas(element, {
        getSize: () => ({ dpr: 2, height: 480, width: 1_440 }),
        policy,
        scheduler: frames.scheduler,
      }).pipe(Effect.scoped),
    );
    expect(element.dataset.khalaCanvas).toBe(state);
    expect(frames.pending()).toBe(0);
  });

  test("falls back to the existing CSS hero when Canvas 2D is unavailable", async () => {
    const element = canvas(null);
    await Effect.runPromise(mountSplashHeroCanvas(element).pipe(Effect.scoped));
    expect(element.dataset.khalaCanvas).toBe("unsupported-static");
  });

  test("cancels offscreen work and resumes with one scheduler when visible again", async () => {
    const frames = controlledScheduler();
    const observerState: {
      callback?: IntersectionObserverCallback;
      disconnected: boolean;
    } = { disconnected: false };
    class ControlledIntersectionObserver {
      constructor(callback: IntersectionObserverCallback) {
        observerState.callback = callback;
      }
      disconnect(): void {
        observerState.disconnected = true;
      }
      observe(): void {}
    }
    vi.stubGlobal("IntersectionObserver", ControlledIntersectionObserver);
    const scope = await Effect.runPromise(Scope.make());
    await Effect.runPromise(
      Scope.provide(scope)(
        mountSplashHeroCanvas(canvas(), {
          policy: {
            focused: true,
            offscreen: false,
            power: "normal",
            quality: "balanced",
            reducedMotion: false,
            visible: true,
          },
          scheduler: frames.scheduler,
        }),
      ),
    );
    expect(frames.pending()).toBe(1);

    observerState.callback?.(
      [{ isIntersecting: false }] as unknown as Array<IntersectionObserverEntry>,
      {} as IntersectionObserver,
    );
    expect(frames.pending()).toBe(0);
    observerState.callback?.(
      [{ isIntersecting: true }] as unknown as Array<IntersectionObserverEntry>,
      {} as IntersectionObserver,
    );
    expect(frames.pending()).toBe(1);
    expect(frames.maximumPending()).toBe(1);

    await Effect.runPromise(Scope.close(scope, Exit.void));
    expect(frames.pending()).toBe(0);
    expect(observerState.disconnected).toBe(true);
  });

  test("stops after two over-budget frame callbacks", () => {
    const frames = controlledScheduler();
    const receipts: SplashCanvasSchedulerReceipt[] = [];
    const clock = { now: 0 };
    const scheduler = makeBoundedSplashCanvasScheduler(
      frames.scheduler,
      () => {
        const value = clock.now;
        clock.now += splashCanvasFrameCostBudgetMillis + 1;
        return value;
      },
      splashCanvasDurationMillis,
      splashCanvasFrameCostBudgetMillis,
      (receipt) => receipts.push(receipt),
    );
    const callback = vi.fn();
    scheduler.request(callback);
    frames.flush(0);
    frames.flush(16);
    expect(callback).toHaveBeenCalledTimes(1);
    scheduler.request(callback);
    frames.flush(32);
    expect(receipts.at(-1)?.stopReason).toBe("frame-cost-budget");
    expect(frames.pending()).toBe(0);
  });

  test("setup-cleanup-setup never leaves more than one scheduled frame", async () => {
    const frames = controlledScheduler();
    const firstScope = await Effect.runPromise(Scope.make());
    await Effect.runPromise(
      Scope.provide(firstScope)(
        mountSplashHeroCanvas(canvas(), {
          policy: { reducedMotion: false },
          scheduler: frames.scheduler,
        }),
      ),
    );
    expect(frames.pending()).toBe(1);
    await Effect.runPromise(Scope.close(firstScope, Exit.void));
    expect(frames.pending()).toBe(0);

    const secondScope = await Effect.runPromise(Scope.make());
    await Effect.runPromise(
      Scope.provide(secondScope)(
        mountSplashHeroCanvas(canvas(), {
          policy: { reducedMotion: false },
          scheduler: frames.scheduler,
        }),
      ),
    );
    expect(frames.pending()).toBe(1);
    expect(frames.maximumPending()).toBe(1);
    await Effect.runPromise(Scope.close(secondScope, Exit.void));
    expect(frames.pending()).toBe(0);
  });
});
