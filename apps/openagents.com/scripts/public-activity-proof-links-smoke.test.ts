import { describe, expect, test, vi } from 'vitest'

const smoke = await import('./public-activity-proof-links-smoke.mjs')

const json = (body: unknown, init?: ResponseInit) => Response.json(body, init)

describe('public activity proof links smoke', () => {
  test('fetches same-origin proof URLs derived from public timeline refs', async () => {
    const fetchImpl = vi.fn(async (input: string | URL | Request) => {
      const url = new URL(input instanceof Request ? input.url : String(input))

      if (url.pathname === '/api/public/activity-timeline') {
        return json({
          events: [
            {
              blockerRefs: [
                'product_promise.pylon.first_real_model_training_run.v1',
              ],
              caveatRefs: [],
              cursor:
                '2026-06-18T19:05:00.000Z:projection_gap:event.public.product_promise.blocked.1',
              eventRef: 'event.public.product_promise.blocked.1',
              kind: 'projection_gap',
              refs: ['route:/api/public/product-promises'],
              runRef: 'run.tassadar.public.1',
              sourceKind: 'projection_gap',
              sourceRefs: [
                'route:/api/public/product-promises',
                'route:/api/public/nexus-pylon/receipts/{receiptRef}',
                'receipt.nexus.public.proof.1',
                'training.window.public.1',
              ],
              text: 'Product promise blocker remains public.',
              ts: '2026-06-18T19:05:00.000Z',
            },
          ],
          generatedAt: '2026-06-18T19:05:01.000Z',
          nextCursor: null,
          schemaVersion: 'openagents.public_activity_timeline.v1',
          sourceLag: [],
          staleness: {
            composition: 'live_at_read',
            contractVersion: 'projection_staleness.v1',
            maxStalenessSeconds: 0,
            rebuildsOn: ['public_activity_timeline_read'],
          },
        })
      }

      if (
        url.pathname === '/api/public/product-promises' ||
        url.pathname ===
          '/api/public/nexus-pylon/receipts/receipt.nexus.public.proof.1' ||
        (url.pathname === '/api/public/training/runs/run.tassadar.public.1' &&
          url.searchParams.get('focusRef') === 'training.window.public.1')
      ) {
        return json({ ok: true })
      }

      return json({ error: 'not found' }, { status: 404 })
    })

    const output = await smoke.runPublicActivityProofLinksSmoke({
      baseUrl: 'https://openagents.com',
      fetchImpl,
      limit: 8,
    })

    expect(output.ok).toBe(true)
    expect(output.timeline.proofUrlCount).toBe(3)
    expect(output.linked.map((item: { url: string }) => item.url)).toEqual([
      'https://openagents.com/api/public/product-promises',
      'https://openagents.com/api/public/nexus-pylon/receipts/receipt.nexus.public.proof.1',
      'https://openagents.com/api/public/training/runs/run.tassadar.public.1?focusRef=training.window.public.1',
    ])
  })

  test('does not turn templated route refs into proof URLs', () => {
    expect(
      smoke.publicActivityProofUrlForRef(
        'route:/api/public/nexus-pylon/receipts/{receiptRef}',
      ),
    ).toBeNull()
    expect(
      smoke.publicActivityProofUrlForRef(
        'receipt.tassadar_poc.live_m1_closeout.assignment.example',
      ),
    ).toBeNull()
  })
})
