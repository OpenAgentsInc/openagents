import { Effect } from "effect"
import { DomServiceTag, EffuseLive, html } from "@openagentsinc/effuse"
import { whitePreset } from "@openagentsinc/hud"
import type { TemplateResult } from "@openagentsinc/effuse"

const deckBackgroundStyle = (): string => {
  const backgroundImage = [
    `radial-gradient(120% 85% at 50% 0%, rgba(255,255,255,0.07) 0%, rgba(255,255,255,0) 55%)`,
    `radial-gradient(ellipse 100% 100% at 50% 50%, transparent 12%, rgba(0,0,0,0.55) 60%, rgba(0,0,0,0.88) 100%)`,
    whitePreset.backgroundImage,
  ].join(", ")

  return `background-color: ${whitePreset.backgroundColor}; background-image: ${backgroundImage};`
}

export const deckPageShellTemplate = (): TemplateResult => {
  return html`
    <div
      data-deck-shell="1"
      class="fixed inset-0 overflow-hidden text-white"
    >
      <div class="absolute inset-0" style="${deckBackgroundStyle()}"></div>
      <div data-deck-slot="content" class="relative h-screen min-h-screen w-full overflow-hidden">
        <div data-hud-bg="dots-grid" class="absolute inset-0 z-0 pointer-events-none"></div>
        <div data-deck-slide class="absolute inset-0 z-10 h-full min-h-full w-full">
          <div class="p-6 text-xs text-white/75">Loading deck…</div>
        </div>
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
