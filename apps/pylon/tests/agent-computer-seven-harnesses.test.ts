import { describe, expect, test } from 'vite-plus/test'
import {
  AGENT_COMPUTER_DEFAULT_GEMINI_MODEL,
  AGENT_COMPUTER_HARNESS_IDS,
  classifyCodexExecFailure,
  classifyHarnessFailure,
  HARNESS_RUNTIME_SECRET_MATERIAL_PATH,
  HARNESS_RUNTIME_SECRET_MATERIAL_SCHEMA,
  harnessExecArgs,
  harnessExecEnv,
  runHarnessTurn,
  runVerificationCommand,
  scanStagedDiffForCredentialMaterial,
  validateVerificationCommand,
  type HarnessExecRun,
  type VerificationRun,
} from '../deploy/agent-computer/turn-runner.ts'

const verificationOutputFixture: VerificationRun = () => ({
  code: 0,
  stderr: 'private stderr token',
  stdout: 'private stdout and path /work/repo',
  truncated: false,
})

describe('Agent Computer seven-harness runtime (#9193)', () => {
  const runtimeGrant = {
    agentToken: 'short-lived-agent-bearer',
    baseUrl: 'https://openagents.example',
    grantRef: 'grant.provider-secret.gemini.turn-1',
    kind: 'gemini_api_key' as const,
    providerAccountRef: 'provider-account.google-gemini.owner',
    runnerSessionId: 'runner-session.turn-1',
    secretRef: 'secret-manager:openagents-gemini-api-key',
  }
  const materialFetch = (secret: string) =>
    (async (url: string | URL | Request, init?: RequestInit) => {
      expect(String(url)).toBe(
        `${runtimeGrant.baseUrl}${HARNESS_RUNTIME_SECRET_MATERIAL_PATH}`,
      )
      expect(JSON.stringify(init)).not.toContain(secret)
      expect(JSON.parse(String(init?.body))).toEqual({
        grantRef: runtimeGrant.grantRef,
        kind: runtimeGrant.kind,
        providerAccountRef: runtimeGrant.providerAccountRef,
        runnerSessionId: runtimeGrant.runnerSessionId,
        secretRef: runtimeGrant.secretRef,
      })
      expect((init?.headers ?? {}) as Record<string, string>).toHaveProperty(
        'authorization',
        `Bearer ${runtimeGrant.agentToken}`,
      )
      return new Response(JSON.stringify({
        schemaVersion: HARNESS_RUNTIME_SECRET_MATERIAL_SCHEMA,
        grantRef: runtimeGrant.grantRef,
        providerAccountRef: runtimeGrant.providerAccountRef,
        runnerSessionId: runtimeGrant.runnerSessionId,
        secretRef: runtimeGrant.secretRef,
        secretValue: secret,
      }), { headers: { 'cache-control': 'private, no-store' }, status: 200 })
    }) as unknown as typeof globalThis.fetch

  test('declares all seven harness identities', () => {
    expect(AGENT_COMPUTER_HARNESS_IDS).toEqual([
      'codex',
      'claude-code',
      'cursor',
      'goose',
      'opencode',
      'pi',
      'grok',
    ])
  })

  test.each(['pi', 'opencode', 'goose'] as const)(
    '%s receives only its runtime Gemini key and pinned model',
    harness => {
      const secret = 'gemini-runtime-secret-never-serialized'
      const env = harnessExecEnv({
        harness,
        runtimeSecret: {
          kind: 'gemini_api_key',
          secretRef: 'secret-manager:openagents-gemini-api-key',
          value: secret,
        },
      })
      const args = harnessExecArgs({ harness, prompt: 'make the change', workingDirectory: '/workspace' })
      expect(Object.values(env ?? {})).toContain(secret)
      if (harness === 'goose') {
        expect(env?.GOOSE_MODEL).toBe(AGENT_COMPUTER_DEFAULT_GEMINI_MODEL)
      } else {
        expect(JSON.stringify(args)).toContain(AGENT_COMPUTER_DEFAULT_GEMINI_MODEL)
      }
      expect(Object.keys(env ?? {}).some(key => key.includes('TOKEN'))).toBe(false)
    },
  )

  test('Claude receives only its brokered subscription credential', () => {
    expect(harnessExecEnv({
      harness: 'claude-code',
      claudeProviderEnv: { CLAUDE_CODE_OAUTH_TOKEN: 'claude-secret', UNRELATED: 'drop-me' },
    })).toEqual(expect.objectContaining({ CLAUDE_CODE_OAUTH_TOKEN: 'claude-secret' }))
  })

  test.each(['cursor', 'grok'] as const)(
    '%s stays fail-closed until its provider broker exists',
    async harness => {
      expect(await runHarnessTurn(
        {
          config: { harness },
          prompt: 'bounded objective',
          workingDirectory: '/work',
        },
        { existsImpl: () => true },
      )).toMatchObject({
        ok: false,
        reasonRef: 'harness.runtime_secret_broker_unsupported',
      })
    },
  )

  test('Gemini Pi materializes inside the guest and excludes all secret bytes', async () => {
    const observed: Array<Parameters<HarnessExecRun>[0]> = []
    const secret = 'gemini-runtime-secret-never-serialized'
    const outcome = await runHarnessTurn(
      {
        config: {
          harness: 'pi',
          runtimeSecretGrant: runtimeGrant,
        },
        prompt: 'private objective prompt',
        workingDirectory: '/private/workspace',
      },
      {
        execRun: input => {
          observed.push(input)
          return { code: 0, stdout: 'private output', stderr: 'private stderr' }
        },
        existsImpl: () => true,
        fetchImpl: materialFetch(secret),
      },
    )
    expect(observed[0]?.env.GEMINI_API_KEY).toBe(secret)
    const serialized = JSON.stringify(outcome)
    for (const forbidden of [secret, 'private objective', 'private output', 'private stderr', '/private/workspace']) {
      expect(serialized).not.toContain(forbidden)
    }
    expect(JSON.stringify(runtimeGrant)).not.toContain(secret)
  })

  test('harness failures are typed and redacted', async () => {
    expect(classifyHarnessFailure('HTTP 429 rate limit exceeded', 1)).toBe('provider_capacity')
    const secret = `AIza${'z'.repeat(36)}`
    const outcome = await runHarnessTurn(
      {
        config: {
          harness: 'opencode',
          runtimeSecretGrant: runtimeGrant,
        },
        prompt: 'prompt',
        workingDirectory: '/work',
      },
      {
        execRun: () => ({ code: 1, stdout: 'raw stdout', stderr: `unauthorized ${secret}` }),
        existsImpl: () => true,
        fetchImpl: materialFetch(secret),
      },
    )
    expect(outcome).toMatchObject({
      ok: false,
      reasonRef: 'harness.exec_failed',
      failureClass: 'auth_required',
    })
    expect(JSON.stringify(outcome)).not.toContain(secret)
    expect(JSON.stringify(outcome)).not.toContain('raw stdout')
  })

  test('rejects a broker response with changed refs before process spawn', async () => {
    let spawned = false
    const outcome = await runHarnessTurn(
      {
        config: { harness: 'pi', runtimeSecretGrant: runtimeGrant },
        prompt: 'prompt',
        workingDirectory: '/work',
      },
      {
        existsImpl: () => true,
        execRun: () => {
          spawned = true
          return { code: 0, stderr: '', stdout: '' }
        },
        fetchImpl: (async () =>
          new Response(JSON.stringify({
            schemaVersion: HARNESS_RUNTIME_SECRET_MATERIAL_SCHEMA,
            grantRef: 'grant.changed',
            providerAccountRef: runtimeGrant.providerAccountRef,
            runnerSessionId: runtimeGrant.runnerSessionId,
            secretRef: runtimeGrant.secretRef,
            secretValue: 'secret-never-used',
          }), { headers: { 'cache-control': 'no-store' }, status: 200 })) as typeof globalThis.fetch,
      },
    )
    expect(outcome).toMatchObject({
      ok: false,
      reasonRef: 'harness.runtime_secret_material_invalid',
    })
    expect(spawned).toBe(false)
    expect(JSON.stringify(outcome)).not.toContain('secret-never-used')
  })

  test.each([
    ['quota exhausted for this account', 1, 'account_exhausted'],
    ['HTTP 429 rate limit; retry after 12s', 1, 'account_rate_limited'],
    ['authentication failed: invalid token', 1, 'auth_rejected'],
    ['requested model is unavailable', 1, 'model_unavailable'],
    ['network error: resolve host failed', 1, 'network_failed'],
    ['process timed out', null, 'exec_timeout'],
    ['unexpected native failure', 2, 'exec_failed'],
  ] as const)('classifies Codex failure without raw output: %s', (stderr, code, expected) => {
    expect(classifyCodexExecFailure(stderr, code)).toBe(expected)
  })
})

