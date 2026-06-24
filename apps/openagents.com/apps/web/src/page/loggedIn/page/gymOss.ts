import type { Html } from 'foldkit/html'
import { html } from 'foldkit/html'

import * as Ui from '../../../ui'
import { gymOssControllerView } from '../gymOss/controller'
import { GPT_OSS_MODEL_ID, MAX_IN_FLIGHT } from '../gymOss/runner'
import type { Message } from '../message'

// Gym — GPT-OSS live latency playground (#6167).
//
// An owner-gated (logged-in route) surface to hammer our own GPT-OSS L4 lane
// (`openagents/khala-oss-20b`, served behind Khala) and watch live TTFT /
// tokens-per-sec / inter-token latency / wall-clock, with P50/P90/P99/mean
// percentiles, an aggregate throughput number, and an optional concurrency ramp
// (1→2→4→8). This lane is billed by the hour, so there is no per-call balance
// gate — but the route is auth/owner-gated and a hard in-flight cap
// (MAX_IN_FLIGHT) keeps a ramp from wedging the box.
//
// The interactive surface, the streaming runner, the percentile aggregation, and
// the live scene all live in the `gymOss/` controller + scene custom elements,
// so this page is a thin house-styled shell that embeds them. Honest numbers
// throughout: `not_measured` is rendered explicitly, never as a fabricated 0,
// and failed samples show as failures, never as fake latency.

export const view = (_model: Model): Html => {
  const h = html<Message>()

  return Ui.container<Message>(
    [
      Ui.pageHeader<Message>({
        eyebrow: 'Gym',
        title: 'GPT-OSS latency playground',
        body: `Fire the same prompt at our own ${GPT_OSS_MODEL_ID} lane (streaming), measure live TTFT / tokens-per-sec / inter-token latency / wall-clock, read P50/P90/P99/mean across samples, and ramp concurrency 1→2→4→8. Hourly lane: no per-call balance gate, hard cap ${MAX_IN_FLIGHT} in flight.`,
      }),
      h.div(
        [Ui.className<Message>('mt-4')],
        [gymOssControllerView<Message>([])],
      ),
    ],
    [Ui.className<Message>('py-4')],
  )
}

// The page reads no logged-in model state of its own (the controller element
// owns its interactive state). Typed against the shared Model so the dispatcher
// passes the same value every other logged-in page receives.
type Model = import('../model').Model
