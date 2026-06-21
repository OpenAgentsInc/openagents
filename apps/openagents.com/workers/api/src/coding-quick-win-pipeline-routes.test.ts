import { describe, expect, it } from 'vitest'
import { Effect } from 'effect'
import { handleCodingQuickWinPipelineApi } from './coding-quick-win-pipeline-routes'
import { scopeQuickWinFromIntake } from './business-quick-win-scope'
import { buildCodingQuickWinProvisioning } from './coding-quick-win-provisioning'
import { buildCodingQuickWinRuntimeInvocation } from './coding-quick-win-runtime-invocation'

describe('coding-quick-win-pipeline-routes', () => {
  const scope = scopeQuickWinFromIntake({
    signupId: 'signup_123',
    helpWith: 'fix the bug in the code', // matches coding_agent_work -> business.coding_quick_win.v1
  })

  const provisioning = buildCodingQuickWinProvisioning({
    scopeRef: scope.quickWinScopedRef,
    repositoryUrl: 'https://github.com/org/repo',
    requestedBranch: 'main',
    status: 'provisioned',
    baseCommitSha: 'sha123',
    worktreeRef: 'worktree_456',
    failureReason: null,
  })

  const invocation = buildCodingQuickWinRuntimeInvocation({
    scopeRef: scope.quickWinScopedRef,
    provisioning,
    runtimeAgentId: 'pylon_claude_bridge',
    status: 'completed',
    executionLogRef: 'log_789',
    candidatePatchRef: 'patch_abc',
    failureReason: null,
  })

  it('rejects GET requests', async () => {
    const request = new Request('http://localhost/api/public/business/coding-quick-win-pipeline', {
      method: 'GET',
    })
    const response = await Effect.runPromise(handleCodingQuickWinPipelineApi(request))
    expect(response.status).toBe(405)
  })

  it('rejects invalid JSON', async () => {
    const request = new Request('http://localhost/api/public/business/coding-quick-win-pipeline', {
      method: 'POST',
      body: 'not json',
    })
    const response = await Effect.runPromise(handleCodingQuickWinPipelineApi(request))
    expect(response.status).toBe(400)
    const json = await response.json()
    expect(json).toEqual({ error: 'invalid_request', reason: 'request body must be valid JSON' })
  })

  it('handles pipeline invariant errors', async () => {
    const request = new Request('http://localhost/api/public/business/coding-quick-win-pipeline', {
      method: 'POST',
      body: JSON.stringify({
        scope,
        provisioning: { ...provisioning, scopeRef: 'wrong' }, // invalid linkage
      }),
    })
    const response = await Effect.runPromise(handleCodingQuickWinPipelineApi(request))
    expect(response.status).toBe(400)
    const json = await response.json()
    expect(json).toMatchObject({
      error: 'pipeline_invariant_error',
      reason: expect.stringContaining('Provisioning scopeRef does not match'),
    })
  })

  it('returns a successful pipeline receipt', async () => {
    const request = new Request('http://localhost/api/public/business/coding-quick-win-pipeline', {
      method: 'POST',
      body: JSON.stringify({
        scope,
        provisioning,
        invocation,
      }),
    })
    const response = await Effect.runPromise(handleCodingQuickWinPipelineApi(request))
    expect(response.status).toBe(200)
    const json = await response.json() as any
    expect(json.inert).toBe(true)
    expect(json.receipt.receiptKind).toBe('business_quick_win')
    expect(json.receipt.offeringPromiseId).toBe('business.coding_quick_win.v1')
  })
})
