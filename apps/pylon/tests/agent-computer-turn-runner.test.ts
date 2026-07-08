import { describe, expect, test } from 'bun:test'
import { mkdtempSync } from 'node:fs'
import { readFile, rm } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'

import {
  AGENT_COMPUTER_DEFAULT_PROVIDER,
  AGENT_COMPUTER_RECEIPT_LANE,
  CLAUDE_PROVIDER_AUTH_MATERIAL_PATH,
  CODEX_PROVIDER_AUTH_MATERIAL_PATH,
  GITHUB_SCM_AUTH_BROKER_HELPER_REF,
  GITHUB_SCM_AUTH_BROKER_PATH,
  GITHUB_SCM_AUTH_BROKER_REQUEST_SCHEMA,
  INFERENCE_ORG_CLOUD_RUNTIME_NO_METER_HEADER,
  KHALA_AGENT_COMPUTER_WRITEBACK_INGEST_PATH,
  KHALA_AGENT_COMPUTER_WRITEBACK_SCHEMA_VERSION,
  KHALA_CLOUD_RUNTIME_USAGE_INGEST_PATH,
  KHALA_CLOUD_RUNTIME_USAGE_SCHEMA_VERSION,
  brokerGitCredential,
  canReplayCodexProviderGrantAfterReclaim,
  chatCompletionUsage,
  chatCompletionText,
  chatCompletionsRequest,
  classifyPushFailure,
  claudeProviderAuthMaterialRequest,
  codexContinuityResultSummary,
  codexProviderAuthMaterialRequest,
  gitCredentialBrokerRequest,
  materializeClaudeProviderAuth,
  materializeCodexProviderAuth,
  parseRepoFullName,
  runModelTurnReceipt,
  runWritebackForTurn,
  shortLivedOpenCodeAuthExpiresAt,
  usageIngestBody,
  type ClaudeProviderAuthConfig,
  type CodexProviderAuthConfig,
  type GitRun,
  type InferenceConfig,
  type WritebackConfig,
} from '../deploy/agent-computer/turn-runner.ts'

const inference: InferenceConfig = {
  baseUrl: 'https://openagents-monolith-staging-ezxz4mgdsq-uc.a.run.app',
  agentToken: 'agent-secret-token-should-never-be-serialized',
  ownerUserId: 'github:300914913',
  model: 'gemini-3.5-flash',
  backendProfile: 'omega-hosted-gemini',
  pylonRef: 'pylon.agent-computer.proof',
}

const codexProviderAuth: CodexProviderAuthConfig = {
  authGrantRef: 'grant.public.codex.owner.turn',
  baseUrl: 'https://openagents.example',
  agentToken: 'agent-secret-token-should-never-be-serialized',
  providerAccountRef: 'provider-account.public.codex.owner',
}

const claudeProviderAuth: ClaudeProviderAuthConfig = {
  authGrantRef: 'grant.public.claude.owner.turn',
  baseUrl: 'https://openagents.example',
  agentToken: 'agent-secret-token-should-never-be-serialized',
  providerAccountRef: 'provider-account.public.claude.owner',
}

const shortLivedAuthContent = JSON.stringify({
  openai: {
    access: 'short-lived-access-token-never-serialized',
    expires: Date.parse('2026-07-08T12:30:00.000Z'),
  },
})

