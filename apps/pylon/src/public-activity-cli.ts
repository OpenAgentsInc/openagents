import { assertPublicProjectionSafe } from "./state.js"

export type PublicActivityCliCommand =
  | "activity"
  | "timeline"
  | "replay"
  | "receipts"
  | "evidence-pack"

export type PublicActivityCliParsed = {
  actorRefs: string[]
  baseUrl: string
  command: PublicActivityCliCommand
  filterKinds: string[]
  filterSources: string[]
  from?: string
  intervalMs: number
  json: boolean
  limit?: number
  maxIterations: number
  replayRef?: string
  runRef?: string
  since?: string
  to?: string
  verificationChallengeRef?: string
  watch: boolean
  windowRef?: string
}

export type PublicActivityCliResult = {
  ok: boolean
  authority: "observation_only"
  baseUrl: string
  command: PublicActivityCliCommand
  fetchedAt: string
  json: boolean
  requestUrls: string[]
  schema: "openagents.pylon.public_activity_cli.v1"
  blockerRefs: string[]
  caveatRefs: string[]
  output:
    | { kind: "activity"; pages: unknown[]; timeline: unknown }
    | { kind: "timeline"; timeline: unknown }
    | { kind: "replay"; bundle: unknown; eventTrack: PublicReplayCliEventTrack }
    | { kind: "receipts"; settlements: unknown; receiptUrls: string[] }
    | {
        kind: "evidence-pack"
        runSummary: unknown
        settlements: unknown
        verificationChallenge: unknown | null
        proofReplay: unknown | null
        productPromises: unknown
        refs: {
          blockerRefs: string[]
          promiseIds: string[]
          receiptRefs: string[]
          receiptUrls: string[]
          verificationChallengeRefs: string[]
          verificationChallengeUrls: string[]
        }
      }
}

export type PublicReplayCliEventTrack = {
  schema: "openagents.pylon.public_replay_event_track.v1"
  bundleRef: string | null
  title: string | null
  generatedAt: string | null
  sourceAuthority: string | null
  privacyLevel: string | null
  claimScope: string | null
  generatedFrom: unknown | null
  staleness: unknown | null
  sourceRefs: unknown[]
  events: PublicReplayCliEventRow[]
  gaps: PublicReplayCliGapRow[]
  captions: PublicReplayCliCaptionRow[]
}

export type PublicReplayCliEventRow = {
  sequenceIndex: number
  eventRef: string
  kind: string
  timelineSecond: number
  timestamp: string | null
  displayText: string
  actorRefs: string[]
  targetRefs: string[]
  sourceRefs: string[]
  caveatRefs: string[]
  captions: string[]
  amountSats: number | null
  rail: string | null
  stateBefore: string | null
  stateAfter: string | null
}

export type PublicReplayCliGapRow = {
  gapRef: string
  reason: string
  affectedRefs: string[]
  sourceRefs: string[]
}

export type PublicReplayCliCaptionRow = {
  captionRef: string
  sequenceIndex: number
  timelineSecond: number
  text: string
  sourceRefs: string[]
}

type RunOptions = {
  env?: Record<string, string | undefined>
  fetchFn?: typeof fetch
  nowIso?: () => string
}

const DEFAULT_BASE_URL = "https://openagents.com"

const filterAliases: Record<string, string[]> = {
  artanis: ["artanis_tick"],
  capacity: ["capacity_snapshot"],
  forum: ["forum_topic_created", "forum_posted"],
  gap: ["projection_gap"],
  gaps: ["projection_gap"],
  pay: ["settlement_recorded", "real_bitcoin_moved"],
  payment: ["settlement_recorded", "real_bitcoin_moved"],
  pylon: ["pylon_registered", "pylon_heartbeat", "wallet_ready", "assignment_ready"],
  settle: ["settlement_recorded", "real_bitcoin_moved"],
  settlement: ["settlement_recorded", "real_bitcoin_moved"],
  trace: ["trace_submitted"],
  verify: ["verification_queued", "verification_verified", "verification_rejected"],
  work: ["work_claimed", "trace_submitted"],
  window: ["window_opened", "window_closed"],
}

