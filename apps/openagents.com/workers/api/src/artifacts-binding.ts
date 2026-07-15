/**
 * Google Cloud Storage artifact resolution.
 *
 * Production requires the `ARTIFACTS_GCS_*` configuration and uses the GCS
 * adapter. The optional `ARTIFACTS` object is an in-process test/local double,
 * not a host binding or production fallback. When neither is present,
 * artifact-dependent operations reject with a typed error.
 */

import { Data } from 'effect'

import { makeGcsArtifactsBucket } from './gcs-artifacts-bucket'

/** Typed rejection when Google Cloud Storage is not configured. */
export class ArtifactsUnavailableError extends Data.TaggedError(
  'ArtifactsUnavailableError',
)<{
  readonly operation: string
}> {
  override get message(): string {
    return `Google Cloud Storage artifacts are not configured: ${this.operation}`
  }
}

const rejectedArtifactsOperation = (operation: string): Promise<never> =>
  Promise.reject(new ArtifactsUnavailableError({ operation }))

/**
 * A stand-in artifact bucket whose every operation rejects with
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
  /** In-process test/local double. Production never configures this field. */
  ARTIFACTS?: R2Bucket | undefined
  /** GCS bucket name, e.g. `openagentsgemini-oa-artifacts`. */
  ARTIFACTS_GCS_BUCKET?: string | undefined
  /** Optional endpoint override (default `https://storage.googleapis.com`). */
  ARTIFACTS_GCS_ENDPOINT?: string | undefined
  /** HMAC key pair for the `oa-artifacts-rw` service account (Secret Manager). */
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
 *    configured;
 * 3. the rejecting stub (typed per-call `ArtifactsUnavailableError`).
 */
export const artifactsBucketForEnv = (env: ArtifactsEnv): R2Bucket =>
  env.ARTIFACTS ?? gcsArtifactsBucketForEnv(env) ?? disabledArtifactsBucket