describe('agent-computer turn-runner: Claude provider-account broker materialization', () => {
  test('auth-material request carries only refs plus the agent bearer', () => {
    const req = claudeProviderAuthMaterialRequest(claudeProviderAuth)
    expect(req.url).toBe(`${claudeProviderAuth.baseUrl}${CLAUDE_PROVIDER_AUTH_MATERIAL_PATH}`)
    expect(req.headers.Authorization).toBe(`Bearer ${claudeProviderAuth.agentToken}`)
    expect(JSON.parse(req.body)).toEqual({
      authGrantRef: claudeProviderAuth.authGrantRef,
      providerAccountRef: claudeProviderAuth.providerAccountRef,
    })
    expect(req.body).not.toContain('claude-oauth-token')
  })

  test('materializes Claude OAuth only as CLAUDE_CODE_OAUTH_TOKEN env', async () => {
    const fetchImpl = (async () =>
      new Response(
        JSON.stringify({
          authMaterial: {
            authContentEnv: 'CLAUDE_CODE_OAUTH_TOKEN',
            authContentValue: 'claude-oauth-token-never-serialized',
          },
        }),
        { status: 200 },
      )) as unknown as typeof globalThis.fetch

    const result = await materializeClaudeProviderAuth(
      { providerAuth: claudeProviderAuth },
      fetchImpl,
    )

    expect(result.ok).toBe(true)
    if (!result.ok) throw new Error('unreachable')
    expect(result.env.CLAUDE_CODE_OAUTH_TOKEN).toBe('claude-oauth-token-never-serialized')
    expect(
      JSON.stringify({
        providerAccountRef: result.providerAccountRef,
        authGrantRef: result.authGrantRef,
      }),
    ).not.toContain('claude-oauth-token')
  })

  test('rejects malformed Claude auth material', async () => {
    const fetchImpl = (async () =>
      new Response(
        JSON.stringify({
          authMaterial: {
            authContentEnv: 'ANTHROPIC_API_KEY',
            authContentValue: 'wrong-shape',
          },
        }),
        { status: 200 },
      )) as unknown as typeof globalThis.fetch

    const result = await materializeClaudeProviderAuth(
      { providerAuth: claudeProviderAuth },
      fetchImpl,
    )

    expect(result).toEqual({
      ok: false,
      reasonRef: 'claude.provider_auth_material_invalid',
      status: 200,
    })
  })
})

describe('agent-computer turn-runner: Codex provider-account broker materialization', () => {
  test('auth-material request carries only refs plus the agent bearer', () => {
    const req = codexProviderAuthMaterialRequest(codexProviderAuth)
    expect(req.url).toBe(`${codexProviderAuth.baseUrl}${CODEX_PROVIDER_AUTH_MATERIAL_PATH}`)
    expect(req.headers.Authorization).toBe(`Bearer ${codexProviderAuth.agentToken}`)
    expect(JSON.parse(req.body)).toEqual({
      authGrantRef: codexProviderAuth.authGrantRef,
      providerAccountRef: codexProviderAuth.providerAccountRef,
    })
    expect(req.body).not.toContain('short-lived-access-token')
  })

  test('materializes short-lived OPENCODE auth into scratch CODEX_HOME only', async () => {
    const root = mkdtempSync(join(tmpdir(), 'agent-computer-codex-auth-'))
    try {
      const calls: Array<{ url: string; init: RequestInit }> = []
      const fetchImpl = (async (url: string | URL | Request, init?: RequestInit) => {
        calls.push({ init: init ?? {}, url: String(url) })
        return new Response(
          JSON.stringify({
            authMaterial: {
              authContentEnv: 'OPENCODE_AUTH_CONTENT',
              authContentJson: shortLivedAuthContent,
            },
          }),
          { status: 200 },
        )
      }) as unknown as typeof globalThis.fetch

      const result = await materializeCodexProviderAuth(
        {
          providerAuth: { ...codexProviderAuth, scratchRoot: root },
          turnId: 'turn-codex-1',
          nowMs: Date.parse('2026-07-08T12:00:00.000Z'),
        },
        fetchImpl,
      )

      expect(result.ok).toBe(true)
      if (!result.ok) throw new Error('unreachable')
      expect(result.codexHome).toBe(join(root, 'turn-codex-1', 'codex-home'))
      expect(result.env.CODEX_HOME).toBe(result.codexHome)
      expect(result.env.OPENCODE_AUTH_CONTENT).toBe(shortLivedAuthContent)
      expect(await readFile(result.authJsonPath, 'utf8')).toBe(`${shortLivedAuthContent}\n`)
      expect(JSON.stringify({
        providerAccountRef: result.providerAccountRef,
        authGrantRef: result.authGrantRef,
        expiresAt: result.expiresAt,
      })).not.toContain('short-lived-access-token')
      expect(calls).toHaveLength(1)
    } finally {
      await rm(root, { recursive: true, force: true })
    }
  })

  test('rejects refresh-bearing or expiring auth material before writing CODEX_HOME', async () => {
    expect(
      shortLivedOpenCodeAuthExpiresAt(
        JSON.stringify({ openai: { access: 'access', refresh: 'refresh', expires: 1 } }),
      ),
    ).toBeNull()

    const fetchImpl = (async () =>
      new Response(
        JSON.stringify({
          authMaterial: {
            authContentEnv: 'OPENCODE_AUTH_CONTENT',
            authContentJson: JSON.stringify({
              openai: {
                access: 'access',
                expires: Date.parse('2026-07-08T12:03:00.000Z'),
              },
            }),
          },
        }),
        { status: 200 },
      )) as unknown as typeof globalThis.fetch
    const result = await materializeCodexProviderAuth(
      {
        providerAuth: codexProviderAuth,
        turnId: 'turn-expiring',
        nowMs: Date.parse('2026-07-08T12:00:00.000Z'),
      },
      fetchImpl,
    )
    expect(result).toEqual({
      ok: false,
      reasonRef: 'codex.provider_auth_material_expiring',
      status: 200,
    })
  })

  test('reclaim receipts make redeemed provider grants non-replayable', () => {
    expect(
      canReplayCodexProviderGrantAfterReclaim({
        scratchWipeReceiptRef: 'sha256:scratch-wiped',
        microvmDestroyReceiptRef: 'sha256:microvm-destroyed',
      }),
    ).toBe(false)
  })
})

