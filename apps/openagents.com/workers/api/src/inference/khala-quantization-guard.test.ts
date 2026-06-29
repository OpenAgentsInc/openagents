import { describe, expect, it } from 'vitest'
import { buildKhalaServedModelDescriptor } from './khala-quantization'
import {
  KhalaSameModelClaimError,
  assertSameModelClaim,
  evaluateSameModelClaim,
  isQuantizationDisclosed,
} from './khala-quantization-guard'

describe('same-model-claim guard (book P1-7 / #6090)', () => {
  it('ALLOWS an unquantized lane under the bare public alias (same product)', () => {
    const descriptor = buildKhalaServedModelDescriptor({
      publicAlias: 'openagents/khala-code',
      servedModelId: 'accounts/fireworks/models/kimi-k2',
      quantization: { precision: 'unquantized', backend: 'vllm' },
    })
    const verdict = evaluateSameModelClaim(descriptor)
    expect(verdict.allowed).toBe(true)
    expect(verdict.reason).toBeNull()
    expect(verdict.isQuantizedLane).toBe(false)
  })

  it('REJECTS an undisclosed quantized lane fronting the unqualified alias (the headline leak)', () => {
    const descriptor = buildKhalaServedModelDescriptor({
      publicAlias: 'openagents/khala-code',
      servedModelId: 'accounts/fireworks/models/kimi-k2-fp8',
      // Quantized precision but NO backend disclosed => undisclosed.
      quantization: { precision: 'fp8' },
    })
    const verdict = evaluateSameModelClaim(descriptor)
    expect(verdict.allowed).toBe(false)
    expect(verdict.reason).toBe(
      'undisclosed_quantization_under_unqualified_alias',
    )
    expect(verdict.isQuantizedLane).toBe(true)
    expect(verdict.disclosed).toBe(false)
  })

  it('REJECTS an unknown-precision lane under the unqualified alias', () => {
    const descriptor = buildKhalaServedModelDescriptor({
      publicAlias: 'openagents/khala-code',
      servedModelId: 'partner/opaque-model',
      // The managed provider does not disclose how it quantizes.
      quantization: {},
    })
    const verdict = evaluateSameModelClaim(descriptor)
    expect(verdict.allowed).toBe(false)
    expect(verdict.reason).toBe('unknown_precision_under_unqualified_alias')
  })

  it('REJECTS a disclosed quantized lane under the unqualified alias when the eval gate has NOT passed', () => {
    const descriptor = buildKhalaServedModelDescriptor({
      publicAlias: 'openagents/khala-code',
      servedModelId: 'accounts/fireworks/models/kimi-k2-fp8',
      // Disclosed (precision + backend) but not yet eval-gate qualified.
      quantization: {
        precision: 'fp8',
        backend: 'sglang',
        scope: 'weights_only',
        evalGatePassed: false,
      },
    })
    const verdict = evaluateSameModelClaim(descriptor)
    expect(verdict.allowed).toBe(false)
    expect(verdict.reason).toBe('quantized_lane_not_eval_gate_qualified')
    expect(verdict.disclosed).toBe(true)
  })

  it('ALLOWS a disclosed, eval-gate-qualified quantized lane under the unqualified alias (proven claim)', () => {
    const descriptor = buildKhalaServedModelDescriptor({
      publicAlias: 'openagents/khala-code',
      servedModelId: 'accounts/fireworks/models/kimi-k2-fp8',
      quantization: {
        precision: 'fp8',
        backend: 'sglang',
        backendVersion: '0.4.1',
        scope: 'weights_only',
        evalGatePassed: true,
        evalGateRef: 'gate:khala-code-fp8:v1',
      },
    })
    const verdict = evaluateSameModelClaim(descriptor)
    expect(verdict.allowed).toBe(true)
    expect(verdict.reason).toBeNull()
    expect(verdict.disclosed).toBe(true)
  })

  it('ALLOWS a quantized lane under a QUALIFIED alias regardless of receipt (the alias is the disclosure)', () => {
    const descriptor = buildKhalaServedModelDescriptor({
      publicAlias: 'openagents/khala-code-fp8',
      servedModelId: 'accounts/fireworks/models/kimi-k2-fp8',
      // Even undisclosed/ungated: the customer addressed a precision-named product.
      quantization: { precision: 'fp8' },
    })
    const verdict = evaluateSameModelClaim(descriptor)
    expect(verdict.allowed).toBe(true)
    expect(verdict.reason).toBeNull()
  })

  it('raises an aggressive-scope WARNING even when the claim is allowed', () => {
    const descriptor = buildKhalaServedModelDescriptor({
      publicAlias: 'openagents/khala-code-int4',
      servedModelId: 'self-hosted/khala-code-int4',
      quantization: {
        precision: 'int4',
        backend: 'vllm',
        scope: 'kv_cache',
      },
    })
    const verdict = evaluateSameModelClaim(descriptor)
    expect(verdict.allowed).toBe(true) // qualified alias
    expect(verdict.aggressiveScopeWarning).toBe(true)
  })
})

describe('isQuantizationDisclosed', () => {
  it('is false for an unquantized lane (nothing to disclose) and an unknown lane', () => {
    expect(
      isQuantizationDisclosed(
        buildKhalaServedModelDescriptor({
          publicAlias: 'openagents/khala-code',
          servedModelId: 'x',
          quantization: { precision: 'unquantized', backend: 'vllm' },
        }),
      ),
    ).toBe(false)
    expect(
      isQuantizationDisclosed(
        buildKhalaServedModelDescriptor({
          publicAlias: 'openagents/khala-code',
          servedModelId: 'x',
          quantization: {},
        }),
      ),
    ).toBe(false)
  })

  it('is true only when a quantized precision AND a named backend are present', () => {
    expect(
      isQuantizationDisclosed(
        buildKhalaServedModelDescriptor({
          publicAlias: 'openagents/khala-code',
          servedModelId: 'x',
          quantization: { precision: 'fp8', backend: 'sglang' },
        }),
      ),
    ).toBe(true)
  })
})

describe('assertSameModelClaim (fail-closed route entry point)', () => {
  it('throws a typed error with the rejection reason on an undisclosed quantized variant', () => {
    const descriptor = buildKhalaServedModelDescriptor({
      publicAlias: 'openagents/khala-code',
      servedModelId: 'x-fp8',
      quantization: { precision: 'fp8' },
    })
    try {
      assertSameModelClaim(descriptor)
      expect.unreachable('should have thrown')
    } catch (error) {
      expect(error).toBeInstanceOf(KhalaSameModelClaimError)
      expect((error as KhalaSameModelClaimError).reason).toBe(
        'undisclosed_quantization_under_unqualified_alias',
      )
    }
  })

  it('returns the verdict (no throw) when the claim is allowed', () => {
    const descriptor = buildKhalaServedModelDescriptor({
      publicAlias: 'openagents/khala-code',
      servedModelId: 'x',
      quantization: { precision: 'unquantized', backend: 'vllm' },
    })
    expect(assertSameModelClaim(descriptor).allowed).toBe(true)
  })
})
