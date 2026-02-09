import { Effect } from 'effect';
import { DomServiceTag, EffuseLive, html } from '@openagentsinc/effuse';
import { hatcheryButton } from '@openagentsinc/effuse-ui';

import type { TemplateResult } from '@openagentsinc/effuse';

/**
 * Renders the marketing layout header (logo + optional nav).
 * When isHome is false or isLogin is true, nav is hidden with visibility/pointer-events so layout does not reflow.
 * When prelaunch is true, nav (Log in, Start for free) is hidden.
 */
export function marketingHeaderTemplate(
  isHome: boolean,
  isLogin: boolean,
  prelaunch = false,
): TemplateResult {
  const showNav = isHome && !isLogin && !prelaunch;
  return html`
    <header class="flex h-14 w-full shrink-0 items-center justify-between px-6">
      <span class="select-none text-lg font-semibold text-white">OpenAgents</span>
      <div class="flex items-center gap-3" aria-hidden="${showNav ? 'false' : 'true'}">
        ${showNav
      ? html`
              <a
                href="/login"
                class="mr-5 text-base font-medium text-white/90 hover:text-white use-font-square721 [font-family:var(--font-square721)]"
              >
                Log in
              </a>
              ${hatcheryButton({
        href: '/login',
        label: 'Start for free',
        variant: 'outline',
      })}
            `
      : ''}
      </div>
    </header>
  `;
}

export function runMarketingHeader(
  container: Element,
  isHome: boolean,
  isLogin: boolean
): Effect.Effect<void> {
  return Effect.gen(function* () {
    const dom = yield* DomServiceTag;
    yield* dom.render(container, marketingHeaderTemplate(isHome, isLogin));
  }).pipe(
    Effect.provide(EffuseLive),
    Effect.catchAll((err) => {
      console.error('[Effuse header]', err);
      return Effect.void;
    })
  );
}
