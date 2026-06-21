// Bun-host Verse/Tassadar turn (#5821).
//
// The default Verse chat bar must talk to Tassadar/OpenAgents, not spawn a
// coding session. This module stays host-side so tokens and network details do
// not cross into the webview. It builds a small public-safe context pack from
// public OpenAgents projections, calls the OpenAI-compatible model gateway, and
// returns only plain response text plus public refs/blocker refs.

import {
  inferenceGatewayChatCompletionsUrl,
  resolveInferenceGatewaySettings,
} from "../shared/inference-gateway.js"
import type { PylonStatsSnapshot } from "../shared/pylon-network-scene.js"
import type {
  VerseTurnContextSummary,
  VerseTurnResponse,
} from "../shared/rpc.js"
import { fetchPublicPylonStats } from "./pylon-network-stats.js"
import { fetchPublicActivityTimeline } from "./public-activity-timeline.js"
import { fetchTrainingPromiseGates } from "./training-runs.js"

const DEFAULT_VERSE_MODEL = "gemini-3.5-flash"
const VERSE_ACTIVITY_LIMIT = 8

export const VERSE_SYSTEM_PROMPT =
  "You are Tassadar, the OpenAgents Verse guide inside Autopilot Desktop. " +
  "Answer the user's message using the public OpenAgents context provided. " +
  "Stay concise, say when a public projection is missing, and do not frame the " +
  "default Verse chat as Codex, Claude Code, cloud-code, repo, worktree, or " +
  "session spawning unless the user explicitly asks for advanced coding."

const NO_TOKEN_MESSAGE =
  "I can't reach Tassadar yet: this desktop does not have an OpenAgents account token configured. " +
  "Set OPENAGENTS_AGENT_TOKEN or finish Pylon onboarding, then send again."

const BRIDGE_GATEWAY_MISSING_MESSAGE =
  "Tassadar is unavailable right now because the OpenAgents model gateway did not return a usable answer. Please try again."

type VerseTurnEnv = Readonly<Record<string, string | undefined>>

export type BuildVerseTurnInput = Readonly<{
  prompt: string
  env: VerseTurnEnv
  agentToken: string | null
  fetchFn?: typeof fetch
  nowIso?: () => string
}>

type PublicTassadarRunSummaryResult = Readonly<{
  ok: boolean
  fetchedAt: string
  sourceUrl: string
  summary: unknown | null
  error: string | null
}>

const normalizeBaseUrl = (baseUrl: string): string => baseUrl.replace(/\/+$/, "")

const envString = (env: VerseTurnEnv, key: string): string | null => {
  const value = env[key]?.trim()
  return value && value.length > 0 ? value : null
}

const isRecord = (value: unknown): value is Record<string, unknown> =>
  typeof value === "object" && value !== null && !Array.isArray(value)

const stringOrNull = (value: unknown): string | null =>
  typeof value === "string" && value.trim() !== "" ? value : null

const numberOrNull = (value: unknown): number | null =>
  typeof value === "number" && Number.isFinite(value) ? value : null

const boundedText = (value: unknown, fallback = ""): string => {
  const text = typeof value === "string" ? value.trim() : fallback
  return text.length > 220 ? `${text.slice(0, 217)}...` : text
}

const unique = (values: ReadonlyArray<string | null | undefined>): readonly string[] =>
  [...new Set(values.map(value => value?.trim() ?? "").filter(Boolean))].sort()

export const publicTassadarRunSummaryUrl = (baseUrl: string): string =>
  `${normalizeBaseUrl(baseUrl)}/api/public/tassadar-run-summary`