describe('Agent Computer managed verification and credential scan', () => {
  const command = {
    argv: ['git', 'diff', '--cached', '--check'],
    commandRef: 'verify.git.cached-diff',
  }

  test('accepts direct staged-diff verification and rejects shell or control input', () => {
    expect(validateVerificationCommand(command)).toBe(true)
    expect(validateVerificationCommand({
      argv: ['sh', '-c', 'git diff --cached --check'],
      commandRef: 'verify.shell',
    })).toBe(false)
    expect(validateVerificationCommand({
      argv: ['git', 'diff\nwhoami'],
      commandRef: 'verify.control',
    })).toBe(false)
  })

  test('verification receipt keeps only bounded counts and digests', () => {
    const receipt = runVerificationCommand(command, '/work/repo', verificationOutputFixture)
    expect(receipt).toMatchObject({ commandRef: command.commandRef, ok: true, exitCode: 0 })
    expect(JSON.stringify(receipt)).not.toContain('private stderr token')
    expect(JSON.stringify(receipt)).not.toContain('private stdout')
    expect(JSON.stringify(receipt)).not.toContain('/work/repo')
  })

  test('fails closed for missing, invalid, failed, and truncated verification', () => {
    expect(runVerificationCommand(undefined, '/work')).toMatchObject({
      ok: false,
      reasonRef: 'verification.command_missing',
    })
    expect(runVerificationCommand(
      { argv: ['bash', '-c', 'true'], commandRef: 'verify.invalid' },
      '/work',
    )).toMatchObject({ ok: false, reasonRef: 'verification.command_invalid' })
    expect(runVerificationCommand(command, '/work', () => ({
      code: 1, stderr: 'failure', stdout: '', truncated: false,
    }))).toMatchObject({ ok: false, reasonRef: 'verification.command_failed' })
    expect(runVerificationCommand(command, '/work', () => ({
      code: 0, stderr: '', stdout: '', truncated: true,
    }))).toMatchObject({ ok: false, reasonRef: 'verification.command_failed', truncated: true })
  })

  test('credential scan emits only typed finding refs', () => {
    const secret = `AIza${'x'.repeat(36)}`
    const findings = scanStagedDiffForCredentialMaterial(`+export const value = '${secret}'`)
    expect(findings).toEqual(['credential.google_api_key'])
    expect(JSON.stringify(findings)).not.toContain(secret)
    expect(scanStagedDiffForCredentialMaterial('+const key = process.env.GEMINI_API_KEY')).toEqual([])
  })
})
