import type { V3 } from "../renderer/math.ts";
import type { DruidAbility } from "./druid-abilities.ts";

export interface SpellForecastRequest {
  castId: string;
  ability: DruidAbility;
  origin: V3;
  direction: V3;
  expectedVersion?: number;
}
export interface SpellForecastPlan {
  version: number;
  prospectiveDestroyed: number[];
  sourceIds: number[];
  cast: { castId: string; ability: DruidAbility; [key: string]: unknown };
  aim: unknown;
}
export interface SpellForecastMetrics {
  beginMs: number;
  longestContinueMs: number;
  longestTickMs: number;
  totalCpuMs: number;
  elapsedMs: number;
  continueCalls: number;
  ticks: number;
  yields: number;
  restarts: number;
}
export interface SpellForecastOptions {
  cancelled?: () => boolean;
  signal?: AbortSignal;
  yieldControl?: () => Promise<void>;
  /** Yield between native requests after this wall-time budget, never inside Wasm. */
  budgetMs?: number;
  now?: () => number;
  /** Prepare invisible fragment buffers, then revalidate against live native timing. */
  prepareVisuals?: (
    plan: SpellForecastPlan,
  ) => boolean | void | Promise<boolean | void>;
}
interface KernelRequests {
  request(message: object): unknown;
}
interface Progress {
  ok: boolean;
  forecastId: number;
  version: number;
  status: "pending" | "ready";
  phase: "cast" | "castle" | "events";
  ticks: number;
  restarts: number;
  error?: { message?: string };
  [key: string]: unknown;
}
async function yieldTask(): Promise<void> {
  const scheduler = (
    globalThis as typeof globalThis & {
      scheduler?: { yield(): Promise<void> };
    }
  ).scheduler;
  if (typeof scheduler?.yield === "function") await scheduler.yield();
  else await new Promise<void>((resolve) => setTimeout(resolve, 0));
}
function checked(reply: Progress): Progress {
  if (!reply?.ok)
    throw new Error(
      reply?.error?.message ?? "The spell could not be prepared.",
    );
  if (
    !Number.isSafeInteger(reply.forecastId) ||
    reply.forecastId < 1 ||
    !Number.isSafeInteger(reply.version) ||
    reply.version < 0 ||
    !["pending", "ready"].includes(reply.status) ||
    !["cast", "castle", "events"].includes(reply.phase) ||
    !Number.isInteger(reply.ticks) ||
    reply.ticks < 0 ||
    reply.ticks > 600 ||
    !Number.isInteger(reply.restarts) ||
    reply.restarts < 0 ||
    reply.restarts > 16
  )
    throw new Error("The spell returned an incomplete preparation result.");
  return reply;
}
function readyPlan(
  reply: Progress,
  request: SpellForecastRequest,
): SpellForecastPlan {
  const ids = (value: unknown): value is number[] =>
    Array.isArray(value) &&
    value.length <= 65_536 &&
    value.every(
      (id) => Number.isSafeInteger(id) && id >= 0 && id <= 0xffff_ffff,
    ) &&
    new Set(value).size === value.length;
  const cast = reply.cast as SpellForecastPlan["cast"] | undefined;
  if (
    !ids(reply.prospectiveDestroyed) ||
    !ids(reply.sourceIds) ||
    !cast ||
    cast.castId !== request.castId ||
    cast.ability !== request.ability ||
    !("aim" in reply)
  )
    throw new Error("The spell returned an incomplete preparation result.");
  return {
    version: reply.version,
    prospectiveDestroyed: reply.prospectiveDestroyed,
    sourceIds: reply.sourceIds,
    cast,
    aim: reply.aim,
  };
}