function parseOptions(args: string[]): Record<string, string | true> {
  const options: Record<string, string | true> = {}
  for (let index = 0; index < args.length; index += 1) {
    const token = args[index]
    if (!token?.startsWith("--")) continue
    const key = token.slice(2)
    const next = args[index + 1]
    if (next !== undefined && !next.startsWith("--")) {
      options[key] = next
      index += 1
    } else {
      options[key] = true
    }
  }
  return options
}

const optionString = (
  options: Record<string, string | true>,
  key: string,
): string | undefined => {
  const value = options[key]
  return typeof value === "string" && value.trim() !== "" ? value.trim() : undefined
}

const optionFlag = (options: Record<string, string | true>, key: string): boolean =>
  options[key] === true || options[key] === "true"

const optionFormat = (options: Record<string, string | true>): "text" | "json" => {
  const value = optionString(options, "format")
  if (value === undefined) return optionFlag(options, "json") ? "json" : "text"
  if (value !== "text" && value !== "json") {
    throw new Error("--format must be text or json")
  }
  return value
}

const splitList = (value: string | undefined): string[] =>
  value === undefined
    ? []
    : value
        .split(",")
        .map((item) => item.trim())
        .filter((item) => item.length > 0)

const unique = (values: readonly string[]): string[] => [...new Set(values)].sort()

function expandFilter(value: string | undefined): string[] {
  return unique(
    splitList(value).flatMap((item) => filterAliases[item] ?? [item]),
  )
}

function expandPairFilter(value: string | undefined): string[] {
  if (value === undefined) return []
  return unique(
    splitList(value).flatMap((item) =>
      item
        .split(/[+:]/)
        .map((part) => part.trim())
        .filter((part) => part.length > 0),
    ),
  )
}

function positiveInt(
  value: string | undefined,
  label: string,
): number | undefined {
  if (value === undefined) return undefined
  if (!/^[1-9][0-9]*$/.test(value)) {
    throw new Error(`${label} must be a positive integer`)
  }
  return Number.parseInt(value, 10)
}

function normalizeBaseUrl(value: string): string {
  const url = new URL(value)
  if (url.protocol !== "http:" && url.protocol !== "https:") {
    throw new Error("--base-url must be an http(s) URL")
  }
  return url.toString().replace(/\/+$/, "")
}

export function parsePublicActivityCliArgs(
  command: PublicActivityCliCommand,
  args: string[],
  env: Record<string, string | undefined> = {},
): PublicActivityCliParsed {
  const options = parseOptions(args)
  const baseUrl = normalizeBaseUrl(
    optionString(options, "base-url") ??
      env.PYLON_OPENAGENTS_BASE_URL ??
      env.OPENAGENTS_BASE_URL ??
      DEFAULT_BASE_URL,
  )
  const watch = command === "activity" && optionFlag(options, "watch")
  const limit = positiveInt(optionString(options, "limit"), "--limit")
  const maxIterations =
    positiveInt(optionString(options, "max-iterations"), "--max-iterations") ??
    1
  const intervalMs =
    positiveInt(optionString(options, "interval-ms"), "--interval-ms") ?? 15_000
  const runRef = optionString(options, "run") ?? optionString(options, "run-ref")
  const windowRef = optionString(options, "window") ?? optionString(options, "window-ref")
  const verificationChallengeRef =
    optionString(options, "challenge-ref") ??
    optionString(options, "verification-challenge-ref")
  const actorRefs = unique([
    ...splitList(optionString(options, "actor") ?? optionString(options, "actor-ref")),
    ...expandPairFilter(optionString(options, "pair")),
  ])

  if (command === "timeline" && (!optionString(options, "from") || !optionString(options, "to"))) {
    throw new Error("pylon timeline requires --from and --to")
  }
  if (command === "replay" && (!optionString(options, "from") || !optionString(options, "to"))) {
    throw new Error("pylon replay requires --from and --to")
  }
  if ((command === "receipts" || command === "evidence-pack") && !runRef) {
    throw new Error(`pylon ${command} requires --run <trainingRunRef>`)
  }

  return {
    actorRefs,
    baseUrl,
    command,
    filterKinds: expandFilter(optionString(options, "filter") ?? optionString(options, "kind")),
    filterSources: splitList(optionString(options, "source")),
    from: optionString(options, "from"),
    intervalMs,
    json: optionFormat(options) === "json",
    limit,
    maxIterations,
    replayRef: optionString(options, "replay-ref") ?? "first-real-settlement",
    runRef,
    since: optionString(options, "since"),
    to: optionString(options, "to"),
    verificationChallengeRef,
    watch,
    windowRef,
  }
}

