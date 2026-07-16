import {
  makeKhalaCanvasBackground,
  type KhalaBackgroundQuality,
  type KhalaCanvasBackgroundPolicy,
  type KhalaCanvasFrameScheduler,
} from "@effect-native/render-canvas";
import { useEffectNativeScopedEffect } from "@effect-native/render-dom/react";
import { Effect, Scope } from "effect";
import { useRef, type ReactElement } from "react";

export const splashCanvasDurationMillis = 1_200;
export const splashCanvasFrameCostBudgetMillis = 8;

type SplashCanvasStopReason = "duration-budget" | "frame-cost-budget";

export type SplashCanvasSchedulerReceipt = Readonly<{
  frameCostViolations: number;
  pendingFrames: number;
  requestedFrames: number;
  stopReason?: SplashCanvasStopReason;
}>;

export type SplashCanvasDependencies = Readonly<{
  frameCostBudgetMillis?: number;
  getSize?: () => Readonly<{ dpr: number; height: number; width: number }>;
  now?: () => number;
  onSchedulerReceipt?: (receipt: SplashCanvasSchedulerReceipt) => void;
  policy?: Partial<KhalaCanvasBackgroundPolicy>;
  scheduler?: KhalaCanvasFrameScheduler;
  sequenceDurationMillis?: number;
}>;

const browserScheduler = (): KhalaCanvasFrameScheduler => ({
  cancel: (handle) => window.cancelAnimationFrame(handle),
  request: (callback) => window.requestAnimationFrame(callback),
});

export const makeBoundedSplashCanvasScheduler = (
  scheduler: KhalaCanvasFrameScheduler,
  now: () => number,
  durationMillis: number,
  frameCostBudgetMillis: number,
  onReceipt?: (receipt: SplashCanvasSchedulerReceipt) => void,
): KhalaCanvasFrameScheduler => {
  const state: {
    frameCostViolations: number;
    pending: Set<number>;
    requestedFrames: number;
    startTimestamp?: number;
    stopReason?: SplashCanvasStopReason;
  } = {
    frameCostViolations: 0,
    pending: new Set<number>(),
    requestedFrames: 0,
  };
  const receipt = (): void =>
    onReceipt?.({
      frameCostViolations: state.frameCostViolations,
      pendingFrames: state.pending.size,
      requestedFrames: state.requestedFrames,
      ...(state.stopReason === undefined ? {} : { stopReason: state.stopReason }),
    });
  const stop = (reason: SplashCanvasStopReason): void => {
    if (state.stopReason !== undefined) return;
    state.stopReason = reason;
    state.pending.forEach((handle) => scheduler.cancel(handle));
    state.pending.clear();
    receipt();
  };

  return {
    cancel: (handle) => {
      if (!state.pending.delete(handle)) return;
      scheduler.cancel(handle);
      receipt();
    },
    request: (callback) => {
      if (state.stopReason !== undefined) return 0;
      const ticket = { handle: 0 };
      ticket.handle = scheduler.request((timestamp) => {
        state.pending.delete(ticket.handle);
        state.startTimestamp ??= timestamp;
        const frameStarted = now();
        callback(timestamp);
        const frameCost = Math.max(0, now() - frameStarted);
        if (frameCost > frameCostBudgetMillis) state.frameCostViolations += 1;
        if (state.frameCostViolations >= 2) stop("frame-cost-budget");
        else if (timestamp - state.startTimestamp >= durationMillis) stop("duration-budget");
        else receipt();
      });
      state.pending.add(ticket.handle);
      state.requestedFrames += 1;
      receipt();
      return ticket.handle;
    },
  };
};

const browserPolicy = (): Partial<KhalaCanvasBackgroundPolicy> => {
  const reducedMotion =
    typeof window.matchMedia === "function" &&
    window.matchMedia("(prefers-reduced-motion: reduce)").matches;
  const navigatorWithCapacity = navigator as Navigator &
    Readonly<{
      connection?: Readonly<{ saveData?: boolean }>;
      deviceMemory?: number;
    }>;
  const constrained =
    navigatorWithCapacity.connection?.saveData === true ||
    (navigatorWithCapacity.deviceMemory ?? 8) <= 4;
  const quality: KhalaBackgroundQuality = constrained ? "constrained" : "balanced";
  return {
    power: constrained ? "low" : "normal",
    quality,
    reducedMotion,
  };
};

export const mountSplashHeroCanvas = (
  canvas: HTMLCanvasElement,
  dependencies: SplashCanvasDependencies = {},
): Effect.Effect<void, never, Scope.Scope> => {
  const now = dependencies.now ?? (() => performance.now());
  const requestedDuration = dependencies.sequenceDurationMillis ?? splashCanvasDurationMillis;
  const durationMillis = Number.isFinite(requestedDuration)
    ? Math.min(8_000, Math.max(120, requestedDuration))
    : splashCanvasDurationMillis;
  const animationStarted = now();
  const animationNow = (): number =>
    animationStarted + ((now() - animationStarted) * 8_000) / durationMillis;
  const policy = { ...browserPolicy(), ...dependencies.policy };
  const scheduler = makeBoundedSplashCanvasScheduler(
    dependencies.scheduler ?? browserScheduler(),
    now,
    durationMillis,
    dependencies.frameCostBudgetMillis ?? splashCanvasFrameCostBudgetMillis,
    (receipt) => {
      if (receipt.stopReason !== undefined) {
        canvas.dataset.khalaCanvas =
          receipt.stopReason === "duration-budget" ? "duration-settled" : "frame-budget-static";
      }
      dependencies.onSchedulerReceipt?.(receipt);
    },
  );

  return makeKhalaCanvasBackground(
    canvas,
    {
      color: "rgba(116, 164, 255, 0.48)",
      kind: "dots",
      origin: [0.5, 0.42],
      seed: 8850,
      shape: "cross",
      size: 1.35,
      spacing: 44,
    },
    {
      ...(dependencies.getSize === undefined ? {} : { getSize: dependencies.getSize }),
      maxActiveSurfaces: 1,
      now: animationNow,
      policy,
      scheduler,
    },
  ).pipe(
    Effect.tap(() =>
      Effect.sync(() => {
        canvas.dataset.khalaCanvas =
          policy.reducedMotion === true
            ? "reduced-static"
            : policy.quality === "constrained" || policy.power === "low"
              ? "constrained-static"
              : "active";
      }),
    ),
    Effect.asVoid,
    Effect.catchCause(() =>
      Effect.sync(() => {
        canvas.dataset.khalaCanvas = "unsupported-static";
      }),
    ),
  );
};

export function SplashHeroCanvas(): ReactElement {
  const canvas = useRef<HTMLCanvasElement>(null);

  useEffectNativeScopedEffect(() => {
    const element = canvas.current;
    return element === null ? Effect.void : mountSplashHeroCanvas(element);
  }, []);

  return (
    <canvas
      aria-hidden="true"
      className="splash-hero-canvas"
      data-khala-canvas="server-static"
      ref={canvas}
    />
  );
}
