import {
  PUBLIC_ACTIVITY_TIMELINE_SCHEMA_VERSION,
  type PublicActivityTimelineEnvelope,
  type PublicActivityTimelineEvent,
  type PublicActivityTimelineEventKind,
  type PublicActivityTimelineSourceKind,
  publicActivityTimelineCursorForEvent,
  publicActivityTimelineLiveAtReadStaleness,
  publicActivityTimelineStoredSnapshotStaleness,
} from "./index.js"

const generatedAt = "2026-06-18T18:00:00.000Z"

const event = (input: {
  eventRef: string
  ts: string
  kind: PublicActivityTimelineEventKind
  sourceKind: PublicActivityTimelineSourceKind
  actorRef?: string
  targetRef?: string
  runRef?: string
  windowRef?: string
  refs?: ReadonlyArray<string>
  sourceRefs?: ReadonlyArray<string>
  blockerRefs?: ReadonlyArray<string>
  caveatRefs?: ReadonlyArray<string>
  amountSats?: number
  realBitcoinMoved?: boolean
  state?: string
  text: string
}): PublicActivityTimelineEvent => {
  const base = {
    refs: [],
    sourceRefs: [],
    blockerRefs: [],
    caveatRefs: [],
    ...input,
  }
  return {
    ...base,
    cursor: publicActivityTimelineCursorForEvent(base),
  }
}

const emptyEnvelope = (
  input: Partial<PublicActivityTimelineEnvelope>,
): PublicActivityTimelineEnvelope => ({
  schemaVersion: PUBLIC_ACTIVITY_TIMELINE_SCHEMA_VERSION,
  generatedAt,
  staleness: publicActivityTimelineLiveAtReadStaleness([
    "public_activity_timeline_read",
  ]),
  nextCursor: null,
  sourceLag: [],
  events: [],
  ...input,
})

export const emptyTimelineFixture: PublicActivityTimelineEnvelope = emptyEnvelope({
  sourceLag: [
    {
      sourceKind: "pylon_presence",
      status: "current",
      latestSourceEventAt: null,
      observedAt: generatedAt,
      lagSeconds: null,
      maxStalenessSeconds: 300,
      sourceRefs: ["source.public.pylon_presence.empty"],
      blockerRefs: [],
      caveatRefs: ["caveat.public.no_activity_in_range"],
    },
  ],
})

