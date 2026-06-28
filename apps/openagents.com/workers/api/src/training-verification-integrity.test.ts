import { describe, expect, it } from 'vitest'

import {
  assessVerificationIntegrity,
  normalizeVerificationModelFamily,
} from './training-verification-integrity'
import { runTrainingVerificationClass } from './training-verification'
import { buildTrainingVerificationChallengeRecord } from './training-verification'

const exactTraceChallenge = (payload: Record<string, unknown>) =>
  buildTrainingVerificationChallengeRecord({
    makeId: () => 'integrity',
    nowIso: '2026-06-28T12:00:00.000Z',
    request: {
      homeworkKind: 'admin_dispatched_homework',
      payload: {
        contributionRefs: ['contribution.training.integrity'],
        replayDigestRef: 'digest.trace.same',
        sampledWindow: { endStep: 10, startStep: 0 },
        traceCommitmentDigestRef: 'digest.trace.same',
        ...payload,
      },
      trainingRunRef: 'training.run.integrity',
      verificationClass: 'exact_trace_replay',
    },
  }).challenge

describe('training verification integrity policy', () => {
  it('normalizes public provider and model ids into enforcement families', () => {
    expect(normalizeVerificationModelFamily('openagents/pylon-codex')).toBe(
      'pylon_codex',
    )
    expect(normalizeVerificationModelFamily('claude-sonnet-4')).toBe(
      'anthropic_claude',
    )
    expect(normalizeVerificationModelFamily('0xSero/GLM-5.2-504B')).toBe('glm')
    expect(normalizeVerificationModelFamily('gpt-oss-120b')).toBe('gpt_oss')
  })

  it('rejects same-family verifier panels and missing agent-channel defenses', () => {
    const assessment = assessVerificationIntegrity({
      channelDefense: { agentChannel: true },
      verifierPanel: [
        { family: 'pylon_codex', ref: 'judge.1' },
        { model: 'openagents/pylon-codex', ref: 'judge.2' },
      ],
      workerFamily: 'pylon_codex',
    })

    expect(assessment.ok).toBe(false)
    expect(assessment.blockers).toEqual([
      'blocker.verification_integrity.agent_channel_cross_model_review_missing',
      'blocker.verification_integrity.agent_channel_paraphrasing_missing',
      'blocker.verification_integrity.effective_independence_below_floor',
      'blocker.verification_integrity.same_family_worker_verifier',
      'blocker.verification_integrity.verifier_panel_family_diversity_low',
    ])
  })

  it('measures nominal nine-judge panels as two effective votes when families collapse', () => {
    const assessment = assessVerificationIntegrity({
      channelDefense: {
        agentChannel: true,
        crossModelReview: true,
        paraphrasing: true,
        steganalysis: true,
      },
      minimumEffectiveVotes: 3,
      verifierPanel: [
        { family: 'anthropic_claude', ref: 'judge.1' },
        { family: 'anthropic_claude', ref: 'judge.2' },
        { family: 'anthropic_claude', ref: 'judge.3' },
        { family: 'anthropic_claude', ref: 'judge.4' },
        { family: 'anthropic_claude', ref: 'judge.5' },
        { family: 'gemini', ref: 'judge.6' },
        { family: 'gemini', ref: 'judge.7' },
        { family: 'gemini', ref: 'judge.8' },
        { family: 'gemini', ref: 'judge.9' },
      ],
      workerFamily: 'pylon_codex',
    })

    expect(assessment.panelSize).toBe(9)
    expect(assessment.effectiveIndependenceVotes).toBe(1.98)
    expect(assessment.blockers).toEqual([
      'blocker.verification_integrity.effective_independence_below_floor',
    ])
  })

  it('passes independent cross-model panels with paraphrasing defenses', () => {
    const assessment = assessVerificationIntegrity({
      channelDefense: {
        agentChannel: true,
        crossModelReview: true,
        paraphrasing: true,
      },
      verifierPanel: [
        { family: 'anthropic_claude', ref: 'judge.1' },
        { family: 'gemini', ref: 'judge.2' },
        { family: 'glm', ref: 'judge.3' },
      ],
      workerFamily: 'pylon_codex',
    })

    expect(assessment.ok).toBe(true)
    expect(assessment.effectiveIndependenceVotes).toBe(3)
    expect(assessment.blockers).toEqual([])
  })

  it('allows a single verifier when its model family differs from the worker', () => {
    const assessment = assessVerificationIntegrity({
      verifierModel: 'claude-sonnet-4',
      workerModel: 'openagents/pylon-codex',
    })

    expect(assessment.ok).toBe(true)
    expect(assessment.effectiveIndependenceVotes).toBe(1)
    expect(assessment.minimumEffectiveVotes).toBe(1)
    expect(assessment.blockers).toEqual([])
  })

  it('fails exact-trace replay when worker and verifier share a model family', async () => {
    await expect(
      runTrainingVerificationClass({
        challenge: exactTraceChallenge({
          channelDefense: {
            agentChannel: true,
            crossModelReview: true,
            paraphrasing: true,
          },
          verifierPanel: [
            { family: 'pylon_codex', ref: 'validator.codex.1' },
            { family: 'pylon_codex', ref: 'validator.codex.2' },
          ],
          workerModelFamily: 'pylon_codex',
        }),
      }),
    ).resolves.toMatchObject({
      failureCodes: ['VerifierIndependenceFailed'],
      publicDetails: {
        effectiveIndependenceVotes: 1,
        integrityBlockers: [
          'blocker.verification_integrity.effective_independence_below_floor',
          'blocker.verification_integrity.same_family_worker_verifier',
          'blocker.verification_integrity.verifier_panel_family_diversity_low',
        ],
        verifierFamilies: ['pylon_codex'],
        workerFamily: 'pylon_codex',
      },
      state: 'Rejected',
    })
  })
})