function appendList(url: URL, key: string, values: readonly string[]) {
  for (const value of values) url.searchParams.append(key, value)
}

function timelineUrl(parsed: PublicActivityCliParsed): string {
  const url = new URL("/api/public/activity-timeline", parsed.baseUrl)
  if (parsed.since) url.searchParams.set("since", parsed.since)
  if (parsed.from) url.searchParams.set("from", parsed.from)
  if (parsed.to) url.searchParams.set("to", parsed.to)
  if (parsed.limit !== undefined) url.searchParams.set("limit", String(parsed.limit))
  appendList(url, "kind", parsed.filterKinds)
  appendList(url, "source", parsed.filterSources)
  return url.toString()
}

function settlementsUrl(parsed: PublicActivityCliParsed): string {
  return new URL(
    `/api/public/training/runs/${encodeURIComponent(parsed.runRef ?? "")}/settlements`,
    parsed.baseUrl,
  ).toString()
}

function runSummaryUrl(parsed: PublicActivityCliParsed): string {
  const url = new URL("/api/public/tassadar-run-summary", parsed.baseUrl)
  if (parsed.runRef) url.searchParams.set("run", parsed.runRef)
  return url.toString()
}

function verificationChallengeUrl(baseUrl: string, ref: string): string {
  return new URL(
    `/api/public/training/verification-challenges/${encodeURIComponent(ref)}`,
    baseUrl,
  ).toString()
}

function receiptUrl(baseUrl: string, ref: string): string {
  return new URL(
    `/api/public/nexus-pylon/receipts/${encodeURIComponent(ref)}`,
    baseUrl,
  ).toString()
}

function proofReplayUrl(parsed: PublicActivityCliParsed): string {
  const url = new URL("/api/public/proof-replays", parsed.baseUrl)
  if (parsed.replayRef) url.searchParams.set("ref", parsed.replayRef)
  return url.toString()
}

function generatedReplayUrl(parsed: PublicActivityCliParsed): string {
  const url = new URL("/api/public/proof-replays", parsed.baseUrl)
  url.searchParams.set("mode", "activity-timeline")
  if (parsed.from) url.searchParams.set("from", parsed.from)
  if (parsed.to) url.searchParams.set("to", parsed.to)
  if (parsed.since) url.searchParams.set("since", parsed.since)
  if (parsed.limit !== undefined) url.searchParams.set("limit", String(parsed.limit))
  if (parsed.runRef) url.searchParams.set("runRef", parsed.runRef)
  if (parsed.windowRef) url.searchParams.set("windowRef", parsed.windowRef)
  appendList(url, "actorRef", parsed.actorRefs)
  appendList(url, "kind", parsed.filterKinds)
  appendList(url, "source", parsed.filterSources)
  return url.toString()
}

async function fetchJson(fetchFn: typeof fetch, url: string): Promise<unknown> {
  const response = await fetchFn(url, { headers: { accept: "application/json" } })
  const text = await response.text()
  const payload = text.trim() ? JSON.parse(text) as unknown : {}
  if (!response.ok) {
    const record = payload && typeof payload === "object" ? payload as Record<string, unknown> : {}
    const reason = typeof record.error === "string" ? record.error : typeof record.reason === "string" ? record.reason : `HTTP ${response.status}`
    throw new Error(`public activity request failed (${response.status}): ${reason}`)
  }
  return payload
}

function collectPublicRefs(value: unknown, predicate: (value: string) => boolean): string[] {
  const refs = new Set<string>()
  const visit = (item: unknown) => {
    if (typeof item === "string") {
      if (predicate(item)) refs.add(item)
      return
    }
    if (Array.isArray(item)) {
      for (const child of item) visit(child)
      return
    }
    if (item !== null && typeof item === "object") {
      for (const child of Object.values(item as Record<string, unknown>)) {
        visit(child)
      }
    }
  }
  visit(value)
  return [...refs].sort()
}