describe('agent-computer turn-runner: Codex continuity re-prime summary', () => {
  test('summarizes bounded Khala Sync replay and explicitly defers persisted CODEX_HOME', () => {
    const summary = codexContinuityResultSummary({
      maxReplayMessages: 24.8,
      persistedCodexHome: false,
      previousTurnCount: 3.2,
      strategy: 'khala_sync_history_reprime',
    })
    expect(summary).toEqual({
      maxReplayMessages: 24,
      persistedCodexHome: false,
      previousTurnCount: 3,
      replaySource: 'khala_sync_history',
      strategy: 'khala_sync_history_reprime',
    })
    expect(JSON.stringify(summary)).not.toMatch(/CODEX_HOME|token|secret|authJson/i)
  })

  test('omits continuity when the work-context has no continuity block', () => {
    expect(codexContinuityResultSummary(undefined)).toBeNull()
  })
})

describe('agent-computer turn-runner: usage + text parsing', () => {
  test('extracts exact usage from an OpenAI-shaped completion (prompt/completion_tokens)', () => {
    const usage = chatCompletionUsage({
      usage: { prompt_tokens: 41, completion_tokens: 17, total_tokens: 58 },
    })
    expect(usage).toEqual({
      inputTokens: 41,
      outputTokens: 17,
      totalTokens: 58,
      reasoningTokens: 0,
      cacheReadTokens: 0,
    })
  })

  test('falls back to input_tokens/output_tokens and clamps total to the sum', () => {
    const usage = chatCompletionUsage({
      usage: { input_tokens: 10, output_tokens: 5, total_tokens: 3 },
    })
    // total can never be below the exact input+output sum.
    expect(usage).toEqual({
      inputTokens: 10,
      outputTokens: 5,
      totalTokens: 15,
      reasoningTokens: 0,
      cacheReadTokens: 0,
    })
  })

  test('EXACT thinking model: folds reasoning out of completion so billable total is unchanged (#8503)', () => {
    // OpenAI convention: reasoning_tokens ⊆ completion_tokens. completion=17
    // includes reasoning=12; we split so output=5, reasoning=12, and the ingest
    // route re-sums output+reasoning = 17 (billable completion unchanged).
    const usage = chatCompletionUsage({
      usage: {
        prompt_tokens: 41,
        completion_tokens: 17,
        completion_tokens_details: { reasoning_tokens: 12 },
        prompt_tokens_details: { cached_tokens: 8 },
        total_tokens: 58,
      },
    })
    expect(usage).toEqual({
      inputTokens: 41,
      outputTokens: 5,
      reasoningTokens: 12,
      cacheReadTokens: 8,
      totalTokens: 58,
    })
    // Billable output the ingest route computes: output + reasoning === 17.
    expect(usage.outputTokens + usage.reasoningTokens).toBe(17)
  })

  test('unfolded reasoning (reasoning > completion) is carried additively, never lost', () => {
    const usage = chatCompletionUsage({
      usage: {
        prompt_tokens: 10,
        completion_tokens: 4,
        completion_tokens_details: { reasoning_tokens: 9 },
      },
    })
    // completion does NOT include reasoning here: output=4, reasoning=9, and the
    // route bills 4+9=13, total = 10+13 = 23. No tokens dropped.
    expect(usage).toEqual({
      inputTokens: 10,
      outputTokens: 4,
      reasoningTokens: 9,
      cacheReadTokens: 0,
      totalTokens: 23,
    })
  })

  test('caps cached_tokens at input tokens (cached ⊆ prompt)', () => {
    const usage = chatCompletionUsage({
      usage: {
        prompt_tokens: 6,
        completion_tokens: 3,
        prompt_tokens_details: { cached_tokens: 999 },
      },
    })
    expect(usage.cacheReadTokens).toBe(6)
  })

  test('extracts assistant text from choices[0].message.content', () => {
    expect(
      chatCompletionText({ choices: [{ message: { content: 'a staged proof note' } }] }),
    ).toBe('a staged proof note')
    expect(chatCompletionText({})).toBe('')
  })
})

