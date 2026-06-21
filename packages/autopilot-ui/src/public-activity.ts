import type { Attribute, Html } from "foldkit/html"
import { html } from "foldkit/html"

import { statusChip, type AutopilotUiMessage, type ChipTone } from "./view.js"

export type PublicActivityStripEvent = Readonly<{
  eventRef: string
  cursor: string
  ts: string
  kind: string
  sourceKind: string
  actorRef?: string | undefined
  targetRef?: string | undefined
  runRef?: string | undefined
  windowRef?: string | undefined
  refs: readonly string[]
  sourceRefs: readonly string[]
  blockerRefs: readonly string[]
  caveatRefs: readonly string[]
  amountSats?: number | undefined
  realBitcoinMoved?: boolean | undefined
  state?: string | undefined
  text: string
}>

export type PublicActivityStripSourceLag = Readonly<{
  sourceKind: string
  status: "current" | "stale" | "unavailable" | "projection_gap" | string
  latestSourceEventAt: string | null
  observedAt: string
  lagSeconds: number | null
  maxStalenessSeconds: number
  sourceRefs: readonly string[]
  blockerRefs: readonly string[]
  caveatRefs: readonly string[]
}>

export type PublicActivityStripEnvelope = Readonly<{
  generatedAt: string
  nextCursor: string | null
  events: readonly PublicActivityStripEvent[]
  sourceLag: readonly PublicActivityStripSourceLag[]
}>

export type PublicActivityStripInput = Readonly<{
  className?: string
  emptyLabel?: string
  envelope: PublicActivityStripEnvelope | null
  maxEvents?: number
  pending?: boolean
  sourceUrl: string | null
  statusLabel?: string
  title?: string
}>

export type PublicActivityCategory =
  | "boot"
  | "forum"
  | "operator"
  | "settle"
  | "verify"
  | "work"

const h = html<AutopilotUiMessage>()

const className = (value: string): Attribute<AutopilotUiMessage> => h.Class(value)

const fleetKinds = new Set([
  "assignment_ready",
  "capacity_snapshot",
  "pylon_heartbeat",
  "pylon_registered",
  "wallet_ready",
])

const moneyKinds = new Set([
  "real_bitcoin_moved",
  "settlement_recorded",
])

const forumKinds = new Set(["forum_posted", "forum_topic_created"])

const workKinds = new Set([
  "trace_submitted",
  "window_closed",
  "window_opened",
  "work_claimed",
])

const verifyKinds = new Set([
  "verification_queued",
  "verification_rejected",
  "verification_verified",
])

const operatorKinds = new Set(["artanis_tick", "projection_gap"])

const displayLabel = (value: string): string =>
  value.replaceAll("_", " ").replace(/\b\w/g, part => part.toUpperCase())

export const publicActivityCategoryForEvent = (
  event: Pick<PublicActivityStripEvent, "kind" | "sourceKind">,
): PublicActivityCategory => {
  if (
    fleetKinds.has(event.kind) ||
    event.sourceKind === "pylon_api" ||
    event.sourceKind === "pylon_presence" ||
    event.sourceKind === "capacity_funnel"
  ) {
    return "boot"
  }

  if (
    workKinds.has(event.kind) ||
    event.sourceKind === "training_window" ||
    event.sourceKind === "training_trace"
  ) {
    return "work"
  }

  if (verifyKinds.has(event.kind) || event.sourceKind === "training_verification") {
    return "verify"
  }

  if (moneyKinds.has(event.kind) || event.sourceKind === "settlement_receipt") {
    return "settle"
  }

  if (forumKinds.has(event.kind) || event.sourceKind === "forum") {
    return "forum"
  }

  if (
    operatorKinds.has(event.kind) ||
    event.sourceKind === "artanis" ||
    event.sourceKind === "projection_gap"
  ) {
    return "operator"
  }

  return "operator"
}

const categoryTone = (category: PublicActivityCategory): ChipTone => {
  switch (category) {
    case "boot":
      return "info"
    case "forum":
      return "neutral"
    case "operator":
      return "warning"
    case "settle":
      return "success"
    case "verify":
      return "warning"
    case "work":
      return "info"
  }
}