export const activeTimelineFixture: PublicActivityTimelineEnvelope = emptyEnvelope({
  nextCursor:
    "2026-06-18T18:00:10.000Z:capacity_funnel:event.public.capacity.snapshot.1",
  sourceLag: [
    {
      sourceKind: "pylon_presence",
      status: "current",
      latestSourceEventAt: "2026-06-18T18:00:02.000Z",
      observedAt: generatedAt,
      lagSeconds: 0,
      maxStalenessSeconds: 300,
      sourceRefs: ["pylon.public.worker.7"],
      blockerRefs: [],
      caveatRefs: [],
    },
    {
      sourceKind: "training_verification",
      status: "current",
      latestSourceEventAt: "2026-06-18T18:00:06.000Z",
      observedAt: generatedAt,
      lagSeconds: 0,
      maxStalenessSeconds: 0,
      sourceRefs: ["training.verification.challenge.public.1"],
      blockerRefs: [],
      caveatRefs: [],
    },
    {
      sourceKind: "inference_receipt",
      status: "current",
      latestSourceEventAt: "2026-06-18T18:00:08.500Z",
      observedAt: generatedAt,
      lagSeconds: 0,
      maxStalenessSeconds: 0,
      sourceRefs: [
        "receipt.inference.charge.chatcmpl_public_1",
        "https://openagents.com/api/public/inference/receipts/receipt.inference.charge.chatcmpl_public_1",
      ],
      blockerRefs: [],
      caveatRefs: [],
    },
  ],
  events: [
    event({
      eventRef: "event.public.pylon.registered.7",
      ts: "2026-06-18T18:00:00.000Z",
      kind: "pylon_registered",
      sourceKind: "pylon_api",
      actorRef: "pylon.public.worker.7",
      refs: ["pylon.public.worker.7"],
      sourceRefs: ["pylon.public.worker.7"],
      text: "Pylon registered with public-safe capability refs.",
    }),
    event({
      eventRef: "event.public.pylon.heartbeat.7",
      ts: "2026-06-18T18:00:02.000Z",
      kind: "pylon_heartbeat",
      sourceKind: "pylon_presence",
      actorRef: "pylon.public.worker.7",
      refs: ["pylon.public.worker.7"],
      sourceRefs: ["pylon.public.worker.7"],
      state: "online",
      text: "Pylon heartbeat observed.",
    }),
    event({
      eventRef: "event.public.pylon.wallet_ready.7",
      ts: "2026-06-18T18:00:03.000Z",
      kind: "wallet_ready",
      sourceKind: "pylon_presence",
      actorRef: "pylon.public.worker.7",
      refs: ["pylon.public.worker.7"],
      sourceRefs: ["pylon.public.worker.7"],
      text: "Pylon is public-wallet-ready without exposing wallet material.",
    }),
    event({
      eventRef: "event.public.pylon.assignment_ready.7",
      ts: "2026-06-18T18:00:04.000Z",
      kind: "assignment_ready",
      sourceKind: "pylon_presence",
      actorRef: "pylon.public.worker.7",
      refs: ["pylon.public.worker.7"],
      sourceRefs: ["pylon.public.worker.7"],
      text: "Pylon is assignment-ready.",
    }),
    event({
      eventRef: "event.public.window.opened.1",
      ts: "2026-06-18T18:00:05.000Z",
      kind: "window_opened",
      sourceKind: "training_window",
      targetRef: "training.window.public.1",
      runRef: "run.tassadar.executor.20260615",
      windowRef: "training.window.public.1",
      refs: ["run.tassadar.executor.20260615", "training.window.public.1"],
      sourceRefs: ["training.window.public.1"],
      state: "open",
      text: "Public training window opened.",
    }),
    event({
      eventRef: "event.public.work.claimed.1",
      ts: "2026-06-18T18:00:06.000Z",
      kind: "work_claimed",
      sourceKind: "training_window",
      actorRef: "pylon.public.worker.7",
      targetRef: "training.window.public.1",
      runRef: "run.tassadar.executor.20260615",
      windowRef: "training.window.public.1",
      refs: ["pylon.public.worker.7", "training.window.public.1"],
      sourceRefs: ["training.window.public.1"],
      state: "claimed",
      text: "Work claimed by public Pylon ref.",
    }),
    event({
      eventRef: "event.public.trace.submitted.1",
      ts: "2026-06-18T18:00:07.000Z",
      kind: "trace_submitted",
      sourceKind: "training_trace",
      actorRef: "pylon.public.worker.7",
      runRef: "run.tassadar.executor.20260615",
      windowRef: "training.window.public.1",
      refs: ["trace.public.digest.1", "training.window.public.1"],
      sourceRefs: ["trace.public.digest.1"],
      text: "Trace contribution submitted by digest ref.",
    }),
    event({
      eventRef: "event.public.verification.queued.1",
      ts: "2026-06-18T18:00:08.000Z",
      kind: "verification_queued",
      sourceKind: "training_verification",
      targetRef: "training.verification.challenge.public.1",
      runRef: "run.tassadar.executor.20260615",
      windowRef: "training.window.public.1",
      refs: ["training.verification.challenge.public.1"],
      sourceRefs: ["training.verification.challenge.public.1"],
      state: "queued",
      text: "Trace verification queued.",
    }),
    event({
      eventRef: "event.public.khala.inference.served.1",
      ts: "2026-06-18T18:00:08.500Z",
      kind: "khala_inference_served",
      sourceKind: "inference_receipt",
      actorRef: "gateway.fireworks.primary",
      targetRef: "receipt.inference.charge.chatcmpl_public_1",
      refs: [
        "receipt.inference.charge.chatcmpl_public_1",
        "openagents/khala-mini",
        "gateway.fireworks.primary",
      ],
      sourceRefs: [
        "receipt.inference.charge.chatcmpl_public_1",
        "https://openagents.com/api/public/inference/receipts/receipt.inference.charge.chatcmpl_public_1",
      ],
      state: "openagents/khala-mini",
      text: "Khala inference served with a public ledger receipt.",
    }),
    event({
      eventRef: "event.public.forum.topic.1",
      ts: "2026-06-18T18:00:09.000Z",
      kind: "forum_topic_created",
      sourceKind: "forum",
      actorRef: "forum.user.public.1",
      targetRef: "forum.topic.public.product_promises.1",
      refs: ["forum.topic.public.product_promises.1"],
      sourceRefs: ["forum.topic.public.product_promises.1"],
      text: "Public Forum topic created.",
    }),
    event({
      eventRef: "event.public.artanis.tick.1",
      ts: "2026-06-18T18:00:10.000Z",
      kind: "artanis_tick",
      sourceKind: "artanis",
      actorRef: "artanis.public.admin",
      refs: ["artanis.tick.public.1"],
      sourceRefs: ["artanis.tick.public.1"],
      state: "no_action",
      text: "Artanis tick recorded no public dispatch action.",
    }),
    event({
      eventRef: "event.public.capacity.snapshot.1",
      ts: "2026-06-18T18:00:10.000Z",
      kind: "capacity_snapshot",
      sourceKind: "capacity_funnel",
      refs: ["pylon.capacity.snapshot.public.1"],
      sourceRefs: ["pylon.capacity.snapshot.public.1"],
      state: "online_now:7",
      text: "Capacity snapshot recorded with public aggregate counts.",
    }),
  ],
})

