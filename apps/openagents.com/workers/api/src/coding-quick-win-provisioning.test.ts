import { describe, expect, it } from 'vitest'
import {
  assertCodingQuickWinProvisioned,
  buildCodingQuickWinProvisioning,
  codingQuickWinProvisionedWorktreeRef,
  CodingQuickWinProvisioningInvariantError,
  publicCodingQuickWinProvisioningProjection,
} from './coding-quick-win-provisioning.js'

describe('CodingQuickWinProvisioning', () => {
  const validPending = {
    scopeRef: 'quick-win-scope:signup-123:coding_agent_work',
    repositoryUrl: 'https://github.com/OpenAgentsInc/openagents',
    requestedBranch: 'main',
    status: 'pending_clone' as const,
    baseCommitSha: null,
    worktreeRef: null,
    failureReason: null,
  }

  const validProvisioned = {
    scopeRef: 'quick-win-scope:signup-123:coding_agent_work',
    repositoryUrl: 'https://github.com/OpenAgentsInc/openagents',
    requestedBranch: 'main',
    status: 'provisioned' as const,
    baseCommitSha: 'a1b2c3d4e5f6',
    worktreeRef: 'sandbox:isolate-789',
    failureReason: null,
  }

  it('builds a valid pending provisioning event', () => {
    const event = buildCodingQuickWinProvisioning(validPending)
    expect(event.eventKind).toBe('coding_quick_win_provisioning')
    expect(event.status).toBe('pending_clone')
  })

  it('builds a valid provisioned event', () => {
    const event = buildCodingQuickWinProvisioning(validProvisioned)
    expect(event.eventKind).toBe('coding_quick_win_provisioning')
    expect(event.status).toBe('provisioned')
    expect(event.baseCommitSha).toBe('a1b2c3d4e5f6')
  })

  it('rejects empty required fields', () => {
    expect(() => buildCodingQuickWinProvisioning({ ...validPending, scopeRef: '   ' }))
      .toThrow(CodingQuickWinProvisioningInvariantError)
    expect(() => buildCodingQuickWinProvisioning({ ...validPending, repositoryUrl: '' }))
      .toThrow(CodingQuickWinProvisioningInvariantError)
    expect(() => buildCodingQuickWinProvisioning({ ...validPending, requestedBranch: '' }))
      .toThrow(CodingQuickWinProvisioningInvariantError)
  })

  it('rejects provisioned status without a commit SHA', () => {
    expect(() => buildCodingQuickWinProvisioning({ ...validProvisioned, baseCommitSha: null }))
      .toThrow(/must lock a baseCommitSha/)
  })

  it('rejects provisioned status without a worktree ref', () => {
    expect(() => buildCodingQuickWinProvisioning({ ...validProvisioned, worktreeRef: null }))
      .toThrow(/must expose a worktreeRef/)
  })

  it('rejects failed clone without a reason', () => {
    expect(() => buildCodingQuickWinProvisioning({
      ...validPending,
      status: 'failed_to_clone',
      failureReason: null,
    })).toThrow(/must include a failureReason/)
  })

  it('asserts readiness for runtime successfully on provisioned', () => {
    const event = buildCodingQuickWinProvisioning(validProvisioned)
    expect(() => assertCodingQuickWinProvisioned(event)).not.toThrow()
  })

  it('asserts readiness fails on pending', () => {
    const event = buildCodingQuickWinProvisioning(validPending)
    expect(() => assertCodingQuickWinProvisioned(event)).toThrow(/status is pending_clone/)
  })

  it('yields worktree ref only when provisioned', () => {
    const event = buildCodingQuickWinProvisioning(validProvisioned)
    expect(codingQuickWinProvisionedWorktreeRef(event)).toBe('sandbox:isolate-789')
  })

  it('projects public fields', () => {
    const event = buildCodingQuickWinProvisioning(validProvisioned)
    const projection = publicCodingQuickWinProvisioningProjection(event)
    
    expect(projection.eventKind).toBe('coding_quick_win_provisioning')
    expect(projection.baseCommitSha).toBe('a1b2c3d4e5f6')
    // Ensure internal refs are hidden
    expect(projection).not.toHaveProperty('worktreeRef')
    expect(projection).not.toHaveProperty('scopeRef')
  })
})
