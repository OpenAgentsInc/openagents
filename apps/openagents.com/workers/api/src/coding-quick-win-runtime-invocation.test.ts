import { describe, expect, it } from 'vitest'
import {
  buildCodingQuickWinRuntimeInvocation,
  codingQuickWinInvocationCandidatePatchRef,
  publicCodingQuickWinRuntimeInvocationProjection,
} from './coding-quick-win-runtime-invocation'
import { buildCodingQuickWinProvisioning } from './coding-quick-win-provisioning'

describe('CodingQuickWinRuntimeInvocation', () => {
  const validProvisioning = buildCodingQuickWinProvisioning({
    scopeRef: 'scope-123',
    repositoryUrl: 'https://github.com/org/repo',
    requestedBranch: 'main',
    status: 'provisioned',
    baseCommitSha: 'sha123',
    worktreeRef: '/tmp/worktree-123',
    failureReason: null,
  })

  const unprovisionedRepo = buildCodingQuickWinProvisioning({
    scopeRef: 'scope-123',
    repositoryUrl: 'https://github.com/org/repo',
    requestedBranch: 'main',
    status: 'pending_clone',
    baseCommitSha: null,
    worktreeRef: null,
    failureReason: null,
  })

  it('builds a valid running invocation', () => {
    const invocation = buildCodingQuickWinRuntimeInvocation({
      scopeRef: 'scope-123',
      provisioning: validProvisioning,
      runtimeAgentId: 'pylon_claude_bridge',
      status: 'running',
    })

    expect(invocation.eventKind).toBe('coding_quick_win_runtime_invocation')
    expect(invocation.provisionedWorktreeRef).toBe('/tmp/worktree-123')
    expect(invocation.status).toBe('running')
  })

  it('builds a valid completed invocation with a patch', () => {
    const invocation = buildCodingQuickWinRuntimeInvocation({
      scopeRef: 'scope-123',
      provisioning: validProvisioning,
      runtimeAgentId: 'pylon_claude_bridge',
      status: 'completed',
      executionLogRef: 'log-456',
      candidatePatchRef: 'patch-789',
    })

    expect(invocation.status).toBe('completed')
    expect(codingQuickWinInvocationCandidatePatchRef(invocation)).toBe('patch-789')
  })

  it('throws if trying to invoke against an unprovisioned repo', () => {
    expect(() =>
      buildCodingQuickWinRuntimeInvocation({
        scopeRef: 'scope-123',
        provisioning: unprovisionedRepo,
        runtimeAgentId: 'pylon_claude_bridge',
        status: 'running',
      })
    ).toThrow(/repository is not ready for runtime/)
  })

  it('throws if a completed run is missing an execution log', () => {
    expect(() =>
      buildCodingQuickWinRuntimeInvocation({
        scopeRef: 'scope-123',
        provisioning: validProvisioning,
        runtimeAgentId: 'pylon_claude_bridge',
        status: 'completed',
      })
    ).toThrow(/must expose an executionLogRef/)
  })

  it('throws if extracting a patch from an incomplete run', () => {
    const invocation = buildCodingQuickWinRuntimeInvocation({
      scopeRef: 'scope-123',
      provisioning: validProvisioning,
      runtimeAgentId: 'pylon_claude_bridge',
      status: 'running',
    })

    expect(() => codingQuickWinInvocationCandidatePatchRef(invocation)).toThrow(
      /runtime is not completed/
    )
  })

  it('throws if extracting a patch from a run that produced no patch', () => {
    const invocation = buildCodingQuickWinRuntimeInvocation({
      scopeRef: 'scope-123',
      provisioning: validProvisioning,
      runtimeAgentId: 'pylon_claude_bridge',
      status: 'completed',
      executionLogRef: 'log-456',
      candidatePatchRef: null,
    })

    expect(() => codingQuickWinInvocationCandidatePatchRef(invocation)).toThrow(
      /did not produce a candidatePatchRef/
    )
  })

  it('projects safely for the public', () => {
    const invocation = buildCodingQuickWinRuntimeInvocation({
      scopeRef: 'scope-123',
      provisioning: validProvisioning,
      runtimeAgentId: 'pylon_claude_bridge',
      status: 'completed',
      executionLogRef: 'log-456',
      candidatePatchRef: 'patch-789',
    })

    const projection = publicCodingQuickWinRuntimeInvocationProjection(invocation)
    expect(projection.eventKind).toBe('coding_quick_win_runtime_invocation')
    expect(projection.runtimeAgentId).toBe('pylon_claude_bridge')
    expect(projection.status).toBe('completed')
    expect(projection.hasCandidatePatch).toBe(true)
    
    // Internal path should be stripped
    expect((projection as any).provisionedWorktreeRef).toBeUndefined()
    expect((projection as any).executionLogRef).toBeUndefined()
    expect((projection as any).candidatePatchRef).toBeUndefined()
  })
})
