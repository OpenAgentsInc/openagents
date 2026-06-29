/**
 * Env-gated resolver that binds the composed hosted Gemini executor
 * (`createHostedGeminiExecutorBinding`) to the live worker `Env`, so the
 * Autopilot route harness can wire it to `dependencies.executeReadyWork`.
 *
 * Advances blocker.product_promises.production_hosted_gemini_executor_binding_missing
 * on api.hosted_gemini.v1. The composition root already assembled the four-layer
 * chain from ONE injected adapter + ONE arming flag, but that flag was a plain
 * boolean: nothing read it off the worker `Env`, so binding hosted Gemini in the
 * live worker still required hand-deciding "armed?" and hand-building the Vertex
 * adapter at the `index.ts` call site. This module is exactly that env seam:
 *
 *   Env  ->  hostedGeminiExecutorArmed(env)  ->  (armed?) ->
 *            makeVertexGeminiAdapter(VERTEX_SA_KEY,...)    ->
 *            createHostedGeminiExecutorBinding({ enabled: true, ... })  ->
 *            AutopilotWorkExecutor
 *
 * HONEST / INERT BY CONSTRUCTION:
 *   - DOUBLE-GATED + INERT by default: the executor is armed ONLY when the
 *     explicit `HOSTED_GEMINI_EXECUTOR_ENABLED` flag is on AND the
 *     `VERTEX_SA_KEY` worker secret is present. Missing either => the resolver
 *     returns `undefined` (no executor, no closeout) — exactly the current
 *     production behaviour. Wiring this into the worker graph changes nothing
 *     until an operator BOTH arms the flag AND provisions the secret.
 *   - It carries NO secrets in its output: the secret is read straight into the
 *     adapter's token provider and never logged, returned, or placed in a ref.
 *     The only refs that reach a closeout are the public-safe ones the bridge
 *     projects (model ref, SHA-256 response-digest ref, token-count usage ref).
 *   - It does not settle, spend, pay out, or imply accepted work.
 *
 * It does NOT clear the blocker: even when bound in `index.ts`, the upstream
 * ref-resolver that dereferences task/acceptance refs into the real content the
 * adapter should act on is still missing (the runner builds a refs-only prompt),
 * and there is no registered-agent production smoke. See
 * docs/launch/vertex-fleet/api.hosted_gemini.v1.md.
 */
import { Effect } from 'effect'

import { createHostedGeminiExecutorBinding } from './autopilot-hosted-gemini-binding'
import type { HostedGeminiRefContentResolver } from './autopilot-hosted-gemini-content-resolver'
import type { AutopilotWorkExecutor } from './autopilot-work-routes'
import type { InferenceProviderAdapter } from './inference/provider-adapter'
import { InferenceAdapterError } from './inference/provider-adapter'
import {
  VERTEX_GEMINI_ADAPTER_ID,
  makeVertexGeminiAdapter,
} from './inference/vertex-gemini-adapter'
import { tokenProviderFromSecret } from './inference/vertex-token'

/** Default Vertex project/location the hosted Gemini lane pins (gateway §3a). */
const DEFAULT_HOSTED_GEMINI_PROJECT = 'openagentsgemini'
const DEFAULT_HOSTED_GEMINI_LOCATION = 'global'

/** Worker `Env` subset this resolver reads. */
export type HostedGeminiExecutorEnv = Readonly<{
  /** Default-OFF arming flag. Only "1"/"true"/"yes"/"on" arms the executor. */
  HOSTED_GEMINI_EXECUTOR_ENABLED?: string | undefined
  /** Full GCP service-account key JSON (worker SECRET; never logged/returned). */
  VERTEX_SA_KEY?: string | undefined
  /** Optional Vertex project override (defaults to openagentsgemini). */
  VERTEX_PROJECT_ID?: string | undefined
  /** Optional Vertex location override (defaults to global). */
  VERTEX_LOCATION?: string | undefined
  /** Optional Gemini model alias override. */
  HOSTED_GEMINI_MODEL?: string | undefined
}>