const sourceLagTone = (status: string): ChipTone =>
  status === "current"
    ? "success"
    : status === "stale"
      ? "warning"
      : status === "unavailable" || status === "projection_gap"
        ? "danger"
        : "neutral"

const safePublicPath = (href: string): string | null => {
  const trimmed = href.trim()
  if (
    trimmed.length === 0 ||
    !trimmed.startsWith("/") ||
    trimmed.startsWith("//") ||
    /(@|\/Users\/|\/home\/|secret|token|private|raw|mnemonic|preimage|invoice)/i.test(trimmed)
  ) {
    return null
  }

  return trimmed
}

export const publicActivityHrefForRef = (
  ref: string,
  event: Pick<PublicActivityStripEvent, "runRef">,
): string | null => {
  const trimmed = ref.trim()
  if (trimmed.startsWith("route:")) {
    return safePublicPath(trimmed.slice("route:".length))
  }

  if (trimmed.startsWith("/")) {
    return safePublicPath(trimmed)
  }

  if (/^receipt\./i.test(trimmed)) {
    return safePublicPath(
      trimmed.startsWith("receipt.forum.")
        ? `/api/forum/receipts/${encodeURIComponent(trimmed)}`
        : `/api/public/nexus-pylon/receipts/${encodeURIComponent(trimmed)}`,
    )
  }

  if (/^training\.verification\.challenge\./i.test(trimmed)) {
    return safePublicPath(
      `/api/public/training/verification-challenges/${encodeURIComponent(trimmed)}`,
    )
  }

  if (
    /^training\.window\./i.test(trimmed) ||
    /^trace\.public\./i.test(trimmed)
  ) {
    const runRef = event.runRef?.trim()
    if (runRef !== undefined && runRef.length > 0) {
      return safePublicPath(
        `/api/public/training/runs/${encodeURIComponent(runRef)}?focusRef=${encodeURIComponent(trimmed)}`,
      )
    }
  }

  if (/^run\./i.test(trimmed)) {
    return safePublicPath(`/api/public/training/runs/${encodeURIComponent(trimmed)}`)
  }

  if (/^pylon\.|^pylon_/i.test(trimmed)) {
    return safePublicPath("/api/public/pylon-stats")
  }

  if (/^forum\./i.test(trimmed)) {
    return safePublicPath("/forum")
  }

  if (/^artanis\./i.test(trimmed)) {
    return safePublicPath("/api/public/artanis/admin-ticks")
  }

  if (/capacity/i.test(trimmed)) {
    return safePublicPath("/api/public/pylon-capacity-funnel/history")
  }

  return null
}

const uniqueRefs = (
  refs: ReadonlyArray<string | null | undefined>,
): readonly string[] => {
  const seen = new Set<string>()
  const result: string[] = []
  for (const ref of refs) {
    const trimmed = ref?.trim() ?? ""
    if (trimmed === "" || seen.has(trimmed)) continue
    seen.add(trimmed)
    result.push(trimmed)
  }
  return result
}

const eventProofRefs = (event: PublicActivityStripEvent): readonly string[] =>
  uniqueRefs([
    ...event.sourceRefs,
    ...event.refs,
    ...event.blockerRefs,
    ...event.caveatRefs,
  ])

const latestEvents = (
  envelope: PublicActivityStripEnvelope,
  limit: number,
): readonly PublicActivityStripEvent[] =>
  [...envelope.events].sort((left, right) => right.cursor.localeCompare(left.cursor)).slice(0, limit)

const publicRefLink = (
  ref: string,
  event: Pick<PublicActivityStripEvent, "runRef">,
): Html => {
  const href = publicActivityHrefForRef(ref, event)
  if (href === null) {
    return h.code([className("public-activity-ref")], [ref])
  }

  return h.a(
    [
      className("public-activity-ref public-activity-link"),
      h.Href(href),
      h.Target("_blank"),
      h.Rel("noreferrer"),
    ],
    [ref],
  )
}

