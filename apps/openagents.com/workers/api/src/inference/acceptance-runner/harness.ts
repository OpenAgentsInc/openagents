// Node-side acceptance-runner HARNESS (EPIC #6017).
//
// THIS RUNS OUT OF THE CF WORKER, on a node with chromium (a Pylon /
// `oa-workroomd` sandbox / a small Cloud Run runner service). It is the consumer
// end of the async dispatch: it takes an `AcceptanceJobMessage`, resolves the
// artifact bytes, runs the merged headless acceptance suite (`runner.ts`,
// Playwright), and posts the resulting `AcceptanceVerdict` back to the gateway's
// authenticated verdict-callback. The gateway then backfills the receipt
// (`unverified` -> `test_passed`/`failed`).
//
// PLUGGABLE EXECUTION HOST. The harness never hard-codes WHERE the artifact comes
// from or HOW the verdict is delivered — both are `RunnerTransport` seams:
//   - `resolveArtifact(artifactRef)` -> the HTML bytes. A Pylon reads its local
//     content store; an `oa-workroomd` sandbox reads the mounted artifact; a Cloud
//     Run service GETs an R2-signed URL.
//   - `postVerdict(callback)` -> POST the verdict to the gateway with the bearer
//     token. A real transport uses `fetch`; a test passes a fake that records it.
// So the SAME harness runs on any host; only the transport differs.
//
// RECOMMENDED HOST (see the PR body / spec doc §"Infra we already have"): a Pylon
// is the natural first host — it is already a programmatic environment (the former
// Probe runtime), it joins the verified-work flywheel (workers run QC for
// revshare), and chromium is the same dependency `three-effect` already uses for
// headless capture. An `oa-workroomd` sandbox is the hardened second option for
// untrusted artifacts; a tiny Cloud Run service is the simplest owner-operated
// fallback. The harness is identical across all three.
//
// IMPORT SAFETY: this module imports `runner.ts`, which imports Playwright, so it is
// NODE/BUN-ONLY and must NEVER be imported by the Worker build. The Worker side
// imports only `../acceptance-dispatch` (pure) for enqueue + callback ingest.

import {
  AcceptanceJobMessage,
  type AcceptanceJobSpec,
} from '../acceptance-dispatch'
import {
  type AcceptanceSpec,
  crossyRoadAcceptanceSpec,
} from '../acceptance-spec'
import type { AcceptanceVerdict } from './verdict'
import { type AcceptanceRunOptions, runAcceptanceSuite } from './runner'

// Re-hydrate the typed `AcceptanceSpec` from the on-the-wire job spec. The runner
// asserts against bounded params, so the harness re-reads them through the SAME
// `acceptance-spec.ts` factory (overriding with the carried thresholds) rather than
// trusting raw wire numbers blindly. Today only the crossy-road lane exists.
export const specFromJobSpec = (jobSpec: AcceptanceJobSpec): AcceptanceSpec =>
  crossyRoadAcceptanceSpec({
    expectedForwardAdvance: jobSpec.params.expectedForwardAdvance,
    forwardMoves: jobSpec.params.forwardMoves,
    maxCameraDeltaPerMove: jobSpec.params.maxCameraDeltaPerMove,
    minWorldRowsAhead: jobSpec.params.minWorldRowsAhead,
  })

// The verdict the harness delivers back to the gateway. Mirrors the
// `AcceptanceVerdictCallbackBody` wire shape (kept structural here so the harness has
// no Effect Schema dependency — it builds a plain object the transport serializes).
export type VerdictCallbackPayload = Readonly<{
  schemaVersion: 'openagents.inference.acceptance_verdict.v1'
  requestId: string
  servedModel: string
  worker: string
  meteringReceiptRef: string | null
  verdict: AcceptanceVerdict
}>

