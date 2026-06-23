import { describe, expect, it } from 'vitest'
import {
  NOT_MEASURED,
  UNKNOWN_QUANTIZATION,
  UNQUANTIZED,
  aliasQualification,
  buildKhalaQuantizationMetadata,
  buildKhalaServedModelDescriptor,
  decodeKhalaQuantizationMetadata,
  decodeKhalaServedModelDescriptor,
  isAggressiveScope,
  isQuantizedPrecision,
} from './khala-quantization'
import {
  buildKhalaTelemetryRecord,
  type KhalaTelemetryInput,
} from './khala-telemetry'
import { Option } from 'effect'

const baseTelemetryInput: KhalaTelemetryInput = {
  requestId: 'req-1',
  requestedModel: 'openagents/khala-code',
  servedModel: 'accounts/fireworks/models/kimi-k2',
  route: 'coding',
  provider: 'fireworks',
  requestClass: 'interactive_stream',
  verificationClass: 'none',
  executedVerdict: 'not_executed',
  settlementState: 'not_applicable',
}

describe('precision classification', () => {
  it('classifies reduced-precision modes as quantized', () => {
    for (const mode of ['fp8', 'mxfp8', 'nvfp4', 'int8', 'int4', 'awq', 'gptq'] as const) {
      expect(isQuantizedPrecision(mode)).toBe(true)
    }
  })

  it('treats unquantized and not_measured as NOT quantized', () => {
    expect(isQuantizedPrecision('unquantized')).toBe(false)
    // not_measured is honestly-unknown, NOT a quantized claim — the guard handles
    // unknown precision as its own rejection, not as "quantized".
    expect(isQuantizedPrecision('not_measured')).toBe(false)
  })

  it('flags only KV-cache and attention scopes as aggressive', () => {
    expect(isAggressiveScope('kv_cache')).toBe(true)
    expect(isAggressiveScope('attention')).toBe(true)
    expect(isAggressiveScope('weights_only')).toBe(false)
    expect(isAggressiveScope('weights_and_activations')).toBe(false)
    expect(isAggressiveScope('none')).toBe(false)
    expect(isAggressiveScope('not_measured')).toBe(false)
  })
})

describe('buildKhalaQuantizationMetadata — honest sentinels', () => {
  it('collapses an absent precision to the honest-unknown sentinel, never a fabricated full precision', () => {
    const meta = buildKhalaQuantizationMetadata({})
    expect(meta.precision).toBe('not_measured')
    expect(meta.backend).toBe('not_measured')
    expect(meta.backendVersion).toBe(NOT_MEASURED)
    expect(meta.scope).toBe('not_measured')
    expect(meta.evalGatePassed).toBe(false)
    expect(meta.evalGateRef).toBeNull()
  })

  it('records a disclosed unquantized lane with scope none', () => {
    const meta = buildKhalaQuantizationMetadata({
      precision: 'unquantized',
      backend: 'vllm',
      backendVersion: '0.6.3',
    })
    expect(meta.precision).toBe('unquantized')
    expect(meta.scope).toBe('none')
    // An unquantized lane is never "eval-gate passed" (the gate is for proving a
    // quantized lane holds quality).
    expect(meta.evalGatePassed).toBe(false)
  })

  it('records a disclosed fp8 weights-only lane and preserves the eval-gate flag', () => {
    const meta = buildKhalaQuantizationMetadata({
      precision: 'fp8',
      backend: 'sglang',
      backendVersion: '0.4.1',
      scope: 'weights_only',
      evalGatePassed: true,
      evalGateRef: 'gate:khala-code-fp8:v1',
    })
    expect(meta.precision).toBe('fp8')
    expect(meta.backend).toBe('sglang')
    expect(meta.scope).toBe('weights_only')
    expect(meta.evalGatePassed).toBe(true)
    expect(meta.evalGateRef).toBe('gate:khala-code-fp8:v1')
  })

  it('keeps scope as not_measured for a quantized lane that did not disclose scope (no fabricated weights_only)', () => {
    const meta = buildKhalaQuantizationMetadata({
      precision: 'int4',
      backend: 'vllm',
    })
    expect(meta.scope).toBe('not_measured')
  })

  it('canonical UNQUANTIZED and UNKNOWN_QUANTIZATION decode cleanly', () => {
    expect(Option.isSome(decodeKhalaQuantizationMetadata(UNQUANTIZED))).toBe(true)
    expect(
      Option.isSome(decodeKhalaQuantizationMetadata(UNKNOWN_QUANTIZATION)),
    ).toBe(true)
    expect(UNQUANTIZED.precision).toBe('unquantized')
    expect(UNKNOWN_QUANTIZATION.precision).toBe('not_measured')
  })
})

describe('aliasQualification — does the public alias disclose precision?', () => {
  it('classifies a bare public alias as unqualified', () => {
    expect(aliasQualification('openagents/khala-code')).toBe('unqualified')
    expect(aliasQualification('openagents/khala-mini')).toBe('unqualified')
  })

  it('classifies a precision-suffixed alias as qualified', () => {
    expect(aliasQualification('openagents/khala-code-fp8')).toBe('qualified')
    expect(aliasQualification('openagents/khala-code-nvfp4')).toBe('qualified')
    expect(aliasQualification('openagents/khala-mini-int4')).toBe('qualified')
  })

  it('is case-insensitive on the suffix', () => {
    expect(aliasQualification('openagents/khala-code-FP8')).toBe('qualified')
  })
})

describe('buildKhalaServedModelDescriptor', () => {
  it('derives alias qualification from the alias so it is always consistent', () => {
    const descriptor = buildKhalaServedModelDescriptor({
      publicAlias: 'openagents/khala-code',
      servedModelId: 'accounts/fireworks/models/kimi-k2',
      quantization: { precision: 'fp8', backend: 'sglang' },
    })
    expect(descriptor.aliasQualification).toBe('unqualified')
    expect(descriptor.quantization.precision).toBe('fp8')
    expect(
      Option.isSome(decodeKhalaServedModelDescriptor(descriptor)),
    ).toBe(true)
  })
})

describe('telemetry record carries quantization (book P1-7)', () => {
  it('populates quant fields from a fixture served-model descriptor', () => {
    const record = buildKhalaTelemetryRecord({
      ...baseTelemetryInput,
      quantization: {
        precision: 'fp8',
        backend: 'sglang',
        backendVersion: '0.4.1',
        scope: 'weights_only',
        evalGatePassed: true,
        evalGateRef: 'gate:khala-code-fp8:v1',
      },
    })
    expect(record.quantization.precision).toBe('fp8')
    expect(record.quantization.backend).toBe('sglang')
    expect(record.quantization.scope).toBe('weights_only')
    expect(record.quantization.evalGatePassed).toBe(true)
  })

  it('records the honest-unknown quant shape when the lane disclosed nothing', () => {
    const record = buildKhalaTelemetryRecord(baseTelemetryInput)
    // The HONESTY contract: an unmeasured precision is a typed sentinel, not a
    // fabricated full-precision claim.
    expect(record.quantization.precision).toBe('not_measured')
    expect(record.quantization.backend).toBe('not_measured')
    expect(record.quantization.scope).toBe('not_measured')
  })
})
