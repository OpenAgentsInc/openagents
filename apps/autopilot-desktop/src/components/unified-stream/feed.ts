import { Effect } from "effect"
import { html } from "../../effuse/template/html.js"
import type { DomService } from "../../effuse/services/dom.js"
import type { UnifiedEvent } from "./types.js"

export const appendEvent = (
  dom: DomService,
  container: Element,
  event: UnifiedEvent,
  index: number
) =>
  Effect.gen(function* () {
    const feed = container.querySelector("[data-role='event-feed']")
    if (!feed) {
      return
    }

    const content = html`
      <article class="border border-border bg-surface p-3">
        <div class="mb-2 text-[10px] uppercase text-muted-foreground">Event #${index}</div>
        <pre class="whitespace-pre-wrap break-words font-mono text-xs text-foreground">${JSON.stringify(
          event,
          null,
          2
        )}</pre>
      </article>
    `

    yield* dom.swap(feed, content, "beforeend").pipe(
      Effect.catchAll(() => Effect.void)
    )

    const empty = container.querySelector("[data-role='feed-empty']")
    if (empty) {
      empty.remove()
    }

    if (feed instanceof HTMLElement) {
      feed.scrollTop = feed.scrollHeight
    }
  })
