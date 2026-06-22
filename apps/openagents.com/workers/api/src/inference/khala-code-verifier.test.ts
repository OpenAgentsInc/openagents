import { describe, expect, test } from 'vitest'

import {
  KHALA_CODE_CROSSY_ROAD_RUBRIC_REF,
  KHALA_CODE_HEADLESS_COMMAND_REF,
  discoverKhalaCodeVerificationCommand,
  extractSingleHtmlArtifact,
  verifyKhalaCodeCompletion,
} from './khala-code-verifier'
import {
  BROKEN_CONTROLS_CROSSY_ROAD_HTML,
  BROKEN_DIFFICULTY_CROSSY_ROAD_HTML,
  BROKEN_EXTERNAL_ASSET_CROSSY_ROAD_HTML,
  BROKEN_RESTART_CROSSY_ROAD_HTML,
  GOOD_CROSSY_ROAD_HTML,
} from './khala-code-verifier.fixtures'

const verify = (content: string) =>
  verifyKhalaCodeCompletion({
    content,
    meteringReceiptRef: 'receipt.inference.charge.chatcmpl-fixture',
    requestId: 'chatcmpl-fixture',
    servedModel: 'openagents/khala-code',
    worker: 'fireworks',
  })

describe('Khala code crossy-road verifier', () => {
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

  test('accepts the known-good crossy-road single-file fixture', () => {
    const verdict = verify(GOOD_CROSSY_ROAD_HTML)

    expect(verdict.verified).toBe(true)
    expect(verdict.verification).toBe('test_passed')
    expect(verdict.scalarReward).toBe(1)
    expect(verdict.failedChecks).toEqual([])
    expect(verdict.receiptRef).toMatch(
      /^receipt\.inference\.khala_code\.verification\.chatcmpl-fixture\./u,
    )
    expect(verdict.sourceRefs).toContain(
      'receipt.inference.charge.chatcmpl-fixture',
    )
    expect(verdict.reward.handoffRef).toContain(
      'accepted_outcome.khala_code.crossy_road.',
    )
  })

  test('rejects a deliberately broken control mapping', () => {
    const verdict = verify(BROKEN_CONTROLS_CROSSY_ROAD_HTML)

    expect(verdict.verified).toBe(false)
    expect(verdict.verification).toBe('failed')
    expect(verdict.failedChecks).toContain('direction_controls')
    expect(verdict.scalarReward).toBeLessThan(1)
  })

  test('rejects a deliberately broken restart reset', () => {
    const verdict = verify(BROKEN_RESTART_CROSSY_ROAD_HTML)

    expect(verdict.verified).toBe(false)
    expect(verdict.failedChecks).toContain('restart_resets_character')
  })

  test('rejects an artifact that depends on an external script', () => {
    const verdict = verify(BROKEN_EXTERNAL_ASSET_CROSSY_ROAD_HTML)

    expect(verdict.verified).toBe(false)
    expect(verdict.failedChecks).toContain('single_html_file')
    expect(verdict.failedChecks).toContain('direction_controls')
  })

  test('rejects an artifact without progress-driven difficulty ramping', () => {
    const verdict = verify(BROKEN_DIFFICULTY_CROSSY_ROAD_HTML)

    expect(verdict.verified).toBe(false)
    expect(verdict.failedChecks).toContain('difficulty_ramps_with_progress')
  })
})
