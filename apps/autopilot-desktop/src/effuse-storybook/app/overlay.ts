/**
 * Effuse Storybook Overlay UI
 */

import { Effect } from "effect"
import { html } from "../../effuse/template/html"
import type { TemplateResult } from "../../effuse/template/types"
import { StorybookService } from "./state"
import { makeEzRegistry } from "../../effuse/ez/registry"
import { mountEzRuntimeWith } from "../../effuse/ez/runtime"
import { getAllStories } from "../story-index"
import { CanvasHost } from "./canvas/host"

// Helper to get raw HTML string from template (simplistic render for v1)
const templateToString = (template: TemplateResult): string =>
  template.parts
    .map((part) => {
      switch (part._tag) {
        case "Text":
          return part.value
        case "Html":
          return part.value
        case "Template":
          return templateToString(part.value)
      }
    })
    .join("")

const renderTemplate = (tmpl: TemplateResult | string): string => {
  if (typeof tmpl === "string") return tmpl
  return templateToString(tmpl)
}

// --- Component ---

export const StorybookOverlay = {
  mount: (container: HTMLElement) =>
    Effect.gen(function* () {
      const service = yield* StorybookService

      // Render Shell
      container.innerHTML = "" // Clear container
      const shell = document.createElement("div")
      shell.id = "sb-overlay"
      shell.className =
        "fixed inset-0 z-[99999] hidden items-center justify-center bg-background/90"
      
      const layoutHtml = renderTemplate(html`
          <div class="grid h-[95vh] w-[95vw] grid-cols-[260px_1fr_300px] grid-rows-[44px_1fr] overflow-hidden border border-border bg-background text-foreground">
            <header class="col-span-3 flex items-center justify-between border-b border-border bg-background px-3">
              <div class="text-xs font-semibold uppercase text-accent">Effuse Storybook</div>
              <button class="text-xs text-muted-foreground hover:text-foreground" data-ez="sb.close" type="button">✕</button>
            </header>
            <nav class="border-r border-border bg-background p-2 text-xs" id="sb-sidebar-list">
              <!-- Story List Injected Here -->
            </nav>
            <main class="relative flex flex-col overflow-hidden bg-background">
              <div id="sb-canvas-root" class="h-full w-full overflow-auto bg-background p-3"></div>
            </main>
            <aside class="border-l border-border bg-background">
              <div class="p-2 text-[11px] text-muted-foreground">Panels (Controls/Actions) coming soon</div>
            </aside>
          </div>
        `)
      
      shell.innerHTML = layoutHtml
      container.appendChild(shell)

      // --- Actions ---

      const renderSidebar = Effect.gen(function* () {
        const state = yield* service.get
        const list = shell.querySelector("#sb-sidebar-list")
        if (!list) return

        const stories = getAllStories()
        // Simple grouping by title (first segment)
        const groups: Record<string, typeof stories> = {}
        for (const s of stories) {
          const group = s.title.split("/")[0]
          if (!groups[group]) groups[group] = []
          groups[group].push(s)
        }

        let htmlContent = ""
        for (const [group, items] of Object.entries(groups)) {
          htmlContent += `<div class="mt-6 mb-2 px-3 text-[10px] font-bold uppercase tracking-wider text-white/40">${group}</div>`
          for (const s of items) {
            const isActive = s.id === state.selectedStoryId
              ? "bg-blue-600 text-white"
              : "text-gray-400 hover:bg-gray-800 hover:text-white"
            htmlContent += `<div class="cursor-pointer rounded-md mx-2 px-3 py-2 text-xs transition-colors mb-0.5 ${isActive}" data-ez="sb.select" data-id="${s.id}">${s.title.split("/").slice(1).join("/")} / ${s.name}</div>`
          }
        }
        list.innerHTML = htmlContent
      })

      const updateVisibility = Effect.gen(function* () {
        const state = yield* service.get
        if (state.isOpen) {
          shell.classList.remove("hidden")
          shell.classList.add("flex")
        } else {
          shell.classList.add("hidden")
          shell.classList.remove("flex")
        }
      })

      const refresh = Effect.all([renderSidebar, updateVisibility]).pipe(
        Effect.asVoid
      )

      // --- Interaction Registry ---

      const registry = makeEzRegistry([
        [
          "sb.close",
          () =>
            service.toggle.pipe(
              Effect.zipRight(refresh)
            ),
        ],
        [
          "sb.select",
          (ctx) =>
            Effect.gen(function* () {
              const el = ctx.el as HTMLElement
              const id = el.dataset.id
              if (id) {
                yield* service.selectStory(id)
                yield* renderSidebar
                
                // Render Canvas
                const canvasRoot = shell.querySelector("#sb-canvas-root") as HTMLElement
                if (canvasRoot) {
                  yield* CanvasHost.render(canvasRoot).pipe(
                    Effect.provideService(StorybookService, service),
                    Effect.catchAll((error) =>
                      Effect.logError(error).pipe(Effect.asVoid)
                    )
                  )
                }
              }
            }).pipe(Effect.asVoid),
        ],
      ])

      yield* mountEzRuntimeWith(shell, registry)

      // Initial Render
      yield* renderSidebar
      yield* updateVisibility

      // Return a "refresh" effect if needed, or setup subscription
      // For v1 manual refresh on toggle is fine
      return {
        refresh,
      }
    }),
}
