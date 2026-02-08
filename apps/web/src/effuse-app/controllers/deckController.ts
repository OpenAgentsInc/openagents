import { Effect } from "effect"
import { DomServiceTag, EffuseLive, html } from "@openagentsinc/effuse"

import { parseDeckJsonString, renderDeck } from "../../effuse-deck/render"

import type { DeckDocument } from "../../effuse-deck/dsl"

export type DeckController = {
  readonly cleanup: () => void
}

const isLocalHost = (host: string): boolean =>
  host === "localhost" || host === "127.0.0.1" || host === "::1"

const resolveDeckSrc = (url: URL): string => {
  const src = url.searchParams.get("src")
  if (src && src.startsWith("/decks/") && src.endsWith(".json")) {
    return src
  }

  const file = url.searchParams.get("file")
  if (file && /^[a-zA-Z0-9._-]+\\.json$/.test(file)) {
    return `/decks/${file}`
  }

  return "/decks/deck.json"
}

const loadingTemplate = (deckSrc: string): ReturnType<typeof html> => html`
  <div class="h-full w-full flex flex-col items-center justify-center gap-3 p-10">
    <div class="text-sm text-text-dim">Deck</div>
    <div class="text-xs text-text-dim">Loading ${deckSrc}…</div>
  </div>
`

const errorTemplate = (deckSrc: string, message: string): ReturnType<typeof html> => html`
  <div class="h-full w-full flex flex-col items-center justify-center gap-3 p-10">
    <div class="text-sm text-text-dim">Deck</div>
    <div class="text-xs text-red-400 max-w-[720px] text-center">Error: ${message}</div>
    <div class="text-xs text-text-dim max-w-[720px] text-center">
      Place a deck JSON file at <span class="font-mono">${deckSrc}</span> via
      <span class="font-mono">apps/web/public${deckSrc}</span> (ignored by git), then refresh.
    </div>
  </div>
`

export const mountDeckController = (input: { readonly container: Element }): DeckController => {
  const root = input.container.querySelector("[data-deck-root]")
  if (!(root instanceof HTMLElement)) {
    return { cleanup: () => {} }
  }

  const url = new URL(window.location.href)
  if (!isLocalHost(url.hostname)) {
    // Should be blocked by route guard, but avoid doing anything on prod if mounted accidentally.
    return { cleanup: () => {} }
  }

  const deckSrc = resolveDeckSrc(url)
  let status: "loading" | "ready" | "error" = "loading"
  let errorText: string | null = null
  let doc: DeckDocument | null = null
  let slideIndex = 0
  let stepIndex = 1
  let totalSteps = 1

  let renderScheduled = false

  const renderNow = () => {
    if (status === "loading") {
      void renderTemplate(loadingTemplate(deckSrc))
      return
    }
    if (status === "error") {
      void renderTemplate(errorTemplate(deckSrc, errorText ?? "Unknown error."))
      return
    }
    if (!doc) {
      void renderTemplate(errorTemplate(deckSrc, "Deck doc missing."))
      return
    }

    const out = renderDeck({ doc, slideIndex, stepIndex })
    slideIndex = out.slideIndex
    stepIndex = out.stepIndex
    totalSteps = out.totalSteps
    void renderTemplate(out.template)
  }

  const scheduleRender = () => {
    if (renderScheduled) return
    renderScheduled = true
    queueMicrotask(() => {
      renderScheduled = false
      renderNow()
    })
  }

  const renderTemplate = async (template: ReturnType<typeof html>) => {
    await Effect.runPromise(
      Effect.gen(function* () {
        const dom = yield* DomServiceTag
        yield* dom.render(root, template)
      }).pipe(
        Effect.provide(EffuseLive),
        Effect.catchAll((err) => {
          console.error("[deck] render failed", err)
          return Effect.void
        }),
      ),
    )
  }

  const loadDeck = async () => {
    status = "loading"
    errorText = null
    scheduleRender()

    try {
      const res = await fetch(deckSrc, { cache: "no-store", credentials: "same-origin" })
      if (!res.ok) {
        status = "error"
        errorText = `Failed to fetch ${deckSrc} (${res.status})`
        scheduleRender()
        return
      }

      const text = await res.text()
      const parsed = parseDeckJsonString(text)
      if (parsed._tag === "Error") {
        status = "error"
        errorText = parsed.message
        scheduleRender()
        return
      }

      doc = parsed.doc
      slideIndex = 0
      stepIndex = 1
      // totalSteps computed in renderDeck
      status = "ready"
      scheduleRender()
    } catch (err) {
      status = "error"
      errorText = err instanceof Error ? err.message : String(err)
      scheduleRender()
    }
  }

  const next = () => {
    if (!doc) return
    if (stepIndex < totalSteps) {
      stepIndex += 1
      scheduleRender()
      return
    }
    if (slideIndex < doc.deck.slides.length - 1) {
      slideIndex += 1
      stepIndex = 1
      scheduleRender()
    }
  }

  const prev = () => {
    if (!doc) return
    if (stepIndex > 1) {
      stepIndex -= 1
      scheduleRender()
      return
    }
    if (slideIndex > 0) {
      slideIndex -= 1
      stepIndex = 1
      scheduleRender()
    }
  }

  const nextSlide = () => {
    if (!doc) return
    if (slideIndex < doc.deck.slides.length - 1) {
      slideIndex += 1
      stepIndex = 1
      scheduleRender()
    }
  }

  const prevSlide = () => {
    if (!doc) return
    if (slideIndex > 0) {
      slideIndex -= 1
      stepIndex = 1
      scheduleRender()
    }
  }

  const onKeyDown = (e: KeyboardEvent) => {
    // Avoid interfering with typing if the deck ever includes inputs.
    const target = e.target
    if (target instanceof HTMLInputElement || target instanceof HTMLTextAreaElement || (target as any)?.isContentEditable) {
      return
    }

    switch (e.key) {
      case "ArrowRight":
      case " ":
        e.preventDefault()
        next()
        break
      case "ArrowLeft":
      case "Backspace":
        e.preventDefault()
        prev()
        break
      case "PageDown":
        e.preventDefault()
        nextSlide()
        break
      case "PageUp":
        e.preventDefault()
        prevSlide()
        break
      case "r":
      case "R":
        e.preventDefault()
        void loadDeck()
        break
      default:
        break
    }
  }

  window.addEventListener("keydown", onKeyDown)

  // Initial load.
  void loadDeck()

  return {
    cleanup: () => {
      window.removeEventListener("keydown", onKeyDown)
    },
  }
}
