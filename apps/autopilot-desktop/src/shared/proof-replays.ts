export const DEFAULT_DESKTOP_PROOF_REPLAY_SLUG = "archived-proof-replay"

export type ProofReplayCatalogSlug =
  | typeof DEFAULT_DESKTOP_PROOF_REPLAY_SLUG
  | "first-real-settlement"

export type ProofReplayCatalogEntry = Readonly<{
  slug: ProofReplayCatalogSlug
  title: string
  description: string
  summary: string
  bundleEndpoint: string
  websitePath: string
  socialPath?: string
}>

export type ProofReplayBundle = Readonly<{
  title?: string
  actors: ReadonlyArray<Record<string, unknown>>
  events: ReadonlyArray<
    Record<string, unknown> & {
      amountSats?: number
      caveat?: string
      displayText: string
      kind:
        | "payment_zap_confirmed"
        | "recipient_confirmation_recorded"
        | "proof_verified"
        | "settlement_blocked_closed"
        | string
    }
  >
  gaps: ReadonlyArray<Record<string, unknown> & { gapRef: string; reason: string; sourceRefs?: ReadonlyArray<string> }>
  sourceRefs: ReadonlyArray<Readonly<{ ref: string; url?: string; kind?: string }>>
}>

export type DesktopGeneratedProofReplayFilters = Readonly<{
  from: string
  to: string
  since?: string
  runRef?: string
  windowRef?: string
  actorRef?: string
  pairRef?: string
  kind?: string
  source?: string
  limit?: number
}>

export type DesktopProofReplayRequest =
  | Readonly<{ mode: "catalog"; slug: ProofReplayCatalogSlug }>
  | Readonly<{
      mode: "generated"
      filters: DesktopGeneratedProofReplayFilters
    }>

export type DesktopGeneratedProofReplayManifest = Readonly<{
  input?: Record<string, unknown>
  caveatRefs?: ReadonlyArray<string>
  source?: { route?: string; url?: string }
  sourceLag?: ReadonlyArray<unknown>
  staleness?: unknown
}>

export type DesktopProofReplaySummary = Readonly<{
  actorCount: number
  confirmedZapSats: number
  durationSecond: number
  eventCount: number
  gapCount: number
  sourceRefCount: number
}>

export type DesktopProofReplayProjection = Readonly<{
  ok: boolean
  fetchedAt: string
  sourceUrl: string
  entry: ProofReplayCatalogEntry | null
  request: DesktopProofReplayRequest
  filterLabel: string
  generatedFrom: DesktopGeneratedProofReplayManifest | null
  caveatRefs: ReadonlyArray<string>
  bundle: ProofReplayBundle | null
  summary: DesktopProofReplaySummary | null
  blockerRefs: ReadonlyArray<string>
  cacheState: "live_https" | "stale_snapshot_unavailable"
  cacheLabel: string
  error?: string
}>

export type DesktopProofReplayFetch = (
  input: string | URL | Request,
  init?: RequestInit,
) => Promise<Response>

const archivedEntry: ProofReplayCatalogEntry = {
  slug: DEFAULT_DESKTOP_PROOF_REPLAY_SLUG,
  title: "Archived proof replay",
  description: "Proof replay bundles were archived with the retired Tassadar/Psionic program.",
  summary: "Archived to backroom.",
  bundleEndpoint: "backroom:openagents-prune-20260708-tassadar-psionic",
  websitePath: "/docs/retired",
}

const firstSettlementArchivedEntry: ProofReplayCatalogEntry = {
  ...archivedEntry,
  slug: "first-real-settlement",
  title: "Archived first settlement",
}

export const desktopProofReplayCatalog = (): ReadonlyArray<ProofReplayCatalogEntry> => [
  firstSettlementArchivedEntry,
  archivedEntry,
]

export const desktopProofReplayEntryForSlug = (slug: string): ProofReplayCatalogEntry | null =>
  desktopProofReplayCatalog().find((entry) => entry.slug === slug) ?? null

export const desktopProofReplayCatalogRequest = (
  slug: ProofReplayCatalogSlug,
): DesktopProofReplayRequest => ({ mode: "catalog", slug })

export const summarizeProofReplayBundle = (
  bundle: ProofReplayBundle,
): DesktopProofReplaySummary => ({
  actorCount: bundle.actors.length,
  confirmedZapSats: bundle.events.reduce((sum, event) => sum + (event.amountSats ?? 0), 0),
  durationSecond: 0,
  eventCount: bundle.events.length,
  gapCount: bundle.gaps.length,
  sourceRefCount: bundle.sourceRefs.length,
})

export const generatedProofReplayFilterLabel = (
  filters: DesktopGeneratedProofReplayFilters,
): string => `${filters.from} -> ${filters.to}`

export const generatedProofReplayBundleEndpoint = (): string =>
  "backroom:openagents-prune-20260708-tassadar-psionic"

const normalizeProofReplayRequest = (
  request: ProofReplayCatalogSlug | DesktopProofReplayRequest,
): DesktopProofReplayRequest =>
  typeof request === "string" ? desktopProofReplayCatalogRequest(request) : request

export const blockedDesktopProofReplayProjection = (
  request: ProofReplayCatalogSlug | DesktopProofReplayRequest,
  error = "Proof replay was archived with the retired Tassadar/Psionic program.",
): DesktopProofReplayProjection => {
  const normalized = normalizeProofReplayRequest(request)
  return {
    request: normalized,
    blockerRefs: ["desktop.proof_replay.archived_to_backroom"],
    bundle: null,
    cacheLabel: "archived to backroom; no live replay bundle",
    cacheState: "stale_snapshot_unavailable",
    caveatRefs: ["backroom.openagents_prune_20260708_tassadar_psionic"],
    entry: normalized.mode === "catalog" ? desktopProofReplayEntryForSlug(normalized.slug) : null,
    error,
    fetchedAt: new Date().toISOString(),
    filterLabel:
      normalized.mode === "generated"
        ? generatedProofReplayFilterLabel(normalized.filters)
        : archivedEntry.title,
    generatedFrom: null,
    ok: false,
    sourceUrl: archivedEntry.bundleEndpoint,
    summary: null,
  }
}

export const loadDesktopProofReplayProjection = async (
  requestOrSlug: ProofReplayCatalogSlug | DesktopProofReplayRequest,
): Promise<DesktopProofReplayProjection> =>
  blockedDesktopProofReplayProjection(requestOrSlug)