describe('agent-computer turn-runner: request contracts', () => {
  test('chat/completions request mirrors runWithHostedKhalaGateway', () => {
    const req = chatCompletionsRequest(inference, 'describe the change')
    expect(req.url).toBe(`${inference.baseUrl}/v1/chat/completions`)
    expect(req.headers.Authorization).toBe(`Bearer ${inference.agentToken}`)
    expect(req.headers['x-openagents-client']).toBe('khala-code-mobile')
    expect(req.headers['x-openagents-demand-kind']).toBe('external')
    expect(req.headers['x-openagents-demand-source']).toBe(
      'khala_mobile_org_cloud_runtime',
    )
    const body = JSON.parse(req.body)
    expect(body).toEqual({
      messages: [{ content: 'describe the change', role: 'user' }],
      model: inference.model,
      stream: false,
    })
    // No secret configured => the single-charge header is NOT sent (fail-closed).
    expect(
      req.headers[INFERENCE_ORG_CLOUD_RUNTIME_NO_METER_HEADER],
    ).toBeUndefined()
  })

  test('single-charge: sends the no-meter header only when a secret is configured (#8503)', () => {
    const req = chatCompletionsRequest(
      { ...inference, noMeterSecret: 's3cr3t-org-cloud' },
      'x',
    )
    expect(req.headers[INFERENCE_ORG_CLOUD_RUNTIME_NO_METER_HEADER]).toBe(
      's3cr3t-org-cloud',
    )
    // The secret is a header value only; the request body never carries it.
    expect(req.body).not.toContain('s3cr3t-org-cloud')
  })

  test('usage ingest body matches the KhalaCloudRuntimeUsageIngestBody schema', () => {
    const body = usageIngestBody({
      inference,
      threadId: 'scope.thread.proof',
      turnId: 'turn-microvm-1',
      observedAt: '2026-07-07T00:00:00.000Z',
      usage: {
        inputTokens: 41,
        outputTokens: 17,
        totalTokens: 58,
        reasoningTokens: 0,
        cacheReadTokens: 0,
      },
      usageRef: 'usage.hosted_khala.abc',
      runtimeEventId: 'evt-1',
    })
    expect(body).toMatchObject({
      schemaVersion: KHALA_CLOUD_RUNTIME_USAGE_SCHEMA_VERSION,
      ownerUserId: 'github:300914913',
      threadId: 'scope.thread.proof',
      turnId: 'turn-microvm-1',
      lane: AGENT_COMPUTER_RECEIPT_LANE,
      provider: AGENT_COMPUTER_DEFAULT_PROVIDER,
      model: inference.model,
      backendProfile: 'omega-hosted-gemini',
      pylonRef: 'pylon.agent-computer.proof',
      runtimeEventId: 'evt-1',
      observedAt: '2026-07-07T00:00:00.000Z',
    })
    expect(body.usage).toEqual({
      usageRef: 'usage.hosted_khala.abc',
      inputTokens: 41,
      outputTokens: 17,
      reasoningTokens: 0,
      cacheReadInputTokens: 0,
      cacheWriteInputTokens: 0,
      totalTokens: 58,
    })
    // The `lane` literal MUST be one the ingest route accepts.
    expect(['codex_app_server', 'claude_pylon', 'hosted_khala']).toContain(
      body.lane,
    )
  })

  test('ingest body carries EXACT reasoning + cache-read tokens (#8503)', () => {
    const body = usageIngestBody({
      inference,
      threadId: 'scope.thread.proof',
      turnId: 'turn-microvm-2',
      observedAt: '2026-07-07T00:00:00.000Z',
      usage: chatCompletionUsage({
        usage: {
          prompt_tokens: 41,
          completion_tokens: 17,
          completion_tokens_details: { reasoning_tokens: 12 },
          prompt_tokens_details: { cached_tokens: 8 },
          total_tokens: 58,
        },
      }),
      usageRef: 'usage.hosted_khala.def',
    })
    expect(body.usage).toEqual({
      usageRef: 'usage.hosted_khala.def',
      inputTokens: 41,
      outputTokens: 5,
      reasoningTokens: 12,
      cacheReadInputTokens: 8,
      cacheWriteInputTokens: 0,
      totalTokens: 58,
    })
  })
})

