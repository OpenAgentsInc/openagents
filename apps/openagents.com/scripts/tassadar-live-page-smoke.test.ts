import { describe, expect, test, vi } from 'vitest'

const smoke = await import('./tassadar-live-page-smoke.mjs')

const json = (body: unknown, init?: ResponseInit) => Response.json(body, init)

const text = (body: string, init?: ResponseInit) =>
  new Response(body, {
    headers: { 'content-type': 'text/html' },
    ...init,
  })

describe('Tassadar live page smoke', () => {
  test('verifies route, assets, run summary, promise gates, and proof route', async () => {
    const fetchImpl = vi.fn(async (input: string | URL | Request) => {
      const url = new URL(input instanceof Request ? input.url : String(input))

      if (url.pathname === '/tassadar') {
        return text(
          '<!doctype html><html><body><div id="root"></div><script type="module" src="/assets/index-abc123.js"></script></body></html>',
        )
      }

      if (url.pathname === '/assets/index-abc123.js') {
        return new Response(
          'export const copy = "Tassadar lives in the Verse"',
        )
      }

      if (url.pathname === '/api/public/tassadar-run-summary') {
        return json({
          generatedAt: '2026-06-17T16:20:10Z',
          realGradient: {
            rejectedReplayPairs: [
              {
                rejectedTraceRef: 'trace.public.rejected.1',
                verificationChallengeRef: 'challenge.public.1',
              },
            ],
          },
          runRef: 'run.tassadar.executor.20260615',
          runState: 'active',
          settlementRows: [
            {
              apiUrl:
                '/api/public/nexus-pylon/receipts/receipt.nexus.tassadar.1',
              receiptRef: 'receipt.nexus.tassadar.1',
            },
          ],
          staleness: {
            composition: 'projection_staleness.v1',
            contractVersion: 'projection_staleness.v1',
            maxStalenessSeconds: 0,
            state: 'live_at_read',
          },
        })
      }

      if (url.pathname === '/api/public/product-promises') {
        return json({
          promises: [
            {
              promiseId: 'training.decentralized_training_launch.v1',
              state: 'green',
            },
            {
              promiseId: 'pylon.install_without_wallet_knowledge.v1',
              state: 'green',
            },
            {
              promiseId: 'models.tassadar_percepta_executor.v1',
              state: 'red',
            },
            {
              promiseId: 'training.public_gradient_windows.v1',
              state: 'planned',
            },
            {
              promiseId: 'pylon.first_real_model_training_run.v1',
              state: 'yellow',
            },
          ],
        })
      }

      if (url.pathname === '/api/public/pylon-stats') {
        return json({
          asOfLabel: 'Just now',
          publicRealSatsSettled24h: 338844,
          pylonsAssignmentReadyNow: 2,
          pylonsOnlineNow: 9,
          trainingAcceptedContributors: 0,
          trainingModelProgressContributors: 6,
        })
      }

      if (
        url.pathname ===
        '/api/public/nexus-pylon/receipts/receipt.nexus.tassadar.1'
      ) {
        return json({
          receiptRef: 'receipt.nexus.tassadar.1',
        })
      }

      return json({ error: 'not found' }, { status: 404 })
    })

    const output = await smoke.runTassadarLivePageSmoke({
      baseUrl: 'https://openagents.com',
      fetchImpl,
    })

    expect(output.ok).toBe(true)
    expect(output.run).toMatchObject({
      generatedAt: '2026-06-17T16:20:10Z',
      runRef: 'run.tassadar.executor.20260615',
      runState: 'active',
      settlementRowCount: 1,
    })
    expect(output.pylonStats).toMatchObject({
      asOfLabel: 'Just now',
      publicRealSatsSettled24h: 338844,
      pylonsOnlineNow: 9,
      trainingAcceptedContributors: 0,
      trainingModelProgressContributors: 6,
    })
    expect(output.proof).toMatchObject({
      url: 'https://openagents.com/api/public/nexus-pylon/receipts/receipt.nexus.tassadar.1',
    })
    expect(output.checks.map((check: { name: string }) => check.name)).toEqual([
      'tassadar_route_200',
      'tassadar_route_has_script_assets',
      'script_asset_reachable',
      'tassadar_web_scene_retired',
      'summary_endpoint_200',
      'summary_has_run_ref',
      'summary_has_generated_at',
      'summary_has_staleness_contract',
      'summary_has_typed_settlement_rows',
      'summary_has_rejected_replay_projection',
      'pylon_stats_endpoint_200',
      'pylon_stats_context_fields_present',
      'product_promises_endpoint_200',
      'product_promise_gate_refs_present',
      'first_settlement_proof_route_200',
    ])
  })

  test('fails when the summary omits the staleness contract', async () => {
    const fetchImpl = vi.fn(async (input: string | URL | Request) => {
      const url = new URL(input instanceof Request ? input.url : String(input))

      if (url.pathname === '/tassadar') {
        return text(
          '<script type="module" src="/assets/index-abc123.js"></script>',
        )
      }

      if (url.pathname === '/assets/index-abc123.js') {
        return new Response('export const copy = "Tassadar lives in the Verse"')
      }

      if (url.pathname === '/api/public/tassadar-run-summary') {
        return json({
          generatedAt: '2026-06-17T16:20:10Z',
          realGradient: { rejectedReplayPairs: [] },
          runRef: 'run.tassadar.executor.20260615',
          settlementRows: [],
        })
      }

      return json({ promises: [] })
    })

    await expect(
      smoke.runTassadarLivePageSmoke({
        baseUrl: 'https://openagents.com',
        fetchImpl,
      }),
    ).rejects.toThrow('summary_has_staleness_contract failed')
  })
})
