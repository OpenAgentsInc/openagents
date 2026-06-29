import {
  buildPromiseSurfacingDraft,
  type ProductPromiseForumTopicSummary,
  type ProductPromiseLedgerDocument,
  type PromiseSurfacingInput,
} from "../shared/promise-surfacing.js"
import type {
  PromiseSurfacingReadinessResponse,
  PromiseSurfacingResponse,
} from "../shared/rpc.js"

export type PromiseSurfacingSettings = {
  readonly baseUrl: string
  readonly forumSlug: "product-promises"
  readonly agentToken: string | null
}

export type PromiseSurfacingEnv = Readonly<Record<string, string | undefined>>

export const resolvePromiseSurfacingSettings = (
  env: PromiseSurfacingEnv,
): PromiseSurfacingSettings => {
  const token =
    env.OPENAGENTS_PROMISE_SURFACING_AGENT_TOKEN ??
    env.OPENAGENTS_AGENT_TOKEN ??
    null
  return {
    baseUrl: env.OPENAGENTS_COM_BASE_URL ?? "https://openagents.com",
    forumSlug: "product-promises",
    agentToken: token?.trim() ? token.trim() : null,
  }
}

const endpoint = (baseUrl: string, path: string): string =>
  new URL(path, baseUrl.endsWith("/") ? baseUrl : `${baseUrl}/`).toString()

const productPromisesPath = "/api/public/product-promises"
const productPromiseTopicsPath = "/api/forum/forums/product-promises/topics"

export const promiseSurfacingReadiness = (
  settings: PromiseSurfacingSettings,
): PromiseSurfacingReadinessResponse => {
  const tokenPresent = settings.agentToken !== null
  return {
    ok: tokenPresent,
    fetchedAt: new Date().toISOString(),
    sourceUrl: "desktop:promise-surfacing-readiness",
    forumSlug: settings.forumSlug,
    baseUrl: settings.baseUrl,
    productPromisesUrl: endpoint(settings.baseUrl, productPromisesPath),
    forumTopicsUrl: endpoint(settings.baseUrl, productPromiseTopicsPath),
    agentTokenPresent: tokenPresent,
    blockerRefs: tokenPresent ? [] : ["env.OPENAGENTS_AGENT_TOKEN"],
  }
}

const readJson = async (response: Response): Promise<unknown> => {
  try {
    return await response.json()
  } catch {
    return null
  }
}

const fetchLedger = async (
  fetchImpl: typeof fetch,
  baseUrl: string,
): Promise<ProductPromiseLedgerDocument> => {
  const response = await fetchImpl(endpoint(baseUrl, productPromisesPath), {
    headers: { accept: "application/json" },
  })
  if (!response.ok) {
    throw new Error(`product promises fetch failed: ${response.status}`)
  }
  const json = await readJson(response)
  if (json === null || typeof json !== "object") {
    throw new Error("product promises response was not an object")
  }
  return json as ProductPromiseLedgerDocument
}

const topicSummary = (
  value: unknown,
  baseUrl: string,
): ProductPromiseForumTopicSummary | null => {
  if (value === null || typeof value !== "object") return null
  const record = value as {
    topicId?: unknown
    id?: unknown
    title?: unknown
    url?: unknown
  }
  const topicId =
    typeof record.topicId === "string"
      ? record.topicId
      : typeof record.id === "string"
        ? record.id
        : null
  const title = typeof record.title === "string" ? record.title : null
  if (topicId === null || title === null) return null
  const url =
    typeof record.url === "string"
      ? record.url
      : endpoint(baseUrl, `/forum/t/${encodeURIComponent(topicId)}`)
  return { topicId, title, url }
}