describe('agent-computer turn-runner: runModelTurnReceipt (mock fetch)', () => {
  test('happy path: inference then exact usage receipt, token never serialized', async () => {
    const calls: Array<{ url: string; init: RequestInit }> = []
    const fetchImpl = (async (url: string | URL | Request, init?: RequestInit) => {
      const u = String(url)
      calls.push({ url: u, init: init ?? {} })
      if (u.endsWith('/v1/chat/completions')) {
        return new Response(
          JSON.stringify({
            choices: [{ message: { content: 'Added AGENT_COMPUTER_TURN.md.' } }],
            usage: { prompt_tokens: 41, completion_tokens: 17, total_tokens: 58 },
          }),
          { status: 200, headers: { 'content-type': 'application/json' } },
        )
      }
      if (u.endsWith(KHALA_CLOUD_RUNTIME_USAGE_INGEST_PATH)) {
        return new Response(
          JSON.stringify({
            insertedTokenUsage: true,
            tokenUsageEventRef: 'event.inference.served-tokens.khala-cloud-runtime.deadbeef',
            tokensServedDelta: 58,
          }),
          { status: 200, headers: { 'content-type': 'application/json' } },
        )
      }
      return new Response('not found', { status: 404 })
    }) as unknown as typeof globalThis.fetch

    const result = await runModelTurnReceipt(
      {
        inference,
        threadId: 'scope.thread.proof',
        turnId: 'turn-microvm-1',
        instructions: 'describe the change',
        runtimeEventId: 'evt-1',
      },
      fetchImpl,
    )

    expect(result.ok).toBe(true)
    if (result.ok) {
      expect(result.usage).toEqual({
        inputTokens: 41,
        outputTokens: 17,
        totalTokens: 58,
        reasoningTokens: 0,
        cacheReadTokens: 0,
      })
      expect(result.insertedTokenUsage).toBe(true)
      expect(result.tokenUsageEventRef).toContain('served-tokens.khala-cloud-runtime')
      expect(result.tokensServedDelta).toBe(58)
      expect(result.text).toBe('Added AGENT_COMPUTER_TURN.md.')
    }

    // Both legs authenticated by the agent bearer; the token is in the header
    // but never in a serialized result field.
    expect(calls).toHaveLength(2)
    expect(JSON.stringify(result)).not.toContain(inference.agentToken)
    const ingestCall = calls.find((c) => c.url.endsWith(KHALA_CLOUD_RUNTIME_USAGE_INGEST_PATH))
    expect(ingestCall).toBeDefined()
    expect((ingestCall!.init.headers as Record<string, string>).Authorization).toBe(
      `Bearer ${inference.agentToken}`,
    )
  })

  test('no receipt when the gateway omits exact usage', async () => {
    const fetchImpl = (async () =>
      new Response(JSON.stringify({ choices: [{ message: { content: 'x' } }], usage: {} }), {
        status: 200,
      })) as unknown as typeof globalThis.fetch
    const result = await runModelTurnReceipt(
      { inference, threadId: 't', turnId: 'u', instructions: 'x' },
      fetchImpl,
    )
    expect(result.ok).toBe(false)
    if (!result.ok) expect(result.stage).toBe('no_exact_usage')
  })

  test('fail-soft on a gateway HTTP error (never throws)', async () => {
    const fetchImpl = (async () =>
      new Response('unauthorized', { status: 401 })) as unknown as typeof globalThis.fetch
    const result = await runModelTurnReceipt(
      { inference, threadId: 't', turnId: 'u', instructions: 'x' },
      fetchImpl,
    )
    expect(result.ok).toBe(false)
    if (!result.ok) {
      expect(result.stage).toBe('inference')
      expect(result.status).toBe(401)
    }
  })

  test('surfaces a usage-receipt rejection reason without throwing', async () => {
    const fetchImpl = (async (url: string | URL | Request) => {
      const u = String(url)
      if (u.endsWith('/v1/chat/completions')) {
        return new Response(
          JSON.stringify({ usage: { prompt_tokens: 5, completion_tokens: 5 } }),
          { status: 200 },
        )
      }
      return new Response(JSON.stringify({ reason: 'owner mismatch' }), { status: 403 })
    }) as unknown as typeof globalThis.fetch
    const result = await runModelTurnReceipt(
      { inference, threadId: 't', turnId: 'u', instructions: 'x' },
      fetchImpl,
    )
    expect(result.ok).toBe(false)
    if (!result.ok) {
      expect(result.stage).toBe('usage_receipt')
      expect(result.status).toBe(403)
      expect(result.error).toBe('owner mismatch')
    }
  })
})

