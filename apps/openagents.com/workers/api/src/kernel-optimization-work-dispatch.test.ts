import { KERNEL_OPTIMIZATION_PARITY_CLASS_ID } from '@openagentsinc/tassadar-executor'
import { describe, expect, test } from 'vitest'

import { decodeCreateForumWorkRequestBody } from './forum-work-request-route-contract'
import {
  KERNEL_OPTIMIZATION_CAPABILITY_REF,
  KernelOptimizationDispatchError,
  type KernelOptimizationJobSpec,
  buildKernelOptimizationWorkRequest,
} from './kernel-optimization-work-dispatch'

// Anchored on the historical 217.md result: Qwen 3.5 0.5B baseline ~328 tok/s.
const baseSpec: KernelOptimizationJobSpec = {
  baselineRecordRef: 'record.public.qwen35-0_5b.cuda.a10g.rmsnorm.328tps',
  baselineTokensPerSecond: 328,
  budgetSats: 50_000,
  deadlineRef: 'deadline.public.2026-07-01',
  device: 'cuda',
  hardwareRef: 'nvidia-a10g',
  kernelRef: 'rmsnorm',
  targetModel: 'qwen-3.5-0.5b',
  validatorDeviceRef: 'device.public.validator.metal.m3',
}

describe('kernel-optimization market dispatch', () => {
  test('builds a dispatch-valid forum work request from a job spec', () => {
    const body = buildKernelOptimizationWorkRequest(baseSpec)

    // Round-trips through the route contract a second time => dispatch-valid.
    expect(decodeCreateForumWorkRequestBody(body)).toEqual(body)
    expect(body.budgetSats).toBe(50_000)
    expect(body.title).toContain('rmsnorm')
    expect(body.title).toContain('qwen-3.5-0.5b')
  })

  test('binds the parity verdict + baseline floor as the verification command', () => {
    const body = buildKernelOptimizationWorkRequest(baseSpec)

    expect(body.verificationCommandRef).toContain(
      KERNEL_OPTIMIZATION_PARITY_CLASS_ID,
    )
    expect(body.verificationCommandRef).toContain('min_tok_s=328')
    expect(body.verificationCommandRef).toContain(baseSpec.baselineRecordRef)
    expect(body.verificationCommandRef).toContain(baseSpec.validatorDeviceRef)
  })

  test('requires the kernel-optimization + tassadar executor capabilities', () => {
    const body = buildKernelOptimizationWorkRequest(baseSpec)
    const caps = body.requiredCapabilityRefs ?? []

    expect(caps).toContain(KERNEL_OPTIMIZATION_CAPABILITY_REF)
    expect(caps).toContain('capability.tassadar_poc.numeric_model_executor')
    expect(caps).toContain('capability.kernel_optimization.device.cuda')
  })

  test('derives a route-valid requested slug', () => {
    const body = buildKernelOptimizationWorkRequest(baseSpec)
    expect(body.requestedSlug).toMatch(
      /^[a-z0-9](?:[a-z0-9-]*[a-z0-9])?$/,
    )
    expect((body.requestedSlug ?? '').length).toBeLessThanOrEqual(80)
  })

  test('rejects a non-positive baseline throughput', () => {
    expect(() =>
      buildKernelOptimizationWorkRequest({
        ...baseSpec,
        baselineTokensPerSecond: 0,
      }),
    ).toThrow(KernelOptimizationDispatchError)
  })

  test('rejects a non-integer budget', () => {
    expect(() =>
      buildKernelOptimizationWorkRequest({ ...baseSpec, budgetSats: 1.5 }),
    ).toThrow(KernelOptimizationDispatchError)
  })

  test('rejects an empty kernel ref', () => {
    expect(() =>
      buildKernelOptimizationWorkRequest({ ...baseSpec, kernelRef: '   ' }),
    ).toThrow(KernelOptimizationDispatchError)
  })
})