/** Live gameplay keeps running; only a ready native capability can reach the cast commit. */
export async function prepareGameplayCast(
  kernel: KernelRequests,
  request: SpellForecastRequest,
  options: SpellForecastOptions = {},
): Promise<{ plan: SpellForecastPlan; metrics: SpellForecastMetrics }> {
  const budget = options.budgetMs ?? 2;
  if (!Number.isFinite(budget) || budget <= 0 || budget > 8)
    throw new Error(
      "Use a spell preparation budget above zero and at most eight milliseconds.",
    );
  const now = options.now ?? (() => performance.now()),
    yieldControl = options.yieldControl ?? yieldTask,
    started = now(),
    metrics: SpellForecastMetrics = {
      beginMs: 0,
      longestContinueMs: 0,
      longestTickMs: 0,
      totalCpuMs: 0,
      elapsedMs: 0,
      continueCalls: 0,
      ticks: 0,
      yields: 0,
      restarts: 0,
    };
  let forecastId: number | undefined,
    completed = false;
  const cancelled = () => {
    if (options.signal?.aborted || options.cancelled?.())
      throw new Error("The spell was canceled.");
  };
  try {
    cancelled();
    const begin = now();
    const initial = kernel.request({
      type: "gameplay_forecast_start",
      ...request,
    }) as Progress;
    if (
      initial?.ok &&
      Number.isSafeInteger(initial.forecastId) &&
      initial.forecastId > 0
    )
      forecastId = initial.forecastId;
    let progress = checked(initial);
    metrics.beginMs = now() - begin;
    metrics.totalCpuMs += metrics.beginMs;
    if (
      progress.status !== "pending" ||
      progress.phase !== "cast" ||
      progress.ticks !== 0 ||
      progress.restarts !== 0
    )
      throw new Error("The spell returned an incomplete preparation result.");
    let batchStarted = begin,
      lastContinueStarted = begin;
    const continueForecast = () => {
      if (metrics.continueCalls >= 604 * 17)
        throw new Error("The spell exceeded its preparation work limit.");
      const previousTicks = progress.ticks,
        previousRestarts = progress.restarts,
        before = now();
      lastContinueStarted = before;
      const next = checked(
        kernel.request({
          type: "gameplay_forecast_continue",
          forecastId,
          expectedVersion: progress.version,
        }) as Progress,
      );
      const elapsed = now() - before;
      metrics.continueCalls++;
      metrics.totalCpuMs += elapsed;
      metrics.longestContinueMs = Math.max(metrics.longestContinueMs, elapsed);
      const restarted =
        next.restarts === previousRestarts + 1 &&
        next.ticks === 0 &&
        next.phase === "cast" &&
        next.status === "pending";
      if (
        next.forecastId !== forecastId ||
        next.version !== progress.version ||
        (!restarted &&
          (next.restarts !== previousRestarts ||
            next.ticks < previousTicks ||
            next.ticks > previousTicks + 1))
      )
        throw new Error("The spell returned an incomplete preparation result.");
      if (!restarted && next.ticks > previousTicks) {
        metrics.longestTickMs = Math.max(metrics.longestTickMs, elapsed);
        metrics.ticks += next.ticks - previousTicks;
      }
      metrics.restarts = next.restarts;
      progress = next;
    };
    for (;;) {
      cancelled();
      if ((progress as Progress).status === "ready") {
        let plan = readyPlan(progress, request);
        if (options.prepareVisuals) {
          const prepared = await options.prepareVisuals(plan);
          cancelled();
          if (prepared === false)
            throw new Error("The spell's fragments could not be prepared.");
          continueForecast();
          if ((progress as Progress).status === "pending") {
            batchStarted = lastContinueStarted;
            continue;
          }
          plan = readyPlan(progress, request);
        }
        cancelled();
        completed = true;
        metrics.elapsedMs = now() - started;
        return { plan, metrics };
      }
      if (now() - batchStarted >= budget) {
        metrics.yields++;
        await yieldControl();
        cancelled();
        batchStarted = now();
      }
      continueForecast();
    }
  } finally {
    if (!completed && forecastId !== undefined) {
      try {
        kernel.request({ type: "gameplay_forecast_cancel", forecastId });
      } catch {
        /* A closed or replaced kernel cannot retain this preparation. */
      }
    }
  }
}
