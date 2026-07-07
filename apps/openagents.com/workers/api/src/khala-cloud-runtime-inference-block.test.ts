import { describe, expect, test } from 'vitest'

import {
  CLOUD_RUNTIME_DEFAULT_BRANCH,
  CLOUD_RUNTIME_INFERENCE_DEFAULT_LANE,
  buildCloudRuntimeInferenceConfig,
  buildCloudRuntimeWorkContext,
  decodeWorkContextB64,
  encodeWorkContextB64,
} from './khala-cloud-runtime-inference-block'

describe('buildCloudRuntimeInferenceConfig', () => {
  test('builds the exact contract with lane default and required fields', () => {
    const cfg = buildCloudRuntimeInferenceConfig({
      agentToken: 'oa_agent_secret',
      baseUrl: 'https://staging.example',
      model: 'openagents/khala',
      ownerUserId: 'github:14167547',
    })
    expect(cfg).toEqual({
      agentToken: 'oa_agent_secret',
      baseUrl: 'https://staging.example',
      lane: CLOUD_RUNTIME_INFERENCE_DEFAULT_LANE,
      model: 'openagents/khala',
      ownerUserId: 'github:14167547',
    })
  })

  test('omits optional keys when undefined (field-presence parity with turn-runner)', () => {
    const cfg = buildCloudRuntimeInferenceConfig({
      agentToken: 't',
      baseUrl: 'https://x',
      model: 'm',
      ownerUserId: 'github:1',
    })
    expect('provider' in cfg).toBe(false)
    expect('backendProfile' in cfg).toBe(false)
    expect('pylonRef' in cfg).toBe(false)
    expect('noMeterSecret' in cfg).toBe(false)
  })

  test('carries provider + noMeterSecret when provided (single-charge header)', () => {
    const cfg = buildCloudRuntimeInferenceConfig({
      agentToken: 't',
      backendProfile: 'omega-hosted-gemini',
      baseUrl: 'https://x',
      lane: 'hosted_khala',
      model: 'm',
      noMeterSecret: 'no-meter',
      ownerUserId: 'github:1',
      provider: 'vertex-gemini',
      pylonRef: 'pylon.agent-computer.1',
    })
    expect(cfg.provider).toBe('vertex-gemini')
    expect(cfg.noMeterSecret).toBe('no-meter')
    expect(cfg.backendProfile).toBe('omega-hosted-gemini')
    expect(cfg.pylonRef).toBe('pylon.agent-computer.1')
  })
})

describe('buildCloudRuntimeWorkContext', () => {
  test('assembles the full work-context with branch/objective defaults', () => {
    const inference = buildCloudRuntimeInferenceConfig({
      agentToken: 't',
      baseUrl: 'https://x',
      model: 'm',
      ownerUserId: 'github:1',
    })
    const wc = buildCloudRuntimeWorkContext({
      commit: '7fd1a60b01f91b314f59955a4e4d4e80d8edf11d',
      inference,
      repo: 'octocat/Hello-World',
      threadRef: 'thread.t1',
      turnId: 'turn.t1',
      workContextRef: 'work-context.agent-computer.wc1',
    })
    expect(wc.branch).toBe(CLOUD_RUNTIME_DEFAULT_BRANCH)
    expect(wc.objective).toContain('octocat/Hello-World@7fd1a60b01f9')
    expect(wc.inference).toBe(inference)
    expect(wc.threadRef).toBe('thread.t1')
    expect(wc.repo).toBe('octocat/Hello-World')
  })
})

describe('encodeWorkContextB64 / decodeWorkContextB64', () => {
  test('standard-base64 alphabet only and round-trips', () => {
    const inference = buildCloudRuntimeInferenceConfig({
      agentToken: 'oa_agent_secret',
      baseUrl: 'https://staging.example',
      model: 'openagents/khala',
      noMeterSecret: 'no-meter',
      ownerUserId: 'github:14167547',
      provider: 'vertex-gemini',
    })
    const wc = buildCloudRuntimeWorkContext({
      commit: '7fd1a60b01f91b314f59955a4e4d4e80d8edf11d',
      inference,
      objective: 'first real cloud-gcp Seam A turn — café ☕',
      repo: 'octocat/Hello-World',
      threadRef: 'thread.t1',
      turnId: 'turn.t1',
      workContextRef: 'work-context.agent-computer.wc1',
    })
    const b64 = encodeWorkContextB64(wc)
    // daemon `is_valid_work_context_b64`: non-empty, standard alphabet only.
    expect(b64.length).toBeGreaterThan(0)
    expect(/^[A-Za-z0-9+/=]+$/.test(b64)).toBe(true)
    // round-trips through the same decode the turn-runner does (JSON.parse).
    expect(decodeWorkContextB64(b64)).toEqual(wc)
    // the agent token survives inside the opaque blob for the guest's call.
    expect(decodeWorkContextB64(b64).inference.agentToken).toBe('oa_agent_secret')
  })
})
