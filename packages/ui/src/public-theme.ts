import { clsx } from 'clsx'
import type { Attribute, Html } from 'foldkit/html'
import { html } from 'foldkit/html'

import { kitFamily } from './primitives'

export type PublicLandingThemeMode = 'light' | 'dark'
export type PublicLandingThemePreference = PublicLandingThemeMode | 'system'

export const publicLandingThemeStorageKey = 'oa.publicLanding.v1:theme'
export const publicLandingThemeShellAttribute = 'public-landing-shell'
export const publicLandingThemeSelectAttribute = 'public-landing-theme-select'

const resolvedMode = (
  preference: PublicLandingThemePreference,
): PublicLandingThemeMode => (preference === 'light' ? 'light' : 'dark')

export const publicLandingThemeScript = (input: {
  storageKey?: string
} = {}): string => {
  const storageKey = input.storageKey ?? publicLandingThemeStorageKey

  return `(() => {
  const THEME_KEY = ${JSON.stringify(storageKey)};
  const themeMedia = window.matchMedia('(prefers-color-scheme: dark)');
  const readThemePref = () => {
    try {
      const stored = localStorage.getItem(THEME_KEY);
      return stored === 'light' || stored === 'dark' ? stored : 'system';
    } catch (_) {
      return 'system';
    }
  };
  const resolveTheme = pref =>
    pref === 'light' || pref === 'dark'
      ? pref
      : themeMedia.matches ? 'dark' : 'light';
  const landingShells = () => Array.from(document.querySelectorAll('[data-public-landing-shell]'));
  const syncThemeSelects = pref => {
    for (const select of document.querySelectorAll('[data-public-landing-theme-select]')) {
      if (select && select.value !== pref) select.value = pref;
    }
  };
  const applyTheme = pref => {
    const resolved = resolveTheme(pref);
    for (const shell of landingShells()) {
      shell.setAttribute('data-public-landing-theme', resolved);
      shell.setAttribute('data-public-landing-theme-preference', pref);
    }
    syncThemeSelects(pref);
  };
  applyTheme(readThemePref());
  document.addEventListener('change', event => {
    const target = event.target;
    const select = target && target.closest ? target.closest('[data-public-landing-theme-select]') : null;
    if (!select) return;
    const value = select.value;
    const pref = value === 'light' || value === 'dark' ? value : 'system';
    try {
      if (pref === 'system') localStorage.removeItem(THEME_KEY);
      else localStorage.setItem(THEME_KEY, pref);
    } catch (_) {}
    applyTheme(pref);
  });
  themeMedia.addEventListener('change', () => {
    if (readThemePref() === 'system') applyTheme('system');
  });
})();`
}

export const publicLandingThemeShell = <Message>(input: {
  children: ReadonlyArray<Html>
  preference?: PublicLandingThemePreference
  mode?: PublicLandingThemeMode
  className?: string
  attrs?: ReadonlyArray<Attribute<Message>>
}): Html => {
  const h = html<Message>()
  const preference = input.preference ?? input.mode ?? 'system'
  const mode = input.mode ?? resolvedMode(preference)

  return h.div(
    [
      ...(input.attrs ?? []),
      kitFamily<Message>('public/theme-shells'),
      h.DataAttribute(publicLandingThemeShellAttribute, ''),
      h.DataAttribute('public-landing-theme', mode),
      h.DataAttribute('public-landing-theme-preference', preference),
      h.Class(
        clsx(
          'bg-public-landing-page text-public-landing-text',
          input.className,
        ),
      ),
    ],
    input.children,
  )
}

export const publicLandingThemeSelector = <Message>(input: {
  preference?: PublicLandingThemePreference
  label?: string
  className?: string
  attrs?: ReadonlyArray<Attribute<Message>>
} = {}): Html => {
  const h = html<Message>()
  const preference = input.preference ?? 'system'

  return h.label(
    [
      ...(input.attrs ?? []),
      kitFamily<Message>('public/theme-selectors'),
      h.Class(clsx('inline-grid gap-1.5 font-mono text-xs', input.className)),
    ],
    [
      h.span([h.Class('text-public-landing-muted')], [
        input.label ?? 'Landing theme',
      ]),
      h.select(
        [
          h.DataAttribute(publicLandingThemeSelectAttribute, ''),
          h.AriaLabel(input.label ?? 'Landing theme'),
          h.Class(
            'min-h-9 border border-public-landing-border bg-public-landing-surface px-2 text-sm text-public-landing-text focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-public-landing-accent',
          ),
        ],
        [
          h.option(
            [
              h.Value('system'),
              ...(preference === 'system' ? [h.Selected(true)] : []),
            ],
            ['System'],
          ),
          h.option(
            [
              h.Value('light'),
              ...(preference === 'light' ? [h.Selected(true)] : []),
            ],
            ['Light'],
          ),
          h.option(
            [
              h.Value('dark'),
              ...(preference === 'dark' ? [h.Selected(true)] : []),
            ],
            ['Dark'],
          ),
        ],
      ),
    ],
  )
}
