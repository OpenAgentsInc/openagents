import { Effect } from 'effect';
import { DomServiceTag, EffuseLive, html } from '@openagentsinc/effuse';
import { hatcheryButton } from './ui/hatcheryButton';

/**
 * Renders the marketing layout header (logo + optional nav).
 * When isHome is false or isLogin is true, nav is hidden with visibility/pointer-events so layout does not reflow.
 */
export function runMarketingHeader(
  container: Element,
  isHome: boolean,
  isLogin: boolean
): Effect.Effect<void> {
  return Effect.gen(function* () {
    const dom = yield* DomServiceTag;
    const showNav = isHome && !isLogin;
    const navVisibility = showNav ? 'visible' : 'hidden';
    const navPointerEvents = showNav ? 'auto' : 'none';
    const content = html`
      <header class="flex h-14 w-full shrink-0 items-center justify-between px-6">
        <a href="/" class="select-none text-lg font-semibold text-white">OpenAgents</a>
        <div
          class="flex items-center gap-3"
          style="visibility: ${navVisibility}; pointer-events: ${navPointerEvents};"
          aria-hidden="${showNav ? 'false' : 'true'}"
        >
          <a
            href="/login"
            class="mr-5 text-base font-medium text-white/90 hover:text-white"
            style="font-family: var(--font-square721);"
          >
            Log in
          </a>
          ${hatcheryButton({
      href: '/login',
      label: 'Start for free',
      variant: 'outline',
    })}
        </div>
      </header>
    `;
    yield* dom.render(container, content);
  }).pipe(
    Effect.provide(EffuseLive),
    Effect.catchAll((err) => {
      console.error('[Effuse header]', err);
      return Effect.void;
    })
  );
}
