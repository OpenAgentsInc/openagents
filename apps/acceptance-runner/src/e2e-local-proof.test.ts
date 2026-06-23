// LOCAL END-TO-END PROOF for the out-of-Worker acceptance runner (EPIC #6017).
//
// Proves the FULL chain in process, no mocks of the load-bearing steps:
//
//   committed passing artifact
//     -> runAcceptanceJob (REAL Playwright/chromium headless suite)   [6/6 verdict]
//     -> postVerdict POSTs to the REAL handleAcceptanceVerdictCallback route
//        (authenticated with a TEST token, against an in-memory verification store)
//     -> the route BACKFILLS the receipt to verified:true / test_passed.
//
// This is the deployable runner's exact runtime behaviour, with the only substitutions
// being (a) the artifact comes from a local file instead of an R2 ref and (b) the
// callback is invoked in-process instead of over the network — neither of which changes
// the verdict or the backfill. Requires a real headless chromium (Playwright); if
// chromium is missing the run produces an honest all-fail verdict (never a fake green),
// so this test would FAIL rather than disguise a missing browser.

import { readFileSync } from 'node:fs'
import { fileURLToPath } from 'node:url'
import { dirname, resolve } from 'node:path'

import { describe, expect, test } from 'bun:test'
import { Effect } from 'effect'

import {
  type RunnerTransport,
  AcceptanceJobMessage,
  crossyRoadAcceptanceSpec,
  handleAcceptanceVerdictCallback,
  makeInMemoryKhalaVerificationStore,
  runAcceptanceJob,
} from './harness-bridge'

const here = dirname(fileURLToPath(import.meta.url))
// apps/acceptance-runner/src -> repo root -> scripts/khala-demo/artifacts
const PASSING_ARTIFACT = resolve(
  here,
  '../../..',
  'scripts/khala-demo/artifacts/khala-crossy-road-northstar-passing.v1.html',
)

const TEST_TOKEN = 'local-proof-token'
const REQUEST_ID = 'acceptance.e2e.local-proof.crossy_road.v1'

describe('local end-to-end: real headless run -> real callback -> receipt backfill', () => {
  test(
    'committed passing artifact => 6/6 verdict => callback backfills verified:true',
    async () => {
      const html = readFileSync(PASSING_ARTIFACT, 'utf8')
      const spec = crossyRoadAcceptanceSpec()

      // The in-memory verification store the REAL route backfills into.
      const store = makeInMemoryKhalaVerificationStore()

      // The verdict poster delivers to the REAL route in-process (no network). It mints a
      // Request exactly like a node-side runner's fetch POST would, with the bearer token.
      const callbackPoster: RunnerTransport['postVerdict'] = async payload => {
        const request = new Request('https://x/v1/inference/acceptance-verdicts', {
          body: JSON.stringify(payload),
          headers: {
            authorization: `Bearer ${TEST_TOKEN}`,
            'content-type': 'application/json',
          },
          method: 'POST',
        })
        const response = await Effect.runPromise(
          handleAcceptanceVerdictCallback(request, {
            callbackToken: TEST_TOKEN,
            enabled: true,
            nowIso: () => new Date().toISOString(),
            store,
          }),
        )
        if (!response.ok) {
          throw new Error(`callback_failed: ${response.status}`)
        }
      }

      const transport: RunnerTransport = {
        postVerdict: callbackPoster,
        resolveArtifact: async () => html,
      }

      const message = AcceptanceJobMessage.make({
        artifactRef: `file://${PASSING_ARTIFACT}`,
        meteringReceiptRef: null,
        requestId: REQUEST_ID,
        schemaVersion: 'openagents.inference.acceptance_job.v1',
        servedModel: 'openagents/khala-code',
        spec: {
          checks: spec.checks,
          kind: spec.kind,
          params: spec.params,
          rubricRef: spec.rubricRef,
        },
        worker: 'khala-code-crossy-road-verifier',
      })

      // RUN: real headless suite, then deliver the verdict through the real route.
      const result = await runAcceptanceJob(transport, message)

      // The runner really executed and got 6/6.
      expect(result.verdict.executed).toBe(true)
      expect(result.verdict.verified).toBe(true)
      expect(result.verdict.scalarReward).toBe(1)
      expect(result.verdict.passedChecks.length).toBe(6)
      expect(result.verdict.failedChecks.length).toBe(0)

      // The verdict was DELIVERED and the route ACCEPTED it.
      expect(result.delivered).toBe(true)

      // The receipt was BACKFILLED to verified:true / test_passed (the gating signal the
      // settlement loop reads). Read straight from the store the route wrote into.
      const record = await Effect.runPromise(store.read(REQUEST_ID))
      expect(record).not.toBeNull()
      expect(record!.executed).toBe(true)
      expect(record!.verified).toBe(true)
      expect(record!.verification).toBe('test_passed')
      expect(record!.scalarReward).toBe(1)
      expect(record!.failedChecks.length).toBe(0)
    },
    120_000,
  )
})
