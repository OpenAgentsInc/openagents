import { Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  buildAgentRunAssignment,
  type RepositoryRef,
} from './omni-runs'
import {
  OpenAgentsRunnerDispatchSecretBoundary,
  OpenAgentsRunnerDispatchSecretBoundaryDenied,
  OpenAgentsRunnerDispatchSecretBoundaryPublicProjection,
  buildOpenAgentsRunnerDispatchSecretBoundary,
  openAgentsRunnerGrantRefsForDispatch,
  projectOpenAgentsRunnerDispatchSecretBoundaryPublic,
} from './runner-secret-boundary'

const repository: RepositoryRef = {
  owner: 'OpenAgentsInc',
  provider: 'github',
  ref: 'main',
  repo: 'autopilot-omega',
}

const shcPayload = () =>
  buildAgentRunAssignment({
    appOrigin: 'https://openagents.com',
    authGrantRef: 'grant.provider_account.codex.account_1',
    backend: 'shc_vm',
    dispatchGoal: 'dispatch.goal.ref.site_builder',
    githubWriteGrantRef: 'grant.github_write.repo_branch',
    goal: 'Build a reviewed Site revision.',
    providerAccountRef: 'provider_account.codex.account_1',
    repository,
    runId: 'run.shc.1',
  })

const containerPayload = () => ({
  artifactManifest: {
    artifactRefs: ['artifact.fake_container.generated_bundle'],
    digestRef: 'digest.fake_container.generated_bundle.sha256',
    manifestRef: 'manifest.fake_container.generated_bundle',
    publicArtifactRefs: ['artifact.fake_container.preview_url'],
    receiptRefs: ['receipt.fake_container.manifest_recorded'],
  },
  assignmentRef: 'assignment.site_builder.fake_container',
  authGrantRef: 'grant.provider_account.codex.account_2',
  backendKind: 'cloudflare_container',
  callbackRef: 'callback.fake_container.redacted_ref',
  githubWriteGrantRef: 'grant.github_write.repo_branch',
  goalRef: 'goal.site_builder.fake_container',
  policyRefs: ['policy.runner.container.fake_staging'],
  providerAccountRef: 'provider_account.codex.account_2',
  repositoryRef: 'github.openagents.autopilot_omega.main',
  requestId: 'runner.fake-container.2',
  runnerId: 'runner.fake_container.staging',
  runtimeRef: 'runtime.codex.default',
  timeoutMs: 300_000,
  trustLevel: 'medium',
})

