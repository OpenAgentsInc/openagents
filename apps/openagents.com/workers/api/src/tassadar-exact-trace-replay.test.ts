import { describe, expect, it } from 'vitest'

import { verifyExactTraceReplay } from './training-verification'

// #5124 regression: an exact_trace_replay pairing where the worker commitment and
// the validator replay carry the SAME trace-digest body (only the
// `commitment.`/`replay.` prefix differs) must resolve Verified — not
// ExecutorTraceMismatch. The first live pairings (0548af61, fc5465b9) Rejected on
// equal bodies because an earlier-deployed verifier compared the full refs; this
// pins the prefix-stripped comparison so it can't regress.
const challenge = {
  challengeRef: 'training.verification.challenge.regression',
  contributionRef: 'contribution.regression',
  samplingPolicy: 'per_contribution',
  verificationClass: 'exact_trace_replay',
} as never

const digest =
  'f2995c4e3c959b42bb1e4afbefffbcf7ba6104099621ccc0ac912862dc932a5b'

describe('exact_trace_replay digest comparison (#5124)', () => {
  it('Verifies when commitment and replay bodies match across the prefix', () => {
    const verdict = verifyExactTraceReplay({
      challenge,
      payload: {
        contributionRefs: ['contribution.regression'],
        replayDigestRef: `trace.tassadar.replay.${digest}`,
        sampledWindow: { endStep: 80, startStep: 0 },
        sampledWindowRef: 'trace.tassadar.window.0_80',
        traceCommitmentDigestRef: `trace.tassadar.commitment.${digest}`,
      },
    } as never)
    expect(verdict.state).toBe('Verified')
    expect(verdict.failureCodes).not.toContain('ExecutorTraceMismatch')
  })

  it('Rejects with ExecutorTraceMismatch when the digest bodies differ', () => {
    const verdict = verifyExactTraceReplay({
      challenge,
      payload: {
        contributionRefs: ['contribution.regression'],
        replayDigestRef: 'trace.tassadar.replay.bbbbbbbb',
        sampledWindow: { endStep: 80, startStep: 0 },
        traceCommitmentDigestRef: 'trace.tassadar.commitment.aaaaaaaa',
      },
    } as never)
    expect(verdict.state).toBe('Rejected')
    expect(verdict.failureCodes).toContain('ExecutorTraceMismatch')
  })
})
