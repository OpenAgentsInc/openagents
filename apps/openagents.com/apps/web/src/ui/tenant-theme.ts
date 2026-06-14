// COORDINATOR WIRING:
//   1. Add to apps/openagents.com/apps/web/src/ui/index.ts:
//        export * from './tenant-theme'
//   2. Apply at the tenant-scoped root only (a tenant subdomain / customer
//      site shell element), NOT the core/default OpenAgents shell. Set the
//      returned string as the inline `style` attribute on that tenant root:
//        html().Div([html().Attr('style', tenantThemeStyle(profile))], [ ...children ])
//      The returned CSS-var overrides cascade to every descendant that reads
//      `var(--oa-*)`. The core/default product surface stays dark-only per
//      DESIGN.md because it never wraps content in this root.
//
// This module is a thin override layer over the existing --oa-* design tokens
// (DESIGN.md "Foundation Tokens" / "Semantic Accent Tokens"). It does NOT fork
// any component. A tenant brand profile maps to a small set of CSS custom
// property overrides scoped to a tenant root element.
//
// POLICY: DESIGN.md's dark-only default applies ONLY to the core OpenAgents
// product/public surface. Tenant subdomains and customer sites are fully
// themeable: 'warm' and 'light' modes are allowed here and ONLY here.

/**
 * The --oa-* foundation + accent tokens a tenant brand profile may override.
 * Names mirror the DESIGN.md token contract exactly. All fields optional: an
 * omitted token falls through to the core/default value already on the page.
 */
export type TenantBrandPalette = Readonly<{
  bg?: string
  panel?: string
  panelActive?: string
  hover?: string
  borderSubtle?: string
  borderActive?: string
  text?: string
  textStrong?: string
  textMuted?: string
  textFaint?: string
  highlight?: string
  positive?: string
  negative?: string
  warning?: string
  info?: string
}>

/**
 * Optional typography accents a tenant may set. Mapped to --oa-font-* custom
 * properties; surfaces that opt in can read them, and they never affect the
 * core surface (which does not apply a tenant root).
 */
export type TenantTypographyAccents = Readonly<{
  fontMono?: string
  fontSans?: string
  letterSpacing?: string
}>

/**
 * Tenant theme mode. 'dark' matches the core default. 'warm' and 'light' are
 * allowed for tenant subdomains / customer sites only.
 */
export type TenantThemeMode = 'dark' | 'warm' | 'light'

/**
 * A bounded, self-contained per-tenant brand profile. `logoRef` is an opaque
 * reference (URL or asset id) resolved by the rendering surface; this module
 * does not load or validate it.
 */
export type TenantBrandProfile = Readonly<{
  id: string
  logoRef?: string
  mode?: TenantThemeMode
  palette?: TenantBrandPalette
  typography?: TenantTypographyAccents
}>

/**
 * The default (core) dark profile. Identical token values to DESIGN.md's
 * Foundation + Semantic Accent tokens. Used as the deterministic base every
 * tenant profile layers on top of.
 */
export const DEFAULT_DARK_PROFILE: TenantBrandProfile = {
  id: 'oa-default-dark',
  mode: 'dark',
  palette: {
    bg: '#000000',
    panel: '#010102',
    panelActive: '#141414',
    hover: '#080808',
    borderSubtle: '#222222',
    borderActive: '#333333',
    text: '#f1efe8',
    textStrong: 'rgba(255, 255, 255, 0.9)',
    textMuted: 'rgba(255, 255, 255, 0.6)',
    textFaint: 'rgba(255, 255, 255, 0.35)',
    highlight: '#ffb400',
    positive: '#00c853',
    negative: '#d32f2f',
    warning: '#ff6f00',
    info: '#2979ff',
  },
}

