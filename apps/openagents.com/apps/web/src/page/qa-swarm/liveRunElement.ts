import {
  type QaSwarmRunProjection,
  assertResolverBackedQaSwarmProjection,
} from '@openagentsinc/qa-swarm-contract'
import { define as defineCustomElement } from 'foldkit/customElement'
import type { Attribute, Html } from 'foldkit/html'

export const qaSwarmLiveRunTagName = 'oa-qa-swarm-live-run'
const RUN_REF_ATTRIBUTE = 'data-run-ref'
const MAX_ACTIVE_POLLS = 30
const ACTIVE_POLL_MS = 2_000

export const qaSwarmProjectionNeedsRefresh = (
  projection: QaSwarmRunProjection,
): boolean =>
  projection.execution?.status === 'scheduled' ||
  projection.execution?.status === 'running'

export const fetchPublishedQaSwarmProjection = async (
  runRef: string,
  fetchFn: typeof fetch = globalThis.fetch,
  signal?: AbortSignal,
): Promise<QaSwarmRunProjection | null> => {
  const response = await fetchFn(
    `/api/public/qa-swarm/runs/${encodeURIComponent(runRef)}`,
    {
      cache: 'no-store',
      headers: { accept: 'application/json' },
      ...(signal === undefined ? {} : { signal }),
    },
  )
  if (response.status === 404) return null
  if (!response.ok) throw new Error(`QA Swarm projection read failed: ${response.status}`)
  const body = (await response.json()) as { projection?: unknown }
  return assertResolverBackedQaSwarmProjection(body.projection)
}

const appendText = (
  parent: HTMLElement,
  tag: string,
  text: string,
  className?: string,
): HTMLElement => {
  const child = document.createElement(tag)
  child.textContent = text
  if (className !== undefined) child.className = className
  parent.append(child)
  return child
}

export const renderPublishedQaSwarmProjection = (
  root: HTMLElement,
  projection: QaSwarmRunProjection,
): void => {
  const article = document.createElement('article')
  article.dataset.component = 'qa-swarm-published-run'
  article.dataset.runRef = projection.runRef
  article.className = 'grid gap-5 border border-[var(--oa-color-khala-border)] bg-[var(--oa-color-khala-surface-raised)] p-4 sm:p-5'

  appendText(article, 'p', 'Published QA Swarm run', 'm-0 text-[0.68rem] font-semibold uppercase tracking-[0.14em] text-[var(--oa-color-khala-text-faint)]')
  appendText(article, 'h1', projection.title, 'm-0 text-3xl font-semibold text-[var(--oa-color-khala-text-bright)]')
  appendText(article, 'code', projection.runRef, 'break-all text-xs text-[var(--oa-color-khala-text-dim)]')

  const state = document.createElement('div')
  state.className = 'flex flex-wrap gap-3 text-sm text-[var(--oa-color-khala-text-muted)]'
  appendText(state, 'span', `Verdict: ${projection.verdict}`)
  appendText(state, 'span', `Execution: ${projection.execution?.status ?? 'not reported'}`)
  appendText(state, 'span', `Generated: ${projection.generatedAt}`)
  article.append(state)

  if (projection.execution !== undefined) {
    const tiers = document.createElement('section')
    tiers.dataset.qaSwarmTiers = 'true'
    appendText(tiers, 'h2', 'Execution tiers', 'text-sm font-semibold uppercase tracking-wide')
    const list = document.createElement('ul')
    list.className = 'grid gap-2 p-0'
    for (const tier of projection.execution.tiers) {
      const item = appendText(list, 'li', `${tier.backend}: ${tier.status}`)
      item.dataset.tierBackend = tier.backend
      item.dataset.tierStatus = tier.status
      if (tier.reason !== undefined) appendText(item, 'small', ` — ${tier.reason}`)
    }
    tiers.append(list)
    article.append(tiers)
  }

  const board = document.createElement('section')
  board.dataset.qaSwarmBoard = projection.boardGraph.schemaVersion
  appendText(board, 'h2', 'Arbiter swarm board', 'text-sm font-semibold uppercase tracking-wide')
  const links = document.createElement('ul')
  links.className = 'grid gap-2 p-0'
  for (const link of projection.boardGraph.links) {
    const item = appendText(links, 'li', `${link.label}: ${link.status}`)
    item.dataset.linkId = link.id
    item.dataset.status = link.status
  }
  board.append(links)
  article.append(board)

  if (projection.blockerRefs.length > 0) {
    appendText(article, 'p', `Blockers: ${projection.blockerRefs.join(', ')}`, 'text-xs text-[var(--oa-color-khala-text-dim)]')
  }
  root.replaceChildren(article)
}

const liveRunElement = defineCustomElement({
  events: {},
  properties: {},
  tag: qaSwarmLiveRunTagName,
})

const makeLiveRunElement = (): CustomElementConstructor =>
  class QaSwarmLiveRunElement extends HTMLElement {
    #abort: AbortController | null = null
    #timer: ReturnType<typeof setTimeout> | null = null
    #polls = 0

    connectedCallback(): void {
      void this.#refresh()
    }

    disconnectedCallback(): void {
      this.#abort?.abort()
      if (this.#timer !== null) clearTimeout(this.#timer)
    }

    async #refresh(): Promise<void> {
      const runRef = this.getAttribute(RUN_REF_ATTRIBUTE)
      if (runRef === null || this.#polls >= MAX_ACTIVE_POLLS) return
      this.#abort?.abort()
      this.#abort = new AbortController()
      try {
        const projection = await fetchPublishedQaSwarmProjection(
          runRef,
          globalThis.fetch,
          this.#abort.signal,
        )
        if (!this.isConnected || projection === null || projection.runRef !== runRef) return
        renderPublishedQaSwarmProjection(this, projection)
        this.#polls += 1
        if (qaSwarmProjectionNeedsRefresh(projection)) {
          this.#timer = setTimeout(() => void this.#refresh(), ACTIVE_POLL_MS)
        }
      } catch {
        // Public read failures disclose nothing and retain the unavailable shell.
      }
    }
  }

const register = (): void => {
  if (typeof customElements === 'undefined' || typeof HTMLElement === 'undefined') return
  if (customElements.get(qaSwarmLiveRunTagName) !== undefined) return
  customElements.define(qaSwarmLiveRunTagName, makeLiveRunElement())
}

export const qaSwarmLiveRunView = <Message>(
  attributes: ReadonlyArray<Attribute<Message>> = [],
  children: ReadonlyArray<Html> = [],
): Html => {
  register()
  const element = liveRunElement.withMessage<Message>()
  return element([...attributes], children)
}
