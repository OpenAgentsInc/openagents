import { Effect } from "effect"
import {
  KhalaToolRuntimeError,
  type KhalaWebSearchInput,
  type KhalaWebSearchItem,
  type KhalaWebSearchResult,
  type KhalaWebSearchService,
} from "@openagentsinc/khala-tools"

type DuckDuckGoTopic = {
  readonly FirstURL?: unknown
  readonly Name?: unknown
  readonly Text?: unknown
  readonly Topics?: unknown
}

type DuckDuckGoResponse = {
  readonly AbstractText?: unknown
  readonly AbstractURL?: unknown
  readonly Heading?: unknown
  readonly RelatedTopics?: unknown
  readonly Results?: unknown
}

export function createDuckDuckGoKhalaWebSearchService(fetchFn: typeof fetch = fetch): KhalaWebSearchService {
  return {
    marker: "khala.web_search_service",
    search: input =>
      Effect.tryPromise({
        try: () => searchDuckDuckGo(input, fetchFn),
        catch: error => new KhalaToolRuntimeError({
          code: "web_search_provider_failed",
          reason: error instanceof Error ? error.message : String(error),
        }),
      }),
  }
}

async function searchDuckDuckGo(
  input: KhalaWebSearchInput,
  fetchFn: typeof fetch,
): Promise<KhalaWebSearchResult> {
  const query = providerQuery(input)
  const url = new URL("https://api.duckduckgo.com/")
  url.searchParams.set("format", "json")
  url.searchParams.set("no_html", "1")
  url.searchParams.set("no_redirect", "1")
  url.searchParams.set("q", query)
  url.searchParams.set("skip_disambig", "1")

  const response = await fetchFn(url)
  if (!response.ok) throw new Error(`DuckDuckGo search failed with ${response.status}`)
  const body = await response.json() as DuckDuckGoResponse
  return {
    provider: "duckduckgo-instant-answer",
    results: extractDuckDuckGoItems(body).slice(0, input.limit),
    searchedAtMs: Date.now(),
  }
}

function providerQuery(input: KhalaWebSearchInput): string {
  const domains = input.domains.map(domain => `site:${domain}`).join(" ")
  const recency = input.recencyDays === undefined
    ? ""
    : ` after:${dateDaysAgo(input.recencyDays)}`
  return `${input.query}${domains.length === 0 ? "" : ` ${domains}`}${recency}`.trim()
}

function dateDaysAgo(days: number): string {
  const date = new Date(Date.now() - days * 24 * 60 * 60 * 1000)
  return date.toISOString().slice(0, 10)
}

function extractDuckDuckGoItems(body: DuckDuckGoResponse): ReadonlyArray<KhalaWebSearchItem> {
  const items: KhalaWebSearchItem[] = []
  const abstractUrl = asNonEmptyString(body.AbstractURL)
  const abstractText = asNonEmptyString(body.AbstractText)
  if (abstractUrl !== undefined && abstractText !== undefined) {
    items.push({
      snippet: abstractText,
      title: asNonEmptyString(body.Heading) ?? titleFromSnippet(abstractText),
      url: abstractUrl,
    })
  }
  collectTopics(body.Results, items)
  collectTopics(body.RelatedTopics, items)
  return dedupeByUrl(items)
}

function collectTopics(value: unknown, items: KhalaWebSearchItem[]): void {
  if (!Array.isArray(value)) return
  for (const entry of value) {
    if (!isRecord(entry)) continue
    const topic = entry as DuckDuckGoTopic
    collectTopics(topic.Topics, items)
    const url = asNonEmptyString(topic.FirstURL)
    const text = asNonEmptyString(topic.Text)
    if (url === undefined || text === undefined) continue
    items.push({
      snippet: text,
      title: asNonEmptyString(topic.Name) ?? titleFromSnippet(text),
      url,
    })
  }
}

function dedupeByUrl(items: ReadonlyArray<KhalaWebSearchItem>): ReadonlyArray<KhalaWebSearchItem> {
  const seen = new Set<string>()
  const deduped: KhalaWebSearchItem[] = []
  for (const item of items) {
    if (seen.has(item.url)) continue
    seen.add(item.url)
    deduped.push(item)
  }
  return deduped
}

function titleFromSnippet(snippet: string): string {
  const beforeDash = snippet.split(" - ")[0]?.trim()
  const title = beforeDash && beforeDash.length > 0 ? beforeDash : snippet.trim()
  return title.length <= 90 ? title : `${title.slice(0, 87)}...`
}

function asNonEmptyString(value: unknown): string | undefined {
  return typeof value === "string" && value.trim().length > 0 ? value.trim() : undefined
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value)
}