const flagEnabled = (value: string | undefined): boolean =>
  value !== undefined &&
  ['1', 'on', 'true', 'yes'].includes(value.trim().toLowerCase())

const secretPresent = (value: string | undefined): boolean =>
  value !== undefined && value.trim() !== ''

/**
 * The hosted Gemini executor is armed ONLY when the explicit flag is on AND the
 * Vertex service-account secret is present. Either missing => INERT.
 */
export const hostedGeminiExecutorArmed = (
  env: HostedGeminiExecutorEnv,
): boolean =>
  flagEnabled(env.HOSTED_GEMINI_EXECUTOR_ENABLED) &&
  secretPresent(env.VERTEX_SA_KEY)

/**
 * Build the real Vertex Gemini provider adapter from the worker env. The token
 * provider is minted from `VERTEX_SA_KEY`; with no secret it returns a typed,
 * non-retryable error (the chain folds that into a declined closeout).
 */
const buildVertexGeminiAdapterFromEnv = (
  env: HostedGeminiExecutorEnv,
): InferenceProviderAdapter =>
  makeVertexGeminiAdapter({
    location: env.VERTEX_LOCATION ?? DEFAULT_HOSTED_GEMINI_LOCATION,
    project: env.VERTEX_PROJECT_ID ?? DEFAULT_HOSTED_GEMINI_PROJECT,
    resolveModelId: undefined,
    tokenProvider: () => {
      const provider = tokenProviderFromSecret(env.VERTEX_SA_KEY)
      return provider === undefined
        ? Effect.fail(
            new InferenceAdapterError({
              adapterId: VERTEX_GEMINI_ADAPTER_ID,
              reason:
                'Hosted Gemini executor is not configured (missing VERTEX_SA_KEY).',
              retryable: false,
            }),
          )
        : provider()
    },
  })

export type HostedGeminiExecuteReadyWorkDeps = Readonly<{
  /** Override the adapter builder (tests inject a spy adapter). */
  buildAdapter?: (env: HostedGeminiExecutorEnv) => InferenceProviderAdapter
  /**
   * Optional INJECTED resolver that dereferences the work order's task +
   * acceptance refs into public-safe content for the prompt. A deployment with a
   * datastore-backed resolver passes it here; when omitted the bound executor
   * keeps the existing refs-only frame (current production behaviour).
   */
  resolveRefContent?: HostedGeminiRefContentResolver | undefined
}>

/**
 * Resolve an `AutopilotWorkExecutor` for the given env, or `undefined` when the
 * executor is not armed. Wire the returned `executeReadyWork` callback to
 * `dependencies.executeReadyWork` in the live worker; it stays INERT until the
 * env both arms the flag and provisions the secret.
 */
export const resolveHostedGeminiExecutor = (
  env: HostedGeminiExecutorEnv,
  deps: HostedGeminiExecuteReadyWorkDeps = {},
): AutopilotWorkExecutor | undefined => {
  if (!hostedGeminiExecutorArmed(env)) {
    return undefined
  }
  const buildAdapter = deps.buildAdapter ?? buildVertexGeminiAdapterFromEnv
  return createHostedGeminiExecutorBinding({
    adapter: buildAdapter(env),
    enabled: true,
    model: env.HOSTED_GEMINI_MODEL,
    resolveRefContent: deps.resolveRefContent,
  })
}

/**
 * Build the `(env, input) => Promise<closeout | undefined>` callback shape the
 * route harness's `dependencies.executeReadyWork` expects. INERT (resolves to
 * `undefined` without touching the adapter) whenever the env is not armed.
 */
export const makeHostedGeminiExecuteReadyWork =
  (deps: HostedGeminiExecuteReadyWorkDeps = {}) =>
  (
    env: HostedGeminiExecutorEnv,
    input: Parameters<AutopilotWorkExecutor>[0],
  ): ReturnType<AutopilotWorkExecutor> => {
    const executor = resolveHostedGeminiExecutor(env, deps)
    return executor === undefined ? Promise.resolve(undefined) : executor(input)
  }
