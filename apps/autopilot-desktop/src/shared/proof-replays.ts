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
  input?: {
    actorRefs?: ReadonlyArray<string>
    filterKinds?: ReadonlyArray<string>
    filterSources?: ReadonlyArray<string>
    from?: string | null
    limit?: number
    runRefs?: ReadonlyArray<string>
    since?: string | null
    to?: string | null
    windowRefs?: ReadonlyArray<string>
  }
  caveatRefs?: ReadonlyArray<string>
  source?: {
    route?: string
    url?: string
  }
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

export const desktopProofReplayCatalogRequest = (
  slug: ProofReplayCatalogSlug,
): DesktopProofReplayRequest => ({ mode: "catalog", slug })

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

const optionalParam = (value: string | undefined): string | null => {
  const trimmed = value?.trim() ?? ""
  return trimmed === "" ? null : trimmed
}

const splitList = (value: string | undefined): ReadonlyArray<string> =>
  value === undefined
    ? []
    : value
        .split(",")
        .map(item => item.trim())
        .filter(item => item !== "")

const pairActorRefs = (value: string | undefined): ReadonlyArray<string> =>
  splitList(value).flatMap(item =>
    item
      .split(/[+:]/)
      .map(part => part.trim())
      .filter(part => part !== ""),
  )

const actorRefsForFilters = (
  filters: DesktopGeneratedProofReplayFilters,
): ReadonlyArray<string> =>
  [...new Set([
    ...splitList(filters.actorRef),
    ...pairActorRefs(filters.pairRef),
  ])].sort()

const safeLimit = (value: number | undefined): number => {
  if (value === undefined || !Number.isFinite(value)) return 20
  return Math.min(Math.max(1, Math.trunc(value)), 200)
}

const isSafeIsoRange = (from: string, to: string): boolean => {
  const fromMillis = Date.parse(from)
  const toMillis = Date.parse(to)
  return (
    Number.isFinite(fromMillis) &&
    Number.isFinite(toMillis) &&
    fromMillis <= toMillis
  )
}

export const generatedProofReplayFilterLabel = (
  filters: DesktopGeneratedProofReplayFilters,
): string => {
  const parts = [
    optionalParam(filters.runRef),
    optionalParam(filters.windowRef),
    optionalParam(filters.actorRef),
    optionalParam(filters.pairRef),
    optionalParam(filters.kind),
    optionalParam(filters.source),
    optionalParam(filters.since),
  ].filter((part): part is string => part !== null)

  return parts.length === 0
    ? `${filters.from} → ${filters.to}`
    : `${filters.from} → ${filters.to} · ${parts.join(" · ")}`
}

export const generatedProofReplayBundleEndpoint = (
  filters: DesktopGeneratedProofReplayFilters,
  origin: string = OPENAGENTS_PUBLIC_ORIGIN,
): string => {
  const url = new URL("/api/public/proof-replays", origin)
  url.searchParams.set("mode", "activity-timeline")
  url.searchParams.set("from", filters.from.trim())
  url.searchParams.set("to", filters.to.trim())
  url.searchParams.set("limit", String(safeLimit(filters.limit)))

  const since = optionalParam(filters.since)
  const runRef = optionalParam(filters.runRef)
  const windowRef = optionalParam(filters.windowRef)
  const kind = optionalParam(filters.kind)
  const source = optionalParam(filters.source)

  if (since !== null) url.searchParams.set("since", since)
  if (runRef !== null) url.searchParams.set("runRef", runRef)
  if (windowRef !== null) url.searchParams.set("windowRef", windowRef)
  for (const actorRef of actorRefsForFilters(filters)) {
    url.searchParams.append("actorRef", actorRef)
  }
  for (const filterKind of splitList(kind ?? undefined)) {
    url.searchParams.append("kind", filterKind)
  }
  for (const sourceKind of splitList(source ?? undefined)) {
    url.searchParams.append("source", sourceKind)
  }

  return url.toString()
}

const normalizeProofReplayRequest = (
  request: ProofReplayCatalogSlug | DesktopProofReplayRequest,
): DesktopProofReplayRequest =>
  typeof request === "string" ? desktopProofReplayCatalogRequest(request) : request

const generatedManifestFromBundle = (
  bundle: ProofReplayBundle,
): DesktopGeneratedProofReplayManifest | null => {
  const generatedFrom = (bundle as ProofReplayBundle & {
    generatedFrom?: DesktopGeneratedProofReplayManifest
  }).generatedFrom
  return generatedFrom === undefined ? null : generatedFrom
}

const caveatRefsFrom = (
  bundle: ProofReplayBundle | null,
  generatedFrom: DesktopGeneratedProofReplayManifest | null,
): ReadonlyArray<string> => {
  const eventCaveats =
    bundle?.events.flatMap(event =>
      event.caveat === undefined ? [] : event.caveat.split(","),
    ) ?? []
  const gapCaveats = bundle?.gaps.flatMap(gap => gap.sourceRefs) ?? []
  return [
    ...new Set(
      [
        ...eventCaveats,
        ...gapCaveats,
        ...(generatedFrom?.caveatRefs ?? []),
      ].map(ref => ref.trim()).filter(ref => ref !== ""),
    ),
  ].sort()
}

