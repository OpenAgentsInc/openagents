import { escapeHtml, html } from "@openagentsinc/effuse"

import type { Story, StoryMeta } from "../storybook/types"

const groupByKind = (stories: ReadonlyArray<StoryMeta>): Record<string, Array<StoryMeta>> => {
  const out: Record<string, Array<StoryMeta>> = {}
  for (const s of stories) {
    const key = s.kind
    ;(out[key] ??= []).push(s)
  }
  return out
}

const kindLabel = (kind: string): string => {
  switch (kind) {
    case "atom":
      return "Atoms"
    case "molecule":
      return "Molecules"
    case "organism":
      return "Organisms"
    default:
      return kind
  }
}

export const storybookManagerTemplate = (input: {
  readonly stories: ReadonlyArray<StoryMeta>
  readonly defaultStoryId: string | null
}): ReturnType<typeof html> => {
  const grouped = groupByKind(input.stories)
  const kinds = Object.keys(grouped).sort((a, b) => a.localeCompare(b))
  const defaultSrc = input.defaultStoryId ? `/__storybook/canvas/${encodeURIComponent(input.defaultStoryId)}` : "about:blank"

  return html`
    <div class="min-h-screen bg-bg-primary text-text-primary font-mono" data-oa-storybook="1">
      <header class="h-12 px-4 flex items-center justify-between border-b border-border-dark bg-bg-secondary">
        <div class="flex items-center gap-2">
          <span class="text-xs uppercase tracking-wider text-text-dim">Effuse Storybook</span>
          <span class="text-[10px] text-text-muted">Autopilot components</span>
        </div>
        <div class="text-[10px] text-text-muted">Click a story to load it in the canvas.</div>
      </header>

      <main class="h-[calc(100vh-3rem)] min-h-0 flex">
        <aside class="w-80 min-w-0 border-r border-border-dark bg-bg-secondary overflow-y-auto overseer-scroll">
          <div class="p-3">
            ${kinds.map((kind) => {
              const list = grouped[kind] ?? []
              return html`
                <section class="mt-3 first:mt-0">
                  <div class="text-[10px] uppercase tracking-wider text-text-dim">${kindLabel(kind)}</div>
                  <div class="mt-2 flex flex-col gap-1">
                    ${list.map((s) => {
                      const href = `/__storybook/canvas/${encodeURIComponent(s.id)}`
                      return html`
                        <a
                          href="${href}"
                          target="oa-storybook-canvas"
                          class="rounded px-2 py-1 text-xs text-text-muted hover:text-text-primary hover:bg-surface-primary focus:outline-none focus-visible:ring-2 focus-visible:ring-border-focus"
                          title="${escapeHtml(s.title)}"
                        >
                          ${s.title.split("/").slice(-1)[0]}
                        </a>
                      `
                    })}
                  </div>
                </section>
              `
            })}
          </div>
        </aside>

        <section class="flex-1 min-w-0 bg-bg-primary">
          <iframe
            title="Story canvas"
            name="oa-storybook-canvas"
            src="${defaultSrc}"
            class="h-full w-full bg-bg-primary"
            referrerpolicy="no-referrer"
          ></iframe>
        </section>
      </main>
    </div>
  `
}

export const storybookCanvasTemplate = (story: Story): ReturnType<typeof html> => {
  return html`
    <div class="min-h-screen bg-bg-primary text-text-primary font-mono" data-oa-storybook-canvas="1">
      <header class="h-10 px-4 flex items-center justify-between border-b border-border-dark bg-bg-secondary">
        <div class="text-[11px] text-text-primary">${story.title}</div>
        <div class="text-[10px] text-text-muted">${story.id}</div>
      </header>

      <main class="p-4" data-story-ready="1" data-story-id="${story.id}">
        ${story.render()}
      </main>
    </div>
  `
}