const receiptRefsFrom = (value: unknown): string[] =>
  collectPublicRefs(value, (ref) => ref.startsWith("receipt."))

const blockerRefsFrom = (value: unknown): string[] =>
  collectPublicRefs(value, (ref) => ref.startsWith("blocker."))

function promiseIdsFrom(value: unknown): string[] {
  const ids = new Set<string>()
  const visit = (item: unknown) => {
    if (Array.isArray(item)) {
      for (const child of item) visit(child)
      return
    }
    if (item === null || typeof item !== "object") return
    for (const [key, child] of Object.entries(item as Record<string, unknown>)) {
      if (
        (key === "promiseId" || key === "productPromiseId") &&
        typeof child === "string" &&
        /^[a-z0-9_.-]+\.v[0-9]+$/i.test(child)
      ) {
        ids.add(child)
        continue
      }
      visit(child)
    }
  }
  visit(value)
  return [...ids].sort()
}

const verificationChallengeRefsFrom = (value: unknown): string[] =>
  collectPublicRefs(value, (ref) => ref.startsWith("training.verification.challenge."))

function eventRows(timeline: unknown): Array<Record<string, unknown>> {
  const record = timeline as { events?: unknown }
  return Array.isArray(record.events)
    ? record.events.filter((event): event is Record<string, unknown> => event !== null && typeof event === "object")
    : []
}

function settlementRows(settlements: unknown): Array<Record<string, unknown>> {
  const record = settlements as { settlementRows?: unknown }
  return Array.isArray(record.settlementRows)
    ? record.settlementRows.filter((row): row is Record<string, unknown> => row !== null && typeof row === "object")
    : []
}

function stringList(value: unknown): string[] {
  return Array.isArray(value)
    ? value.filter((item): item is string => typeof item === "string")
    : []
}

function numericValue(value: unknown): number | null {
  return typeof value === "number" && Number.isFinite(value) ? value : null
}

function stringValue(value: unknown): string | null {
  return typeof value === "string" ? value : null
}

function caveatRefsFromEvent(event: Record<string, unknown>): string[] {
  const caveat = stringValue(event.caveat)
  return caveat === null
    ? []
    : caveat
        .split(",")
        .map((item) => item.trim())
        .filter((item) => item.length > 0)
        .sort()
}

function replayBundleEvents(bundle: unknown): Array<Record<string, unknown>> {
  const record = bundle as { events?: unknown }
  return Array.isArray(record.events)
    ? record.events.filter((event): event is Record<string, unknown> => event !== null && typeof event === "object")
    : []
}

function replayBundleCaptions(bundle: unknown): Array<Record<string, unknown>> {
  const record = bundle as { captions?: unknown }
  return Array.isArray(record.captions)
    ? record.captions.filter((caption): caption is Record<string, unknown> => caption !== null && typeof caption === "object")
    : []
}

function replayBundleGaps(bundle: unknown): Array<Record<string, unknown>> {
  const record = bundle as { gaps?: unknown }
  return Array.isArray(record.gaps)
    ? record.gaps.filter((gap): gap is Record<string, unknown> => gap !== null && typeof gap === "object")
    : []
}