// ---------------------------------------------------------------------------
// MM-C5 (#8477): branch/PR writeback under the user's GitHub authorization.
// ---------------------------------------------------------------------------

const writeback: WritebackConfig = {
  baseBranch: 'main',
  branch: 'pylon/agent-computer-turn-proof-1',
  ingestPath: KHALA_AGENT_COMPUTER_WRITEBACK_INGEST_PATH,
  mode: 'pull_request',
  repositoryFullName: 'AgentFlampy/agent-computer-proof',
}

const BROKER_TOKEN = 'gho_fake_broker_token_never_serialized'

/** A gitRun spy that records every invocation and always succeeds. */
const recordingGitRun = () => {
  const calls: Array<string[]> = []
  const gitRun: GitRun = (_cwd, args) => {
    calls.push(args)
    return { code: 0, stderr: '', stdout: '' }
  }
  return { calls, gitRun }
}

/** A fetch fake routing broker / GitHub-API / writeback-ingest calls. */
const writebackFetch = (opts: {
  broker?: Response
  pullsPost?: Response
  pullsList?: Response
  ingest?: Response
} = {}) => {
  const calls: Array<{ url: string; init: RequestInit }> = []
  const fetchImpl = (async (url: string | URL | Request, init?: RequestInit) => {
    const u = String(url)
    calls.push({ init: init ?? {}, url: u })
    if (u.endsWith(GITHUB_SCM_AUTH_BROKER_PATH)) {
      return (
        opts.broker ??
        new Response(
          JSON.stringify({ username: 'x-access-token', password: BROKER_TOKEN, expiresAt: '2026-07-07T00:05:00Z' }),
          { status: 200 },
        )
      )
    }
    if (u.includes('/pulls?')) {
      return opts.pullsList ?? new Response(JSON.stringify([]), { status: 200 })
    }
    if (u.endsWith('/pulls')) {
      return (
        opts.pullsPost ??
        new Response(
          JSON.stringify({ html_url: 'https://github.com/AgentFlampy/agent-computer-proof/pull/7', number: 7 }),
          { status: 201 },
        )
      )
    }
    if (u.endsWith(KHALA_AGENT_COMPUTER_WRITEBACK_INGEST_PATH)) {
      return (
        opts.ingest ??
        new Response(
          JSON.stringify({ ok: true, decision: 'recorded', eventId: 'event.private.agent_computer.writeback.abc' }),
          { status: 200 },
        )
      )
    }
    return new Response('not found', { status: 404 })
  }) as unknown as typeof globalThis.fetch
  return { calls, fetchImpl }
}

const runWriteback = (
  fetchImpl: typeof globalThis.fetch,
  gitRun: GitRun,
  overrides: Partial<Parameters<typeof runWritebackForTurn>[0]> = {},
) =>
  runWritebackForTurn(
    {
      changedFileCount: 1,
      commitMessage: 'chore(agent-computer): staged change',
      inference,
      prBody: 'body',
      prTitle: 'Agent Computer: proof',
      turnId: 'turn-proof-1',
      workingDirectory: '/tmp/ws',
      writeback,
      ...overrides,
    },
    { fetchImpl, gitRun },
  )

