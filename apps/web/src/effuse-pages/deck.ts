import { Effect } from "effect"
import { DomServiceTag, EffuseLive, html } from "@openagentsinc/effuse"
import type { TemplateResult } from "@openagentsinc/effuse"

export const deckPageShellTemplate = (): TemplateResult => {
  return html`
    <div
      data-deck-root="1"
      class="fixed inset-0 overflow-hidden bg-bg-primary text-text-primary font-mono"
    >
      <div class="p-6 text-xs text-text-dim">
        Loading deck…
      </div>
    </div>
  `
}

export const runDeckShell = (container: Element): Effect.Effect<void> =>
  Effect.gen(function* () {
    const dom = yield* DomServiceTag
    yield* dom.render(container, deckPageShellTemplate())
  }).pipe(
    Effect.provide(EffuseLive),
    Effect.catchAll((err) => {
      console.error("[Effuse deck shell]", err)
      return Effect.void
    }),
  )

