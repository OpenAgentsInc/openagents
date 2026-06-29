import { describe, expect, it } from 'vitest'

import {
  DEFAULT_DARK_PROFILE,
  type TenantBrandProfile,
  resolveTenantPalette,
  tenantThemeStyle,
  tenantThemeVars,
} from './tenant-theme'

describe('tenant-theme', () => {
  it('default dark profile keeps the core foundation tokens', () => {
    const palette = resolveTenantPalette(DEFAULT_DARK_PROFILE)
    expect(palette.bg).toBe('#000000')
    expect(palette.text).toBe('#f1efe8')

    const vars = tenantThemeVars(DEFAULT_DARK_PROFILE)
    expect(vars['--oa-bg']).toBe('#000000')
    expect(vars['--oa-text']).toBe('#f1efe8')
    expect(vars['--oa-highlight']).toBe('#ffb400')
  })

  it('an empty dark tenant profile produces no overrides', () => {
    const profile: TenantBrandProfile = { id: 'plain', mode: 'dark' }
    expect(tenantThemeStyle(profile)).toBe('')
    expect(tenantThemeVars(profile)).toEqual({})
  })

  it('a warm profile overrides --oa-bg and --oa-text away from dark', () => {
    const profile: TenantBrandProfile = { id: 'acme-warm', mode: 'warm' }
    const vars = tenantThemeVars(profile)
    expect(vars['--oa-bg']).toBe('#1c160f')
    expect(vars['--oa-text']).toBe('#f7ede0')
    expect(vars['--oa-bg']).not.toBe('#000000')

    const style = tenantThemeStyle(profile)
    expect(style).toContain('--oa-bg:#1c160f')
    expect(style).toContain('--oa-text:#f7ede0')
  })

  it('a light profile overrides --oa-bg and --oa-text correctly', () => {
    const profile: TenantBrandProfile = { id: 'acme-light', mode: 'light' }
    const vars = tenantThemeVars(profile)
    expect(vars['--oa-bg']).toBe('#faf8f2')
    expect(vars['--oa-text']).toBe('#1a1814')
  })

  it('explicit palette tokens override the mode preset', () => {
    const profile: TenantBrandProfile = {
      id: 'acme-brand',
      mode: 'light',
      palette: { bg: '#fff0e0', highlight: '#ff3366' },
    }
    const vars = tenantThemeVars(profile)
    expect(vars['--oa-bg']).toBe('#fff0e0')
    // light preset text still flows through
    expect(vars['--oa-text']).toBe('#1a1814')
    // accent override applied
    expect(vars['--oa-highlight']).toBe('#ff3366')
  })

  it('maps typography accents to --oa-font-* tokens', () => {
    const profile: TenantBrandProfile = {
      id: 'acme-type',
      mode: 'warm',
      typography: {
        fontSans: 'Brand Sans, sans-serif',
        fontMono: 'Brand Mono, monospace',
        letterSpacing: '0.01em',
      },
    }
    const vars = tenantThemeVars(profile)
    expect(vars['--oa-font-sans']).toBe('Brand Sans, sans-serif')
    expect(vars['--oa-font-mono']).toBe('Brand Mono, monospace')
    expect(vars['--oa-letter-spacing']).toBe('0.01em')
  })

  it('is pure and deterministic for repeated and equal inputs', () => {
    const a: TenantBrandProfile = {
      id: 'x',
      mode: 'warm',
      palette: { bg: '#111111' },
    }
    const b: TenantBrandProfile = {
      id: 'x',
      mode: 'warm',
      palette: { bg: '#111111' },
    }
    expect(tenantThemeStyle(a)).toBe(tenantThemeStyle(a))
    expect(tenantThemeStyle(a)).toBe(tenantThemeStyle(b))
    expect(tenantThemeVars(a)).toEqual(tenantThemeVars(b))
  })

  it('emits tokens in a stable, fixed order', () => {
    const profile: TenantBrandProfile = {
      id: 'order',
      palette: { text: '#abcdef', bg: '#123456' },
    }
    // --oa-bg precedes --oa-text in the canonical token order regardless of
    // the order keys were declared on the input palette.
    const style = tenantThemeStyle(profile)
    expect(style.indexOf('--oa-bg')).toBeLessThan(style.indexOf('--oa-text'))
  })
})
