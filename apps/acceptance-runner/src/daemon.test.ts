// Unit proof for the out-of-Worker runner DAEMON loop (EPIC #6017).
//
// No chromium, no network: a fake JobSource hands a job, a fake RunnerTransport returns
// a verdict and records the POST, and we assert the daemon tick leases -> runs -> posts
// -> acks, and that the idle/lease-error cases back off without fabricating work. The
// REAL headless run is proven separately by run-once.ts against the committed artifact
// and by the worker-side acceptance-dispatch.test.ts.

import { describe, expect, test } from 'bun:test'

import {
  AcceptanceJobMessage,
  type AcceptanceVerdict,
  crossyRoadAcceptanceSpec,
  type RunAcceptanceJobResult,
  type RunnerTransport,
  type VerdictCallbackPayload,
} from './harness-bridge'
import { runDaemonTick, type RunJobFn, type RunnerDaemonDeps } from './daemon'
import type { JobSource, LeasedJob } from './job-source'
import type { RunnerServiceConfig } from './config'

const config: RunnerServiceConfig = {
  artifactResolveMode: 'http',
  bearerToken: 'tok',
  idleBackoffMs: 1,
  jobAckUrl: 'https://x/ack',
  jobLeaseUrl: 'https://x/lease',
  navTimeoutMs: 5000,
  pollIntervalMs: 1,
  verdictCallbackUrl: 'https://x/verdict',
}

const message = (requestId: string): AcceptanceJobMessage => {
  const spec = crossyRoadAcceptanceSpec()
  return AcceptanceJobMessage.make({
    artifactRef: `artifact://${requestId}`,
    meteringReceiptRef: null,
    requestId,
    schemaVersion: 'openagents.inference.acceptance_job.v1',
    servedModel: 'openagents/khala-code',
    spec: {
      checks: spec.checks,
      kind: spec.kind,
      params: spec.params,
      rubricRef: spec.rubricRef,
    },
    worker: 'w',
  })
}

// A fake transport that records the verdict it would POST (no browser, no network).
const fakeTransport = (posted: VerdictCallbackPayload[]): RunnerTransport => ({
  postVerdict: async payload => {
    posted.push(payload)
  },
  resolveArtifact: async () => '<html><body>ok</body></html>',
})

const passVerdict = (): AcceptanceVerdict => {
  const spec = crossyRoadAcceptanceSpec()
  return {
    checks: spec.checks.map(id => ({ detail: 'ok', id, passed: true })),
    consoleErrors: [],
    executed: true,
    failedChecks: [],
    kind: spec.kind,
    pageErrors: [],
    passedChecks: spec.checks,
    rubricRef: spec.rubricRef,
    scalarReward: 1,
    verified: true,
  }
}

// A browser-free `runJob` that mirrors the harness contract: resolve the artifact, then
// post the verdict (delivered = post succeeded). Proves the daemon loop wiring without
// launching chromium; the REAL headless run is proven by run-once.ts + the worker test.
const makeFakeRunJob =
  (verdict: AcceptanceVerdict): RunJobFn =>
  async (transport, message): Promise<RunAcceptanceJobResult> => {
    await transport.resolveArtifact(message.artifactRef)
    const delivered = await transport
      .postVerdict({
        meteringReceiptRef: message.meteringReceiptRef ?? null,
        requestId: message.requestId,
        schemaVersion: 'openagents.inference.acceptance_verdict.v1',
        servedModel: message.servedModel,
        verdict,
        worker: message.worker,
      })
      .then(() => true)
      .catch(() => false)
    return { delivered, verdict }
  }

const fakeSource = (
  queue: LeasedJob[],
  acks: Array<{ leaseId: string; delivered: boolean }>,
  opts?: { leaseThrows?: boolean },
): JobSource => ({
  ack: async input => {
    acks.push(input)
  },
  label: 'fake',
  lease: async () => {
    if (opts?.leaseThrows) throw new Error('lease boom')
    return queue.shift() ?? null
  },
})

describe('runner daemon tick', () => {
  test('lease -> run -> post verdict -> ack delivered', async () => {
    const posted: VerdictCallbackPayload[] = []
    const acks: Array<{ leaseId: string; delivered: boolean }> = []
    const deps: RunnerDaemonDeps = {
      config,
      runJob: makeFakeRunJob(passVerdict()),
      source: fakeSource([{ leaseId: 'L1', message: message('r1') }], acks),
      transport: fakeTransport(posted),
    }
    const outcome = await runDaemonTick(deps)
    expect(outcome.kind).toBe('ran')
    // The harness ran the (fake) artifact and posted a verdict for the right request.
    expect(posted).toHaveLength(1)
    expect(posted[0]!.requestId).toBe('r1')
    expect(posted[0]!.schemaVersion).toBe(
      'openagents.inference.acceptance_verdict.v1',
    )
    // Delivered -> acked delivered (the Worker removes the job).
    expect(acks).toEqual([{ delivered: true, leaseId: 'L1' }])
  })

  test('delivery failure -> ack retryable (job re-leased later)', async () => {
    const acks: Array<{ leaseId: string; delivered: boolean }> = []
    const failingTransport: RunnerTransport = {
      postVerdict: async () => {
        throw new Error('callback down')
      },
      resolveArtifact: async () => '<html></html>',
    }
    const deps: RunnerDaemonDeps = {
      config,
      runJob: makeFakeRunJob(passVerdict()),
      source: fakeSource([{ leaseId: 'L2', message: message('r2') }], acks),
      transport: failingTransport,
    }
    const outcome = await runDaemonTick(deps)
    expect(outcome.kind).toBe('ran')
    expect(acks).toEqual([{ delivered: false, leaseId: 'L2' }])
  })

  test('empty queue -> idle (no post, no ack)', async () => {
    const posted: VerdictCallbackPayload[] = []
    const acks: Array<{ leaseId: string; delivered: boolean }> = []
    const deps: RunnerDaemonDeps = {
      config,
      source: fakeSource([], acks),
      transport: fakeTransport(posted),
    }
    const outcome = await runDaemonTick(deps)
    expect(outcome.kind).toBe('idle')
    expect(posted).toHaveLength(0)
    expect(acks).toHaveLength(0)
  })

  test('lease transport fault -> lease_error (no fabricated verdict)', async () => {
    const posted: VerdictCallbackPayload[] = []
    const acks: Array<{ leaseId: string; delivered: boolean }> = []
    const deps: RunnerDaemonDeps = {
      config,
      source: fakeSource([], acks, { leaseThrows: true }),
      transport: fakeTransport(posted),
    }
    const outcome = await runDaemonTick(deps)
    expect(outcome.kind).toBe('lease_error')
    expect(posted).toHaveLength(0)
    expect(acks).toHaveLength(0)
  })
})