describe('runner dispatch secret boundary', () => {
  test('models grant refs, resolution receipts, and scrub receipts for SHC dispatch', () => {
    const boundary = buildOpenAgentsRunnerDispatchSecretBoundary({
      authGrantRef: 'grant.provider_account.codex.account_1',
      backendKind: 'shc_vm',
      callbackRef: 'runner_callback_token',
      dispatchPayload: shcPayload(),
      dispatchRef: 'dispatch.shc.run_1',
      githubWriteGrantRef: 'grant.github_write.repo_branch',
      providerAccountRef: 'provider_account.codex.account_1',
      runnerSessionRef: 'run.shc.1',
    })

    expect(boundary).not.toBeInstanceOf(
      OpenAgentsRunnerDispatchSecretBoundaryDenied,
    )
    expect(S.decodeUnknownSync(OpenAgentsRunnerDispatchSecretBoundary)(boundary))
      .toMatchObject({
        backendKind: 'shc_vm',
        status: 'ready',
      })
    if (!(boundary instanceof OpenAgentsRunnerDispatchSecretBoundaryDenied)) {
      expect(boundary.grantRefs.map(grant => grant.grantKind)).toEqual([
        'provider_account',
        'github_write',
        'callback',
      ])
      expect(boundary.resolutionReceipts).toHaveLength(3)
      expect(boundary.scrubReceipts).toHaveLength(3)
    }
  })

  test('models Container-compatible dispatch refs without credential material', () => {
    const boundary = buildOpenAgentsRunnerDispatchSecretBoundary({
      authGrantRef: 'grant.provider_account.codex.account_2',
      backendKind: 'cloudflare_container',
      callbackRef: 'callback.fake_container.redacted_ref',
      dispatchPayload: containerPayload(),
      dispatchRef: 'dispatch.container.run_2',
      githubWriteGrantRef: 'grant.github_write.repo_branch',
      providerAccountRef: 'provider_account.codex.account_2',
      runnerSessionRef: 'run.container.2',
    })

    expect(boundary).not.toBeInstanceOf(
      OpenAgentsRunnerDispatchSecretBoundaryDenied,
    )
    if (!(boundary instanceof OpenAgentsRunnerDispatchSecretBoundaryDenied)) {
      expect(boundary.backendKind).toBe('cloudflare_container')
      expect(boundary.status).toBe('ready')
      expect(boundary.denialReasons).toEqual([])
      expect(boundary.resolutionReceipts.map(receipt => receipt.status))
        .toEqual(['resolved', 'resolved', 'resolved'])
      expect(boundary.scrubReceipts.map(receipt => receipt.status)).toEqual([
        'scrubbed',
        'scrubbed',
        'scrubbed',
      ])
    }
  })

  test('keeps public projection free of provider-account and credential refs', () => {
    const boundary = buildOpenAgentsRunnerDispatchSecretBoundary({
      authGrantRef: 'grant.provider_account.codex.account_1',
      backendKind: 'shc_vm',
      callbackRef: 'runner_callback_token',
      dispatchPayload: shcPayload(),
      dispatchRef: 'dispatch.shc.run_1',
      githubWriteGrantRef: 'grant.github_write.repo_branch',
      providerAccountRef: 'provider_account.codex.account_1',
      runnerSessionRef: 'run.shc.1',
    })

    if (boundary instanceof OpenAgentsRunnerDispatchSecretBoundaryDenied) {
      throw boundary
    }

    const projection =
      projectOpenAgentsRunnerDispatchSecretBoundaryPublic(boundary)
    const projectionText = JSON.stringify(projection)

    expect(
      S.decodeUnknownSync(
        OpenAgentsRunnerDispatchSecretBoundaryPublicProjection,
      )(projection),
    ).toEqual(projection)
    expect(projection).toMatchObject({
      grantCount: 3,
      hasRequiredGrants: true,
      status: 'ready',
    })
    expect(projectionText).not.toContain('provider_account')
    expect(projectionText).not.toContain('grant.provider_account')
    expect(projectionText).not.toContain('grant.github_write')
    expect(projectionText).not.toContain('runner_callback_token')
  })

  test('denies missing required grant refs', () => {
    const grantRefs = openAgentsRunnerGrantRefsForDispatch({
      githubWriteGrantRef: 'grant.github_write.repo_branch',
      runnerSessionRef: 'run.shc.1',
    })
    const boundary = buildOpenAgentsRunnerDispatchSecretBoundary({
      backendKind: 'shc_vm',
      dispatchPayload: { callback: { tokenRef: 'runner_callback_token' } },
      dispatchRef: 'dispatch.shc.missing_required',
      githubWriteGrantRef: 'grant.github_write.repo_branch',
      runnerSessionRef: 'run.shc.1',
    })

    expect(grantRefs.map(grant => grant.grantKind)).toEqual(['github_write'])
    expect(boundary).toMatchObject({
      denialReasons: ['missing_required_grant_ref'],
      status: 'denied',
    })
  })

  test('rejects raw provider, OAuth, cookie, GitHub, API key, and callback tokens', () => {
    const unsafePayloads = [
      { providerToken: 'provider_token.raw' },
      { oauth_access_token: 'oauth_access_token.raw' },
      { cookie: 'session=abc' },
      { githubToken: 'ghp_rawgithubsecret' },
      { openai_api_key: 'sk-rawsecret' },
      { callbackTokenValue: 'callback_token_value.raw' },
    ]
    const denials = unsafePayloads.map(dispatchPayload =>
      buildOpenAgentsRunnerDispatchSecretBoundary({
        authGrantRef: 'grant.provider_account.codex.account_1',
        backendKind: 'shc_vm',
        callbackRef: 'runner_callback_token',
        dispatchPayload,
        dispatchRef: 'dispatch.shc.unsafe',
        providerAccountRef: 'provider_account.codex.account_1',
        runnerSessionRef: 'run.shc.unsafe',
      }),
    )

    expect(
      denials.every(
        denial => denial instanceof OpenAgentsRunnerDispatchSecretBoundaryDenied,
      ),
    ).toBe(true)
    expect(
      denials.map(denial =>
        denial instanceof OpenAgentsRunnerDispatchSecretBoundaryDenied
          ? denial._tag
          : 'not_denied',
      ),
    ).toEqual([
      'OpenAgentsRunnerDispatchSecretBoundaryDenied',
      'OpenAgentsRunnerDispatchSecretBoundaryDenied',
      'OpenAgentsRunnerDispatchSecretBoundaryDenied',
      'OpenAgentsRunnerDispatchSecretBoundaryDenied',
      'OpenAgentsRunnerDispatchSecretBoundaryDenied',
      'OpenAgentsRunnerDispatchSecretBoundaryDenied',
    ])
  })
})
