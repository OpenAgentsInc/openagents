import { Effect } from "effect"
import { DomServiceTag, EffuseLive, html } from "@openagentsinc/effuse"
import { whitePreset } from "@openagentsinc/hud"

import { cleanupHudBackground, runHudDotsBackground } from "./hudBackground"
import { marketingHeaderTemplate } from "./header"

import type { TemplateResult } from "@openagentsinc/effuse"

const marketingBackgroundStyle = (): string => {
  const backgroundImage = [
    `radial-gradient(120% 85% at 50% 0%, rgba(255,255,255,0.07) 0%, rgba(255,255,255,0) 55%)`,
    `radial-gradient(ellipse 100% 100% at 50% 50%, transparent 12%, rgba(0,0,0,0.55) 60%, rgba(0,0,0,0.88) 100%)`,
    whitePreset.backgroundImage,
  ].join(", ")

  return `background-color: ${whitePreset.backgroundColor}; background-image: ${backgroundImage};`
}

export const marketingShellTemplate = (input: {
  readonly isHome: boolean
  readonly isLogin: boolean
  readonly content: TemplateResult
}): TemplateResult => {
  return html`
    <div class="fixed inset-0 overflow-hidden text-white" data-marketing-shell="1">
      <div class="absolute inset-0" style="${marketingBackgroundStyle()}">
        <div data-hud-bg="dots-grid" class="absolute inset-0 pointer-events-none"></div>
      </div>
      <div class="relative z-10 flex h-screen min-h-0 w-full flex-col">
        <div data-marketing-slot="header">
          ${marketingHeaderTemplate(input.isHome, input.isLogin)}
        </div>
        <div data-marketing-slot="content" class="flex flex-1 min-h-0 flex-col">
          ${input.content}
        </div>
      </div>
    </div>
  `
}

export const hydrateMarketingDotsGridBackground = (container: Element): Effect.Effect<void> => {
  return Effect.gen(function* () {
    const bg = container.querySelector('[data-hud-bg="dots-grid"]')
    if (!(bg instanceof Element)) return

    yield* runHudDotsBackground(bg, {
      distance: whitePreset.distance,
      dotsColor: whitePreset.dotsColor,
      dotsSettings: { type: "circle", size: 2 },
    })
  })
}

export const cleanupMarketingDotsGridBackground = (container: Element): void => {
  const bg = container.querySelector('[data-hud-bg="dots-grid"]')
  if (!(bg instanceof Element)) return
  cleanupHudBackground(bg)
}

/**
 * Render a marketing shell once, then update only the header/content slots on subsequent calls.
 *
 * This keeps the HUD background stable across state updates (avoid tearing down canvases).
 */
export const runMarketingShell = (
  container: Element,
  input: {
    readonly isHome: boolean
    readonly isLogin: boolean
    readonly content: TemplateResult
  },
): Effect.Effect<void> => {
  return Effect.gen(function* () {
    const dom = yield* DomServiceTag

    const shell = container.querySelector(`[data-marketing-shell]`)
    if (!shell) {
      yield* dom.render(container, marketingShellTemplate(input))
      return
    }

    const headerSlot = container.querySelector(`[data-marketing-slot="header"]`)
    if (headerSlot instanceof Element) {
      yield* dom.render(headerSlot, marketingHeaderTemplate(input.isHome, input.isLogin))
    }

    const contentSlot = container.querySelector(`[data-marketing-slot="content"]`)
    if (contentSlot instanceof Element) {
      yield* dom.render(contentSlot, input.content)
    }
  }).pipe(
    Effect.provide(EffuseLive),
    Effect.catchAll((err) => {
      console.error("[Effuse marketing shell]", err)
      return Effect.void
    }),
  )
}
