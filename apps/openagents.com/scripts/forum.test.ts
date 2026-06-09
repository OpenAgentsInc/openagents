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

  test('builds a self-claim tip wallet request with repeated public readiness refs', async () => {
    const parsed = forumCli.parseForumArgs([
      'claim-tip-wallet',
      '--wallet-ref',
      'wallet.public.mdk_agent_wallet.route_test',
      '--receive-capability-ref',
      'receive_capability.public.mdk_agent_wallet.route_test',
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
      'OPENAGENTS_AGENT_TOKEN is required for reply.',
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

  test('pay-reward-post preflights, pays a private L402 payload, and redeems with public-safe output', async () => {
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

      throw new Error(`Unexpected redeem request: ${request.path}`)
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
