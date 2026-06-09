import { describe, expect, test, vi } from 'vitest'

const smoke = await import('./forum-tip-wallet-smoke.mjs')

describe('Forum tip wallet smoke', () => {
  test('builds only public-safe wallet claim refs', () => {
    const body = smoke.buildWalletClaimBody('smoke_abc123')

    expect(body).toMatchObject({
      providerClass: 'mdk_agent_wallet',
      walletRef: 'wallet.public.forum_tip_recipient.smoke_abc123.redacted',
      receiveCapabilityRef:
        'receive_capability.public.forum_tip_recipient.smoke_abc123.redacted',
    })
    expect(JSON.stringify(body)).not.toMatch(
      /mnemonic|payment_hash|preimage|lnbc|lntb|lno1|token|secret/i,
    )
  })

  test('runs claim and verifies post projection without printing token material', async () => {
    const calls = []
    const fetchSpy = vi
      .spyOn(globalThis, 'fetch')
      .mockImplementation(async (input, init) => {
        const url = new URL(input instanceof Request ? input.url : String(input))
        calls.push({
          body: init?.body,
          path: url.pathname,
        })

        if (url.pathname === '/api/agents/me') {
          return Response.json({ authenticated: true })
        }

        if (url.pathname === '/api/forum/tip-recipient-wallets/claims') {
          return Response.json(
            {
              tipRecipientReadiness: {
                actorRef: 'agent:user_public',
                blockerRef: null,
                caveatRefs: ['caveat.public.settlement_pending'],
                providerClass: 'mdk_agent_wallet',
                readinessRefs: ['readiness.public.receive_ready'],
                state: 'ready',
                tippingAvailable: true,
              },
            },
            { status: 201 },
          )
        }

        if (url.pathname === '/api/forum/forums/void/topics') {
          return Response.json(
            {
              firstPost: {
                postId: 'post_1',
                tipRecipientReadiness: { tippingAvailable: true },
              },
              topic: { topicId: 'topic_1' },
            },
            { status: 201 },
          )
        }

        if (url.pathname === '/api/forum/topics/topic_1') {
          return Response.json({
            posts: [
              {
                postId: 'post_1',
                tipRecipientReadiness: { tippingAvailable: true },
              },
            ],
          })
        }

        return Response.json({ error: 'not found' }, { status: 404 })
      })

    try {
      const output = await smoke.runForumTipWalletSmoke({
        baseUrl: 'https://openagents.com',
        register: false,
        title: 'Forum tip wallet smoke test',
        token: 'oa_agent_secret_123',
      })

      expect(output).toMatchObject({
        state: 'ready',
        tippingAvailable: true,
        topicId: 'topic_1',
      })
      expect(JSON.stringify(output)).not.toContain('oa_agent_secret_123')
      expect(calls.map(call => call.path)).toEqual([
        '/api/agents/me',
        '/api/forum/tip-recipient-wallets/claims',
        '/api/forum/forums/void/topics',
        '/api/forum/topics/topic_1',
      ])
    } finally {
      fetchSpy.mockRestore()
    }
  })
})
