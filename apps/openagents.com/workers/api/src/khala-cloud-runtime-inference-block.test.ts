import { describe, expect, test } from 'vitest'

import {
  CLOUD_RUNTIME_DEFAULT_BRANCH,
  CLOUD_RUNTIME_INFERENCE_DEFAULT_LANE,
  CLOUD_RUNTIME_WRITEBACK_BRANCH_PREFIX,
  CLOUD_RUNTIME_WRITEBACK_DEFAULT_INGEST_PATH,
  CLOUD_RUNTIME_WRITEBACK_DEFAULT_MODE,
  buildCloudRuntimeInferenceConfig,
  buildCloudRuntimeWorkContext,
  buildCloudRuntimeWritebackConfig,
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

describe('buildCloudRuntimeWritebackConfig (MM-C5 #8477)', () => {
  test('defaults: deterministic scoped branch, pull_request mode, default ingest path', () => {
    const wb = buildCloudRuntimeWritebackConfig({
      repositoryFullName: 'AgentFlampy/agent-computer-proof',
      turnId: 'turn.seam-a.abc-123',
    })
    expect(wb.repositoryFullName).toBe('AgentFlampy/agent-computer-proof')
    expect(wb.baseBranch).toBe(CLOUD_RUNTIME_DEFAULT_BRANCH)
    expect(wb.mode).toBe(CLOUD_RUNTIME_WRITEBACK_DEFAULT_MODE)
    expect(wb.ingestPath).toBe(CLOUD_RUNTIME_WRITEBACK_DEFAULT_INGEST_PATH)
    expect(wb.branch.startsWith(CLOUD_RUNTIME_WRITEBACK_BRANCH_PREFIX)).toBe(true)
    // deterministic from the turn id (same turn => same branch => same PR).
    expect(wb.branch).toBe(
      buildCloudRuntimeWritebackConfig({
        repositoryFullName: 'AgentFlampy/agent-computer-proof',
        turnId: 'turn.seam-a.abc-123',
      }).branch,
    )
    // the scoped branch is never the base branch.
    expect(wb.branch).not.toBe(wb.baseBranch)
  })

  test('scoped branch sanitizes unsafe turn-id characters', () => {
    const wb = buildCloudRuntimeWritebackConfig({
      repositoryFullName: 'o/n',
      turnId: 'turn/with spaces & weird:chars',
    })
    expect(/^pylon\/agent-computer-[A-Za-z0-9._-]+$/.test(wb.branch)).toBe(true)
    expect(wb.branch).not.toContain(' ')
  })

  test('honors branch_only + a caller-pinned scoped branch and base branch', () => {
    const wb = buildCloudRuntimeWritebackConfig({
      baseBranch: 'develop',
      branch: 'pylon/agent-computer-custom',
      mode: 'branch_only',
      repositoryFullName: 'o/n',
      turnId: 'turn.1',
    })
    expect(wb.mode).toBe('branch_only')
    expect(wb.branch).toBe('pylon/agent-computer-custom')
    expect(wb.baseBranch).toBe('develop')
  })

  test('rejects a caller-pinned branch that is not scoped or equals the base', () => {
    const notScoped = buildCloudRuntimeWritebackConfig({
      branch: 'main',
      repositoryFullName: 'o/n',
      turnId: 'turn.1',
    })
    expect(notScoped.branch.startsWith(CLOUD_RUNTIME_WRITEBACK_BRANCH_PREFIX)).toBe(true)
    const equalsBase = buildCloudRuntimeWritebackConfig({
      baseBranch: 'pylon/x',
      branch: 'pylon/x',
      repositoryFullName: 'o/n',
      turnId: 'turn.1',
    })
    expect(equalsBase.branch).not.toBe('pylon/x')
  })

  test('work-context threads the writeback block only when provided (no credential)', () => {
    const inference = buildCloudRuntimeInferenceConfig({
      agentToken: 'oa_agent_secret',
      baseUrl: 'https://x',
      model: 'm',
      ownerUserId: 'github:300914913',
    })
    const without = buildCloudRuntimeWorkContext({
      commit: '7fd1a60b01f91b314f59955a4e4d4e80d8edf11d',
      inference,
      repo: 'AgentFlampy/agent-computer-proof',
      threadRef: 'thread.t1',
      turnId: 'turn.t1',
      workContextRef: 'wc1',
    })
    expect('writeback' in without).toBe(false)
    const writeback = buildCloudRuntimeWritebackConfig({
      repositoryFullName: 'AgentFlampy/agent-computer-proof',
      turnId: 'turn.t1',
    })
    const withWb = buildCloudRuntimeWorkContext({
      commit: '7fd1a60b01f91b314f59955a4e4d4e80d8edf11d',
      inference,
      repo: 'AgentFlampy/agent-computer-proof',
      threadRef: 'thread.t1',
      turnId: 'turn.t1',
      workContextRef: 'wc1',
      writeback,
    })
    expect(withWb.writeback).toEqual(writeback)
    // the writeback block never carries a token/secret field.
    expect(JSON.stringify(withWb.writeback)).not.toContain('oa_agent_secret')
    expect(JSON.stringify(withWb.writeback)).not.toMatch(/token|secret|password/i)
  })

  test('threads Codex continuity as refs + bounded replay, never persisted homes', () => {
    const inference = buildCloudRuntimeInferenceConfig({
      agentToken: 'oa_agent_secret',
      baseUrl: 'https://x',
      model: 'm',
      ownerUserId: 'github:300914913',
    })
    const wc = buildCloudRuntimeWorkContext({
      codexContinuity: {
        maxReplayMessages: 24,
        persistedCodexHome: false,
        previousTurnCount: 3,
        strategy: 'khala_sync_history_reprime',
      },
      commit: '7fd1a60b01f91b314f59955a4e4d4e80d8edf11d',
      inference,
      providerAuth: {
        agentToken: 'oa_agent_secret',
        authGrantRef: 'grant.codex.thread_1',
        baseUrl: 'https://x',
        providerAccountRef: 'provider-account.codex.owner_1',
      },
      repo: 'AgentFlampy/agent-computer-proof',
      threadRef: 'thread.t1',
      turnId: 'turn.t1',
      workContextRef: 'wc1',
    })
    expect(wc.providerAuth?.providerAccountRef).toBe('provider-account.codex.owner_1')
    expect(wc.codexContinuity).toEqual({
      maxReplayMessages: 24,
      persistedCodexHome: false,
      previousTurnCount: 3,
      strategy: 'khala_sync_history_reprime',
    })
    expect(JSON.stringify(wc.codexContinuity)).not.toMatch(/CODEX_HOME|authJson|token|secret/i)
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