const fetchRelatedTopics = async (
  fetchImpl: typeof fetch,
  baseUrl: string,
  promiseId: string,
): Promise<readonly ProductPromiseForumTopicSummary[]> => {
  const response = await fetchImpl(endpoint(baseUrl, productPromiseTopicsPath), {
    headers: { accept: "application/json" },
  })
  if (!response.ok) return []
  const json = await readJson(response)
  if (json === null || typeof json !== "object") return []
  const topics = (json as { topics?: unknown }).topics
  if (!Array.isArray(topics)) return []
  return topics
    .map(topic => topicSummary(topic, baseUrl))
    .filter((topic): topic is ProductPromiseForumTopicSummary => topic !== null)
    .filter(topic => topic.title.includes(promiseId))
}

const postTopic = async (input: {
  readonly fetchImpl: typeof fetch
  readonly settings: PromiseSurfacingSettings
  readonly title: string
  readonly requestedSlug: string
  readonly bodyText: string
  readonly idempotencyKey: string
}): Promise<{ readonly topicId: string | null; readonly topicUrl: string | null }> => {
  if (input.settings.agentToken === null) {
    throw new Error("agent token unavailable")
  }
  const response = await input.fetchImpl(
    endpoint(input.settings.baseUrl, productPromiseTopicsPath),
    {
      method: "POST",
      headers: {
        accept: "application/json",
        authorization: `Bearer ${input.settings.agentToken}`,
        "content-type": "application/json",
        "idempotency-key": input.idempotencyKey,
      },
      body: JSON.stringify({
        title: input.title,
        requestedSlug: input.requestedSlug,
        bodyText: input.bodyText,
      }),
    },
  )
  const json = await readJson(response)
  if (!response.ok) {
    const message =
      json !== null &&
      typeof json === "object" &&
      typeof (json as { error?: unknown }).error === "string"
        ? (json as { error: string }).error
        : `forum topic create failed: ${response.status}`
    throw new Error(message)
  }
  const topic =
    json !== null && typeof json === "object"
      ? (json as { topic?: { topicId?: unknown; url?: unknown } }).topic
      : null
  const topicId =
    topic && typeof topic.topicId === "string" ? topic.topicId : null
  return {
    topicId,
    topicUrl:
      topic && typeof topic.url === "string"
        ? topic.url
        : topicId === null
          ? null
          : endpoint(input.settings.baseUrl, `/forum/t/${encodeURIComponent(topicId)}`),
  }
}

export const surfacePromiseGapReport = async (input: {
  readonly settings: PromiseSurfacingSettings
  readonly report: PromiseSurfacingInput
  readonly fetchImpl?: typeof fetch
}): Promise<PromiseSurfacingResponse> => {
  const fetchImpl = input.fetchImpl ?? fetch
  try {
    const [ledger, relatedTopics] = await Promise.all([
      fetchLedger(fetchImpl, input.settings.baseUrl),
      fetchRelatedTopics(
        fetchImpl,
        input.settings.baseUrl,
        input.report.promiseId.trim(),
      ),
    ])
    const draft = buildPromiseSurfacingDraft({
      report: input.report,
      ledger,
      relatedTopics,
      observedAt: new Date().toISOString(),
    })
    const readiness = promiseSurfacingReadiness(input.settings)
    if (!readiness.ok) {
      return {
        ok: false,
        mode: "drafted",
        draft,
        blockerRefs: readiness.blockerRefs,
      }
    }
    const posted = await postTopic({
      fetchImpl,
      settings: input.settings,
      title: draft.title,
      requestedSlug: draft.requestedSlug,
      bodyText: draft.bodyText,
      idempotencyKey: `promise-surface-${crypto.randomUUID()}`,
    })
    return {
      ok: true,
      mode: "posted",
      draft,
      topicId: posted.topicId,
      topicUrl: posted.topicUrl,
      blockerRefs: [],
    }
  } catch (error) {
    return {
      ok: false,
      mode: "blocked",
      draft: null,
      blockerRefs: ["blocker.autopilot.promise_surfacing.request_failed"],
      error: error instanceof Error ? error.message : String(error),
    }
  }
}