export const blockedDesktopProofReplayProjection = (
  request: ProofReplayCatalogSlug | DesktopProofReplayRequest,
  error: string,
  origin: string = OPENAGENTS_PUBLIC_ORIGIN,
): DesktopProofReplayProjection => {
  const normalized = normalizeProofReplayRequest(request)
  const entry =
    normalized.mode === "catalog"
      ? desktopProofReplayEntryForSlug(normalized.slug, origin)
      : null
  return {
    request: normalized,
    blockerRefs: ["desktop.proof_replay.bundle_unavailable"],
    bundle: null,
    cacheLabel: "no offline snapshot; live public bundle unavailable",
    cacheState: "stale_snapshot_unavailable",
    caveatRefs: [],
    entry,
    error,
    fetchedAt: new Date().toISOString(),
    filterLabel:
      normalized.mode === "generated"
        ? generatedProofReplayFilterLabel(normalized.filters)
        : "catalog preset",
    generatedFrom: null,
    ok: false,
    sourceUrl:
      normalized.mode === "generated"
        ? generatedProofReplayBundleEndpoint(normalized.filters, origin)
        : proofReplayBundleEndpointForSlug(normalized.slug, origin),
    summary: null,
  }
}

export const loadDesktopProofReplayProjection = async (
  requestOrSlug: ProofReplayCatalogSlug | DesktopProofReplayRequest,
  options: Readonly<{
    fetcher?: ProofReplayFetch
    origin?: string
  }> = {},
): Promise<DesktopProofReplayProjection> => {
  const origin = options.origin ?? OPENAGENTS_PUBLIC_ORIGIN
  const request = normalizeProofReplayRequest(requestOrSlug)
  const entry =
    request.mode === "catalog"
      ? desktopProofReplayEntryForSlug(request.slug, origin)
      : null
  if (request.mode === "catalog" && entry === null) {
    return blockedDesktopProofReplayProjection(
      request,
      `unknown proof replay: ${request.slug}`,
      origin,
    )
  }

  if (
    request.mode === "generated" &&
    !isSafeIsoRange(request.filters.from.trim(), request.filters.to.trim())
  ) {
    return {
      blockerRefs: ["desktop.proof_replay.generated_range_required"],
      bundle: null,
      cacheLabel: "generated replay requires a bounded public ISO range",
      cacheState: "stale_snapshot_unavailable",
      caveatRefs: [],
      entry: null,
      error: "generated replay requires valid from/to ISO bounds",
      fetchedAt: new Date().toISOString(),
      filterLabel: generatedProofReplayFilterLabel(request.filters),
      generatedFrom: null,
      ok: false,
      request,
      sourceUrl: generatedProofReplayBundleEndpoint(request.filters, origin),
      summary: null,
    }
  }

  const fetcher = options.fetcher ?? fetch
  const fetchedAt = new Date().toISOString()
  const sourceUrl =
    request.mode === "generated"
      ? generatedProofReplayBundleEndpoint(request.filters, origin)
      : entry?.bundleEndpoint ?? proofReplayBundleEndpointForSlug(request.slug, origin)

  try {
    const response = await fetcher(sourceUrl, {
      headers: { accept: "application/json" },
    })
    if (!response.ok) {
      return {
        blockerRefs: ["desktop.proof_replay.http_unavailable"],
        bundle: null,
        cacheLabel: "no offline snapshot; live public bundle unavailable",
        cacheState: "stale_snapshot_unavailable",
        caveatRefs: [],
        entry,
        error: `HTTP ${response.status}`,
        fetchedAt,
        filterLabel:
          request.mode === "generated"
            ? generatedProofReplayFilterLabel(request.filters)
            : entry?.title ?? "catalog preset",
        generatedFrom: null,
        ok: false,
        request,
        sourceUrl,
        summary: null,
      }
    }

    const bundle = (await response.json()) as ProofReplayBundle
    assertProofReplayBundleShipmentGate(bundle)
    const generatedFrom = generatedManifestFromBundle(bundle)

    return {
      blockerRefs: [],
      bundle,
      cacheLabel: "live HTTPS read from openagents.com; no offline snapshot",
      cacheState: "live_https",
      caveatRefs: caveatRefsFrom(bundle, generatedFrom),
      entry,
      fetchedAt,
      filterLabel:
        request.mode === "generated"
          ? generatedProofReplayFilterLabel(request.filters)
          : entry?.title ?? "catalog preset",
      generatedFrom,
      ok: true,
      request,
      sourceUrl,
      summary: summarizeProofReplayBundle(bundle),
    }
  } catch (error) {
    return {
      blockerRefs: ["desktop.proof_replay.shipment_gate_blocked"],
      bundle: null,
      cacheLabel: "no offline snapshot; live public bundle rejected",
      cacheState: "stale_snapshot_unavailable",
      caveatRefs: [],
      entry,
      error: publicErrorText(error),
      fetchedAt,
      filterLabel:
        request.mode === "generated"
          ? generatedProofReplayFilterLabel(request.filters)
          : entry?.title ?? "catalog preset",
      generatedFrom: null,
      ok: false,
      request,
      sourceUrl,
      summary: null,
    }
  }
}
