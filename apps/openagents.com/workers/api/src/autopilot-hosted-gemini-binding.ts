/**
 * Single composition root for the hosted Gemini Autopilot executor binding.
 *
 * Advances blocker.product_promises.production_hosted_gemini_executor_binding_missing
 * on api.hosted_gemini.v1. All four layers of the hosted Gemini chain already
 * existed as separate, individually-tested factories:
 *
 *   InferenceProviderAdapter (makeVertexGeminiAdapter)
 *     -> createHostedGeminiRequestRunner   (work order -> InferenceRequest -> Effect)
 *     -> createVertexGeminiHostedCaller     (InferenceResult -> public-safe REFS)
 *     -> createHostedGeminiWorkExecutor     (REFS -> AutopilotWorkExecutor)
 *
 * The MISSING piece was the connective tissue that assembles those four layers
 * into ONE `AutopilotWorkExecutor` behind ONE arming flag, so a live worker can
 * bind hosted Gemini to `dependencies.executeReadyWork` with a single call (and
 * a single flag to flip) rather than hand-wiring the chain at the call site.
 * This module is exactly that composition root.
 *
 * HONEST / INERT BY CONSTRUCTION:
 *   - The single `enabled` flag is propagated to EVERY layer (defense in depth):
 *     with `enabled: false` the runner never touches the adapter, the caller
 *     never runs inference, and the executor returns `undefined` — exactly the
 *     current production behaviour (no execution, no closeout). Wiring this into
 *     the worker graph changes nothing until an operator arms the one flag.
 *   - The provider adapter is INJECTED, so this module never reaches live Vertex
 *     quota by itself: a caller hands it a configured `makeVertexGeminiAdapter`.
 *   - It carries NO secrets, NO provider credentials, and NO raw model output:
 *     the only refs that reach a closeout are the public-safe ones the bridge
 *     projects (model ref, SHA-256 response-digest ref, token-count usage ref),
 *     each re-validated by the executor against the route's public-safe guard.
 *   - It does not settle, spend, pay out, or imply accepted work.
 *
 * It does NOT clear the blocker: this composition root is still INJECTED (it is
 * not yet bound in the live worker dependency graph behind an env-gated flag),
 * the upstream ref-resolver that dereferences task/acceptance refs into the real
 * content the adapter should act on is still missing, and there is no
 * registered-agent production smoke. See
 * docs/launch/vertex-fleet/api.hosted_gemini.v1.md.
 */
import type { HostedGeminiRefContentResolver } from './autopilot-hosted-gemini-content-resolver'
import type { HostedGeminiContentDigest } from './autopilot-hosted-gemini-inference-bridge'
import { createVertexGeminiHostedCaller } from './autopilot-hosted-gemini-inference-bridge'
import { createHostedGeminiRequestRunner } from './autopilot-hosted-gemini-request-runner'
import { createHostedGeminiWorkExecutor } from './autopilot-hosted-gemini-executor'
import type { AutopilotWorkExecutor } from './autopilot-work-routes'
import type { InferenceProviderAdapter } from './inference/provider-adapter'

export type HostedGeminiExecutorBindingConfig = Readonly<{
  /** Injected provider adapter (e.g. `makeVertexGeminiAdapter(...)`). */
  adapter: InferenceProviderAdapter
  /**
   * Single default-OFF arming flag for the WHOLE chain. When false every layer
   * is INERT: the resulting executor returns `undefined` and the adapter is
   * never invoked.
   */
  enabled: boolean
  /** Override the content digest (defaults to SHA-256 via WebCrypto). */
  digest?: HostedGeminiContentDigest | undefined
  /** Output-token ceiling for the hosted Gemini generation. */
  maxOutputTokens?: number | undefined
  /** Requested Gemini model alias. */
  model?: string | undefined
  /**
   * Optional INJECTED resolver that dereferences the work order's task +
   * acceptance refs into public-safe content. When provided (and the chain is
   * armed) the request runner embeds the resolved content so the adapter acts
   * on the real task; when absent the chain keeps its existing refs-only frame.
   * Threading it through this single factory is the only way a deployment can
   * provision a live datastore-backed resolver without hand-wiring the chain.
   */
  resolveRefContent?: HostedGeminiRefContentResolver | undefined
}>

/**
 * Build the fully-composed hosted Gemini `AutopilotWorkExecutor`. Wire the
 * returned function to `dependencies.executeReadyWork` to serve `hosted_gemini`
 * placements end-to-end from a single injected adapter + a single arming flag.
 */
export const createHostedGeminiExecutorBinding = (
  config: HostedGeminiExecutorBindingConfig,
): AutopilotWorkExecutor => {
  const runInference = createHostedGeminiRequestRunner({
    adapter: config.adapter,
    enabled: config.enabled,
    maxOutputTokens: config.maxOutputTokens,
    model: config.model,
    resolveRefContent: config.resolveRefContent,
  })
  const inferenceCaller = createVertexGeminiHostedCaller({
    digest: config.digest,
    enabled: config.enabled,
    runInference,
  })
  return createHostedGeminiWorkExecutor({
    enabled: config.enabled,
    inferenceCaller,
  })
}