export const staleTimelineFixture: PublicActivityTimelineEnvelope = emptyEnvelope({
  generatedAt: "2026-06-18T19:00:00.000Z",
  staleness: publicActivityTimelineStoredSnapshotStaleness(300, [
    "forum_post_created",
    "pylon_heartbeat_ingested",
  ]),
  sourceLag: [
    {
      sourceKind: "forum",
      status: "stale",
      latestSourceEventAt: "2026-06-18T18:30:00.000Z",
      observedAt: "2026-06-18T19:00:00.000Z",
      lagSeconds: 1800,
      maxStalenessSeconds: 300,
      sourceRefs: ["forum.context.public.product_promises"],
      blockerRefs: ["blocker.public.forum_activity_projection_lag"],
      caveatRefs: ["caveat.public.source_lag_exceeds_contract"],
    },
  ],
  events: [
    event({
      eventRef: "event.public.projection_gap.forum.stale.1",
      ts: "2026-06-18T19:00:00.000Z",
      kind: "projection_gap",
      sourceKind: "projection_gap",
      refs: ["forum.context.public.product_promises"],
      blockerRefs: ["blocker.public.forum_activity_projection_lag"],
      caveatRefs: ["caveat.public.source_lag_exceeds_contract"],
      text: "Forum source lag exceeded the public activity timeline contract.",
    }),
  ],
})