// Mode presets supply sensible foundation defaults for non-dark tenant
// surfaces. A tenant palette still overrides any of these per-token.
const MODE_PRESETS: Readonly<Record<TenantThemeMode, TenantBrandPalette>> = {
  dark: {},
  warm: {
    bg: '#1c160f',
    panel: '#241c12',
    panelActive: '#33271a',
    hover: '#2a2014',
    borderSubtle: '#4a3a26',
    borderActive: '#6b5436',
    text: '#f7ede0',
    textStrong: 'rgba(255, 248, 238, 0.92)',
    textMuted: 'rgba(255, 248, 238, 0.62)',
    textFaint: 'rgba(255, 248, 238, 0.38)',
  },
  light: {
    bg: '#faf8f2',
    panel: '#ffffff',
    panelActive: '#f0ece2',
    hover: '#f4f1e9',
    borderSubtle: '#e0dccf',
    borderActive: '#c9c3b2',
    text: '#1a1814',
    textStrong: 'rgba(20, 18, 14, 0.92)',
    textMuted: 'rgba(20, 18, 14, 0.62)',
    textFaint: 'rgba(20, 18, 14, 0.38)',
  },
}

// Stable mapping from palette field -> CSS custom property name. Order here
// fixes the deterministic emission order of the produced style string.
const PALETTE_TOKEN_MAP: ReadonlyArray<
  readonly [keyof TenantBrandPalette, string]
> = [
  ['bg', '--oa-bg'],
  ['panel', '--oa-panel'],
  ['panelActive', '--oa-panel-active'],
  ['hover', '--oa-hover'],
  ['borderSubtle', '--oa-border-subtle'],
  ['borderActive', '--oa-border-active'],
  ['text', '--oa-text'],
  ['textStrong', '--oa-text-strong'],
  ['textMuted', '--oa-text-muted'],
  ['textFaint', '--oa-text-faint'],
  ['highlight', '--oa-highlight'],
  ['positive', '--oa-positive'],
  ['negative', '--oa-negative'],
  ['warning', '--oa-warning'],
  ['info', '--oa-info'],
]

const TYPOGRAPHY_TOKEN_MAP: ReadonlyArray<
  readonly [keyof TenantTypographyAccents, string]
> = [
  ['fontMono', '--oa-font-mono'],
  ['fontSans', '--oa-font-sans'],
  ['letterSpacing', '--oa-letter-spacing'],
]

/**
 * Resolve the effective palette for a profile: mode preset first, then the
 * profile's own palette overrides on top. Pure: depends only on its input.
 */
export const resolveTenantPalette = (
  profile: TenantBrandProfile,
): TenantBrandPalette => {
  const mode: TenantThemeMode = profile.mode ?? 'dark'
  return { ...MODE_PRESETS[mode], ...(profile.palette ?? {}) }
}

/**
 * Produce the CSS-var override style string for a tenant root.
 *
 * Pure and deterministic: same profile in -> identical string out, with a
 * fixed token order. Returns an inline `style` value (e.g.
 * `--oa-bg:#faf8f2;--oa-text:#1a1814;`). An empty profile (pure dark, no
 * overrides) yields an empty string, leaving the core default untouched.
 */
export const tenantThemeStyle = (profile: TenantBrandProfile): string => {
  const palette = resolveTenantPalette(profile)
  const decls: Array<string> = []

  for (const [field, cssVar] of PALETTE_TOKEN_MAP) {
    const value = palette[field]
    if (value !== undefined) {
      decls.push(`${cssVar}:${value}`)
    }
  }

  const typography = profile.typography
  if (typography !== undefined) {
    for (const [field, cssVar] of TYPOGRAPHY_TOKEN_MAP) {
      const value = typography[field]
      if (value !== undefined) {
        decls.push(`${cssVar}:${value}`)
      }
    }
  }

  return decls.length === 0 ? '' : `${decls.join(';')};`
}

/**
 * Produce the override declarations as an object (useful for tests or for
 * surfaces that prefer a style record over a string). Pure/deterministic.
 */
export const tenantThemeVars = (
  profile: TenantBrandProfile,
): Readonly<Record<string, string>> => {
  const palette = resolveTenantPalette(profile)
  const out: Record<string, string> = {}

  for (const [field, cssVar] of PALETTE_TOKEN_MAP) {
    const value = palette[field]
    if (value !== undefined) {
      out[cssVar] = value
    }
  }

  const typography = profile.typography
  if (typography !== undefined) {
    for (const [field, cssVar] of TYPOGRAPHY_TOKEN_MAP) {
      const value = typography[field]
      if (value !== undefined) {
        out[cssVar] = value
      }
    }
  }

  return out
}