// THE PLUGGABLE EXECUTION-HOST SEAM. A Pylon / sandbox / Cloud Run service supplies
// the two host-specific operations; everything else (run the suite, build the
// payload) is host-independent and lives in `runAcceptanceJob`.
export type RunnerTransport = Readonly<{
  // Resolve an artifact ref to its HTML bytes. Host-specific: local store, mounted
  // file, or a signed R2 GET. Rejects when the ref cannot be resolved (the harness
  // turns that into an honest all-fail verdict, never a silent skip).
  resolveArtifact: (artifactRef: string) => Promise<string>
  // Deliver the verdict back to the gateway's authenticated callback. Host-specific:
  // a real `fetch` POST with the bearer token, or a fake in tests. The harness treats
  // a delivery failure as retryable (it returns the failure to the queue consumer).
  postVerdict: (payload: VerdictCallbackPayload) => Promise<void>
}>

export type RunAcceptanceJobResult = Readonly<{
  // The verdict the runner produced (always present — a crash is an all-fail verdict,
  // never a throw). Useful for the host's own logging/CI gating.
  verdict: AcceptanceVerdict
  // Whether the verdict was successfully delivered back to the gateway. False when the
  // transport's `postVerdict` rejected (the consumer should retry the job).
  delivered: boolean
}>

// Run one acceptance job end to end on the node host:
//   resolve artifact -> run the merged headless suite -> post the verdict back.
// The suite NEVER throws into the harness (a load crash is `loads_without_errors:
// false` + downstream fails). An artifact-resolution failure is itself turned into an
// honest all-fail verdict (the artifact we could not load cannot be verified), so the
// receipt is backfilled to `failed`, never left dangling. Only a verdict-DELIVERY
// failure is surfaced as `delivered: false` for the consumer to retry.
export const runAcceptanceJob = async (
  transport: RunnerTransport,
  message: AcceptanceJobMessage,
  options?: AcceptanceRunOptions,
): Promise<RunAcceptanceJobResult> => {
  const spec = specFromJobSpec(message.spec)

  let verdict: AcceptanceVerdict
  try {
    const artifactHtml = await transport.resolveArtifact(message.artifactRef)
    verdict = await runAcceptanceSuite({ artifactHtml, spec }, options)
  } catch (error) {
    // Could not even resolve/load the artifact -> honest all-fail verdict. We mark
    // every spec check failed with the resolution error, so the receipt backfills to
    // `failed` (never a false green, never a silent skip).
    const messageText = error instanceof Error ? error.message : String(error)
    verdict = {
      checks: spec.checks.map(id => ({
        detail: `Could not resolve/run artifact ${message.artifactRef}: ${messageText.slice(0, 160)}`,
        id,
        passed: false,
      })),
      consoleErrors: [],
      executed: true,
      failedChecks: spec.checks,
      kind: spec.kind,
      pageErrors: [`resolve_error: ${messageText.slice(0, 200)}`],
      passedChecks: [],
      rubricRef: spec.rubricRef,
      scalarReward: 0,
      verified: false,
    }
  }

  const payload: VerdictCallbackPayload = {
    meteringReceiptRef: message.meteringReceiptRef ?? null,
    requestId: message.requestId,
    schemaVersion: 'openagents.inference.acceptance_verdict.v1',
    servedModel: message.servedModel,
    verdict,
    worker: message.worker,
  }

  const delivered = await transport
    .postVerdict(payload)
    .then(() => true)
    .catch(() => false)

  return { delivered, verdict }
}

// A `fetch`-backed `postVerdict` builder for the real hosts (Pylon / Cloud Run). It
// POSTs the verdict to the gateway's callback URL with the bearer token. The token is
// supplied out of band (the host's local secret), NEVER hard-coded. Rejects on a
// non-2xx so the consumer retries.
export const makeFetchVerdictPoster = (
  config: Readonly<{
    callbackUrl: string
    bearerToken: string
    fetchFn?: typeof fetch
  }>,
): RunnerTransport['postVerdict'] => {
  const fetchFn = config.fetchFn ?? fetch
  return async payload => {
    const response = await fetchFn(config.callbackUrl, {
      body: JSON.stringify(payload),
      headers: {
        authorization: `Bearer ${config.bearerToken}`,
        'content-type': 'application/json',
      },
      method: 'POST',
    })
    if (!response.ok) {
      throw new Error(`verdict_callback_failed: ${response.status}`)
    }
  }
}

// Decode an unknown queue body into a typed `AcceptanceJobMessage`, so a real queue
// consumer on the host validates the payload before running. Re-exported here so the
// host-side entrypoint needs only this module.
export { AcceptanceJobMessage }
