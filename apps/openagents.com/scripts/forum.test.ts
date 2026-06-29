import { mkdtemp, rm, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { describe, expect, test, vi } from 'vitest'

const forumCli = await import('./forum.mjs')

const walletExecutorFor = responses => {
  const executor = vi.fn(async commandSpec => {
    const key =
      commandSpec.command === 'init' && commandSpec.args?.includes('--show')
        ? 'init --show'
        : commandSpec.command
    const response = responses[key]

    if (response === undefined) {
      throw new Error(`Unexpected wallet command: ${key}`)
    }

    return response
  })

  return executor
}

const readyWalletExecutor = () =>
  walletExecutorFor({
    balance: {
      exitCode: 0,
      stdout: '{"balance_sats":150}',
    },
    'init --show': {
      exitCode: 0,
      stdout:
        '{"walletId":"wallet_private_1","mnemonic":"<redacted>","configPath":"/Users/private/.mdk-wallet/config.json"}',
    },
    status: {
      exitCode: 0,
      stdout: '{"running":true}',
    },
  })

describe('forum CLI helpers', () => {
  test('builds a deterministic idempotency key from public write inputs', () => {
    const first = forumCli.stableIdempotencyKey('reply', {
      bodyText: 'Public-safe reply',
      topic: 'topic_1',
    })
    const second = forumCli.stableIdempotencyKey('reply', {
      bodyText: 'Public-safe reply',
      topic: 'topic_1',
    })
    const changed = forumCli.stableIdempotencyKey('reply', {
      bodyText: 'Different public-safe reply',
      topic: 'topic_1',
    })

    expect(first).toBe(second)
    expect(first).not.toBe(changed)
    expect(first).toMatch(/^forum-reply-[a-f0-9]{32}$/)
    expect(first).not.toContain('Public-safe reply')
  })

  test('redacts bearer and agent tokens from printable output', () => {
    expect(
      forumCli.redactSecrets(
        'Authorization: Bearer oa_agent_secret_123 and token oa_agent_secret_456',
      ),
    ).toBe('Authorization: Bearer <redacted> and token oa_agent_<redacted>')
  })

  test('creates a listed-forum topic request without leaking token in summary', async () => {
    const parsed = forumCli.parseForumArgs([
      'create-topic',
      '--forum',
      'site-builder-help',
      '--title',
      'Hello agents',
      '--body',
      'Public-safe hello world.',
    ])
    const request = await forumCli.buildForumRequest(parsed, {
      OPENAGENTS_AGENT_TOKEN: 'oa_agent_secret_123',
    })
    const summary = forumCli.safeRequestSummary(request)

    expect(request.method).toBe('POST')
    expect(request.path).toBe('/api/forum/forums/site-builder-help/topics')
    expect(request.headers.authorization).toBe('Bearer oa_agent_secret_123')
    expect(request.headers['idempotency-key']).toMatch(
      /^forum-topic-[a-f0-9]{32}$/,
    )
    expect(JSON.stringify(summary)).not.toContain('oa_agent_secret_123')
    expect(summary.headers.authorization).toBe('Bearer <redacted>')
  })

  test('reads an agent apiKey from a local credential file for replies', async () => {
    const dir = await mkdtemp(join(tmpdir(), 'openagents-forum-cli-'))
    const credentialFile = join(dir, 'agent.json')

    try {
      await writeFile(
        credentialFile,
        JSON.stringify({
          apiKey: 'test_agent_token_from_file',
          displayName: 'Local Agent',
        }),
      )
      const parsed = forumCli.parseForumArgs([
        'reply',
        '--credential-file',
        credentialFile,
        '--topic',
        'topic_1',
        '--body',
        'Public-safe reply from file auth.',
      ])
      const request = await forumCli.buildForumRequest(parsed, {})
      const summary = forumCli.safeRequestSummary(request)

      expect(request.method).toBe('POST')
      expect(request.path).toBe('/api/forum/topics/topic_1/posts')
      expect(request.headers.authorization).toBe(
        'Bearer test_agent_token_from_file',
      )
      expect(request.headers['idempotency-key']).toMatch(
        /^forum-reply-[a-f0-9]{32}$/,
      )
      expect(JSON.stringify(summary)).not.toContain('test_agent_token_from_file')
      expect(summary.headers.authorization).toBe('Bearer <redacted>')
    } finally {
      await rm(dir, { force: true, recursive: true })
    }
  })

  test('builds a self-claim tip wallet request with repeated public readiness refs', async () => {
    const parsed = forumCli.parseForumArgs([
      'claim-tip-wallet',
      '--wallet-ref',
      'wallet.public.mdk_agent_wallet.route_test',
      '--receive-capability-ref',
      'receive_capability.public.mdk_agent_wallet.route_test',
      '--bolt12-offer',
      'lno1qpzry9x8gf2tvdw0s3jn54khce6mua7lqpzry9x8gf2tvdw0s3j',
      '--readiness-ref',
      'readiness.public.mdk_agent.daemon_running',
      '--readiness-ref',
      'readiness.public.mdk_agent.setup_present',
      '--readiness-ref',
      'readiness.public.mdk_agent.receive_ready',
      '--caveat-ref',
      'caveat.public.forum_tip_recipient.claim_doc_pending',
      '--claim-policy-ref',
      'policy.public.forum_tip_recipient.claimed_by_cli',
      '--custody-policy-ref',
      'policy.public.forum_tip_recipient.self_custody',
    ])
    const request = await forumCli.buildForumRequest(parsed, {
      OPENAGENTS_AGENT_TOKEN: 'oa_agent_secret_123',
    })
    const summary = forumCli.safeRequestSummary(request)

    expect(request.method).toBe('POST')
    expect(request.path).toBe('/api/forum/tip-recipient-wallets/claims')
    expect(request.headers.authorization).toBe('Bearer oa_agent_secret_123')
    expect(request.headers['idempotency-key']).toMatch(
      /^forum-tip-wallet-claim-[a-f0-9]{32}$/,
    )
    expect(request.body).toMatchObject({
      bolt12Offer: 'lno1qpzry9x8gf2tvdw0s3jn54khce6mua7lqpzry9x8gf2tvdw0s3j',
      caveatRefs: ['caveat.public.forum_tip_recipient.claim_doc_pending'],
      claimPolicyRefs: ['policy.public.forum_tip_recipient.claimed_by_cli'],
      custodyPolicyRefs: ['policy.public.forum_tip_recipient.self_custody'],
      payoutTargetApprovalRef: null,
      providerClass: 'mdk_agent_wallet',
      readinessRefs: [
        'readiness.public.mdk_agent.daemon_running',
        'readiness.public.mdk_agent.setup_present',
        'readiness.public.mdk_agent.receive_ready',
      ],
      receiveCapabilityRef:
        'receive_capability.public.mdk_agent_wallet.route_test',
      sourceRef: 'source.public.forum_tip_recipient.agent_self_claim',
      walletRef: 'wallet.public.mdk_agent_wallet.route_test',
    })
    expect(JSON.stringify(summary)).not.toContain('oa_agent_secret_123')
    expect(JSON.stringify(summary)).not.toContain(
      'wallet.public.mdk_agent_wallet.route_test',
    )
    expect(JSON.stringify(summary)).not.toContain(
      'receive_capability.public.mdk_agent_wallet.route_test',
    )
  })

  test('builds a Spark self-claim tip wallet request without a BOLT 12 offer', async () => {
    const sparkAddress =
      'spark1pgssyuuuhnrrdjswal5c3s3rafw9w3y5dd4cjy3duxlf7hjzkp0rqx6dj6mrhu'
    const parsed = forumCli.parseForumArgs([
      'claim-tip-wallet',
      '--wallet-ref',
      'wallet.public.spark.route_test',
      '--receive-capability-ref',
      'receive_capability.public.spark.route_test',
      '--spark-address',
      sparkAddress,
      '--readiness-ref',
      'readiness.public.spark_address.offline_receive_ready',
      '--readiness-ref',
      'readiness.public.spark_primary.agent_balance',
      '--custody-policy-ref',
      'policy.public.forum_tip_recipient.spark_self_custody',
    ])
    const request = await forumCli.buildForumRequest(parsed, {
      OPENAGENTS_AGENT_TOKEN: 'oa_agent_secret_123',
    })
    const summary = forumCli.safeRequestSummary(request)

    expect(request.method).toBe('POST')
    expect(request.path).toBe('/api/forum/tip-recipient-wallets/claims')
    expect(request.body).toMatchObject({
      bolt12Offer: null,
      sparkAddress,
      custodyPolicyRefs: [
        'policy.public.forum_tip_recipient.spark_self_custody',
      ],
      providerClass: 'mdk_agent_wallet',
      readinessRefs: [
        'readiness.public.spark_address.offline_receive_ready',
        'readiness.public.spark_primary.agent_balance',
      ],
      receiveCapabilityRef: 'receive_capability.public.spark.route_test',
      walletRef: 'wallet.public.spark.route_test',
    })
    expect(JSON.stringify(summary)).not.toContain(sparkAddress)
  })

  test('builds a recipient settlement claim request with repeated evidence refs', async () => {
    const parsed = forumCli.parseForumArgs([
      'claim-tip-settlement',
      '--receipt',
      'receipt.forum.route_test',
      '--settlement-ref',
      'settlement.public.route_test.creator_wallet.receipt',
      '--settlement-evidence-ref',
      'settlement_evidence.public.mdk_agent_wallet.receive_confirmed',
      '--settlement-evidence-ref',
      'settlement_evidence.public.mdk_agent_wallet.payment_history_checked',
      '--source-ref',
      'source.public.route_test.agent_wallet',
    ])
    const request = await forumCli.buildForumRequest(parsed, {
      OPENAGENTS_AGENT_TOKEN: 'oa_agent_secret_123',
    })
    const summary = forumCli.safeRequestSummary(request)

    expect(request.method).toBe('POST')
    expect(request.path).toBe(
      '/api/forum/receipts/receipt.forum.route_test/settlement-claims',
    )
    expect(request.headers.authorization).toBe('Bearer oa_agent_secret_123')
    expect(request.headers['idempotency-key']).toMatch(
      /^forum-tip-settlement-claim-[a-f0-9]{32}$/,
    )
    expect(request.body).toStrictEqual({
      settlementEvidenceRefs: [
        'settlement_evidence.public.mdk_agent_wallet.receive_confirmed',
        'settlement_evidence.public.mdk_agent_wallet.payment_history_checked',
      ],
      settlementRef: 'settlement.public.route_test.creator_wallet.receipt',
      sourceRef: 'source.public.route_test.agent_wallet',
    })
    expect(JSON.stringify(summary)).not.toContain('oa_agent_secret_123')
  })

  test('requires an agent token for mutating commands', async () => {
    const parsed = forumCli.parseForumArgs([
      'reply',
      '--topic',
      'topic_1',
      '--body',
      'Public-safe reply.',
    ])

    await expect(forumCli.buildForumRequest(parsed, {})).rejects.toThrow(
      'OPENAGENTS_AGENT_TOKEN or --credential-file is required for reply.',
    )
  })

  test('builds participation and notification acknowledgement requests', async () => {
    const watch = await forumCli.buildForumRequest(
      forumCli.parseForumArgs([
        'watch-topic',
        '--topic',
        'topic_1',
        '--idempotency-key',
        'watch-topic-1',
      ]),
      { OPENAGENTS_AGENT_TOKEN: 'oa_agent_secret_123' },
    )
    const markRead = await forumCli.buildForumRequest(
      forumCli.parseForumArgs([
        'mark-notification-read',
        '--notification',
        'mention:post_1',
      ]),
      { OPENAGENTS_AGENT_TOKEN: 'oa_agent_secret_123' },
    )

    expect(watch).toMatchObject({
      method: 'POST',
      path: '/api/forum/topics/topic_1/watches',
    })
    expect(watch.headers['idempotency-key']).toBe('watch-topic-1')
    expect(markRead.method).toBe('POST')
    expect(markRead.path).toBe(
      '/api/agents/notifications/mention%3Apost_1/read',
    )
    expect(markRead.headers['idempotency-key']).toMatch(
      /^forum-notification-read-[a-f0-9]{32}$/,
    )
  })

  test('builds edit, tombstone, and report requests', async () => {
    const env = { OPENAGENTS_AGENT_TOKEN: 'oa_agent_secret_123' }
    const edit = await forumCli.buildForumRequest(
      forumCli.parseForumArgs([
        'edit-post',
        '--post',
        'post_1',
        '--body',
        'Updated public-safe body.',
      ]),
      env,
    )
    const tombstone = await forumCli.buildForumRequest(
      forumCli.parseForumArgs([
        'tombstone-post',
        '--post',
        'post_1',
        '--reason',
        'mistake',
      ]),
      env,
    )
    const report = await forumCli.buildForumRequest(
      forumCli.parseForumArgs([
        'report-topic',
        '--topic',
        'topic_1',
        '--reason',
        'off_topic',
      ]),
      env,
    )

    expect(edit).toMatchObject({
      body: { bodyText: 'Updated public-safe body.' },
      method: 'PATCH',
      path: '/api/forum/posts/post_1',
    })
    expect(tombstone).toMatchObject({
      body: { reason: 'mistake' },
      method: 'DELETE',
      path: '/api/forum/posts/post_1',
    })
    expect(report).toMatchObject({
      body: { reason: 'off_topic' },
      method: 'POST',
      path: '/api/forum/topics/topic_1/reports',
    })
  })

  test('builds paid-action preview and redeem requests with redacted summaries', async () => {
    const env = { OPENAGENTS_AGENT_TOKEN: 'oa_agent_secret_123' }
    const preview = await forumCli.buildForumRequest(
      forumCli.parseForumArgs([
        'reward-post',
        '--post',
        'post_1',
        '--spend-cap-amount',
        '100',
        '--spend-cap-asset',
        'bitcoin',
      ]),
      env,
    )
    const redeem = await forumCli.buildForumRequest(
      forumCli.parseForumArgs([
        'redeem-paid-action',
        '--challenge',
        'challenge_1',
        '--l402-proof-ref',
        'mdk_public_ref_should_not_print',
        '--l402-credential-header',
        'oa-l402-v1.private_credential_should_not_print:mdk_public_ref_should_not_print',
        '--path',
        '/api/forum/posts/post_1/rewards',
        '--request-body-digest',
        preview.body.requestBodyDigest,
        '--route-params-json',
        '{"postId":"post_1"}',
      ]),
      env,
    )
    const redeemSummary = forumCli.safeRequestSummary(redeem)

    expect(preview).toMatchObject({
      body: {
        spendCap: {
          amount: 100,
          asset: 'sats',
        },
      },
      method: 'POST',
      path: '/api/forum/posts/post_1/rewards',
    })
    expect(preview.body.requestBodyDigest).toMatch(/^sha256:[a-f0-9]{64}$/)
    expect(redeem).toMatchObject({
      body: {
        challengeId: 'challenge_1',
        method: 'POST',
        path: '/api/forum/posts/post_1/rewards',
        routeParams: { postId: 'post_1' },
      },
      method: 'POST',
      path: '/api/forum/paid-actions/redeem',
    })
    expect(JSON.stringify(redeemSummary)).not.toContain(
      'mdk_public_ref_should_not_print',
    )
    expect(JSON.stringify(redeemSummary)).not.toContain(
      'private_credential_should_not_print',
    )
    expect(redeemSummary.body.l402ProofRef).toBe('<redacted>')
    expect(redeemSummary.headers['x-openagents-l402']).toBe('<redacted>')
  })

  test('builds a public Forum tip leaderboard request', async () => {
    await expect(
      forumCli.buildForumRequest(
        forumCli.parseForumArgs(['tip-leaderboards', '--limit', '10']),
        {},
      ),
    ).resolves.toMatchObject({
      method: 'GET',
      path: '/api/forum/tip-leaderboards?limit=10',
    })
  })

  test('preflights a ready MDK agent wallet without printing private wallet output', async () => {
    const executor = readyWalletExecutor()
    const result = await forumCli.runForumWalletPreflight({
      executor,
      spendCap: { amount: 100, asset: 'bitcoin' },
    })
    const serialized = JSON.stringify(result)

    expect(result).toMatchObject({
      kind: 'forum_agent_wallet_preflight',
      livePaymentAttempted: false,
      publicSafe: true,
      ready: true,
      spendCap: { amount: 100, asset: 'sats' },
      status: 'ready',
      walletRef: 'wallet.public.mdk_agent_wallet.redacted',
    })
    expect(result.checks.map(check => check.commandRef)).toEqual([
      'mdk_agent_wallet.status',
      'mdk_agent_wallet.init_show',
      'mdk_agent_wallet.balance',
    ])
    expect(executor).toHaveBeenCalledTimes(3)
    expect(serialized).not.toContain('wallet_private_1')
    expect(serialized).not.toContain('/Users/private/.mdk-wallet/config.json')
  })

  test('preflights a ready signet wallet when the smoke requires signet', async () => {
    const executor = walletExecutorFor({
      balance: {
        exitCode: 0,
        stdout: '{"balance_sats":150}',
      },
      'init --show': {
        exitCode: 0,
        stdout:
          '{"walletId":"wallet_private_1","network":"signet","mnemonic":"<redacted>"}',
      },
      status: {
        exitCode: 0,
        stdout: '{"running":true}',
      },
    })
    const result = await forumCli.runForumWalletPreflight({
      executor,
      spendCap: { amount: 100, asset: 'bitcoin' },
      walletNetwork: 'signet',
    })

    expect(result.ready).toBe(true)
    expect(result.status).toBe('ready')
    expect(executor).toHaveBeenCalledTimes(3)
  })

  test('blocks signet smoke preflight when the wallet network is mainnet', async () => {
    const executor = walletExecutorFor({
      'init --show': {
        exitCode: 0,
        stdout:
          '{"walletId":"wallet_private_1","network":"mainnet","mnemonic":"<redacted>"}',
      },
      status: {
        exitCode: 0,
        stdout: '{"running":true}',
      },
    })
    const result = await forumCli.runForumWalletPreflight({
      executor,
      spendCap: { amount: 100, asset: 'bitcoin' },
      walletNetwork: 'signet',
    })

    expect(result.ready).toBe(false)
    expect(result.blocker).toMatchObject({
      code: 'agent_wallet_network_mismatch',
      ownerApprovalRequired: false,
    })
    expect(result.checks.map(check => check.commandRef)).toEqual([
      'mdk_agent_wallet.status',
      'mdk_agent_wallet.init_show',
    ])
    expect(executor).toHaveBeenCalledTimes(2)
  })

  test('blocks signet smoke preflight when wallet network cannot be verified', async () => {
    const executor = walletExecutorFor({
      'init --show': {
        exitCode: 0,
        stdout: '{"walletId":"wallet_private_1","mnemonic":"<redacted>"}',
      },
      status: {
        exitCode: 0,
        stdout: '{"running":true}',
      },
    })
    const result = await forumCli.runForumWalletPreflight({
      executor,
      spendCap: { amount: 100, asset: 'bitcoin' },
      walletNetwork: 'signet',
    })

    expect(result.ready).toBe(false)
    expect(result.blocker).toMatchObject({
      code: 'agent_wallet_network_unverifiable',
      ownerApprovalRequired: false,
    })
    expect(result.checks.map(check => check.commandRef)).toEqual([
      'mdk_agent_wallet.status',
      'mdk_agent_wallet.init_show',
    ])
    expect(executor).toHaveBeenCalledTimes(2)
  })

  test('blocks wallet preflight when no wallet exists and does not run balance', async () => {
    const executor = walletExecutorFor({
      'init --show': {
        exitCode: 1,
        stdout:
          '{"error":"wallet_not_initialized","mnemonic":"secret recovery phrase"}',
      },
      status: {
        exitCode: 0,
        stdout: '{"running":true}',
      },
    })
    const result = await forumCli.runForumWalletPreflight({
      executor,
      spendCap: { amount: 100, asset: 'sats' },
    })
    const serialized = JSON.stringify(result)

    expect(result.ready).toBe(false)
    expect(result.blocker).toMatchObject({
      code: 'agent_wallet_missing',
      ownerApprovalRequired: true,
    })
    expect(result.checks.map(check => check.commandRef)).toEqual([
      'mdk_agent_wallet.status',
      'mdk_agent_wallet.init_show',
    ])
    expect(executor).toHaveBeenCalledTimes(2)
    expect(serialized).not.toContain('secret recovery phrase')
  })

  test('blocks wallet preflight when balance is below the spend cap', async () => {
    const executor = walletExecutorFor({
      balance: {
        exitCode: 0,
        stdout: '{"balance_sats":99}',
      },
      'init --show': {
        exitCode: 0,
        stdout: '{"walletId":"wallet_private_1","mnemonic":"<redacted>"}',
      },
      status: {
        exitCode: 0,
        stdout: '{"running":true}',
      },
    })
    const result = await forumCli.runForumWalletPreflight({
      executor,
      spendCap: { amount: 100, asset: 'sats' },
    })

    expect(result.ready).toBe(false)
    expect(result.blocker.code).toBe('agent_wallet_insufficient_balance')
    expect(result.checks.at(-1)).toMatchObject({
      commandRef: 'mdk_agent_wallet.balance',
      status: 'blocked',
    })
    expect(JSON.stringify(result)).not.toContain('99')
  })

  test('blocks wallet preflight on invalid JSON stdout', async () => {
    const executor = walletExecutorFor({
      status: {
        exitCode: 0,
        stdout: 'not json',
      },
    })
    const result = await forumCli.runForumWalletPreflight({
      executor,
      spendCap: { amount: 100, asset: 'sats' },
    })

    expect(result.ready).toBe(false)
    expect(result.blocker.code).toBe('agent_wallet_status_invalid_json')
    expect(result.checks).toEqual([
      {
        commandRef: 'mdk_agent_wallet.status',
        name: 'status',
        reasonRef: 'reason.public.agent_wallet_status_invalid_json',
        status: 'blocked',
      },
    ])
  })

  test('blocks wallet preflight on command failure and timeout', async () => {
    const failedBalance = await forumCli.runForumWalletPreflight({
      executor: walletExecutorFor({
        balance: {
          exitCode: 1,
          stdout: '{"error":"balance unavailable"}',
        },
        'init --show': {
          exitCode: 0,
          stdout: '{"walletId":"wallet_private_1","mnemonic":"<redacted>"}',
        },
        status: {
          exitCode: 0,
          stdout: '{"running":true}',
        },
      }),
      spendCap: { amount: 100, asset: 'sats' },
    })
    const timedOutStatus = await forumCli.runForumWalletPreflight({
      executor: walletExecutorFor({
        status: {
          exitCode: 124,
          stdout: '',
          timedOut: true,
        },
      }),
      spendCap: { amount: 100, asset: 'sats' },
    })

    expect(failedBalance.ready).toBe(false)
    expect(failedBalance.blocker.code).toBe('agent_wallet_balance_failed')
    expect(timedOutStatus.ready).toBe(false)
    expect(timedOutStatus.blocker.code).toBe('agent_wallet_status_timeout')
  })

  test('wallet-status CLI emits public-safe JSON and never performs a live payment', async () => {
    const output = await forumCli.runForumCli(
      [
        'wallet-status',
        '--spend-cap-amount',
        '100',
        '--spend-cap-asset',
        'bitcoin',
      ],
      {},
      {
        walletExecutor: walletExecutorFor({
          balance: {
            exitCode: 0,
            stdout:
              '{"balance_sats":150,"payment_hash":"abc123paymenthash","payoutTarget":"raw_payout_target"}',
          },
          'init --show': {
            exitCode: 0,
            stdout:
              '{"walletId":"wallet_private_1","mnemonic":"secret recovery phrase","configPath":"/Users/private/.mdk-wallet/config.json"}',
          },
          status: {
            exitCode: 0,
            stdout:
              '{"running":true,"invoice":"lnbc1privateinvoice","preimage":"private_preimage"}',
          },
        }),
      },
    )
    const body = JSON.parse(output)

    expect(body.ready).toBe(true)
    expect(body.livePaymentAttempted).toBe(false)
    expect(output).not.toContain('lnbc1privateinvoice')
    expect(output).not.toContain('private_preimage')
    expect(output).not.toContain('secret recovery phrase')
    expect(output).not.toContain('abc123paymenthash')
    expect(output).not.toContain('/Users/private/.mdk-wallet/config.json')
    expect(output).not.toContain('raw_payout_target')
  })

  test('pay-reward-post preflights, pays a private L402 payload, and confirms with public-safe output', async () => {
    const walletExecutor = readyWalletExecutor()
    const requestJson = vi.fn(async request => {
      if (request.path === '/api/forum/posts/post_1/rewards') {
        return {
          challenge: {
            challengeId: '77777777-7777-4777-8777-777777777777',
            l402: {
              environment: 'production',
              invoiceRef: 'mdk_invoice.redacted.public_only',
              paymentHashRef: 'mdk_payment_hash.redacted.public_only',
              provider: 'mdk_hosted',
              sandbox: false,
            },
          },
          paymentRequired: true,
        }
      }

      if (request.path === '/api/forum/paid-actions/private-payment') {
        expect(request.body).toMatchObject({
          challengeId: '77777777-7777-4777-8777-777777777777',
          path: '/api/forum/posts/post_1/rewards',
          routeParams: { postId: 'post_1' },
          spendCap: { amount: 100, asset: 'sats' },
        })

        return {
          challenge: {
            challengeId: '77777777-7777-4777-8777-777777777777',
          },
          privatePayment: {
            bolt11: 'lnbc1privateinvoice',
            credential: 'oa-l402-v1.private_token',
            l402ProofRef: 'payment_proof.public.forum_reward.post_1',
          },
        }
      }

      if (request.path === '/api/forum/paid-actions/redeem') {
        expect(request.body).toMatchObject({
          challengeId: '77777777-7777-4777-8777-777777777777',
          l402ProofRef: 'payment_proof.public.forum_reward.post_1',
          path: '/api/forum/posts/post_1/rewards',
          routeParams: { postId: 'post_1' },
        })
        expect(request.headers['x-openagents-l402']).toBe(
          'oa-l402-v1.private_token:payment_proof.public.forum_reward.post_1',
        )
        expect(JSON.stringify(request)).not.toContain('private_preimage')
        expect(JSON.stringify(request)).not.toContain('lnbc1privateinvoice')

        return {
          entitlementRef:
            'forum_entitlement:77777777-7777-4777-8777-777777777777',
          originalReceiptRef: null,
          receiptRef: 'receipt.forum.77777777-7777-4777-8777-777777777777',
          replayed: false,
        }
      }

      if (
        request.path ===
        '/api/forum/receipts/receipt.forum.77777777-7777-4777-8777-777777777777'
      ) {
        return {
          receiptRef: 'receipt.forum.77777777-7777-4777-8777-777777777777',
          targetPostPermalink:
            'https://openagents.com/forum/t/topic_1#post-post_1',
          tipSettlement: {
            creatorReceivedSpendableValue: false,
            state: 'paid',
          },
        }
      }

      throw new Error(`Unexpected request path: ${request.path}`)
    })
    walletExecutor.mockImplementation(async commandSpec => {
      if (commandSpec.command === 'send') {
        expect(commandSpec.args).toEqual(['lnbc1privateinvoice'])

        return {
          exitCode: 0,
          stdout:
            '{"payment_hash":"raw_payment_hash","preimage":"private_preimage"}',
        }
      }

      if (commandSpec.command === 'status') {
        return {
          exitCode: 0,
          stdout:
            '{"running":true,"invoice":"lnbc1statusprivate","preimage":"status_private_preimage"}',
        }
      }

      if (
        commandSpec.command === 'init' &&
        commandSpec.args?.includes('--show')
      ) {
        return {
          exitCode: 0,
          stdout:
            '{"walletId":"wallet_private_1","network":"signet","mnemonic":"secret recovery phrase","configPath":"/Users/private/.mdk-wallet/config.json"}',
        }
      }

      if (commandSpec.command === 'balance') {
        return { exitCode: 0, stdout: '{"balance_sats":1000}' }
      }

      throw new Error(`Unexpected wallet command: ${commandSpec.command}`)
    })
    const output = await forumCli.runForumCli(
      [
        'pay-reward-post',
        '--post',
        'post_1',
        '--spend-cap-amount',
        '100',
        '--spend-cap-asset',
        'bitcoin',
        '--wallet-network',
        'signet',
        '--approve-live-spend',
      ],
      {
        OPENAGENTS_AGENT_TOKEN: 'oa_agent_secret_123',
      },
      {
        requestJson,
        walletExecutor,
      },
    )
    const body = JSON.parse(output)

    expect(body).toMatchObject({
      challenge: {
        challengeId: '77777777-7777-4777-8777-777777777777',
        environment: 'production',
        provider: 'mdk_hosted',
        sandbox: false,
      },
      livePaymentAttempted: true,
      payment: {
        commandRef: 'mdk_agent_wallet.send',
        credentialPresent: true,
        preimageCaptured: true,
        proofRef: 'payment_proof.public.forum_reward.post_1',
        status: 'paid',
      },
      receipt: {
        postLink: 'https://openagents.com/forum/t/topic_1#post-post_1',
        receiptLink:
          'https://openagents.com/forum/receipts/receipt.forum.77777777-7777-4777-8777-777777777777',
        receiptRef: 'receipt.forum.77777777-7777-4777-8777-777777777777',
        replayed: false,
        settlement: {
          creatorReceivedSpendableValue: false,
          label: 'Payment verified',
          state: 'paid',
        },
      },
      status: 'receipt_created',
    })
    expect(output).not.toContain('lnbc1privateinvoice')
    expect(output).not.toContain('oa-l402-v1.private_token')
    expect(output).not.toContain('private_preimage')
    expect(output).not.toContain('raw_payment_hash')
    expect(output).not.toContain('secret recovery phrase')
  })

  test('pay-reward-post refuses sandbox challenges without sending wallet payment', async () => {
    const walletExecutor = readyWalletExecutor()
    const requestJson = vi.fn(async () => ({
      challenge: {
        challengeId: '77777777-7777-4777-8777-777777777777',
        l402: {
          environment: 'sandbox',
          provider: 'mdk_hosted',
          sandbox: true,
        },
      },
      paymentRequired: true,
    }))
    const output = await forumCli.runForumCli(
      [
        'pay-reward-post',
        '--post',
        'post_1',
        '--spend-cap-amount',
        '100',
        '--spend-cap-asset',
        'bitcoin',
        '--approve-live-spend',
      ],
      {
        OPENAGENTS_AGENT_TOKEN: 'oa_agent_secret_123',
      },
      {
        requestJson,
        walletExecutor,
      },
    )
    const body = JSON.parse(output)

    expect(body.status).toBe('blocked')
    expect(body.reasonRef).toBe('reason.public.forum_reward_sandbox_no_spend')
    expect(walletExecutor).not.toHaveBeenCalledWith(
      expect.objectContaining({ command: 'send' }),
    )
    expect(requestJson).toHaveBeenCalledTimes(1)
  })

  test('pay-reward-post refuses live spend without explicit approval', async () => {
    const walletExecutor = readyWalletExecutor()
    const requestJson = vi.fn(async () => ({
      challenge: {
        challengeId: '77777777-7777-4777-8777-777777777777',
        l402: {
          environment: 'production',
          provider: 'mdk_hosted',
          sandbox: false,
        },
        privatePayment: {
          invoice: 'lnbc1privateinvoice',
          token: 'oa-l402-v1.private_token',
        },
      },
      paymentRequired: true,
    }))
    const output = await forumCli.runForumCli(
      [
        'pay-reward-post',
        '--post',
        'post_1',
        '--spend-cap-amount',
        '100',
        '--spend-cap-asset',
        'bitcoin',
      ],
      {
        OPENAGENTS_AGENT_TOKEN: 'oa_agent_secret_123',
      },
      {
        requestJson,
        walletExecutor,
      },
    )
    const body = JSON.parse(output)

    expect(body.status).toBe('blocked')
    expect(body.reasonRef).toBe(
      'reason.public.forum_reward_live_spend_not_approved',
    )
    expect(walletExecutor).not.toHaveBeenCalledWith(
      expect.objectContaining({ command: 'send' }),
    )
    expect(output).not.toContain('lnbc1privateinvoice')
    expect(output).not.toContain('oa-l402-v1.private_token')
  })

  test('pay-reward-post refuses current public-safe challenge refs when private invoice material is absent', async () => {
    const walletExecutor = readyWalletExecutor()
    const requestJson = vi.fn(async () => ({
      challenge: {
        challengeId: '77777777-7777-4777-8777-777777777777',
        l402: {
          environment: 'production',
          invoiceRef: 'mdk_invoice.redacted.public_only',
          paymentHashRef: 'mdk_payment_hash.redacted.public_only',
          provider: 'mdk_hosted',
          sandbox: false,
        },
      },
      paymentRequired: true,
    }))
    const output = await forumCli.runForumCli(
      [
        'pay-reward-post',
        '--post',
        'post_1',
        '--spend-cap-amount',
        '100',
        '--spend-cap-asset',
        'bitcoin',
        '--approve-live-spend',
      ],
      {
        OPENAGENTS_AGENT_TOKEN: 'oa_agent_secret_123',
      },
      {
        requestJson,
        walletExecutor,
      },
    )
    const body = JSON.parse(output)

    expect(body.status).toBe('blocked')
    expect(body.reasonRef).toBe(
      'reason.public.forum_reward_private_l402_payload_missing',
    )
    expect(walletExecutor).not.toHaveBeenCalledWith(
      expect.objectContaining({ command: 'send' }),
    )
    expect(requestJson).toHaveBeenCalledTimes(2)
  })

  test('pay-reward-post stops after wallet send failure and does not create a receipt', async () => {
    const walletExecutor = readyWalletExecutor()
    const requestJson = vi.fn(async request => {
      if (request.path === '/api/forum/posts/post_1/rewards') {
        return {
          challenge: {
            challengeId: '77777777-7777-4777-8777-777777777777',
            l402: {
              environment: 'production',
              invoiceRef: 'mdk_invoice.redacted.public_only',
              paymentHashRef: 'mdk_payment_hash.redacted.public_only',
              provider: 'mdk_hosted',
              sandbox: false,
            },
          },
          paymentRequired: true,
        }
      }

      if (request.path === '/api/forum/paid-actions/private-payment') {
        return {
          privatePayment: {
            bolt11: 'lnbc1privateinvoice',
            credential: 'oa-l402-v1.private_token',
          },
        }
      }

      throw new Error(`Unexpected confirm request: ${request.path}`)
    })
    walletExecutor.mockImplementation(async commandSpec => {
      if (commandSpec.command === 'send') {
        return {
          exitCode: 1,
          stdout:
            '{"error":"payment failed","payment_hash":"raw_payment_hash"}',
        }
      }

      return readyWalletExecutor()(commandSpec)
    })
    const output = await forumCli.runForumCli(
      [
        'pay-reward-post',
        '--post',
        'post_1',
        '--spend-cap-amount',
        '100',
        '--spend-cap-asset',
        'bitcoin',
        '--approve-live-spend',
      ],
      {
        OPENAGENTS_AGENT_TOKEN: 'oa_agent_secret_123',
      },
      {
        requestJson,
        walletExecutor,
      },
    )
    const body = JSON.parse(output)

    expect(body.status).toBe('payment_failed')
    expect(body.receipt).toBeNull()
    expect(requestJson).toHaveBeenCalledTimes(2)
    expect(output).not.toContain('lnbc1privateinvoice')
    expect(output).not.toContain('oa-l402-v1.private_token')
    expect(output).not.toContain('raw_payment_hash')
  })

  test('pay-reward-post recovers receipt creation after wallet send timeout when credential is available', async () => {
    const walletExecutor = readyWalletExecutor()
    let privatePaymentCalls = 0
    const requestJson = vi.fn(async request => {
      if (request.path === '/api/forum/posts/post_1/rewards') {
        return {
          challenge: {
            challengeId: '77777777-7777-4777-8777-777777777777',
            l402: {
              environment: 'production',
              invoiceRef: 'mdk_invoice.redacted.public_only',
              paymentHashRef: 'mdk_payment_hash.redacted.public_only',
              provider: 'mdk_hosted',
              sandbox: false,
            },
          },
          paymentRequired: true,
        }
      }

      if (request.path === '/api/forum/paid-actions/private-payment') {
        privatePaymentCalls += 1

        return privatePaymentCalls === 1
          ? {
              privatePayment: {
                bolt11: 'lnbc1privateinvoice',
                credential: 'oa-l402-v1.private_token',
                l402ProofRef: 'payment_proof.public.forum_reward.recovered',
              },
            }
          : {
              privatePayment: {
                credential: 'oa-l402-v1.private_token',
                l402ProofRef: 'payment_proof.public.forum_reward.recovered',
              },
            }
      }

      if (request.path === '/api/forum/paid-actions/redeem') {
        expect(request.headers['x-openagents-l402']).toContain(
          'payment_proof.public.forum_reward.recovered',
        )

        return {
          receiptRef: 'receipt.forum.recovered',
        }
      }

      if (request.path === '/api/forum/receipts/receipt.forum.recovered') {
        return {
          tipSettlement: {
            creatorReceivedSpendableValue: false,
            state: 'paid',
          },
        }
      }

      throw new Error(`Unexpected request: ${request.path}`)
    })
    walletExecutor.mockImplementation(async commandSpec => {
      if (commandSpec.command === 'send') {
        return {
          stdout: '',
          timedOut: true,
        }
      }

      return readyWalletExecutor()(commandSpec)
    })
    const output = await forumCli.runForumCli(
      [
        'pay-reward-post',
        '--post',
        'post_1',
        '--spend-cap-amount',
        '100',
        '--spend-cap-asset',
        'bitcoin',
        '--approve-live-spend',
      ],
      {
        OPENAGENTS_AGENT_TOKEN: 'oa_agent_secret_123',
      },
      {
        requestJson,
        walletExecutor,
      },
    )
    const body = JSON.parse(output)

    expect(body.status).toBe('receipt_created')
    expect(body.payment.recoveredAfterTimeout).toBe(true)
    expect(body.receipt.receiptRef).toBe('receipt.forum.recovered')
    expect(privatePaymentCalls).toBe(2)
    expect(output).not.toContain('lnbc1privateinvoice')
    expect(output).not.toContain('oa-l402-v1.private_token')
  })

  test('tip-post pays a ready post BOLT 12 offer and records direct settlement evidence', async () => {
    const walletExecutor = readyWalletExecutor()
    const requestJson = vi.fn(async request => {
      if (request.path === '/api/forum/posts/post_1') {
        return {
          post: {
            permalink: 'https://openagents.com/forum/t/topic_1#post-post_1',
            postId: 'post_1',
            tipRecipientReadiness: {
              actorRef: 'actor.recipient',
              directPayment: {
                bolt12Offer:
                  'lno1qpzry9x8gf2tvdw0s3jn54khce6mua7lqpzry9x8gf2tvdw0s3j',
                kind: 'bolt12_offer',
                settlementAuthority: 'recipient_wallet_direct',
              },
              state: 'ready',
              tippingAvailable: true,
            },
          },
        }
      }

      if (request.path === '/api/forum/posts/post_1/direct-tips') {
        expect(request.body.amount).toStrictEqual({
          amount: 15,
          asset: 'sats',
        })
        expect(request.body.paymentEvidence).toMatchObject({
          paymentMode: 'live',
          providerRef: 'provider.public.mdk_agent_wallet',
          status: 'confirmed',
        })
        expect(request.body.paymentEvidence.externalRef).toMatch(
          /^external\.public\.mdk_agent_wallet\.[a-f0-9]{32}$/,
        )
        expect(request.body.paymentEvidence.redactedEvidenceRef).toMatch(
          /^evidence\.public\.mdk_agent_wallet\.[a-f0-9]{32}$/,
        )
        expect(JSON.stringify(request)).not.toContain('payment_hash_raw')
        expect(JSON.stringify(request)).not.toContain('private_preimage')
        expect(JSON.stringify(request)).not.toContain('lno1qpzry9')

        return {
          amount: { amount: 15, asset: 'sats' },
          attemptId: '77777777-7777-4777-8777-777777777777',
          idempotent: false,
          payerActorRef: 'agent:payer',
          paymentEvidence: request.body.paymentEvidence,
          postId: 'post_1',
          receipt: {
            receiptRef: 'receipt.forum.direct.1',
            tipSettlement: {
              creatorReceivedSpendableValue: true,
              state: 'settled',
            },
          },
          recipientActorRef: 'actor.recipient',
          status: 'settled',
          targetPostPermalink:
            'https://openagents.com/forum/t/topic_1#post-post_1',
        }
      }

      throw new Error(`Unexpected request: ${request.path}`)
    })
    walletExecutor.mockImplementation(async commandSpec => {
      if (commandSpec.command === 'send') {
        expect(commandSpec.args).toEqual([
          'lno1qpzry9x8gf2tvdw0s3jn54khce6mua7lqpzry9x8gf2tvdw0s3j',
          '15',
        ])

        return {
          exitCode: 0,
          stdout:
            '{"payment_hash":"payment_hash_raw","preimage":"private_preimage"}',
        }
      }

      return readyWalletExecutor()(commandSpec)
    })

    const output = await forumCli.runForumCli(
      [
        'tip-post',
        '--post',
        'post_1',
        '--tip-amount',
        '15',
        '--approve-live-spend',
      ],
      {
        OPENAGENTS_AGENT_TOKEN: 'oa_agent_secret_123',
      },
      {
        requestJson,
        walletExecutor,
      },
    )
    const body = JSON.parse(output)

    expect(body).toMatchObject({
      kind: 'forum_direct_bolt12_tip',
      livePaymentAttempted: true,
      payment: {
        commandRef: 'mdk_agent_wallet.send',
        preimageCaptured: true,
        status: 'paid',
      },
      receipt: {
        receiptRef: 'receipt.forum.direct.1',
        tipSettlement: {
          creatorReceivedSpendableValue: true,
          state: 'settled',
        },
      },
      status: 'settled',
      target: {
        postId: 'post_1',
        postLink: 'https://openagents.com/forum/t/topic_1#post-post_1',
        recipientActorRef: 'actor.recipient',
      },
    })
    expect(output).not.toContain('lno1qpzry9')
    expect(output).not.toContain('payment_hash_raw')
    expect(output).not.toContain('private_preimage')
  })

  test('tip-post-smoke records smooth direct-tip evidence and post stats', async () => {
    const walletExecutor = readyWalletExecutor()
    const requestJson = vi.fn(async request => {
      if (request.path === '/api/forum/posts/post_1') {
        return {
          post: {
            permalink: 'https://openagents.com/forum/t/topic_1#post-post_1',
            postId: 'post_1',
            tipRecipientReadiness: {
              actorRef: 'actor.recipient',
              directPayment: {
                bolt12Offer:
                  'lno1qpzry9x8gf2tvdw0s3jn54khce6mua7lqpzry9x8gf2tvdw0s3j',
                kind: 'bolt12_offer',
                settlementAuthority: 'recipient_wallet_direct',
              },
              state: 'ready',
              tippingAvailable: true,
            },
            tipStats: {
              tipCount: 1,
              totalPaidSats: 15,
              totalSettledSats: 15,
            },
          },
        }
      }

      if (request.path === '/api/forum/posts/post_1/direct-tips') {
        return {
          amount: { amount: 15, asset: 'sats' },
          attemptId: '77777777-7777-4777-8777-777777777777',
          idempotent: false,
          payerActorRef: 'agent:payer',
          paymentEvidence: request.body.paymentEvidence,
          postId: 'post_1',
          receipt: {
            paymentEvent: {
              challengeId: '77777777-7777-4777-8777-777777777777',
            },
            receiptRef: 'receipt.forum.direct.1',
            tipSettlement: {
              creatorReceivedSpendableValue: true,
              state: 'settled',
            },
          },
          recipientActorRef: 'actor.recipient',
          status: 'settled',
          targetPostPermalink:
            'https://openagents.com/forum/t/topic_1#post-post_1',
        }
      }

      throw new Error(`Unexpected request: ${request.path}`)
    })
    walletExecutor.mockImplementation(async commandSpec => {
      if (commandSpec.command === 'send') {
        return {
          exitCode: 0,
          stdout:
            '{"payment_hash":"payment_hash_raw","preimage":"private_preimage"}',
        }
      }

      return readyWalletExecutor()(commandSpec)
    })

    const output = await forumCli.runForumCli(
      [
        'tip-post-smoke',
        '--post',
        'post_1',
        '--tip-amount',
        '15',
        '--approve-live-spend',
        '--strict-smooth',
      ],
      {
        OPENAGENTS_AGENT_TOKEN: 'oa_agent_secret_123',
      },
      {
        requestJson,
        walletExecutor,
      },
    )
    const body = JSON.parse(output)

    expect(body).toMatchObject({
      balanceAfter: {
        balance: { amount: 150, asset: 'sats' },
        status: 'ready',
      },
      balanceBefore: {
        balance: { amount: 150, asset: 'sats' },
        status: 'ready',
      },
      directTip: {
        attemptId: '77777777-7777-4777-8777-777777777777',
        paymentStatus: 'paid',
        receiptRef: 'receipt.forum.direct.1',
        status: 'settled',
      },
      kind: 'forum_direct_bolt12_tip_smoke',
      mode: 'strict_smooth',
      postStatsAfter: {
        tipCount: 1,
        totalPaidSats: 15,
        totalSettledSats: 15,
      },
      recoveredAfterTimeout: false,
      status: 'passed',
    })
    expect(output).not.toContain('lno1qpzry9')
    expect(output).not.toContain('payment_hash_raw')
    expect(output).not.toContain('private_preimage')
  })

  test('tip-post blocks before wallet spend when the post has no BOLT 12 offer', async () => {
    const walletExecutor = readyWalletExecutor()
    const requestJson = vi.fn(async request => {
      expect(request.path).toBe('/api/forum/posts/post_1')

      return {
        post: {
          permalink: 'https://openagents.com/forum/t/topic_1#post-post_1',
          postId: 'post_1',
          tipRecipientReadiness: {
            actorRef: 'actor.recipient',
            directPayment: null,
            state: 'ready',
            tippingAvailable: false,
          },
        },
      }
    })

    const output = await forumCli.runForumCli(
      [
        'tip-post',
        '--post',
        'post_1',
        '--tip-amount',
        '15',
        '--approve-live-spend',
      ],
      {
        OPENAGENTS_AGENT_TOKEN: 'oa_agent_secret_123',
      },
      {
        requestJson,
        walletExecutor,
      },
    )
    const body = JSON.parse(output)

    expect(body.status).toBe('blocked')
    expect(body.reasonRef).toBe(
      'reason.public.forum_tip_recipient_bolt12_offer_missing',
    )
    expect(walletExecutor).not.toHaveBeenCalledWith(
      expect.objectContaining({ command: 'send' }),
    )
  })

  test('tip-post records failed wallet sends without creating a public receipt', async () => {
    const walletExecutor = readyWalletExecutor()
    const requestJson = vi.fn(async request => {
      if (request.path === '/api/forum/posts/post_1') {
        return {
          post: {
            permalink: 'https://openagents.com/forum/t/topic_1#post-post_1',
            postId: 'post_1',
            tipRecipientReadiness: {
              actorRef: 'actor.recipient',
              directPayment: {
                bolt12Offer:
                  'lno1qpzry9x8gf2tvdw0s3jn54khce6mua7lqpzry9x8gf2tvdw0s3j',
                kind: 'bolt12_offer',
                settlementAuthority: 'recipient_wallet_direct',
              },
              state: 'ready',
              tippingAvailable: true,
            },
          },
        }
      }

      if (request.path === '/api/forum/posts/post_1/direct-tips') {
        expect(request.body.paymentEvidence.status).toBe('failed')

        return {
          amount: { amount: 15, asset: 'sats' },
          attemptId: '77777777-7777-4777-8777-777777777777',
          idempotent: false,
          payerActorRef: 'agent:payer',
          paymentEvidence: request.body.paymentEvidence,
          postId: 'post_1',
          receipt: null,
          recipientActorRef: 'actor.recipient',
          status: 'failed',
          targetPostPermalink:
            'https://openagents.com/forum/t/topic_1#post-post_1',
        }
      }

      throw new Error(`Unexpected request: ${request.path}`)
    })
    walletExecutor.mockImplementation(async commandSpec => {
      if (commandSpec.command === 'send') {
        return {
          exitCode: 1,
          stdout: '{"error":"insufficient balance"}',
        }
      }

      return readyWalletExecutor()(commandSpec)
    })

    const output = await forumCli.runForumCli(
      [
        'tip-post',
        '--post',
        'post_1',
        '--tip-amount',
        '15',
        '--approve-live-spend',
      ],
      {
        OPENAGENTS_AGENT_TOKEN: 'oa_agent_secret_123',
      },
      {
        requestJson,
        walletExecutor,
      },
    )
    const body = JSON.parse(output)

    expect(body.status).toBe('payment_failed')
    expect(body.receipt).toBeNull()
    expect(body.payment).toMatchObject({
      commandRef: 'mdk_agent_wallet.send',
      reasonRef: 'reason.public.agent_wallet_send_failed',
      status: 'failed',
    })
  })

  test('tip-post recovers a completed wallet payment after send timeout and records confirmed evidence', async () => {
    const walletExecutor = readyWalletExecutor()
    const paymentsSeen: string[] = []
    const requestJson = vi.fn(async request => {
      if (request.path === '/api/forum/posts/post_1') {
        return {
          post: {
            permalink: 'https://openagents.com/forum/t/topic_1#post-post_1',
            postId: 'post_1',
            tipRecipientReadiness: {
              actorRef: 'actor.recipient',
              directPayment: {
                bolt12Offer:
                  'lno1qpzry9x8gf2tvdw0s3jn54khce6mua7lqpzry9x8gf2tvdw0s3j',
                kind: 'bolt12_offer',
                settlementAuthority: 'recipient_wallet_direct',
              },
              state: 'ready',
              tippingAvailable: true,
            },
          },
        }
      }

      if (request.path === '/api/forum/posts/post_1/direct-tips') {
        expect(request.idempotencyKey).toMatch(
          /^forum-direct-tip-recovered-payment-[a-f0-9]{32}$/,
        )
        expect(request.idempotencyKey).not.toContain('wallet_payment_1')
        expect(request.body.paymentEvidence).toMatchObject({
          paymentMode: 'live',
          providerRef: 'provider.public.mdk_agent_wallet',
          status: 'confirmed',
        })
        expect(JSON.stringify(request)).not.toContain('lno1qpzry9')
        expect(JSON.stringify(request)).not.toContain('wallet_payment_1')

        return {
          amount: { amount: 15, asset: 'sats' },
          attemptId: '77777777-7777-4777-8777-777777777777',
          idempotent: true,
          payerActorRef: 'agent:payer',
          paymentEvidence: request.body.paymentEvidence,
          postId: 'post_1',
          receipt: {
            receiptRef: 'receipt.forum.direct.recovered',
            tipSettlement: {
              creatorReceivedSpendableValue: true,
              state: 'settled',
            },
          },
          recipientActorRef: 'actor.recipient',
          status: 'settled',
          targetPostPermalink:
            'https://openagents.com/forum/t/topic_1#post-post_1',
        }
      }

      throw new Error(`Unexpected request: ${request.path}`)
    })
    walletExecutor.mockImplementation(async commandSpec => {
      if (commandSpec.command === 'send') {
        return {
          exitCode: 124,
          stdout: '{"paymentId":"wallet_payment_1"}',
          timedOut: true,
        }
      }

      if (commandSpec.command === 'payments') {
        paymentsSeen.push('payments')

        return {
          exitCode: 0,
          stdout: JSON.stringify({
            payments: [
              {
                amountSats: 16,
                direction: 'outbound',
                paymentId: 'wallet_payment_1',
                status: 'completed',
                timestamp: 100,
              },
              {
                amountSats: 15,
                direction: 'outbound',
                paymentId: 'wallet_payment_1',
                status: paymentsSeen.length === 1 ? 'pending' : 'settled',
                timestamp: paymentsSeen.length,
              },
            ],
          }),
        }
      }

      return readyWalletExecutor()(commandSpec)
    })

    const output = await forumCli.runForumCli(
      [
        'tip-post',
        '--post',
        'post_1',
        '--tip-amount',
        '15',
        '--approve-live-spend',
        '--recovery-wait-ms',
        '2',
      ],
      {
        OPENAGENTS_AGENT_TOKEN: 'oa_agent_secret_123',
      },
      {
        recoveryPollMs: 0,
        requestJson,
        sleep: async () => {},
        walletExecutor,
      },
    )
    const body = JSON.parse(output)

    expect(body).toMatchObject({
      kind: 'forum_direct_bolt12_tip',
      livePaymentAttempted: true,
      payment: {
        commandRef: 'mdk_agent_wallet.send',
        recoveredAfterTimeout: true,
        recoveryPolls: 2,
        status: 'paid',
      },
      receipt: {
        receiptRef: 'receipt.forum.direct.recovered',
        tipSettlement: {
          creatorReceivedSpendableValue: true,
          state: 'settled',
        },
      },
      status: 'settled',
    })
    expect(output).not.toContain('lno1qpzry9')
    expect(output).not.toContain('wallet_payment_1')
  })

  test('tip-post reports payment_failed when timeout recovery finds a failed payment', async () => {
    const walletExecutor = readyWalletExecutor()
    const requestJson = vi.fn(async request => {
      if (request.path === '/api/forum/posts/post_1') {
        return {
          post: {
            permalink: 'https://openagents.com/forum/t/topic_1#post-post_1',
            postId: 'post_1',
            tipRecipientReadiness: {
              actorRef: 'actor.recipient',
              directPayment: {
                bolt12Offer:
                  'lno1qpzry9x8gf2tvdw0s3jn54khce6mua7lqpzry9x8gf2tvdw0s3j',
                kind: 'bolt12_offer',
                settlementAuthority: 'recipient_wallet_direct',
              },
              state: 'ready',
              tippingAvailable: true,
            },
          },
        }
      }

      throw new Error(`Unexpected request: ${request.path}`)
    })
    walletExecutor.mockImplementation(async commandSpec => {
      if (commandSpec.command === 'send') {
        return {
          exitCode: 124,
          stdout: '{"paymentId":"wallet_payment_failed"}',
          timedOut: true,
        }
      }

      if (commandSpec.command === 'payments') {
        return {
          exitCode: 0,
          stdout: JSON.stringify({
            payments: [
              {
                amountSats: 15,
                direction: 'outbound',
                paymentId: 'wallet_payment_failed',
                status: 'failed',
                timestamp: 1,
              },
            ],
          }),
        }
      }

      return readyWalletExecutor()(commandSpec)
    })

    const output = await forumCli.runForumCli(
      [
        'tip-post',
        '--post',
        'post_1',
        '--tip-amount',
        '15',
        '--approve-live-spend',
        '--recovery-wait-ms',
        '1',
      ],
      {
        OPENAGENTS_AGENT_TOKEN: 'oa_agent_secret_123',
      },
      {
        recoveryPollMs: 0,
        requestJson,
        sleep: async () => {},
        walletExecutor,
      },
    )
    const body = JSON.parse(output)

    expect(body).toMatchObject({
      payment: {
        commandRef: 'mdk_agent_wallet.send',
        reasonRef: 'reason.public.agent_wallet_send_failed',
        recoveredAfterTimeout: true,
        status: 'failed',
      },
      reasonRef: 'reason.public.agent_wallet_send_failed',
      receipt: null,
      status: 'payment_failed',
    })
    expect(requestJson).not.toHaveBeenCalledWith(
      expect.objectContaining({
        path: '/api/forum/posts/post_1/direct-tips',
      }),
    )
  })

  test('tip-post keeps recovery_pending when timeout recovery reaches its deadline', async () => {
    const walletExecutor = readyWalletExecutor()
    const requestJson = vi.fn(async request => {
      if (request.path === '/api/forum/posts/post_1') {
        return {
          post: {
            permalink: 'https://openagents.com/forum/t/topic_1#post-post_1',
            postId: 'post_1',
            tipRecipientReadiness: {
              actorRef: 'actor.recipient',
              directPayment: {
                bolt12Offer:
                  'lno1qpzry9x8gf2tvdw0s3jn54khce6mua7lqpzry9x8gf2tvdw0s3j',
                kind: 'bolt12_offer',
                settlementAuthority: 'recipient_wallet_direct',
              },
              state: 'ready',
              tippingAvailable: true,
            },
          },
        }
      }

      if (request.path === '/api/forum/posts/post_1/direct-tips') {
        expect(request.body.paymentEvidence.status).toBe('observed')

        return {
          amount: { amount: 15, asset: 'sats' },
          attemptId: '77777777-7777-4777-8777-777777777777',
          idempotent: false,
          payerActorRef: 'agent:payer',
          paymentEvidence: request.body.paymentEvidence,
          postId: 'post_1',
          receipt: null,
          recipientActorRef: 'actor.recipient',
          status: 'recovery_pending',
          targetPostPermalink:
            'https://openagents.com/forum/t/topic_1#post-post_1',
        }
      }

      throw new Error(`Unexpected request: ${request.path}`)
    })
    walletExecutor.mockImplementation(async commandSpec => {
      if (commandSpec.command === 'send') {
        return {
          exitCode: 124,
          stdout: '{"paymentId":"wallet_payment_pending"}',
          timedOut: true,
        }
      }

      if (commandSpec.command === 'payments') {
        return {
          exitCode: 0,
          stdout: JSON.stringify({
            payments: [
              {
                amountSats: 15,
                direction: 'outbound',
                paymentId: 'wallet_payment_pending',
                status: 'pending',
                timestamp: 1,
              },
            ],
          }),
        }
      }

      return readyWalletExecutor()(commandSpec)
    })

    const output = await forumCli.runForumCli(
      [
        'tip-post',
        '--post',
        'post_1',
        '--tip-amount',
        '15',
        '--approve-live-spend',
        '--recovery-wait-ms',
        '1',
      ],
      {
        OPENAGENTS_AGENT_TOKEN: 'oa_agent_secret_123',
      },
      {
        recoveryPollMs: 0,
        requestJson,
        sleep: async () => {},
        walletExecutor,
      },
    )
    const body = JSON.parse(output)

    expect(body).toMatchObject({
      attemptId: '77777777-7777-4777-8777-777777777777',
      payment: {
        commandRef: 'mdk_agent_wallet.send',
        recoveryDeadlineHit: true,
        recoveryWaitMs: 1,
        status: 'recovery_pending',
      },
      status: 'recovery_pending',
    })
    expect(output).not.toContain('lno1qpzry9')
    expect(output).not.toContain('wallet_payment_pending')
  })

  test('tip-post-smoke strict mode fails when timeout recovery is needed', async () => {
    const walletExecutor = readyWalletExecutor()
    const requestJson = vi.fn(async request => {
      if (request.path === '/api/forum/posts/post_1') {
        return {
          post: {
            permalink: 'https://openagents.com/forum/t/topic_1#post-post_1',
            postId: 'post_1',
            tipRecipientReadiness: {
              actorRef: 'actor.recipient',
              directPayment: {
                bolt12Offer:
                  'lno1qpzry9x8gf2tvdw0s3jn54khce6mua7lqpzry9x8gf2tvdw0s3j',
                kind: 'bolt12_offer',
                settlementAuthority: 'recipient_wallet_direct',
              },
              state: 'ready',
              tippingAvailable: true,
            },
            tipStats: {
              tipCount: 0,
              totalPaidSats: 0,
              totalSettledSats: 0,
            },
          },
        }
      }

      if (request.path === '/api/forum/posts/post_1/direct-tips') {
        expect(request.body.paymentEvidence.status).toBe('observed')

        return {
          amount: { amount: 15, asset: 'sats' },
          attemptId: '77777777-7777-4777-8777-777777777777',
          idempotent: false,
          payerActorRef: 'agent:payer',
          paymentEvidence: request.body.paymentEvidence,
          postId: 'post_1',
          receipt: null,
          recipientActorRef: 'actor.recipient',
          status: 'recovery_pending',
          targetPostPermalink:
            'https://openagents.com/forum/t/topic_1#post-post_1',
        }
      }

      throw new Error(`Unexpected request: ${request.path}`)
    })
    walletExecutor.mockImplementation(async commandSpec => {
      if (commandSpec.command === 'send') {
        return {
          exitCode: 124,
          stdout: '',
          timedOut: true,
        }
      }

      return readyWalletExecutor()(commandSpec)
    })

    const output = await forumCli.runForumCli(
      [
        'tip-post-smoke',
        '--post',
        'post_1',
        '--tip-amount',
        '15',
        '--approve-live-spend',
        '--recovery-wait-ms',
        '1',
        '--strict-smooth',
      ],
      {
        OPENAGENTS_AGENT_TOKEN: 'oa_agent_secret_123',
      },
      {
        recoveryPollMs: 0,
        requestJson,
        sleep: async () => {},
        walletExecutor,
      },
    )
    const body = JSON.parse(output)

    expect(body).toMatchObject({
      directTip: {
        attemptId: '77777777-7777-4777-8777-777777777777',
        paymentStatus: 'recovery_pending',
        receiptRef: null,
        status: 'recovery_pending',
      },
      mode: 'strict_smooth',
      reasonRef: 'reason.public.forum_tip_smoke_recovery_used',
      recoveredAfterTimeout: true,
      status: 'failed',
    })
    expect(output).not.toContain('lno1qpzry9')
  })

  test('builds authenticated unlisted search only when requested', async () => {
    const parsed = forumCli.parseForumArgs([
      'search',
      '--query',
      'hello',
      '--include-unlisted',
    ])
    const request = await forumCli.buildForumRequest(parsed, {
      OPENAGENTS_AGENT_TOKEN: 'oa_agent_secret_123',
    })

    expect(request.path).toBe('/api/forum/search?include=unlisted&q=hello')
    expect(request.headers.authorization).toBe('Bearer oa_agent_secret_123')
  })
})

describe('forum tip failure classification and self-pay preflight', () => {
  const BECH32_CHARSET = 'qpzry9x8gf2tvdw0s3jn54khce6mua7l'

  const syntheticOffer = (
    records: Array<{ type: number; value: number[] }>,
  ) => {
    const bytes: number[] = []

    for (const record of records) {
      bytes.push(record.type)
      bytes.push(record.value.length)
      bytes.push(...record.value)
    }

    let accumulator = 0
    let bits = 0
    let encoded = ''

    for (const byte of bytes) {
      accumulator = (accumulator << 8) | byte
      bits += 8

      while (bits >= 5) {
        bits -= 5
        encoded += BECH32_CHARSET[(accumulator >> bits) & 31]
      }
    }

    if (bits > 0) {
      encoded += BECH32_CHARSET[(accumulator << (5 - bits)) & 31]
    }

    return `lno1${encoded}`
  }

  const issuerRecord = (firstByte: number) => ({
    type: 22,
    value: [2, ...Array.from({ length: 31 }, () => firstByte), 7],
  })

  const pathRecord = (scidByte: number) => ({
    type: 16,
    value: [
      0,
      ...Array.from({ length: 8 }, () => scidByte),
      2,
      ...Array.from({ length: 32 }, () => 9),
      0,
    ],
  })

  const lspPubkeyPathRecord = (pubkeyByte: number) => ({
    type: 16,
    value: [
      2,
      ...Array.from({ length: 32 }, () => pubkeyByte),
      2,
      ...Array.from({ length: 32 }, () => 9),
      0,
    ],
  })

  test('classifies stalled sends by payment-hash presence', () => {
    expect(
      forumCli.classifyStalledTipSendFromPayments(
        [
          {
            amountSats: 15,
            direction: 'outbound',
            paymentHash: null,
            status: 'pending',
            timestamp: 2,
          },
        ],
        15,
      ),
    ).toBe('no_invoice_fetched')
    expect(
      forumCli.classifyStalledTipSendFromPayments(
        [
          {
            amountSats: 15,
            direction: 'outbound',
            paymentHash: 'a'.repeat(64),
            status: 'pending',
            timestamp: 2,
          },
        ],
        15,
      ),
    ).toBe('route_unresolved')
    expect(forumCli.classifyStalledTipSendFromPayments([], 15)).toBe(
      'unclassified',
    )
    expect(
      forumCli.classifyStalledTipSendFromPayments(
        [
          {
            amountSats: 15,
            direction: 'outbound',
            paymentHash: null,
            status: 'pending',
            timestamp: 1,
          },
          {
            amountSats: 15,
            direction: 'outbound',
            paymentHash: 'b'.repeat(64),
            status: 'pending',
            timestamp: 5,
          },
        ],
        15,
      ),
    ).toBe('route_unresolved')
  })

  test('extracts shared identity refs from offers minted by the same wallet session', () => {
    const sharedPathA = syntheticOffer([pathRecord(0x41)])
    const sharedPathB = syntheticOffer([pathRecord(0x41)])
    const otherPath = syntheticOffer([pathRecord(0x42)])
    const sharedIssuerA = syntheticOffer([issuerRecord(0x21)])
    const sharedIssuerB = syntheticOffer([issuerRecord(0x21)])

    expect(forumCli.offersShareSelfPayIdentity(sharedPathA, sharedPathB)).toBe(
      true,
    )
    expect(forumCli.offersShareSelfPayIdentity(sharedPathA, otherPath)).toBe(
      false,
    )
    expect(
      forumCli.offersShareSelfPayIdentity(sharedIssuerA, sharedIssuerB),
    ).toBe(true)
    expect(
      forumCli.offersShareSelfPayIdentity('not-an-offer', sharedPathA),
    ).toBe(null)
  })

  test('does not treat a shared LSP introduction pubkey as self-pay identity', () => {
    const walletA = syntheticOffer([lspPubkeyPathRecord(0x33)])
    const walletB = syntheticOffer([lspPubkeyPathRecord(0x33)])

    expect(forumCli.bolt12OfferIdentityRefs(walletA)).toBeNull()
    expect(forumCli.offersShareSelfPayIdentity(walletA, walletB)).toBeNull()
  })

  test('blocks self-pay before any live spend is attempted', async () => {
    const recipientOffer = syntheticOffer([pathRecord(0x41)])
    const payerOffer = syntheticOffer([pathRecord(0x41)])
    const sendCalls: string[] = []
    const walletExecutor = vi.fn(async (commandSpec: any) => {
      if (commandSpec.command === 'send') {
        sendCalls.push('send')
        return { exitCode: 0, stdout: '{"status":"completed"}' }
      }

      if (commandSpec.command === 'receive-bolt12') {
        return { exitCode: 0, stdout: JSON.stringify({ offer: payerOffer }) }
      }

      return readyWalletExecutor()(commandSpec)
    })
    const requestJson = vi.fn(async (request: any) => {
      if (request.path === '/api/forum/posts/post_self') {
        return {
          post: {
            permalink: 'https://openagents.com/forum/t/topic_1#post-post_self',
            postId: 'post_self',
            tipRecipientReadiness: {
              actorRef: 'actor.recipient',
              directPayment: {
                bolt12Offer: recipientOffer,
                kind: 'bolt12_offer',
                settlementAuthority: 'recipient_wallet_direct',
              },
              state: 'ready',
              tippingAvailable: true,
            },
            tipStats: { tipCount: 0, totalPaidSats: 0, totalSettledSats: 0 },
          },
        }
      }

      throw new Error(`Unexpected request: ${request.path}`)
    })

    const output = await forumCli.runForumCli(
      [
        'tip-post',
        '--post',
        'post_self',
        '--tip-amount',
        '15',
        '--approve-live-spend',
      ],
      { OPENAGENTS_AGENT_TOKEN: 'oa_agent_secret_123' },
      { requestJson, walletExecutor },
    )
    const body = JSON.parse(output)

    expect(body).toMatchObject({
      reasonRef: 'reason.public.forum_tip_self_pay_blocked',
      selfPayCheck: 'blocked',
      status: 'self_pay_blocked',
    })
    expect(sendCalls).toHaveLength(0)
    expect(output).not.toContain(recipientOffer)
  })

  test('strict smoke surfaces no_invoice_fetched classification on timeout', async () => {
    const recipientOffer = syntheticOffer([pathRecord(0x55)])
    const payerOffer = syntheticOffer([pathRecord(0x66)])
    const walletExecutor = vi.fn(async (commandSpec: any) => {
      if (commandSpec.command === 'send') {
        return { exitCode: 124, stdout: '', timedOut: true }
      }

      if (commandSpec.command === 'receive-bolt12') {
        return { exitCode: 0, stdout: JSON.stringify({ offer: payerOffer }) }
      }

      if (commandSpec.command === 'payments') {
        return {
          exitCode: 0,
          stdout: JSON.stringify({
            payments: [
              {
                amountSats: 15,
                direction: 'outbound',
                paymentHash: null,
                status: 'pending',
                timestamp: 99,
              },
            ],
          }),
        }
      }

      return readyWalletExecutor()(commandSpec)
    })
    const requestJson = vi.fn(async (request: any) => {
      if (request.path === '/api/forum/posts/post_cls') {
        return {
          post: {
            permalink: 'https://openagents.com/forum/t/topic_1#post-post_cls',
            postId: 'post_cls',
            tipRecipientReadiness: {
              actorRef: 'actor.recipient',
              directPayment: {
                bolt12Offer: recipientOffer,
                kind: 'bolt12_offer',
                settlementAuthority: 'recipient_wallet_direct',
              },
              state: 'ready',
              tippingAvailable: true,
            },
            tipStats: { tipCount: 0, totalPaidSats: 0, totalSettledSats: 0 },
          },
        }
      }

      if (request.path === '/api/forum/posts/post_cls/direct-tips') {
        return {
          amount: { amount: 15, asset: 'sats' },
          attemptId: '88888888-8888-4888-8888-888888888888',
          idempotent: false,
          payerActorRef: 'agent:payer',
          paymentEvidence: request.body.paymentEvidence,
          postId: 'post_cls',
          receipt: null,
          recipientActorRef: 'actor.recipient',
          status: 'recovery_pending',
          targetPostPermalink:
            'https://openagents.com/forum/t/topic_1#post-post_cls',
        }
      }

      throw new Error(`Unexpected request: ${request.path}`)
    })

    const output = await forumCli.runForumCli(
      [
        'tip-post-smoke',
        '--post',
        'post_cls',
        '--tip-amount',
        '15',
        '--approve-live-spend',
        '--recovery-wait-ms',
        '1',
        '--strict-smooth',
      ],
      { OPENAGENTS_AGENT_TOKEN: 'oa_agent_secret_123' },
      { recoveryPollMs: 0, requestJson, sleep: async () => {}, walletExecutor },
    )
    const body = JSON.parse(output)

    expect(body).toMatchObject({
      failureClassification: 'no_invoice_fetched',
      mode: 'strict_smooth',
      reasonRef: 'reason.public.forum_tip_smoke_no_invoice_fetched',
      recoveredAfterTimeout: true,
      selfPayCheck: 'passed',
      status: 'failed',
    })
    expect(output).not.toContain(recipientOffer)
    expect(output).not.toContain(payerOffer)
  })
})
