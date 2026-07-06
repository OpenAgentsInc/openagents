/**
 * Fail-soft `ARTIFACTS` (R2) binding resolution (#8516, precursor to the
 * CFG-8 R2→GCS migration in #8523).
 *
 * The account-level Cloudflare R2 feature was disabled during the
 * Cloudflare→GCP consolidation (#8515), which made every `wrangler deploy`
 * carrying an `r2_buckets` binding fail with API error 10136 — the same
 * deploy-freeze class as the Analytics Engine binding removed in #8516.
 * The `ARTIFACTS` binding was therefore removed from `wrangler.jsonc`
 * (prod + staging) and `WorkerBindings.ARTIFACTS` became optional.
 *
 * Every consumer resolves the bucket through {@link artifactsBucketForEnv}:
 * when the binding is present (a future GCS-backed BlobStore shim, local
 * dev with a real binding, or tests injecting a double) behavior is
 * unchanged; when absent, R2-dependent operations reject with the typed
 * {@link ArtifactsUnavailableError} instead of crashing at wiring time —
 * matching the failure mode those calls already had while the account
 * feature was disabled (every R2 API call failed). Artifact-dependent
 * features (trace blobs, raw Codex event archives, image generation,
 * site assets, packfile archives) degrade per-call; nothing on the
 * critical mobile/Khala Sync path reads through this binding.
 *
 * Do not re-add an `r2_buckets` binding — the replacement is the owned
 * BlobStore primitive on GCS (#8517/#8523).
 */

import { Data } from 'effect'

/** Typed rejection for R2 operations attempted while no ARTIFACTS
 * binding is configured (account-level R2 disabled, #8516). */
export class ArtifactsUnavailableError extends Data.TaggedError(
  'ArtifactsUnavailableError',
)<{
  readonly operation: string
}> {
  override get message(): string {
    return `ARTIFACTS R2 binding is not configured (account-level R2 disabled, #8516; R2→GCS migration is #8523): ${this.operation}`
  }
}

const rejectedArtifactsOperation = (operation: string): Promise<never> =>
  Promise.reject(new ArtifactsUnavailableError({ operation }))

/**
 * A stand-in `R2Bucket` whose every operation rejects with
 * {@link ArtifactsUnavailableError}. R2 bucket methods are all
 * promise-returning, so callers' existing `.catch`/`try-await` fail-soft
 * handling sees an ordinary async failure — never a synchronous crash at
 * store-construction time.
 */
const disabledArtifactsBucket: R2Bucket = new Proxy({} as R2Bucket, {
  get: (_target, property) => {
    // Never present as a thenable: `await bucket` (or a stray
    // `Promise.resolve(bucket)`) must resolve to the bucket itself, not
    // hang on a fake `then` method.
    if (typeof property !== 'string' || property === 'then') {
      return undefined
    }
    return (..._args: ReadonlyArray<unknown>) =>
      rejectedArtifactsOperation(property)
  },
})

/**
 * Resolve the ARTIFACTS bucket for an env, falling back to the rejecting
 * stub when the binding is absent. See the module doc for why the binding
 * may be absent in production.
 */
export const artifactsBucketForEnv = (
  env: Readonly<{ ARTIFACTS?: R2Bucket | undefined }>,
): R2Bucket => env.ARTIFACTS ?? disabledArtifactsBucket