export async function fetchPublicTassadarRunSummary(input: {
  readonly baseUrl: string
  readonly fetchFn?: typeof fetch
  readonly nowIso?: () => string
}): Promise<PublicTassadarRunSummaryResult> {
  const fetchFn = input.fetchFn ?? fetch
  const fetchedAt = input.nowIso?.() ?? new Date().toISOString()
  const sourceUrl = publicTassadarRunSummaryUrl(input.baseUrl)
  try {
    const response = await fetchFn(sourceUrl, {
      headers: { accept: "application/json" },
    })
    if (!response.ok) {
      return {
        ok: false,
        fetchedAt,
        sourceUrl,
        summary: null,
        error: `public Tassadar summary ${response.status}`,
      }
    }
    return {
      ok: true,
      fetchedAt,
      sourceUrl,
      summary: await response.json(),
      error: null,
    }
  } catch (error) {
    return {
      ok: false,
      fetchedAt,
      sourceUrl,
      summary: null,
      error: error instanceof Error ? error.message : String(error),
    }
  }
}

const metricValue = (metrics: Record<string, unknown>, key: string): number | null => {
  const metric = metrics[key]
  if (!isRecord(metric)) return null
  return numberOrNull(metric.value)
}

const summarizeTassadar = (summary: unknown): VerseTurnContextSummary["training"] => {
  const record = isRecord(summary) ? summary : {}
  const metrics = isRecord(record.metrics) ? record.metrics : {}
  const corpus = isRecord(record.corpus) ? record.corpus : {}
  return {
    runRef: stringOrNull(record.runRef),
    runState: stringOrNull(record.runState),
    acceptedTraceCount: numberOrNull(corpus.acceptedTraceCount),
    qualifiedContributorCount: metricValue(metrics, "qualifiedContributorCount"),
    settledPayoutSats: metricValue(metrics, "providerConfirmedSettledPayoutSats"),
  }
}

const nipSettlementSats = (
  snapshot: PylonStatsSnapshot | null,
  key: "satsSettled24h" | "satsSettledTotal",
): number | null => {
  const stats = snapshot?.nip90MarketSettlementStats
  if (!stats) return null
  const values = [stats.compute, stats.data, stats.labor].map(stream =>
    numberOrNull(stream?.[key]),
  )
  const known = values.filter((value): value is number => value !== null)
  return known.length === 0 ? null : known.reduce((sum, value) => sum + value, 0)
}

const summarizePylon = (
  snapshot: PylonStatsSnapshot | null,
): VerseTurnContextSummary["pylon"] => ({
  onlineNow: numberOrNull(snapshot?.pylonsOnlineNow),
  assignmentReadyNow: numberOrNull(snapshot?.pylonsAssignmentReadyNow),
  walletReadyNow: numberOrNull(snapshot?.pylonsWalletReadyNow),
  satsSettled24h: nipSettlementSats(snapshot, "satsSettled24h"),
  satsSettledTotal: nipSettlementSats(snapshot, "satsSettledTotal"),
})

const summarizeActivity = (
  envelope: Awaited<ReturnType<typeof fetchPublicActivityTimeline>>["envelope"],
): VerseTurnContextSummary["activity"] => {
  if (envelope === null) return { eventCount: null, recent: [] }
  return {
    eventCount: envelope.events.length,
    recent: envelope.events.slice(0, 5).map(event => ({
      kind: event.kind,
      text: boundedText(event.text),
      refs: unique([...event.refs, ...event.sourceRefs]).slice(0, 5),
    })),
  }
}

const emptyContext = (
  fetchedAt: string,
  blockerRefs: ReadonlyArray<string>,
): VerseTurnContextSummary => ({
  fetchedAt,
  sourceRefs: [],
  blockerRefs,
  pylon: {
    onlineNow: null,
    assignmentReadyNow: null,
    walletReadyNow: null,
    satsSettled24h: null,
    satsSettledTotal: null,
  },
  training: {
    runRef: null,
    runState: null,
    acceptedTraceCount: null,
    qualifiedContributorCount: null,
    settledPayoutSats: null,
  },
  promises: {
    registryVersion: null,
    green: null,
    yellow: null,
    red: null,
    trackedTrainingPromises: null,
  },
  activity: { eventCount: null, recent: [] },
})

const compactNumber = (value: number | null): string =>
  value === null ? "unknown" : new Intl.NumberFormat("en-US").format(value)