function buildPublicReplayEventTrack(bundle: unknown): PublicReplayCliEventTrack {
  const record = bundle !== null && typeof bundle === "object"
    ? bundle as Record<string, unknown>
    : {}
  const captions = replayBundleCaptions(bundle)
    .map((caption): PublicReplayCliCaptionRow => ({
      captionRef: stringValue(caption.captionRef) ?? "caption.unknown",
      sequenceIndex: numericValue(caption.sequenceIndex) ?? -1,
      timelineSecond: numericValue(caption.timelineSecond) ?? -1,
      text: stringValue(caption.text) ?? "",
      sourceRefs: stringList(caption.sourceRefs),
    }))
    .sort((left, right) =>
      left.sequenceIndex - right.sequenceIndex ||
      left.timelineSecond - right.timelineSecond ||
      left.captionRef.localeCompare(right.captionRef),
    )
  const captionsBySequence = new Map<number, string[]>()
  for (const caption of captions) {
    const existing = captionsBySequence.get(caption.sequenceIndex) ?? []
    captionsBySequence.set(caption.sequenceIndex, [...existing, caption.text])
  }
  const events = replayBundleEvents(bundle)
    .map((event): PublicReplayCliEventRow => {
      const sequenceIndex = numericValue(event.sequenceIndex) ?? -1
      return {
        sequenceIndex,
        eventRef: stringValue(event.eventRef) ?? "event.unknown",
        kind: stringValue(event.kind) ?? "event",
        timelineSecond: numericValue(event.timelineSecond) ?? -1,
        timestamp: stringValue(event.observedAt),
        displayText: stringValue(event.displayText) ?? "",
        actorRefs: stringList(event.actorRefs),
        targetRefs: stringList(event.targetRefs),
        sourceRefs: stringList(event.sourceRefs),
        caveatRefs: caveatRefsFromEvent(event),
        captions: captionsBySequence.get(sequenceIndex) ?? [],
        amountSats: numericValue(event.amountSats),
        rail: stringValue(event.rail),
        stateBefore: stringValue(event.stateBefore),
        stateAfter: stringValue(event.stateAfter),
      }
    })
    .sort((left, right) =>
      left.sequenceIndex - right.sequenceIndex ||
      left.timelineSecond - right.timelineSecond ||
      left.eventRef.localeCompare(right.eventRef),
    )
  const gaps = replayBundleGaps(bundle)
    .map((gap): PublicReplayCliGapRow => ({
      gapRef: stringValue(gap.gapRef) ?? "gap.unknown",
      reason: stringValue(gap.reason) ?? "",
      affectedRefs: stringList(gap.affectedRefs),
      sourceRefs: stringList(gap.sourceRefs),
    }))
    .sort((left, right) => left.gapRef.localeCompare(right.gapRef))

  return {
    schema: "openagents.pylon.public_replay_event_track.v1",
    bundleRef: stringValue(record.bundleRef),
    title: stringValue(record.title),
    generatedAt: stringValue(record.generatedAt),
    sourceAuthority: stringValue(record.sourceAuthority),
    privacyLevel: stringValue(record.privacyLevel),
    claimScope: stringValue(record.claimScope),
    generatedFrom: record.generatedFrom ?? null,
    staleness: record.staleness ?? null,
    sourceRefs: Array.isArray(record.sourceRefs) ? record.sourceRefs : [],
    events,
    gaps,
    captions,
  }
}

function assertTimelineEventsDereferenceable(timeline: unknown) {
  for (const event of eventRows(timeline)) {
    const sourceRefs = Array.isArray(event.sourceRefs) ? event.sourceRefs : []
    const blockerRefs = Array.isArray(event.blockerRefs) ? event.blockerRefs : []
    if (sourceRefs.length === 0 && blockerRefs.length === 0) {
      throw new Error("public activity event missing sourceRefs or blockerRefs")
    }
  }
}

function safeResult(result: PublicActivityCliResult): PublicActivityCliResult {
  assertPublicProjectionSafe(result)
  return result
}

