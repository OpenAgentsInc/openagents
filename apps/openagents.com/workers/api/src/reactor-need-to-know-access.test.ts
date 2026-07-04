import { Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  REACTOR_NEED_TO_KNOW_ADVERSARIAL_DOCUMENTS,
  REACTOR_NEED_TO_KNOW_ADVERSARIAL_ORACLES,
  REACTOR_NEED_TO_KNOW_ADVERSARIAL_RECEIPTS,
  REACTOR_NEED_TO_KNOW_ADVERSARIAL_REQUESTS,
  REACTOR_NEED_TO_KNOW_BROKEN_ALLOW_ALL_RULESET_FIXTURE,
  REACTOR_NEED_TO_KNOW_RULESET_V1,
  ReactorCorpusAccessDecisionReceipt,
  ReactorNeedToKnowRuleSet,
  evaluateReactorNeedToKnowAccess,
} from '@openagentsinc/reactor-contracts'

describe('Reactor need-to-know access adversarial sweep', () => {
  test('receipts allowed access without logging raw corpus or generated summaries', () => {
    const receipt = S.decodeUnknownSync(ReactorCorpusAccessDecisionReceipt)(
      REACTOR_NEED_TO_KNOW_ADVERSARIAL_RECEIPTS.aliceAllowed,
    )

    expect(receipt.selectedDocumentRefs).toEqual([
      REACTOR_NEED_TO_KNOW_ADVERSARIAL_DOCUMENTS.aliceStrategyMemo.documentRef,
    ])
    expect(receipt.selectedCitationRefs).toEqual([
      'citation.reactor.fixture.alice.strategy_memo',
    ])
    expect(receipt.deniedDocumentRefs).toEqual([])
    expect(receipt.rawDocumentContentLogged).toBe(false)
    expect(receipt.generatedSummaryContentLogged).toBe(false)
    expect(receipt.ruleSetRef).toBe(REACTOR_NEED_TO_KNOW_RULESET_V1.ruleSetRef)
    expect(receipt.ruleSetVersion).toBe(REACTOR_NEED_TO_KNOW_RULESET_V1.version)
    expect(receipt.subjectUserRef).toBe('user.alice')
    expect(receipt.workspaceRef).toBe('workspace.reactor.fixture.customer_one')
    expect(receipt.matterRef).toBe('matter.reactor.fixture.alice')
    expect(receipt.documentDecisions[0]).toMatchObject({
      blockerRefs: [],
      hardDecisionStatus: 'passed',
      oracleDecisionStatus: 'passed',
      oracleVerdictRef:
        REACTOR_NEED_TO_KNOW_ADVERSARIAL_ORACLES.alicePlausible.verdictRef,
      status: 'allowed',
    })
  })

  test('Bob cannot see Alice through citations or summarization', () => {
    const citationDenied =
      REACTOR_NEED_TO_KNOW_ADVERSARIAL_RECEIPTS.bobAliceCitationDenied
    const summaryDenied =
      REACTOR_NEED_TO_KNOW_ADVERSARIAL_RECEIPTS.bobAliceSummaryDenied

    for (const receipt of [citationDenied, summaryDenied]) {
      expect(receipt.selectedDocumentRefs).toEqual([])
      expect(receipt.selectedCitationRefs).toEqual([])
      expect(receipt.deniedDocumentRefs).toEqual([
        REACTOR_NEED_TO_KNOW_ADVERSARIAL_DOCUMENTS.aliceStrategyMemo.documentRef,
      ])
      expect(receipt.deniedCitationRefs).toEqual([
        'citation.reactor.fixture.alice.strategy_memo',
      ])
      expect(receipt.rawDocumentContentLogged).toBe(false)
      expect(receipt.generatedSummaryContentLogged).toBe(false)
      expect(receipt.documentDecisions[0]).toMatchObject({
        hardDecisionStatus: 'failed',
        oracleDecisionStatus: 'skipped_hard_denied',
        status: 'denied_hard_rule',
      })
      expect(receipt.documentDecisions[0]?.blockerRefs).toEqual(
        expect.arrayContaining([
          'blocker.reactor.need_to_know.matter_scope_mismatch',
          'blocker.reactor.need_to_know.role_or_user_scope_missing',
        ]),
      )
      expect(JSON.stringify(receipt)).not.toContain('Alice private strategy text')
    }

    expect(summaryDenied.documentDecisions[0]?.oracleVerdictRef).toBe(
      REACTOR_NEED_TO_KNOW_ADVERSARIAL_ORACLES.bobAlicePlausibleButHardDenied
        .verdictRef,
    )
  })

  test('soft oracle can only further deny after hard rules pass', () => {
    const receipt = REACTOR_NEED_TO_KNOW_ADVERSARIAL_RECEIPTS.aliceSoftDenied

    expect(receipt.selectedDocumentRefs).toEqual([])
    expect(receipt.documentDecisions[0]).toMatchObject({
      hardDecisionStatus: 'passed',
      oracleDecisionStatus: 'failed',
      oracleVerdictRef:
        REACTOR_NEED_TO_KNOW_ADVERSARIAL_ORACLES.aliceNotNeeded.verdictRef,
      status: 'denied_soft_oracle',
    })
    expect(receipt.documentDecisions[0]?.blockerRefs).toContain(
      'blocker.reactor.need_to_know.oracle_not_plausible',
    )
  })

  test('deliberately broken allow-all and missing-oracle fixtures fail closed', () => {
    expect(() =>
      S.decodeUnknownSync(ReactorNeedToKnowRuleSet)(
        REACTOR_NEED_TO_KNOW_BROKEN_ALLOW_ALL_RULESET_FIXTURE,
      ),
    ).toThrow()

    const missingOracle = evaluateReactorNeedToKnowAccess({
      decidedAt: '2026-07-04T15:09:00.000Z',
      documents: [REACTOR_NEED_TO_KNOW_ADVERSARIAL_DOCUMENTS.aliceStrategyMemo],
      receiptRef: 'reactor.corpus_access.worker.alice.missing_oracle.001',
      request: REACTOR_NEED_TO_KNOW_ADVERSARIAL_REQUESTS.aliceDirect,
      ruleSet: REACTOR_NEED_TO_KNOW_RULESET_V1,
      sourceRefs: ['apps/openagents.com/workers/api/src/reactor-need-to-know-access.test.ts'],
    })

    expect(missingOracle.selectedDocumentRefs).toEqual([])
    expect(missingOracle.documentDecisions[0]).toMatchObject({
      blockerRefs: ['blocker.reactor.need_to_know.oracle_verdict_missing'],
      hardDecisionStatus: 'passed',
      oracleDecisionStatus: 'failed',
      status: 'denied_soft_oracle',
    })
  })
})
