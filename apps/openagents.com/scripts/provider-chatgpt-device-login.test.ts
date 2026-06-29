import { execFile } from 'node:child_process'
import {
  createServer,
  type IncomingMessage,
  type ServerResponse,
} from 'node:http'
import { fileURLToPath } from 'node:url'

import { afterEach, describe, expect, test } from 'vitest'

const scriptPath = fileURLToPath(
  new URL('./provider-chatgpt-device-login.mjs', import.meta.url),
)

type JsonValue =
  | null
  | boolean
  | number
  | string
  | ReadonlyArray<JsonValue>
  | { readonly [key: string]: JsonValue }

const writeJson = (
  response: ServerResponse,
  status: number,
  payload: JsonValue,
) => {
  response.writeHead(status, {
    'content-type': 'application/json',
  })
  response.end(JSON.stringify(payload))
}

const readBody = async (request: IncomingMessage): Promise<unknown> => {
  const chunks: Array<Buffer> = []

  for await (const chunk of request) {
    chunks.push(Buffer.from(chunk))
  }

  const text = Buffer.concat(chunks).toString('utf8')

  return text === '' ? {} : JSON.parse(text)
}

const startMockOperatorApi = async () => {
  const requests: Array<
    Readonly<{ method: string; path: string; body: unknown }>
  > = []
  const server = createServer(async (request, response) => {
    const path = request.url ?? '/'
    const body =
      request.method === 'POST' ? await readBody(request) : ({} as unknown)
    requests.push({ method: request.method ?? 'GET', path, body })

    if (
      path ===
      '/api/operator/provider-accounts/chatgpt-codex/device-login/start'
    ) {
      writeJson(response, 201, {
        status: 'pending',
        targetUser: {
          userId: 'user_chris',
          email: 'chris@openagents.com',
        },
        accountLabel: 'account 1',
        attemptId: 'provider_attempt_1',
        providerAccountRef: 'provider-account_ref_1',
        verificationUrl: 'https://auth.openai.com/codex/device',
        userCode: 'ABCD-EFGH',
        expiresAt: '2099-01-01T00:00:00.000Z',
        intervalSeconds: 5,
        nextPollCommand:
          'node scripts/provider-chatgpt-device-login.mjs poll provider_attempt_1',
      })

      return
    }

    if (
      path ===
      '/api/operator/provider-accounts/chatgpt-codex/device-login/provider_attempt_pending'
    ) {
      writeJson(response, 200, {
        status: 'pending',
        failureReason: null,
        attemptId: 'provider_attempt_pending',
        providerAccountRef: 'provider-account_ref_1',
        providerAccountStatus: 'pending',
        providerAccountHealth: 'unknown',
        accountLabel: 'account 1',
        expiresAt: '2099-01-01T00:00:00.000Z',
        completedAt: null,
        failedAt: null,
      })

      return
    }

    if (
      path ===
      '/api/operator/provider-accounts/chatgpt-codex/device-login/provider_attempt_connected'
    ) {
      writeJson(response, 200, {
        status: 'connected',
        failureReason: null,
        attemptId: 'provider_attempt_connected',
        providerAccountRef: 'provider-account_ref_1',
        providerAccountStatus: 'connected',
        providerAccountHealth: 'healthy',
        accountLabel: 'account 1',
        expiresAt: '2099-01-01T00:00:00.000Z',
        completedAt: '2026-06-05T00:00:00.000Z',
        failedAt: null,
      })

      return
    }

    if (
      path ===
      '/api/operator/provider-accounts/chatgpt-codex/device-login/provider_attempt_expired'
    ) {
      writeJson(response, 200, {
        status: 'expired',
        failureReason: 'device_login_expired',
        attemptId: 'provider_attempt_expired',
        providerAccountRef: 'provider-account_ref_1',
        providerAccountStatus: 'expired',
        providerAccountHealth: 'requires_reauth',
        accountLabel: 'account 1',
        expiresAt: '2026-06-05T00:00:00.000Z',
        completedAt: null,
        failedAt: '2026-06-05T00:00:00.000Z',
      })

      return
    }

    if (path === '/api/operator/provider-accounts/chatgpt-codex/sanity') {
      writeJson(response, 200, {
        summary: {
          total: 5,
          healthy: 4,
          requiresAttention: 1,
          collisionCount: 0,
        },
        probeRunId: 'provider_parallel_probe_run_1',
        parallel: 5,
        checks: [
          {
            classification: 'healthy',
            providerAccountRef: 'provider-account_ref_1',
            accountLabel: 'account 1',
            summary: 'ChatGPT/Codex sanity check passed.',
            probeId: 'provider_probe_1',
            leaseId: 'provider_probe_lease_1',
            collisionClass: 'none',
          },
          {
            classification: 'rate_limited',
            providerAccountRef: 'provider-account_ref_2',
            accountLabel: 'account 2',
            summary: 'ChatGPT/Codex account is currently rate limited.',
            probeId: 'provider_probe_2',
            leaseId: 'provider_probe_lease_2',
            collisionClass: 'none',
          },
        ],
      })

      return
    }

    if (path === '/api/operator/provider-accounts/chatgpt-codex/leases') {
      writeJson(response, 201, {
        leaseRef: 'provider-account-lease_ref_1',
        providerAccountRef: 'provider-account_ref_1',
        accountLabel: 'account 1',
        requestedAction: 'customer_order_fulfillment',
        selectedByPolicyVersion: 'provider-account-lease-policy:v1',
        selectionReason:
          'Selected connected healthy account with 0 active lease(s), priority 100, and no cooldown or low-credit flag.',
        startedAt: '2026-06-05T00:00:00.000Z',
        expiresAt: '2026-06-05T00:15:00.000Z',
      })

      return
    }

    if (
      path === '/api/operator/provider-accounts/chatgpt-codex/leases/explain'
    ) {
      writeJson(response, 200, {
        status: 'selected',
        providerAccountRef: 'provider-account_ref_1',
        accountLabel: 'account 1',
        selectedByPolicyVersion: 'provider-account-lease-policy:v1',
        selectionReason:
          'Selected connected healthy account with 0 active lease(s), priority 100, and no cooldown or low-credit flag.',
      })

      return
    }

    if (
      path === '/api/operator/provider-accounts/chatgpt-codex/leases/active'
    ) {
      writeJson(response, 200, {
        total: 1,
        leases: [
          {
            leaseRef: 'provider-account-lease_ref_1',
            providerAccountRef: 'provider-account_ref_1',
            requestedAction: 'customer_order_fulfillment',
            expiresAt: '2026-06-05T00:15:00.000Z',
          },
        ],
      })

      return
    }

    if (
      path === '/api/operator/provider-accounts/chatgpt-codex/leases/release'
    ) {
      writeJson(response, 200, {
        leaseRef: 'provider-account-lease_ref_1',
        status: 'succeeded',
      })

      return
    }

    if (
      path === '/api/operator/provider-accounts/chatgpt-codex/leases/failover'
    ) {
      writeJson(response, 201, {
        receiptId: 'provider_account_failover_receipt_1',
        outcome: 'retrying',
        failureClass: 'runner_failure',
        accountStateAction: 'do_not_poison_account',
        previousProviderAccountRef: 'provider-account_ref_1',
        nextLease: {
          leaseRef: 'provider-account-lease_ref_2',
          providerAccountRef: 'provider-account_ref_2',
        },
        customerSafeStatus:
          'Work is retrying after an execution environment failure.',
      })

      return
    }

    if (
      path ===
      '/api/operator/provider-accounts/chatgpt-codex/leases/failover-history'
    ) {
      writeJson(response, 200, {
        total: 1,
        receipts: [
          {
            createdAt: '2026-06-05T00:00:00.000Z',
            receiptId: 'provider_account_failover_receipt_1',
            outcome: 'retrying',
            failureClass: 'runner_failure',
            previousProviderAccountRef: 'provider-account_ref_1',
            nextProviderAccountRef: 'provider-account_ref_2',
            customerSafeSummary:
              'Work is retrying through another connected execution account.',
          },
        ],
      })

      return
    }

    if (
      path ===
      '/api/operator/provider-accounts/chatgpt-codex/fleet-dashboard'
    ) {
      writeJson(response, 200, {
        summary: {
          total: 5,
          eligible: 2,
          activeLeaseCount: 1,
          lowCredit: 1,
          requiresReauth: 1,
          cooldown: 1,
        },
        selector: {
          status: 'selected',
          providerAccountRef: 'provider-account_ref_1',
          selectionReason:
            'Selected connected healthy account with 0 active lease(s), priority 100, and no cooldown or low-credit flag.',
        },
        accounts: [
          {
            providerAccountRef: 'provider-account_ref_1',
            operatorLabel: 'account 1',
            accountLabel: 'account 1',
            status: 'connected',
            health: 'healthy',
            eligibility: 'eligible',
            eligibilityReasons: [],
            activeLeaseCount: 0,
            leaseLimit: 1,
            operatorPriority: 100,
            lastSanityCheckResult: 'healthy',
            lastSanityCheckAt: '2026-06-05T00:00:00.000Z',
            lastParallelProbeResult: 'healthy',
            lastParallelProbeAt: '2026-06-05T00:00:00.000Z',
            lastSelectedAt: null,
            lastSuccessfulLaunchAt: null,
            lastFailedLaunchAt: null,
            recentFailureClass: null,
            cooldownUntil: null,
            lowCredit: false,
            reauthRequiredReason: null,
            refillNote: null,
            operatorNote: null,
            sanityCommand:
              'node scripts/provider-chatgpt-device-login.mjs sanity provider-account_ref_1',
            reconnectCommand:
              'node scripts/provider-chatgpt-device-login.mjs start --providerAccountRef provider-account_ref_1',
          },
          {
            providerAccountRef: 'provider-account_ref_low_credit',
            operatorLabel: 'low credit',
            accountLabel: 'low credit',
            status: 'connected',
            health: 'healthy',
            eligibility: 'ineligible',
            eligibilityReasons: ['low_credit'],
            activeLeaseCount: 0,
            leaseLimit: 1,
            operatorPriority: 100,
            lastSanityCheckResult: 'low_credit',
            lastSanityCheckAt: '2026-06-05T00:00:00.000Z',
            lastParallelProbeResult: null,
            lastParallelProbeAt: null,
            lastSelectedAt: null,
            lastSuccessfulLaunchAt: null,
            lastFailedLaunchAt: null,
            recentFailureClass: 'low_credits',
            cooldownUntil: null,
            lowCredit: true,
            reauthRequiredReason: null,
            refillNote: 'Refill before overnight use.',
            operatorNote: null,
            sanityCommand:
              'node scripts/provider-chatgpt-device-login.mjs sanity provider-account_ref_low_credit',
            reconnectCommand:
              'node scripts/provider-chatgpt-device-login.mjs start --providerAccountRef provider-account_ref_low_credit',
          },
        ],
        activeLeases: [
          {
            leaseRef: 'provider-account-lease_ref_1',
            providerAccountRef: 'provider-account_ref_1',
            requestedAction: 'customer_order_fulfillment',
            assignmentId: 'assignment_1',
            expiresAt: '2026-06-05T00:15:00.000Z',
          },
        ],
      })

      return
    }

    writeJson(response, 404, { error: 'not_found', path })
  })

  await new Promise<void>(resolve => {
    server.listen(0, '127.0.0.1', resolve)
  })

  const address = server.address()

  if (address === null || typeof address === 'string') {
    throw new Error('mock server did not bind to a TCP address')
  }

  return {
    baseUrl: `http://127.0.0.1:${address.port}`,
    close: () =>
      new Promise<void>((resolve, reject) => {
        server.close(error => {
          if (error) {
            reject(error)
          } else {
            resolve()
          }
        })
      }),
    requests,
  }
}