export async function runPublicActivityCliCommand(
  command: PublicActivityCliCommand,
  args: string[],
  options: RunOptions = {},
): Promise<PublicActivityCliResult> {
  const parsed = parsePublicActivityCliArgs(command, args, options.env)
  const fetchFn = options.fetchFn ?? fetch
  const fetchedAt = options.nowIso?.() ?? new Date().toISOString()
  const requestUrls: string[] = []
  const common = {
    authority: "observation_only" as const,
    baseUrl: parsed.baseUrl,
    command,
    fetchedAt,
    json: parsed.json,
    schema: "openagents.pylon.public_activity_cli.v1" as const,
  }

  if (command === "activity") {
    const pages: unknown[] = []
    let since = parsed.since
    const iterations = parsed.watch ? parsed.maxIterations : 1
    for (let index = 0; index < iterations; index += 1) {
      const pageUrl = timelineUrl({ ...parsed, since })
      requestUrls.push(pageUrl)
      const page = await fetchJson(fetchFn, pageUrl)
      assertTimelineEventsDereferenceable(page)
      pages.push(page)
      const events = eventRows(page)
      const lastCursor = events.at(-1)?.cursor
      const nextCursor = (page as { nextCursor?: unknown }).nextCursor
      since =
        typeof nextCursor === "string"
          ? nextCursor
          : typeof lastCursor === "string"
            ? lastCursor
            : since
    }
    return safeResult({
      ...common,
      ok: true,
      requestUrls,
      blockerRefs: blockerRefsFrom(pages),
      caveatRefs: ["caveat.pylon_cli.public_activity_observation_only"],
      output: { kind: "activity", pages, timeline: pages.at(-1) ?? {} },
    })
  }

  if (command === "timeline") {
    const url = timelineUrl(parsed)
    requestUrls.push(url)
    const timeline = await fetchJson(fetchFn, url)
    assertTimelineEventsDereferenceable(timeline)
    return safeResult({
      ...common,
      ok: true,
      requestUrls,
      blockerRefs: blockerRefsFrom(timeline),
      caveatRefs: ["caveat.pylon_cli.public_activity_observation_only"],
      output: { kind: "timeline", timeline },
    })
  }

  if (command === "replay") {
    const url = generatedReplayUrl(parsed)
    requestUrls.push(url)
    const bundle = await fetchJson(fetchFn, url)
    const eventTrack = buildPublicReplayEventTrack(bundle)
    return safeResult({
      ...common,
      ok: true,
      requestUrls,
      blockerRefs: blockerRefsFrom(bundle),
      caveatRefs: unique([
        "caveat.pylon_cli.public_replay_observation_only",
        ...eventTrack.events.flatMap((event) => event.caveatRefs),
        ...collectPublicRefs(bundle, (ref) => ref.startsWith("caveat.")),
      ]),
      output: { kind: "replay", bundle, eventTrack },
    })
  }

  if (command === "receipts") {
    const url = settlementsUrl(parsed)
    requestUrls.push(url)
    const settlements = await fetchJson(fetchFn, url)
    const receiptRefs = receiptRefsFrom(settlements)
    const receiptUrls = receiptRefs.map((ref) => receiptUrl(parsed.baseUrl, ref))
    return safeResult({
      ...common,
      ok: true,
      requestUrls,
      blockerRefs: blockerRefsFrom(settlements),
      caveatRefs: ["caveat.pylon_cli.receipts_are_public_projection_only"],
      output: { kind: "receipts", settlements, receiptUrls },
    })
  }

  const summaryUrl = runSummaryUrl(parsed)
  const settlementUrl = settlementsUrl(parsed)
  const promisesUrl = new URL("/api/public/product-promises", parsed.baseUrl).toString()
  const replayUrl = proofReplayUrl(parsed)
  requestUrls.push(summaryUrl, settlementUrl, promisesUrl, replayUrl)
  const runSummary = await fetchJson(fetchFn, summaryUrl)
  const settlements = await fetchJson(fetchFn, settlementUrl)
  const productPromises = await fetchJson(fetchFn, promisesUrl)
  const proofReplay = await fetchJson(fetchFn, replayUrl)
  const verificationChallengeRefs = unique([
    ...(parsed.verificationChallengeRef ? [parsed.verificationChallengeRef] : []),
    ...verificationChallengeRefsFrom(runSummary),
    ...verificationChallengeRefsFrom(settlements),
  ])
  const challengeUrl =
    verificationChallengeRefs[0] === undefined
      ? null
      : verificationChallengeUrl(parsed.baseUrl, verificationChallengeRefs[0])
  const verificationChallenge =
    challengeUrl === null ? null : await fetchJson(fetchFn, challengeUrl)
  if (challengeUrl !== null) requestUrls.push(challengeUrl)
  const receiptRefs = unique([
    ...receiptRefsFrom(runSummary),
    ...receiptRefsFrom(settlements),
  ])
  const receiptUrls = receiptRefs.map((ref) => receiptUrl(parsed.baseUrl, ref))
  return safeResult({
    ...common,
    ok: true,
    requestUrls,
    blockerRefs: unique([
      ...blockerRefsFrom(runSummary),
      ...blockerRefsFrom(settlements),
      ...blockerRefsFrom(productPromises),
      ...blockerRefsFrom(verificationChallenge),
    ]),
    caveatRefs: ["caveat.pylon_cli.evidence_pack_observation_only"],
    output: {
      kind: "evidence-pack",
      runSummary,
      settlements,
      verificationChallenge,
      proofReplay,
      productPromises,
      refs: {
        blockerRefs: unique([
          ...blockerRefsFrom(runSummary),
          ...blockerRefsFrom(settlements),
          ...blockerRefsFrom(productPromises),
          ...blockerRefsFrom(verificationChallenge),
        ]),
        promiseIds: promiseIdsFrom(productPromises),
        receiptRefs,
        receiptUrls,
        verificationChallengeRefs,
        verificationChallengeUrls: verificationChallengeRefs.map((ref) =>
          verificationChallengeUrl(parsed.baseUrl, ref),
        ),
      },
    },
  })
}

