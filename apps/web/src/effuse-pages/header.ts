import { Effect } from "effect";
import { DomServiceTag, EffuseLive, html } from "@openagentsinc/effuse";

/**
 * Renders the marketing layout header (logo + optional nav).
 * When isHome is false, nav is hidden with visibility/pointer-events so layout does not reflow.
 */
export function runMarketingHeader(
  container: Element,
  isHome: boolean
): Effect.Effect<void> {
  return Effect.gen(function* () {
    const dom = yield* DomServiceTag;
    const navVisibility = isHome ? "visible" : "hidden";
    const navPointerEvents = isHome ? "auto" : "none";
    const content = html`
      <header class="-mx-4 flex h-14 w-full shrink-0 items-center justify-between px-6">
        <a href="/" class="select-none text-lg font-semibold text-white">OpenAgents</a>
        <div
          class="flex items-center gap-3"
          style="visibility: ${navVisibility}; pointer-events: ${navPointerEvents};"
          aria-hidden="${isHome ? "false" : "true"}"
        >
          <a
            href="/login"
            class="mr-5 text-base font-medium text-white/90 hover:text-white"
            style="font-family: var(--font-square721);"
          >
            Log in
          </a>
          <a
            href="/login"
            class="inline-flex h-9 min-h-9 items-center justify-center rounded-lg border border-white/90 bg-transparent px-4 text-sm font-medium uppercase tracking-wide text-white transition-opacity hover:opacity-100 focus:outline-none focus-visible:ring-2 focus-visible:ring-white/50"
          >
            Start for free
          </a>
        </div>
      </header>
    `;
    yield* dom.render(container, content);
  }).pipe(
    Effect.provide(EffuseLive),
    Effect.catchAll((err) => {
      console.error("[Effuse header]", err);
      return Effect.void;
    })
  );
}