export const replayRangeTimelineFixture: PublicActivityTimelineEnvelope = emptyEnvelope({
  nextCursor:
    "2026-06-18T18:02:04.000Z:training_verification:event.public.verification.verified.range.1",
  range: {
    from: "2026-06-18T18:02:00.000Z",
    to: "2026-06-18T18:02:04.000Z",
    since: null,
    limit: 100,
    filterKinds: [
      "window_opened",
      "work_claimed",
      "trace_submitted",
      "verification_verified",
    ],
  },
  events: [
    event({
      eventRef: "event.public.window.opened.range.1",
      ts: "2026-06-18T18:02:00.000Z",
      kind: "window_opened",
      sourceKind: "training_window",
      runRef: "run.tassadar.executor.20260615",
      windowRef: "training.window.public.range.1",
      refs: ["training.window.public.range.1"],
      sourceRefs: ["training.window.public.range.1"],
      text: "Replay range window opened.",
    }),
    event({
      eventRef: "event.public.work.claimed.range.1",
      ts: "2026-06-18T18:02:01.000Z",
      kind: "work_claimed",
      sourceKind: "training_window",
      actorRef: "pylon.public.worker.range",
      runRef: "run.tassadar.executor.20260615",
      windowRef: "training.window.public.range.1",
      refs: ["pylon.public.worker.range", "training.window.public.range.1"],
      sourceRefs: ["training.window.public.range.1"],
      text: "Replay range work claimed.",
    }),
    event({
      eventRef: "event.public.trace.submitted.range.1",
      ts: "2026-06-18T18:02:02.000Z",
      kind: "trace_submitted",
      sourceKind: "training_trace",
      actorRef: "pylon.public.worker.range",
      runRef: "run.tassadar.executor.20260615",
      windowRef: "training.window.public.range.1",
      refs: ["trace.public.digest.range.1"],
      sourceRefs: ["trace.public.digest.range.1"],
      text: "Replay range trace submitted by digest.",
    }),
    event({
      eventRef: "event.public.verification.verified.range.1",
      ts: "2026-06-18T18:02:04.000Z",
      kind: "verification_verified",
      sourceKind: "training_verification",
      actorRef: "validator.public.worker.range",
      targetRef: "training.verification.challenge.public.range.1",
      runRef: "run.tassadar.executor.20260615",
      windowRef: "training.window.public.range.1",
      refs: ["training.verification.challenge.public.range.1"],
      sourceRefs: ["training.verification.challenge.public.range.1"],
      state: "verified",
      text: "Replay range trace verified.",
    }),
  ],
})

export const simulationOnlyTimelineFixture: PublicActivityTimelineEnvelope = emptyEnvelope({
  events: [
    event({
      eventRef: "event.public.settlement.simulation.1",
      ts: "2026-06-18T18:03:00.000Z",
      kind: "settlement_recorded",
      sourceKind: "settlement_receipt",
      actorRef: "treasury.public.simulation",
      targetRef: "pylon.public.worker.simulation",
      runRef: "run.tassadar.executor.20260615",
      windowRef: "training.window.public.simulation.1",
      refs: ["receipt.public.simulation.1"],
      sourceRefs: ["receipt.public.simulation.1"],
      caveatRefs: ["caveat.public.simulation_not_real_bitcoin"],
      amountSats: 1005,
      realBitcoinMoved: false,
      state: "settled_simulation",
      text: "Simulation settlement recorded; no real Bitcoin movement.",
    }),
  ],
})

export const realBitcoinTimelineFixture: PublicActivityTimelineEnvelope = emptyEnvelope({
  nextCursor:
    "2026-06-18T18:04:02.000Z:settlement_receipt:event.public.real_bitcoin_moved.1",
  events: [
    event({
      eventRef: "event.public.settlement.real.1",
      ts: "2026-06-18T18:04:00.000Z",
      kind: "settlement_recorded",
      sourceKind: "settlement_receipt",
      actorRef: "treasury.public.spark",
      targetRef: "pylon.public.worker.real",
      runRef: "run.tassadar.executor.20260615",
      windowRef: "training.window.public.real.1",
      refs: ["receipt.public.real.1"],
      sourceRefs: ["receipt.public.real.1"],
      amountSats: 1000,
      realBitcoinMoved: true,
      state: "settled",
      text: "Receipt-backed settlement recorded.",
    }),
    event({
      eventRef: "event.public.real_bitcoin_moved.1",
      ts: "2026-06-18T18:04:02.000Z",
      kind: "real_bitcoin_moved",
      sourceKind: "settlement_receipt",
      actorRef: "treasury.public.spark",
      targetRef: "pylon.public.worker.real",
      runRef: "run.tassadar.executor.20260615",
      windowRef: "training.window.public.real.1",
      refs: ["receipt.public.real.1"],
      sourceRefs: ["receipt.public.real.1"],
      amountSats: 1000,
      realBitcoinMoved: true,
      state: "confirmed",
      text: "Receipt-backed real Bitcoin movement confirmed.",
    }),
  ],
})

export const publicActivityTimelineFixtures: ReadonlyArray<PublicActivityTimelineEnvelope> = [
  emptyTimelineFixture,
  activeTimelineFixture,
  staleTimelineFixture,
  replayRangeTimelineFixture,
  simulationOnlyTimelineFixture,
  realBitcoinTimelineFixture,
]