const formatRecentActivity = (
  recent: VerseTurnContextSummary["activity"]["recent"],
): string =>
  recent.length === 0
    ? "none available"
    : recent.map(event => `${event.kind}: ${event.text}`).join(" | ")

export const verseContextPrompt = (
  prompt: string,
  context: VerseTurnContextSummary,
): string =>
  [
    "PUBLIC OPENAGENTS CONTEXT PACK",
    `fetchedAt: ${context.fetchedAt}`,
    `sourceRefs: ${context.sourceRefs.join(", ") || "none"}`,
    `blockerRefs: ${context.blockerRefs.join(", ") || "none"}`,
    "",
    "Pylon network:",
    `online=${compactNumber(context.pylon.onlineNow)}, assignmentReady=${compactNumber(context.pylon.assignmentReadyNow)}, walletReady=${compactNumber(context.pylon.walletReadyNow)}, sats24h=${compactNumber(context.pylon.satsSettled24h)}, satsTotal=${compactNumber(context.pylon.satsSettledTotal)}`,
    "",
    "Tassadar training:",
    `runRef=${context.training.runRef ?? "unknown"}, state=${context.training.runState ?? "unknown"}, acceptedTraceCount=${compactNumber(context.training.acceptedTraceCount)}, qualifiedContributors=${compactNumber(context.training.qualifiedContributorCount)}, settledPayoutSats=${compactNumber(context.training.settledPayoutSats)}`,
    "",
    "Training/product promises:",
    `registryVersion=${context.promises.registryVersion ?? "unknown"}, green=${compactNumber(context.promises.green)}, yellow=${compactNumber(context.promises.yellow)}, red=${compactNumber(context.promises.red)}, trackedTrainingPromises=${compactNumber(context.promises.trackedTrainingPromises)}`,
    "",
    "Recent public activity:",
    formatRecentActivity(context.activity.recent),
    "",
    "USER MESSAGE",
    prompt,
  ].join("\n")

const parseAssistantText = (body: unknown): string | null => {
  if (!isRecord(body)) return null
  const choices = body.choices
  if (!Array.isArray(choices) || choices.length === 0) return null
  const first = choices[0]
  if (!isRecord(first) || !isRecord(first.message)) return null
  const content = first.message.content
  return typeof content === "string" && content.trim().length > 0
    ? content
    : null
}

const parseErrorMessage = (body: unknown, status: number): string => {
  if (isRecord(body)) {
    const message = stringOrNull(body.message)
    if (message !== null) return message
    const error = stringOrNull(body.error)
    if (error !== null) return error
  }
  return `request failed (${status})`
}

const buildContext = async (
  input: BuildVerseTurnInput,
  baseUrl: string,
): Promise<VerseTurnContextSummary> => {
  const nowIso = input.nowIso ?? (() => new Date().toISOString())
  const fetchOptions =
    input.fetchFn === undefined ? {} : { fetchFn: input.fetchFn }
  const [tassadar, pylon, activity, promises] = await Promise.all([
    fetchPublicTassadarRunSummary({
      baseUrl,
      nowIso,
      ...fetchOptions,
    }),
    fetchPublicPylonStats({ baseUrl, nowIso, ...fetchOptions }),
    fetchPublicActivityTimeline({
      baseUrl,
      limit: VERSE_ACTIVITY_LIMIT,
      nowIso,
      ...fetchOptions,
    }),
    fetchTrainingPromiseGates({ baseUrl, nowIso, ...fetchOptions }),
  ])

  const blockerRefs = unique([
    tassadar.ok ? null : "verse.context.tassadar_run_summary_unavailable",
    pylon.ok ? null : "verse.context.pylon_stats_unavailable",
    activity.ok ? null : "verse.context.activity_timeline_unavailable",
    promises.ok ? null : "verse.context.product_promises_unavailable",
  ])

  return {
    fetchedAt: nowIso(),
    sourceRefs: unique([
      tassadar.sourceUrl,
      pylon.sourceUrl,
      activity.sourceUrl,
      promises.sourceUrl,
    ]),
    blockerRefs,
    pylon: summarizePylon(pylon.snapshot),
    training: summarizeTassadar(tassadar.summary),
    promises: {
      registryVersion: promises.ok ? promises.registryVersion : null,
      green: promises.ok ? promises.stateCounts.green : null,
      yellow: promises.ok ? promises.stateCounts.yellow : null,
      red: promises.ok ? promises.stateCounts.red : null,
      trackedTrainingPromises: promises.ok ? promises.promises.length : null,
    },
    activity: summarizeActivity(activity.envelope),
  }
}

