import { Effect } from "effect"

import { getAppLayer, makeAppRuntime } from "../effect/runtime"
import type { AppConfig } from "../effect/config"

import type { WorkerEnv } from "./env"

const parsePrelaunch = (v: string | undefined): boolean =>
  v === "1" || v === "true" || v === "yes"

export const getWorkerAppConfig = (env: WorkerEnv): AppConfig => {
  const convexUrl = env.VITE_CONVEX_URL ?? process.env.VITE_CONVEX_URL
  if (!convexUrl) {
    throw new Error("missing VITE_CONVEX_URL (Worker env var)")
  }
  const prelaunch = parsePrelaunch(env.VITE_PRELAUNCH ?? process.env.VITE_PRELAUNCH)
  const prelaunchBypassKey =
    (env.PRELAUNCH_BYPASS_KEY ?? process.env.PRELAUNCH_BYPASS_KEY) ?? null
  return { convexUrl, prelaunch, prelaunchBypassKey }
}

/**
 * Shared runtime for the Worker host.
 *
 * We still run request-scoped programs by overriding `RequestContextService`
 * at the edge (SSR, RPC, auth, DO proxy endpoints).
 */
export const getWorkerRuntime = (env: WorkerEnv) => {
  const config = getWorkerAppConfig(env)
  const runtime = makeAppRuntime(config)
  const layer = getAppLayer(config)
  return { config, runtime, memoMap: runtime.memoMap, layer }
}

/**
 * Small helper for running an Effect and ensuring we always return a Response
 * on unexpected failure.
 */
export const runOr500 = <T>(
  effect: Effect.Effect<T, unknown, never>,
  onSuccess: (value: T) => Response,
): Promise<Response> =>
  Effect.runPromise(
    effect.pipe(
      Effect.match({
        onFailure: (error) =>
          new Response(JSON.stringify({ error: String(error) }), {
            status: 500,
            headers: { "content-type": "application/json; charset=utf-8" },
          }),
        onSuccess,
      }),
    ),
  )
