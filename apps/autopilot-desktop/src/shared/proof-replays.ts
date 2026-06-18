import {
  assertProofReplayBundleShipmentGate,
  buildReplayRenderPlan,
  FIRST_REAL_SETTLEMENT_REPLAY_SLUG,
  OPENAGENTS_PUBLIC_ORIGIN,
  proofReplayBundleEndpointForSlug,
  proofReplayCatalog,
  proofReplayCatalogEntryForSlug,
  type ProofReplayBundle,
  type ProofReplayCatalogEntry,
  type ProofReplayCatalogSlug,
} from "@openagentsinc/proof-replay"

export const DEFAULT_DESKTOP_PROOF_REPLAY_SLUG =
  FIRST_REAL_SETTLEMENT_REPLAY_SLUG

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
  bundle: ProofReplayBundle | null
  summary: DesktopProofReplaySummary | null
  blockerRefs: ReadonlyArray<string>
  cacheState: "live_https" | "stale_snapshot_unavailable"
  cacheLabel: string
  error?: string
}>

type ProofReplayFetch = (
  input: string | URL | Request,
  init?: RequestInit,
) => Promise<Response>

const publicErrorText = (error: unknown): string => {
  const message = error instanceof Error ? error.message : String(error)
  return message
    .replace(/\/Users\/[^\s)"']+/g, "local path redacted")
    .replace(/[A-Za-z0-9_./-]*\.secrets\/[A-Za-z0-9_./-]*/g, "secret path redacted")
}

export const desktopProofReplayCatalog = (
  origin: string = OPENAGENTS_PUBLIC_ORIGIN,
): ReadonlyArray<ProofReplayCatalogEntry> => proofReplayCatalog(origin)

export const desktopProofReplayEntryForSlug = (
  slug: string,
  origin: string = OPENAGENTS_PUBLIC_ORIGIN,
): ProofReplayCatalogEntry | null =>
  proofReplayCatalogEntryForSlug(slug, origin) ?? null

export const summarizeProofReplayBundle = (
  bundle: ProofReplayBundle,
): DesktopProofReplaySummary => {
  const plan = buildReplayRenderPlan(bundle)
  const confirmedZapSats = bundle.events
    .filter(event => event.kind === "payment_zap_confirmed")
    .reduce((sum, event) => sum + (event.amountSats ?? 0), 0)

  return {
    actorCount: bundle.actors.length,
    confirmedZapSats,
    durationSecond: plan.durationSecond,
    eventCount: bundle.events.length,
    gapCount: bundle.gaps.length,
    sourceRefCount: bundle.sourceRefs.length,
  }
}

export const blockedDesktopProofReplayProjection = (
  slug: string,
  error: string,
  origin: string = OPENAGENTS_PUBLIC_ORIGIN,
): DesktopProofReplayProjection => ({
  blockerRefs: ["desktop.proof_replay.bundle_unavailable"],
  bundle: null,
  cacheLabel: "no offline snapshot; live public bundle unavailable",
  cacheState: "stale_snapshot_unavailable",
  entry: desktopProofReplayEntryForSlug(slug, origin),
  error,
  fetchedAt: new Date().toISOString(),
  ok: false,
  sourceUrl: proofReplayBundleEndpointForSlug(slug, origin),
  summary: null,
})

export const loadDesktopProofReplayProjection = async (
  slug: ProofReplayCatalogSlug,
  options: Readonly<{
    fetcher?: ProofReplayFetch
    origin?: string
  }> = {},
): Promise<DesktopProofReplayProjection> => {
  const origin = options.origin ?? OPENAGENTS_PUBLIC_ORIGIN
  const entry = desktopProofReplayEntryForSlug(slug, origin)
  if (entry === null) {
    return blockedDesktopProofReplayProjection(
      slug,
      `unknown proof replay: ${slug}`,
      origin,
    )
  }

  const fetcher = options.fetcher ?? fetch
  const fetchedAt = new Date().toISOString()

  try {
    const response = await fetcher(entry.bundleEndpoint, {
      headers: { accept: "application/json" },
    })
    if (!response.ok) {
      return {
        blockerRefs: ["desktop.proof_replay.http_unavailable"],
        bundle: null,
        cacheLabel: "no offline snapshot; live public bundle unavailable",
        cacheState: "stale_snapshot_unavailable",
        entry,
        error: `HTTP ${response.status}`,
        fetchedAt,
        ok: false,
        sourceUrl: entry.bundleEndpoint,
        summary: null,
      }
    }

    const bundle = (await response.json()) as ProofReplayBundle
    assertProofReplayBundleShipmentGate(bundle)

    return {
      blockerRefs: [],
      bundle,
      cacheLabel: "live HTTPS read from openagents.com; no offline snapshot",
      cacheState: "live_https",
      entry,
      fetchedAt,
      ok: true,
      sourceUrl: entry.bundleEndpoint,
      summary: summarizeProofReplayBundle(bundle),
    }
  } catch (error) {
    return {
      blockerRefs: ["desktop.proof_replay.shipment_gate_blocked"],
      bundle: null,
      cacheLabel: "no offline snapshot; live public bundle rejected",
      cacheState: "stale_snapshot_unavailable",
      entry,
      error: publicErrorText(error),
      fetchedAt,
      ok: false,
      sourceUrl: entry.bundleEndpoint,
      summary: null,
    }
  }
}