export const buildVerseTurn = async (
  input: BuildVerseTurnInput,
): Promise<VerseTurnResponse> => {
  const prompt = input.prompt.trim()
  const fetchedAt = input.nowIso?.() ?? new Date().toISOString()
  if (prompt === "") {
    return {
      ok: false,
      text: "Type a message for Tassadar and send it again.",
      context: emptyContext(fetchedAt, ["verse.prompt.empty"]),
    }
  }

  const token =
    input.agentToken !== null && input.agentToken.trim().length > 0
      ? input.agentToken.trim()
      : null
  if (token === null) {
    return {
      ok: false,
      text: NO_TOKEN_MESSAGE,
      context: emptyContext(fetchedAt, ["verse.auth.token_missing"]),
    }
  }

  const fetchFn = input.fetchFn ?? fetch
  const settings = resolveInferenceGatewaySettings(input.env)
  const baseUrl =
    envString(input.env, "OPENAGENTS_VERSE_CONTEXT_BASE_URL") ?? settings.baseUrl
  const model =
    envString(input.env, "OPENAGENTS_VERSE_MODEL") ??
    envString(input.env, "OPENAGENTS_SHELL_MODEL") ??
    envString(input.env, "OPENAGENTS_INFERENCE_GATEWAY_MODEL") ??
    DEFAULT_VERSE_MODEL
  const context = await buildContext(input, baseUrl)

  try {
    const response = await fetchFn(inferenceGatewayChatCompletionsUrl(settings.baseUrl), {
      method: "POST",
      headers: {
        accept: "application/json",
        authorization: `Bearer ${token}`,
        "content-type": "application/json",
      },
      body: JSON.stringify({
        model,
        stream: false,
        messages: [
          { role: "system", content: VERSE_SYSTEM_PROMPT },
          { role: "user", content: verseContextPrompt(prompt, context) },
        ],
      }),
    })

    let body: unknown = null
    try {
      body = await response.json()
    } catch {
      body = null
    }

    if (!response.ok) {
      if (response.status === 401 || response.status === 403) {
        return {
          ok: false,
          text: "Tassadar could not authenticate with openagents.com. Refresh the OpenAgents account token and try again.",
          context: {
            ...context,
            blockerRefs: unique([...context.blockerRefs, "verse.auth.gateway_rejected_token"]),
          },
        }
      }
      if (response.status === 402) {
        return {
          ok: false,
          text: "Tassadar is out of model allowance for this account. Add OpenAgents credit, then send again.",
          context: {
            ...context,
            blockerRefs: unique([...context.blockerRefs, "verse.billing.allowance_exhausted"]),
          },
        }
      }
      return {
        ok: false,
        text: `Tassadar could not answer right now: ${parseErrorMessage(body, response.status)}.`,
        context: {
          ...context,
          blockerRefs: unique([...context.blockerRefs, "verse.gateway.request_failed"]),
        },
      }
    }

    const text = parseAssistantText(body)
    if (text === null) {
      return {
        ok: false,
        text: BRIDGE_GATEWAY_MISSING_MESSAGE,
        context: {
          ...context,
          blockerRefs: unique([...context.blockerRefs, "verse.gateway.empty_response"]),
        },
      }
    }

    return { ok: true, text, context }
  } catch (error) {
    const reason = error instanceof Error ? error.message : "network error"
    return {
      ok: false,
      text: `Tassadar could not reach the model (${reason}). Check the connection and try again.`,
      context: {
        ...context,
        blockerRefs: unique([...context.blockerRefs, "verse.gateway.network_failed"]),
      },
    }
  }
}