describe('turn-runner writeback: request contracts', () => {
  test('parseRepoFullName splits owner/name and rejects malformed', () => {
    expect(parseRepoFullName('AgentFlampy/agent-computer-proof')).toEqual({
      name: 'agent-computer-proof',
      owner: 'AgentFlampy',
    })
    expect(parseRepoFullName('no-slash')).toBeNull()
    expect(parseRepoFullName('a/b/c')).toBeNull()
  })

  test('git credential broker request matches the #8475 broker contract', () => {
    const req = gitCredentialBrokerRequest(inference, 'AgentFlampy/agent-computer-proof')!
    expect(req.url).toBe(`${inference.baseUrl}${GITHUB_SCM_AUTH_BROKER_PATH}`)
    expect(req.headers.Authorization).toBe(`Bearer ${inference.agentToken}`)
    const body = JSON.parse(req.body)
    expect(body).toEqual({
      authRefs: [`github-identity:token:${inference.ownerUserId}`],
      helperRef: GITHUB_SCM_AUTH_BROKER_HELPER_REF,
      host: 'github.com',
      path: '/AgentFlampy/agent-computer-proof',
      protocol: 'https',
      repositoryRef: 'repo.github/AgentFlampy/agent-computer-proof',
      schema: GITHUB_SCM_AUTH_BROKER_REQUEST_SCHEMA,
    })
  })

  test('classifyPushFailure maps stderr to public-safe reason refs', () => {
    expect(classifyPushFailure('remote: Permission to o/n denied to user')).toBe(
      'writeback.permission.github_write_permission_missing',
    )
    expect(classifyPushFailure('! [rejected] main -> main (non-fast-forward)')).toBe(
      'writeback.branch_update_rejected',
    )
    expect(classifyPushFailure('some other transient error')).toBe('writeback.push_failed')
  })

  test('brokerGitCredential maps HTTP statuses to reason refs', async () => {
    const at = async (status: number) => {
      const { fetchImpl } = writebackFetch({ broker: new Response('{}', { status }) })
      return brokerGitCredential(inference, 'o/n', fetchImpl)
    }
    expect((await at(409) as { reasonRef: string }).reasonRef).toBe(
      'writeback.permission.github_write_connection_required',
    )
    expect((await at(403) as { reasonRef: string }).reasonRef).toBe(
      'writeback.permission.github_write_permission_missing',
    )
    expect((await at(401) as { reasonRef: string }).reasonRef).toBe(
      'writeback.permission.github_write_connection_unusable',
    )
  })
})

