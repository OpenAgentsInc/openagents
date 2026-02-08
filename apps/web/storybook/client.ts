import "../src/app.css"

import { Effect } from "effect"
import { DomServiceTag, EffuseLive, html, makeEzRegistry, mountEzRuntimeWith } from "@openagentsinc/effuse"

import { initFontReady } from "../src/effuse-app/fontReady"
import { UiBlobStore } from "../src/effuse-app/blobStore"
import { getStoryById, listStoryMeta } from "../src/storybook"
import { storybookCanvasTemplate, storybookManagerTemplate } from "../src/effuse-pages/storybook"

const normalizePath = (pathname: string): string => pathname.replace(/\/+$/, "") || "/"

const renderNotFound = (pathname: string) => html`
  <div class="min-h-screen bg-bg-primary text-text-primary font-mono p-6">
    <div class="text-sm">Not found</div>
    <div class="mt-2 text-xs text-text-muted">${pathname}</div>
  </div>
`

const program = Effect.gen(function* () {
  initFontReady()

  const dom = yield* DomServiceTag
  const root = document.getElementById("root")
  if (!(root instanceof Element)) {
    throw new Error("storybook: missing #root")
  }

  // Enable the default tool-part "View full" affordance in stories.
  const ez = makeEzRegistry()
  ez.set("effuse.blob.view", ({ params }) =>
    Effect.sync(() => {
      const blobId = params.blobId ?? params.id ?? ""
      if (!blobId) return html`[missing blobId]`
      const text = UiBlobStore.getText(blobId)
      if (text == null) return html`[blob not found: ${blobId}]`
      return html`${text}`
    }),
  )
  yield* mountEzRuntimeWith(root, ez)

  const url = new URL(location.href)
  const pathname = normalizePath(url.pathname)

  // Support both:
  // - Vite dev server root `/`
  // - canonical routes used in the Worker host (`/__storybook/*`)
  if (pathname === "/" || pathname === "/__storybook") {
    const stories = listStoryMeta()
    const defaultStoryId = stories.length > 0 ? stories[0]!.id : null
    yield* dom.render(root, storybookManagerTemplate({ stories, defaultStoryId }))
    return
  }

  const prefix = "/__storybook/canvas/"
  if (pathname.startsWith(prefix)) {
    const raw = pathname.slice(prefix.length)
    const storyId = raw ? decodeURIComponent(raw) : ""
    const story = storyId ? getStoryById(storyId) : null
    if (!story) {
      yield* dom.render(root, renderNotFound(pathname))
      return
    }
    yield* dom.render(root, storybookCanvasTemplate(story))
    return
  }

  yield* dom.render(root, renderNotFound(pathname))
}).pipe(Effect.provide(EffuseLive))

Effect.runPromise(program).catch((err) => {
  console.error("[storybook] boot failed", err)
})

