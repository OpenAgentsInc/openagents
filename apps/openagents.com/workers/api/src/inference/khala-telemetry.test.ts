import { Option } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  NOT_MEASURED,
  buildKhalaTelemetryBlock,
  buildKhalaTelemetryRecord,
  decodeKhalaTelemetryBlock,
  decodeKhalaTelemetryRecord,
  deriveMarginBucket,
  hashCacheAffinityKey,
  isMeasured,
  khalaTelemetryBlockFromRecord,
  measured,
} from './khala-telemetry'

describe('khala telemetry — honest measured/sentinel discipline', () => {
  test('measured() maps absent/invalid inputs to the honest sentinel, never a fake number', () => {
    expect(measured(undefined)).toBe(NOT_MEASURED)
    expect(measured(null)).toBe(NOT_MEASURED)
    expect(measured(Number.NaN)).toBe(NOT_MEASURED)
    expect(measured(Number.POSITIVE_INFINITY)).toBe(NOT_MEASURED)
    expect(measured(-1)).toBe(NOT_MEASURED)
    // A measured ZERO is a measured value, distinct from `not_measured`.
    expect(measured(0)).toBe(0)
    expect(measured(123)).toBe(123)
    expect(isMeasured(0)).toBe(true)
    expect(isMeasured(NOT_MEASURED)).toBe(false)
  })

  test('a fully-measured record carries every P0-1 field and derives TPS + ITL', () => {
    const record = buildKhalaTelemetryRecord({
      cachedInputTokens: 100,
      completionTokens: 11,
      costBasisMsat: 100,
      executedVerdict: 'passed',
      generationWallClockMs: 1000,
      priceMsat: 170,
      promptTokens: 400,
      provider: 'fireworks',
      providerTimeMs: 900,
      gatewayOverheadMs: 40,
      region: 'us-central1',
      requestClass: 'interactive_stream',
      requestId: 'chatcmpl-x',
      requestedModel: 'openagents/khala-code',
      route: 'coding',
      scalarReward: 1,
      servedModel: 'accounts/fireworks/models/kimi-k2p7-code',
      settlementState: 'pending',
      totalTokens: 411,
      totalWallClockMs: 1200,
      ttftMs: 200,
      verificationClass: 'test_passed',
      verifierReceiptRef: 'verifier.khala_code.executed.v1',
      cacheAffinityKeyRaw: 'account:123|session:abc',
    })

    expect(record.promptTokens).toBe(400)
    expect(record.completionTokens).toBe(11)
    expect(record.totalTokens).toBe(411)
    expect(record.cachedInputTokens).toBe(100)
    // 411 total == 400 prompt + 11 completion => no hidden billed dimension.
    expect(record.unaccountedTokens).toBe(0)
    expect(record.ttftMs).toBe(200)
    expect(record.totalWallClockMs).toBe(1200)
    // ITL = generation wall-clock / (tokens - 1) = 1000 / 10 = 100ms.
    expect(record.interTokenLatencyMs).toBe(100)
    // perceived TPS = tokens / (generation wall-clock seconds) = 11 / 1 = 11.
    expect(record.perceivedTps).toBe(11)
    expect(record.providerTimeMs).toBe(900)
    expect(record.gatewayOverheadMs).toBe(40)
    expect(record.region).toBe('us-central1')
    expect(record.verificationClass).toBe('test_passed')
    expect(record.executedVerdict).toBe('passed')
    expect(record.scalarReward).toBe(1)
    expect(record.settlementState).toBe('pending')
    // margin = 170 - 100 = 70; ratio 70/170 ~= 0.41 => 'rich'.
    expect(record.marginBucket).toBe('rich')
    // The raw cache-affinity key is NEVER stored; only a stable hash.
    expect(record.cacheAffinityKeyHash).toMatch(/^cacheaff:fnv1a32:[0-9a-f]{8}$/)
    expect(JSON.stringify(record)).not.toContain('account:123')
    expect(JSON.stringify(record)).not.toContain('session:abc')
    // The record decodes against the schema (a valid public-safe projection).
    expect(Option.isSome(decodeKhalaTelemetryRecord(record))).toBe(true)
  })

  test('an UNMEASURED request records honest sentinels — never a fabricated number', () => {
    const record = buildKhalaTelemetryRecord({
      // No tokens, no timing, no cost: the worst case (stream with no usage).
      executedVerdict: 'not_executed',
      provider: 'vertex-gemini',
      requestClass: 'async_job',
      requestId: 'chatcmpl-empty',
      requestedModel: 'openagents/khala-mini',
      route: 'gemini',
      servedModel: 'gemini-3.5-flash',
      settlementState: 'not_applicable',
      verificationClass: 'none',
      blockerRefs: ['cost_not_measured', 'verifier_not_executed'],
    })

    expect(record.promptTokens).toBe(NOT_MEASURED)
    expect(record.completionTokens).toBe(NOT_MEASURED)
    expect(record.cachedInputTokens).toBe(NOT_MEASURED)
    // No token counts => the reconciliation is honestly not_measured, never 0.
    expect(record.unaccountedTokens).toBe(NOT_MEASURED)
    expect(record.ttftMs).toBe(NOT_MEASURED)
    expect(record.interTokenLatencyMs).toBe(NOT_MEASURED)
    expect(record.perceivedTps).toBe(NOT_MEASURED)
    expect(record.totalWallClockMs).toBe(NOT_MEASURED)
    expect(record.providerTimeMs).toBe(NOT_MEASURED)
    expect(record.verifierTimeMs).toBe(NOT_MEASURED)
    expect(record.settlementTimeMs).toBe(NOT_MEASURED)
    expect(record.queueWaitMs).toBe(NOT_MEASURED)
    expect(record.batchWaitMs).toBe(NOT_MEASURED)
    expect(record.scalarReward).toBe(NOT_MEASURED)
    expect(record.costBasisMsat).toBe(NOT_MEASURED)
    expect(record.priceMsat).toBe(NOT_MEASURED)
    expect(record.marginBucket).toBe('not_measured')
    expect(record.region).toBe(NOT_MEASURED)
    // No affinity key => null hash (not a fabricated digest).
    expect(record.cacheAffinityKeyHash).toBe(null)
    expect(record.fallbackReason).toBe(null)
    expect(record.blockerRefs).toContain('cost_not_measured')
    expect(Option.isSome(decodeKhalaTelemetryRecord(record))).toBe(true)
  })

  test('reconciles the live totalTokens discrepancy: total 679 != prompt 347 + completion 20 (book P0-2 / #6084)', () => {
    // The exact live numbers: a Gemini-backed khala-mini reply whose
    // totalTokenCount (679) exceeds prompt (347) + completion (20) because of
    // thinking/tool-use tokens. The provider total is recorded receipt-first; the
    // gap is disclosed as unaccountedTokens (679 - 367 = 312), not dropped.
    const record = buildKhalaTelemetryRecord({
      completionTokens: 20,
      executedVerdict: 'not_executed',
      promptTokens: 347,
      provider: 'vertex-gemini',
      requestClass: 'async_job',
      requestId: 'chatcmpl-recon',
      requestedModel: 'openagents/khala-mini',
      route: 'gemini',
      servedModel: 'gemini-3.5-flash',
      settlementState: 'not_applicable',
      totalTokens: 679,
      verificationClass: 'none',
    })
    expect(record.totalTokens).toBe(679)
    expect(record.promptTokens).toBe(347)
    expect(record.completionTokens).toBe(20)
    expect(record.unaccountedTokens).toBe(312)
  })

  test('a degenerate total below prompt+completion floors the unaccounted delta at 0 (never negative)', () => {
    const record = buildKhalaTelemetryRecord({
      completionTokens: 20,
      executedVerdict: 'not_executed',
      promptTokens: 347,
      provider: 'vertex-gemini',
      requestClass: 'async_job',
      requestId: 'chatcmpl-degenerate',
      requestedModel: 'openagents/khala-mini',
      route: 'gemini',
      servedModel: 'gemini-3.5-flash',
      settlementState: 'not_applicable',
      totalTokens: 100,
      verificationClass: 'none',
    })
    expect(record.unaccountedTokens).toBe(0)
    // The provider total is left as reported (never fabricated/corrected).
    expect(record.totalTokens).toBe(100)
  })

  test('single-token completions do not fabricate an inter-token latency', () => {
    const record = buildKhalaTelemetryRecord({
      completionTokens: 1,
      executedVerdict: 'not_executed',
      generationWallClockMs: 500,
      provider: 'fireworks',
      requestClass: 'interactive_stream',
      requestId: 'chatcmpl-one',
      requestedModel: 'openagents/khala-mini',
      route: 'open',
      servedModel: 'm',
      settlementState: 'not_applicable',
      verificationClass: 'none',
    })
    // ITL is undefined for a single token (no inter-token gap) => sentinel.
    expect(record.interTokenLatencyMs).toBe(NOT_MEASURED)
    // perceived TPS is still defined: 1 token / 0.5s = 2.
    expect(record.perceivedTps).toBe(2)
  })

  test('margin bucket coarse-grains the raw margin (never exposes the amount)', () => {
    expect(deriveMarginBucket(NOT_MEASURED, 100)).toBe('not_measured')
    expect(deriveMarginBucket(100, NOT_MEASURED)).toBe('not_measured')
    expect(deriveMarginBucket(120, 100)).toBe('negative')
    expect(deriveMarginBucket(100, 100)).toBe('zero')
    expect(deriveMarginBucket(95, 100)).toBe('thin') // 5/100 = 0.05
    expect(deriveMarginBucket(80, 100)).toBe('standard') // 20/100 = 0.2
    expect(deriveMarginBucket(40, 100)).toBe('rich') // 60/100 = 0.6
  })

  test('cache-affinity hashing is stable and one-way', () => {
    const a = hashCacheAffinityKey('account:1|session:s')
    const b = hashCacheAffinityKey('account:1|session:s')
    const c = hashCacheAffinityKey('account:2|session:s')
    expect(a).toBe(b) // stable: same key => same digest
    expect(a).not.toBe(c) // different key => different digest
    expect(a).not.toContain('account') // the raw key never appears in the digest
  })

  test('the immediate block is the SMALL projection of the full record + a detailRef', () => {
    const record = buildKhalaTelemetryRecord({
      completionTokens: 5,
      executedVerdict: 'passed',
      generationWallClockMs: 400,
      provider: 'fireworks',
      promptTokens: 10,
      requestClass: 'interactive_stream',
      requestId: 'chatcmpl-block',
      requestedModel: 'openagents/khala-code',
      route: 'coding',
      scalarReward: 1,
      servedModel: 'm',
      settlementState: 'pending',
      totalTokens: 15,
      totalWallClockMs: 600,
      ttftMs: 120,
      verificationClass: 'test_passed',
    })
    const detailRef = '/api/public/inference/receipts/receipt.inference.charge.x'
    const block = khalaTelemetryBlockFromRecord(record, detailRef)

    expect(block).toEqual({
      cachedInputTokens: NOT_MEASURED,
      completionTokens: 5,
      detailRef,
      executedVerdict: 'passed',
      promptTokens: 10,
      requestClass: 'interactive_stream',
      scalarReward: 1,
      schemaVersion: 'openagents.khala.telemetry.v1',
      totalTokens: 15,
      totalWallClockMs: 600,
      ttftMs: 120,
      verificationClass: 'test_passed',
    })
    // The block holds ONLY the small summary — the time split / queue wait /
    // economics live in the dereferenceable record, NOT the block.
    expect(block).not.toHaveProperty('providerTimeMs')
    expect(block).not.toHaveProperty('costBasisMsat')
    expect(block).not.toHaveProperty('cacheAffinityKeyHash')
    expect(Option.isSome(decodeKhalaTelemetryBlock(block))).toBe(true)

    // buildKhalaTelemetryBlock is the convenience direct path; it agrees.
    const direct = buildKhalaTelemetryBlock(
      {
        completionTokens: 5,
        executedVerdict: 'passed',
        generationWallClockMs: 400,
        provider: 'fireworks',
        promptTokens: 10,
        requestClass: 'interactive_stream',
        requestId: 'chatcmpl-block',
        requestedModel: 'openagents/khala-code',
        route: 'coding',
        scalarReward: 1,
        servedModel: 'm',
        settlementState: 'pending',
        totalTokens: 15,
        totalWallClockMs: 600,
        ttftMs: 120,
        verificationClass: 'test_passed',
      },
      detailRef,
    )
    expect(direct).toEqual(block)
  })
})
