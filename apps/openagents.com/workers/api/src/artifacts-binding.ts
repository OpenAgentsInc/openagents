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
 * an injected `ARTIFACTS` object (tests/local dev) wins; otherwise, when
 * the `ARTIFACTS_GCS_*` config is present, the CFG-8 (#8523) GCS-backed
 * adapter (`gcs-artifacts-bucket.ts`) serves the same call surface from
 * Google Cloud Storage; only when neither exists do operations reject
 * with the typed
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

import { makeGcsArtifactsBucket } from './gcs-artifacts-bucket'

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

/** Env slice the artifacts resolution reads (all optional by design). */
export type ArtifactsEnv = Readonly<{
  /** Legacy binding slot; still honored first so tests can inject doubles. */
  ARTIFACTS?: R2Bucket | undefined
  /** GCS bucket name (committed wrangler var), e.g. `openagentsgemini-oa-artifacts`. */
  ARTIFACTS_GCS_BUCKET?: string | undefined
  /** Optional endpoint override (default `https://storage.googleapis.com`). */
  ARTIFACTS_GCS_ENDPOINT?: string | undefined
  /** HMAC key pair for the `oa-artifacts-rw` service account (Worker secrets). */
  ARTIFACTS_GCS_HMAC_ACCESS_KEY_ID?: string | undefined
  ARTIFACTS_GCS_HMAC_SECRET?: string | undefined
}>

// One adapter per distinct GCS config; env objects are stable per isolate
// but DOs/tests construct their own env slices, so key by config values.
const gcsArtifactsBuckets = new Map<string, R2Bucket>()

/** The GCS-backed bucket when the env carries complete GCS config, else undefined. */
const gcsArtifactsBucketForEnv = (env: ArtifactsEnv): R2Bucket | undefined => {
  const bucket = env.ARTIFACTS_GCS_BUCKET
  const accessKeyId = env.ARTIFACTS_GCS_HMAC_ACCESS_KEY_ID
  const secretAccessKey = env.ARTIFACTS_GCS_HMAC_SECRET
  if (
    bucket === undefined ||
    bucket === '' ||
    accessKeyId === undefined ||
    accessKeyId === '' ||
    secretAccessKey === undefined ||
    secretAccessKey === ''
  ) {
    return undefined
  }
  const cacheKey = `${bucket}\n${env.ARTIFACTS_GCS_ENDPOINT ?? ''}\n${accessKeyId}`
  const cached = gcsArtifactsBuckets.get(cacheKey)
  if (cached !== undefined) return cached
  const made = makeGcsArtifactsBucket({
    accessKeyId,
    bucket,
    secretAccessKey,
    ...(env.ARTIFACTS_GCS_ENDPOINT === undefined
      ? {}
      : { endpoint: env.ARTIFACTS_GCS_ENDPOINT }),
  })
  gcsArtifactsBuckets.set(cacheKey, made)
  return made
}

/**
 * Resolve the ARTIFACTS bucket for an env:
 * 1. an explicitly injected `ARTIFACTS` binding object (tests, local dev);
 * 2. the GCS-backed adapter when `ARTIFACTS_GCS_BUCKET` +
 *    `ARTIFACTS_GCS_HMAC_ACCESS_KEY_ID` + `ARTIFACTS_GCS_HMAC_SECRET` are
 *    configured (#8523 — the R2 replacement);
 * 3. the rejecting stub (typed per-call `ArtifactsUnavailableError`).
 */
export const artifactsBucketForEnv = (env: ArtifactsEnv): R2Bucket =>
  env.ARTIFACTS ?? gcsArtifactsBucketForEnv(env) ?? disabledArtifactsBucket