export function formatPublicActivityCliText(result: PublicActivityCliResult): string {
  if (result.output.kind === "activity" || result.output.kind === "timeline") {
    const timeline =
      result.output.kind === "activity" ? result.output.timeline : result.output.timeline
    const lines = eventRows(timeline).map((event) => {
      const blockers = Array.isArray(event.blockerRefs) && event.blockerRefs.length > 0
        ? ` blockers=${event.blockerRefs.join(",")}`
        : ""
      return `${event.ts ?? "unknown"} ${event.kind ?? "event"} ${event.sourceKind ?? "source"} ${event.eventRef ?? ""}${blockers}`
    })
    return lines.length > 0 ? `${lines.join("\n")}\n` : "No public activity events.\n"
  }
  if (result.output.kind === "receipts") {
    const rows = settlementRows(result.output.settlements).map((row) =>
      `${row.receiptRef ?? "receipt"} amount=${row.amountSats ?? "unknown"} realBitcoinMoved=${row.realBitcoinMoved ?? false}`,
    )
    return rows.length > 0 ? `${rows.join("\n")}\n` : "No public settlement receipt rows.\n"
  }
  if (result.output.kind === "replay") {
    const track = result.output.eventTrack
    const header = [
      `${track.title ?? "Generated public replay"} (${track.bundleRef ?? "bundle.unknown"})`,
      `authority=${track.sourceAuthority ?? "unknown"} privacy=${track.privacyLevel ?? "unknown"} claimScope=${track.claimScope ?? "unknown"}`,
    ]
    const rows = track.events.map((event) => {
      const when = event.timestamp ?? `+${event.timelineSecond}s`
      const refs = event.sourceRefs.length > 0 ? event.sourceRefs.join(",") : "none"
      const caveats = event.caveatRefs.length > 0 ? event.caveatRefs.join(",") : "none"
      const captions = event.captions.length > 0
        ? `\n  captions=${event.captions.join(" | ")}`
        : ""
      return [
        `${when} +${event.timelineSecond}s #${event.sequenceIndex} ${event.kind} ${event.eventRef}`,
        `  text=${event.displayText}`,
        `  refs=${refs}`,
        `  caveats=${caveats}`,
        `  actors=${event.actorRefs.length > 0 ? event.actorRefs.join(",") : "none"} targets=${event.targetRefs.length > 0 ? event.targetRefs.join(",") : "none"}${captions}`,
      ].join("\n")
    })
    const gaps = track.gaps.map((gap) =>
      `gap ${gap.gapRef}: ${gap.reason} refs=${gap.sourceRefs.length > 0 ? gap.sourceRefs.join(",") : "none"} affected=${gap.affectedRefs.length > 0 ? gap.affectedRefs.join(",") : "none"}`,
    )
    return [
      ...header,
      rows.length > 0 ? rows.join("\n") : "No public replay events.",
      ...(gaps.length > 0 ? ["Gaps:", ...gaps] : []),
    ].join("\n") + "\n"
  }
  const refs = result.output.refs
  return [
    `run evidence pack fetched from ${result.baseUrl}`,
    `receipts=${refs.receiptRefs.length}`,
    `verificationChallenges=${refs.verificationChallengeRefs.length}`,
    `blockers=${refs.blockerRefs.length}`,
    `promises=${refs.promiseIds.length}`,
  ].join("\n") + "\n"
}