const eventRow = (event: PublicActivityStripEvent): Html => {
  const category = publicActivityCategoryForEvent(event)
  const refs = eventProofRefs(event).slice(0, 4)
  const sourceLabel = displayLabel(event.sourceKind)
  const amount =
    typeof event.amountSats === "number" && Number.isFinite(event.amountSats)
      ? `${event.amountSats.toLocaleString()} sats`
      : null

  return h.article(
    [
      className("public-activity-event"),
      h.DataAttribute("public-activity-event", event.eventRef),
      h.DataAttribute("public-activity-kind", event.kind),
      h.DataAttribute("public-activity-category", category),
    ],
    [
      h.div([className("public-activity-event-head")], [
        statusChip({
          label: category,
          tone: categoryTone(category),
          attrs: [h.DataAttribute("public-activity-category-chip", category)],
        }),
        h.code([className("public-activity-event-kind")], [event.kind]),
        h.time([className("public-activity-time")], [event.ts]),
      ]),
      h.p([className("public-activity-copy")], [event.text]),
      h.div([className("public-activity-meta")], [
        h.span([], [sourceLabel]),
        h.span([], [event.state ?? "observed"]),
        amount === null ? h.empty : h.span([], [amount]),
        event.runRef === undefined
          ? h.empty
          : h.code([className("public-activity-inline-ref")], [event.runRef]),
      ]),
      h.div(
        [className("public-activity-refs")],
        refs.length === 0
          ? [h.span([className("public-activity-muted")], ["no refs"])]
          : refs.map(ref => publicRefLink(ref, event)),
      ),
    ],
  )
}

const sourceLagWarnings = (
  lags: readonly PublicActivityStripSourceLag[],
): readonly Html[] =>
  lags
    .filter(lag => lag.status !== "current")
    .map(lag => {
      const refs = uniqueRefs([
        ...lag.sourceRefs,
        ...lag.blockerRefs,
        ...lag.caveatRefs,
      ]).slice(0, 3)
      const seconds =
        lag.lagSeconds === null ? "unknown lag" : `${lag.lagSeconds}s lag`
      return h.div(
        [
          className("public-activity-lag-row"),
          h.DataAttribute("public-activity-source-lag", lag.sourceKind),
        ],
        [
          statusChip({
            label: lag.status,
            tone: sourceLagTone(lag.status),
            attrs: [h.DataAttribute("public-activity-source-status", lag.status)],
          }),
          h.span([className("public-activity-lag-text")], [
            `${displayLabel(lag.sourceKind)} · ${seconds}`,
          ]),
          ...refs.map(ref =>
            h.code([className("public-activity-lag-ref")], [ref]),
          ),
        ],
      )
    })

export const PublicActivityStrip = (input: PublicActivityStripInput): Html => {
  const envelope = input.envelope
  const maxEvents = input.maxEvents ?? 6
  const events = envelope === null ? [] : latestEvents(envelope, maxEvents)
  const warnings = envelope === null ? [] : sourceLagWarnings(envelope.sourceLag)
  const statusLabel =
    input.statusLabel ??
    (input.pending === true
      ? "loading"
      : envelope === null
        ? "unavailable"
        : `${events.length}/${envelope.events.length} events`)
  const rootClass = [
    "public-activity-strip",
    input.className ?? "",
  ].filter(Boolean).join(" ")

  return h.section(
    [
      className(rootClass),
      h.DataAttribute("public-activity-strip", ""),
    ],
    [
      h.header([className("public-activity-head")], [
        h.div([className("public-activity-title-group")], [
          h.h2([className("public-activity-title")], [
            input.title ?? "Public Activity",
          ]),
          h.p([className("public-activity-source")], [
            input.sourceUrl ?? "waiting for public timeline",
          ]),
        ]),
        statusChip({
          label: statusLabel,
          tone:
            input.pending === true
              ? "info"
              : envelope === null
                ? "warning"
                : warnings.length > 0
                  ? "warning"
                  : "success",
          attrs: [h.DataAttribute("public-activity-status", statusLabel)],
        }),
      ]),
      warnings.length === 0
        ? h.empty
        : h.div([className("public-activity-lag-list")], warnings),
      events.length === 0
        ? h.p([className("public-activity-empty")], [
            input.emptyLabel ?? "No public activity events loaded.",
          ])
        : h.div([className("public-activity-list")], events.map(eventRow)),
    ],
  )
}
