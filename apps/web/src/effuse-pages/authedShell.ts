import { Effect } from "effect";
import { DomServiceTag, EffuseLive, html } from "@openagentsinc/effuse";
import { whitePreset } from "@openagentsinc/hud";
import { cleanupHudBackground, runHudDotsGridBackground } from "./hudBackground";

import type { TemplateResult } from "@openagentsinc/effuse";

const authedBackgroundStyle = (): string => {
  const backgroundImage = [
    `radial-gradient(120% 85% at 50% 0%, rgba(255,255,255,0.08) 0%, rgba(255,255,255,0) 55%)`,
    `radial-gradient(ellipse 100% 100% at 50% 50%, transparent 12%, rgba(0,0,0,0.55) 60%, rgba(0,0,0,0.88) 100%)`,
    whitePreset.backgroundImage,
  ].join(", ");

  return `background-color: ${whitePreset.backgroundColor}; background-image: ${backgroundImage};`;
};

export const authedShellTemplate = (content: TemplateResult): TemplateResult => {
  return html`
    <div class="fixed inset-0 overflow-hidden text-text-primary font-mono" data-authed-shell="1">
      <div class="absolute inset-0" style="${authedBackgroundStyle()}">
        <div data-hud-bg="dots-grid" class="absolute inset-0 pointer-events-none"></div>
      </div>
      <div data-authed-content class="relative z-10 flex h-screen min-h-0 w-full flex-col overflow-hidden">
        ${content}
      </div>
    </div>
  `;
};

export const hydrateAuthedDotsGridBackground = (container: Element): Effect.Effect<void> => {
  return Effect.gen(function* () {
    const bg = container.querySelector('[data-hud-bg="dots-grid"]');
    if (!(bg instanceof Element)) return;

    yield* runHudDotsGridBackground(bg, {
      distance: whitePreset.distance,
      dotsColor: "hsla(0, 0%, 100%, 0.035)",
      lineColor: "hsla(0, 0%, 100%, 0.03)",
      dotsSettings: { type: "circle", size: 2 },
    });
  });
};

export const cleanupAuthedDotsGridBackground = (container: Element): void => {
  const bg = container.querySelector('[data-hud-bg="dots-grid"]');
  if (!(bg instanceof Element)) return;
  cleanupHudBackground(bg);
};

/**
 * Render an authed page shell once, then update only the content slot on subsequent calls.
 *
 * This keeps the HUD background stable across data refreshes (avoids tearing down canvases).
 */
export const runAuthedShell = (container: Element, content: TemplateResult): Effect.Effect<void> => {
  return Effect.gen(function* () {
    const dom = yield* DomServiceTag;

    const shell = container.querySelector(`[data-authed-shell]`);
    if (!shell) {
      yield* dom.render(container, authedShellTemplate(content));
      return;
    }

    const slot = container.querySelector("[data-authed-content]");
    if (!(slot instanceof Element)) return;
    yield* dom.render(slot, content);
  }).pipe(
    Effect.provide(EffuseLive),
    Effect.catchAll((err) => {
      console.error("[Effuse authed shell]", err);
      return Effect.void;
    }),
  );
};