const servers: Array<Awaited<ReturnType<typeof startMockOperatorApi>>> = []

const runCli = async (
  baseUrl: string,
  args: ReadonlyArray<string>,
  expectFailure = false,
): Promise<string> =>
  new Promise((resolve, reject) => {
    execFile(
      'node',
      [scriptPath, ...args],
      {
        encoding: 'utf8',
        env: {
          ...process.env,
          OPENAGENTS_ADMIN_API_TOKEN: 'test-admin-token',
          OPENAGENTS_BASE_URL: baseUrl,
        },
      },
      (error, stdout) => {
        if (error !== null && !expectFailure) {
          reject(error)

          return
        }

        resolve(stdout)
      },
    )
  })

describe('provider ChatGPT device login CLI', () => {
  afterEach(async () => {
    await Promise.all(servers.splice(0).map(server => server.close()))
  })

  test('runs the fleet smoke workflow against a mocked operator API without secrets', async () => {
    const server = await startMockOperatorApi()
    servers.push(server)

    const outputs = [
      await runCli(server.baseUrl, [
        'start',
        '--email',
        'chris@openagents.com',
        '--label',
        'account 1',
        '--create-new',
      ]),
      await runCli(server.baseUrl, ['poll', 'provider_attempt_pending']),
      await runCli(server.baseUrl, ['poll', 'provider_attempt_connected']),
      await runCli(server.baseUrl, ['poll', 'provider_attempt_expired'], true),
      await runCli(
        server.baseUrl,
        [
          'sanity',
          '--all',
          '--parallel',
          '5',
          '--email',
          'chris@openagents.com',
        ],
        true,
      ),
      await runCli(server.baseUrl, [
        'explain-lease',
        '--email',
        'chris@openagents.com',
      ]),
      await runCli(server.baseUrl, [
        'dashboard',
        '--email',
        'chris@openagents.com',
      ]),
      await runCli(server.baseUrl, [
        'lease',
        '--action',
        'customer_order_fulfillment',
        '--assignmentId',
        'assignment_1',
        '--orderId',
        'order_1',
        '--email',
        'chris@openagents.com',
      ]),
      await runCli(server.baseUrl, ['leases', '--email', 'chris@openagents.com']),
      await runCli(server.baseUrl, [
        'release-lease',
        '--leaseRef',
        'provider-account-lease_ref_1',
        '--status',
        'succeeded',
      ]),
      await runCli(server.baseUrl, [
        'failover',
        '--previousLeaseRef',
        'provider-account-lease_ref_1',
        '--failureClass',
        'runner_failure',
        '--action',
        'customer_order_fulfillment',
        '--attemptNumber',
        '1',
        '--maxAttempts',
        '3',
        '--email',
        'chris@openagents.com',
      ]),
      await runCli(server.baseUrl, [
        'failover-history',
        '--assignmentId',
        'assignment_1',
        '--email',
        'chris@openagents.com',
      ]),
    ].join('\n')

    expect(outputs).toContain('ChatGPT/Codex device login started')
    expect(outputs).toContain('Device login status: pending')
    expect(outputs).toContain('Device login status: connected')
    expect(outputs).toContain('Device login status: expired')
    expect(outputs).toContain(
      'ChatGPT/Codex sanity checks: 4/5 healthy, 0 collisions',
    )
    expect(outputs).toContain('Parallel probe: provider_parallel_probe_run_1')
    expect(outputs).toContain('Lease selector status: selected')
    expect(outputs).toContain('ChatGPT/Codex fleet dashboard')
    expect(outputs).toContain('low-credit')
    expect(outputs).toContain('ChatGPT/Codex account lease acquired')
    expect(outputs).toContain('Active ChatGPT/Codex leases: 1')
    expect(outputs).toContain('Lease released: provider-account-lease_ref_1')
    expect(outputs).toContain('Receipt ID: provider_account_failover_receipt_1')
    expect(outputs).toContain('ChatGPT/Codex failover receipts: 1')
    expect(outputs).not.toContain('test-admin-token')
    expect(outputs).not.toContain('access-token')
    expect(outputs).not.toContain('refresh-token')
    expect(outputs).not.toContain('codex-auth://')
    expect(outputs).not.toContain('auth.json')
    expect(outputs).not.toContain('secretRef')
    expect(outputs).not.toContain('grant_ref')
    expect(server.requests.map(request => request.path)).toEqual(
      expect.arrayContaining([
        '/api/operator/provider-accounts/chatgpt-codex/device-login/start',
        '/api/operator/provider-accounts/chatgpt-codex/sanity',
        '/api/operator/provider-accounts/chatgpt-codex/leases/explain',
        '/api/operator/provider-accounts/chatgpt-codex/fleet-dashboard',
        '/api/operator/provider-accounts/chatgpt-codex/leases',
        '/api/operator/provider-accounts/chatgpt-codex/leases/active',
        '/api/operator/provider-accounts/chatgpt-codex/leases/release',
        '/api/operator/provider-accounts/chatgpt-codex/leases/failover',
        '/api/operator/provider-accounts/chatgpt-codex/leases/failover-history',
      ]),
    )
  })
})