describe('turn-runner writeback: runWritebackForTurn', () => {
  test('happy pull_request: broker -> commit -> push (no force) -> PR -> record', async () => {
    const { calls, fetchImpl } = writebackFetch()
    const { calls: gitCalls, gitRun } = recordingGitRun()
    const result = await runWriteback(fetchImpl, gitRun)

    expect(result.outcome.status).toBe('pull_request_opened')
    expect(result.outcome.pullRequestUrl).toBe(
      'https://github.com/AgentFlampy/agent-computer-proof/pull/7',
    )
    expect(result.outcome.pullRequestNumber).toBe(7)
    expect(result.outcome.changedFileCount).toBe(1)
    expect(result.recorded).toBe(true)
    expect(result.recordDecision).toBe('recorded')

    // Exactly one push, to the scoped branch, NEVER force, NEVER the base.
    const pushCalls = gitCalls.filter(a => a[0] === 'push')
    expect(pushCalls).toHaveLength(1)
    const push = pushCalls[0]!
    expect(push).toContain('HEAD:refs/heads/pylon/agent-computer-turn-proof-1')
    expect(push).not.toContain('--force')
    expect(push).not.toContain('-f')
    expect(push.join(' ')).not.toContain('main:refs/heads/main')

    // A real commit happened before the push.
    expect(gitCalls.some(a => a.includes('commit'))).toBe(true)

    // The ingest body matches the server schema and the owner attribution.
    const ingest = calls.find(c => c.url.endsWith(KHALA_AGENT_COMPUTER_WRITEBACK_INGEST_PATH))!
    const ingestBody = JSON.parse(ingest.init.body as string)
    expect(ingestBody.schemaVersion).toBe(KHALA_AGENT_COMPUTER_WRITEBACK_SCHEMA_VERSION)
    expect(ingestBody.ownerUserId).toBe(inference.ownerUserId)
    expect(ingestBody.turnId).toBe('turn-proof-1')
    expect(ingestBody.outcome.status).toBe('pull_request_opened')

    // The brokered token and the agent bearer never appear in the outcome/result.
    expect(JSON.stringify(result.outcome)).not.toContain(BROKER_TOKEN)
    expect(JSON.stringify(result)).not.toContain(BROKER_TOKEN)
    expect(JSON.stringify(result)).not.toContain(inference.agentToken)
    // git push URL embedded the token but never in a git argv we serialize here.
    expect(JSON.stringify(gitCalls)).toContain(BROKER_TOKEN) // sanity: it IS in the git argv
  })

  test('branch_only mode pushes the branch but never opens a PR', async () => {
    const { calls, fetchImpl } = writebackFetch()
    const { calls: gitCalls, gitRun } = recordingGitRun()
    const result = await runWriteback(fetchImpl, gitRun, {
      writeback: { ...writeback, mode: 'branch_only' },
    })
    expect(result.outcome.status).toBe('branch_pushed')
    expect(result.outcome.pullRequestUrl).toBeUndefined()
    expect(calls.some(c => c.url.endsWith('/pulls'))).toBe(false)
    expect(gitCalls.filter(a => a[0] === 'push')).toHaveLength(1)
    expect(result.recorded).toBe(true)
  })

  test('broker failure => typed failed outcome, no push, still recorded', async () => {
    const { calls, fetchImpl } = writebackFetch({ broker: new Response('{}', { status: 409 }) })
    const { calls: gitCalls, gitRun } = recordingGitRun()
    const result = await runWriteback(fetchImpl, gitRun)
    expect(result.outcome.status).toBe('failed')
    expect(result.outcome.reasonRef).toBe('writeback.permission.github_write_connection_required')
    expect(gitCalls.some(a => a[0] === 'push')).toBe(false)
    // the failed outcome is still POSTed so the thread shows an honest state.
    expect(calls.some(c => c.url.endsWith(KHALA_AGENT_COMPUTER_WRITEBACK_INGEST_PATH))).toBe(true)
  })

  test('push permission failure => typed failed outcome (raw stderr never surfaced)', async () => {
    const { fetchImpl } = writebackFetch()
    const gitRun: GitRun = (_cwd, args) => {
      if (args[0] === 'push') {
        return { code: 1, stderr: 'remote: Permission to AgentFlampy/x denied to bot.', stdout: '' }
      }
      return { code: 0, stderr: '', stdout: '' }
    }
    const result = await runWriteback(fetchImpl, gitRun)
    expect(result.outcome.status).toBe('failed')
    expect(result.outcome.reasonRef).toBe('writeback.permission.github_write_permission_missing')
    expect(JSON.stringify(result)).not.toContain('denied to bot')
  })

  test('non-scoped branch is rejected before any broker/push', async () => {
    const { calls, fetchImpl } = writebackFetch()
    const { calls: gitCalls, gitRun } = recordingGitRun()
    const result = await runWriteback(fetchImpl, gitRun, {
      writeback: { ...writeback, branch: 'main' },
    })
    expect(result.outcome.status).toBe('failed')
    expect(result.outcome.reasonRef).toBe('writeback.branch_refspec_rejected')
    expect(calls.some(c => c.url.endsWith(GITHUB_SCM_AUTH_BROKER_PATH))).toBe(false)
    expect(gitCalls.some(a => a[0] === 'push')).toBe(false)
  })

  test('existing PR (422) => reused, links the open PR', async () => {
    const { fetchImpl } = writebackFetch({
      pullsPost: new Response(JSON.stringify({ message: 'A pull request already exists' }), { status: 422 }),
      pullsList: new Response(
        JSON.stringify([{ html_url: 'https://github.com/AgentFlampy/agent-computer-proof/pull/3', number: 3 }]),
        { status: 200 },
      ),
    })
    const { gitRun } = recordingGitRun()
    const result = await runWriteback(fetchImpl, gitRun)
    expect(result.outcome.status).toBe('pull_request_reused')
    expect(result.outcome.pullRequestNumber).toBe(3)
  })

  test('PR open failure after a successful push degrades to branch_pushed', async () => {
    const { fetchImpl } = writebackFetch({
      pullsPost: new Response('{}', { status: 500 }),
      pullsList: new Response(JSON.stringify([]), { status: 200 }),
    })
    const { gitRun } = recordingGitRun()
    const result = await runWriteback(fetchImpl, gitRun)
    expect(result.outcome.status).toBe('branch_pushed')
    expect(result.outcome.pullRequestUrl).toBeUndefined()
  })

  test('a rejected ingest (server gate blocked) surfaces recorded=false honestly', async () => {
    const { fetchImpl } = writebackFetch({
      ingest: new Response(
        JSON.stringify({ ok: false, decision: 'permission_blocked', reason: 'github_write_connection_required' }),
        { status: 200 },
      ),
    })
    const { gitRun } = recordingGitRun()
    const result = await runWriteback(fetchImpl, gitRun)
    // the microVM pushed, but the server gate blocked the success record.
    expect(result.outcome.status).toBe('pull_request_opened')
    expect(result.recorded).toBe(false)
    expect(result.recordDecision).toBe('permission_blocked')
  })
})
