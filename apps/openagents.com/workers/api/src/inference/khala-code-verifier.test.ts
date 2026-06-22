import { describe, expect, test } from 'vitest'

import type { AcceptanceVerdict } from './acceptance-runner/verdict'
import { crossyRoadAcceptanceSpec } from './acceptance-spec'
import { assembleAcceptanceVerdict } from './acceptance-runner/verdict'
import {
  KHALA_CODE_CROSSY_ROAD_RUBRIC_REF,
  KHALA_CODE_HEADLESS_COMMAND_REF,
  discoverKhalaCodeVerificationCommand,
  extractSingleHtmlArtifact,
  prescreenKhalaCodeArtifact,
  verifyKhalaCodeCompletion,
} from './khala-code-verifier'
import {
  BROKEN_EXTERNAL_ASSET_CROSSY_ROAD_HTML,
  GOOD_CROSSY_ROAD_HTML,
} from './khala-code-verifier.fixtures'

const verify = (
  content: string,
  acceptance?: AcceptanceVerdict | undefined,
) =>
  verifyKhalaCodeCompletion({
    content,
    meteringReceiptRef: 'receipt.inference.charge.chatcmpl-fixture',
    requestId: 'chatcmpl-fixture',
    servedModel: 'openagents/khala-code',
    worker: 'fireworks',
    ...(acceptance === undefined ? {} : { acceptance }),
  })

const executedVerdict = (allPass: boolean): AcceptanceVerdict => {
  const spec = crossyRoadAcceptanceSpec()
  return assembleAcceptanceVerdict({
    checks: spec.checks.map((id, index) => ({
      detail: 'x',
      id,
      passed: allPass ? true : index === 0,
    })),
    consoleErrors: [],
    pageErrors: [],
    spec,
  })
}

describe('Khala code verifier — honest downgrade (EPIC #6017)', () => {
  test('extracts a single HTML artifact from raw or fenced assistant content', () => {
    expect(extractSingleHtmlArtifact(GOOD_CROSSY_ROAD_HTML)).toBe(
      GOOD_CROSSY_ROAD_HTML,
    )
    expect(
      extractSingleHtmlArtifact(
        `Here is the file:\n\n\`\`\`html\n${GOOD_CROSSY_ROAD_HTML}\n\`\`\``,
      ),
    ).toBe(GOOD_CROSSY_ROAD_HTML)
  })

  test('declares a reusable headless verification command contract', () => {
    expect(discoverKhalaCodeVerificationCommand()).toEqual({
      commandRef: KHALA_CODE_HEADLESS_COMMAND_REF,
      kind: 'headless_html_probe',
      rubricRef: KHALA_CODE_CROSSY_ROAD_RUBRIC_REF,
      target: 'crossy-road-single-html',
    })
  })

  test('prescreen is a gate-to-attempt, not a verdict', () => {
    const pass = prescreenKhalaCodeArtifact(GOOD_CROSSY_ROAD_HTML)
    expect(pass.attemptExecution).toBe(true)
    const fail = prescreenKhalaCodeArtifact(
      BROKEN_EXTERNAL_ASSET_CROSSY_ROAD_HTML,
    )
    expect(fail.attemptExecution).toBe(false)
    expect(fail.checks.find(c => c.id === 'single_html_file')?.passed).toBe(
      false,
    )
  })

  test('a prescreen-passing artifact that was NOT executed is unverified — NOT test_passed', () => {
    const verdict = verify(GOOD_CROSSY_ROAD_HTML)

    // This is the core of the honest downgrade: looks fine on paper, but we did not
    // run it, so we do NOT certify it.
    expect(verdict.verification).toBe('unverified')
    expect(verdict.verified).toBe(false)
    expect(verdict.executed).toBe(false)
    expect(verdict.scalarReward).toBe(0)
    expect(verdict.reward.scalar).toBe(0)
    expect(verdict.prescreen.attemptExecution).toBe(true)
  })

  test('an artifact that fails the prescreen is failed and not executed', () => {
    const verdict = verify(BROKEN_EXTERNAL_ASSET_CROSSY_ROAD_HTML)
    expect(verdict.verification).toBe('failed')
    expect(verdict.verified).toBe(false)
    expect(verdict.executed).toBe(false)
    expect(verdict.scalarReward).toBe(0)
    expect(verdict.failedChecks).toContain('single_html_file')
  })

  test('verdict is test_passed only when an EXECUTED acceptance suite fully passed', () => {
    const verdict = verify(GOOD_CROSSY_ROAD_HTML, executedVerdict(true))
    expect(verdict.executed).toBe(true)
    expect(verdict.verification).toBe('test_passed')
    expect(verdict.verified).toBe(true)
    expect(verdict.scalarReward).toBe(1)
    expect(verdict.failedChecks).toEqual([])
    expect(verdict.sourceRefs).toContain(
      'receipt.inference.charge.chatcmpl-fixture',
    )
    expect(verdict.reward.handoffRef).toContain(
      'accepted_outcome.khala_code.crossy_road.',
    )
  })

  test('an executed acceptance suite that did not fully pass is failed with a dense reward', () => {
    const verdict = verify(GOOD_CROSSY_ROAD_HTML, executedVerdict(false))
    expect(verdict.executed).toBe(true)
    expect(verdict.verification).toBe('failed')
    expect(verdict.verified).toBe(false)
    expect(verdict.scalarReward).toBeGreaterThan(0)
    expect(verdict.scalarReward).toBeLessThan(1)
    expect(verdict.failedChecks.length).toBeGreaterThan(0)
  })
})
