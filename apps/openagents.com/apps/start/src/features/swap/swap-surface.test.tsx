import { renderToStaticMarkup } from 'react-dom/server'
import { describe, expect, test } from 'vitest'

import { isKnownStartDocumentPath } from '../../route-table'
import { swapSurfaceEnabled } from './gate'
import { SwapHistoryPage } from './history-page'
import {
  SWAP_LOG_MAX_ENTRIES,
  appendSwapLog,
  buildSwapLogBundle,
  readSwapLog,
} from './log'
import { SwapRescuePage } from './rescue-page'
import {
  DEFAULT_SWAP_RELAY_URL,
  SWAP_SETTINGS_STORAGE_KEY,
  decodeSwapSettings,
  defaultSwapSettings,
  loadSwapSettings,
  saveSwapSettings,
  validateSwapRelayUrl,
} from './settings'
import { SwapSettingsPage } from './settings-page'
import { SwapSurfaceGateClosed } from './shell'
import { SwapIndexPage } from './swap-page'

const fakeStorage = () => {
  const map = new Map<string, string>()
  return {
    getItem: (key: string) => map.get(key) ?? null,
    setItem: (key: string, value: string) => void map.set(key, value),
  }
}

const surfaces: ReadonlyArray<[name: string, html: string]> = [
  ['swap', renderToStaticMarkup(<SwapIndexPage />)],
  ['rescue', renderToStaticMarkup(<SwapRescuePage />)],
  [
    'rescue action deep link',
    renderToStaticMarkup(<SwapRescuePage actionRef="action-1" />),
  ],
  ['history', renderToStaticMarkup(<SwapHistoryPage />)],
  [
    'session deep link',
    renderToStaticMarkup(<SwapHistoryPage sessionId="session-1" />),
  ],
  ['settings', renderToStaticMarkup(<SwapSettingsPage />)],
  ['gate closed', renderToStaticMarkup(<SwapSurfaceGateClosed />)],
]

describe('swap serving gate (SWAP-7, #9322)', () => {
  test('fails closed for everything except the exact string true', () => {
    expect(swapSurfaceEnabled('true')).toBe(true)
    for (const configured of [
      undefined,
      '',
      'false',
      'TRUE',
      'True',
      '1',
      'yes',
      ' true',
    ]) {
      expect(swapSurfaceEnabled(configured)).toBe(false)
    }
  })

  test('the gate-closed view claims no capability and states provenance', () => {
    const html = renderToStaticMarkup(<SwapSurfaceGateClosed />)
    expect(html).toContain('data-swap-surface-gate-closed=""')
    expect(html).toContain('Swap surface not enabled')
    expect(html).toContain('data-swap-build-provenance')
    expect(html).not.toContain('data-swap-nav')
  })
})

describe('swap routes and nav (SWAP-7, #9322)', () => {
  test.each([
    '/swap',
    '/swap/',
    '/swap/history',
    '/swap/rescue',
    '/swap/rescue/action-1',
    '/swap/s/session-1',
    '/swap/settings',
  ])('document route %s is admitted by the shared route table', pathname => {
    expect(isKnownStartDocumentPath(pathname)).toBe(true)
  })

  test('unclaimed /swap descendants stay unadmitted', () => {
    expect(isKnownStartDocumentPath('/swap/admin')).toBe(false)
    expect(isKnownStartDocumentPath('/swap/rescue/a/b')).toBe(false)
  })

  test('every swap page renders the shared nav and Help/Docs links', () => {
    for (const [name, html] of surfaces) {
      if (name === 'gate closed') continue
      expect(html, name).toContain('data-swap-nav=""')
      expect(html, name).toContain('href="/swap"')
      expect(html, name).toContain('href="/swap/rescue"')
      expect(html, name).toContain('href="/swap/history"')
      expect(html, name).toContain('href="/swap/settings"')
      expect(html, name).toContain('href="/forum"')
      expect(html, name).toContain('href="/docs"')
    }
  })

  test('build provenance renders on every swap surface', () => {
    for (const [name, html] of surfaces) {
      expect(html, name).toContain('data-swap-build-provenance')
    }
  })

  test('deep links surface their coordinate honestly', () => {
    const rescue = renderToStaticMarkup(<SwapRescuePage actionRef="action-1" />)
    expect(rescue).toContain('data-swap-rescue-action-ref="action-1"')
    const history = renderToStaticMarkup(
      <SwapHistoryPage sessionId="session-1" />,
    )
    expect(history).toContain('data-swap-session-ref="session-1"')
  })
})

describe('unbuilt surfaces stay honest (SWAP-7, #9322)', () => {
  test('the swap widget is mounted, and says plainly it has no live market', () => {
    // SWAP-0 (#9315) landed the shell, so the widget itself is no longer a
    // placeholder. What is still missing is the live relay subscription and
    // the wasm engine, and the surface must say so instead of implying a
    // working market.
    const html = renderToStaticMarkup(<SwapIndexPage />)
    expect(html).toContain('data-swap-widget=""')
    expect(html).toContain('data-swap-primary-action=""')
    expect(html).toContain('data-swap-not-yet-available="swap-live-inputs"')
    expect(html).toContain('SWAP-0 (openagents#9315)')
  })

  test('the mounted widget renders its primary action disabled and explained', () => {
    // Oracle for openagents_web.swap_widget.primary_action_law.v1: the
    // primary-action law asserted on the real surface — the control is
    // always rendered, never enabled before the engine is ready, and never
    // blank.
    const html = renderToStaticMarkup(<SwapIndexPage />)
    expect(html).toContain('data-swap-widget-state="EngineLoading"')
    expect(html).toContain(
      'data-swap-primary-action-key="swap.widget.engine_loading"',
    )
    expect(html).toContain('aria-busy="true"')
    expect(html).toContain('disabled=""')
    expect(html).toContain('Loading the swap engine')
  })

  test('Rescue renders not-yet-available naming SWAP-4', () => {
    const html = renderToStaticMarkup(<SwapRescuePage />)
    expect(html).toContain('data-swap-not-yet-available="rescue"')
    expect(html).toContain('SWAP-4 (openagents#9319)')
  })

  test('History renders not-yet-available naming SWAP-5', () => {
    const html = renderToStaticMarkup(<SwapHistoryPage />)
    expect(html).toContain('data-swap-not-yet-available="history"')
    expect(html).toContain('SWAP-5 (openagents#9320)')
  })

  test('the secret-store settings section names SWAP-4 instead of faking controls', () => {
    const html = renderToStaticMarkup(<SwapSettingsPage />)
    expect(html).toContain('data-swap-not-yet-available="secret-store"')
    expect(html).toContain('SWAP-4 (openagents#9319)')
  })
})

describe('content-security posture (SWAP-7, #9322)', () => {
  test('no swap surface loads a third-party resource', () => {
    // The rollout plan §2.1 and #9322 §6: no third-party script on a surface
    // that holds key material. Anchors may point off-origin (the provenance
    // commit link); loaded resources may not.
    for (const [name, html] of surfaces) {
      expect(html, name).not.toMatch(/<script/i)
      expect(html, name).not.toMatch(/<iframe/i)
      expect(html, name).not.toMatch(
        /<(?:img|link|source|video|audio|embed|object)[^>]*(?:src|href)="(?:https?:)?\/\//i,
      )
    }
  })
})

describe('swap settings persistence (SWAP-7, #9322)', () => {
  test('defaults are sats, point separator, privacy off, the public relay', () => {
    const settings = defaultSwapSettings()
    expect(settings.denomination).toBe('sats')
    expect(settings.decimalSeparator).toBe('.')
    expect(settings.privacyMode).toBe(false)
    expect(settings.locale).toBe('en')
    expect(settings.relays).toEqual([
      { url: DEFAULT_SWAP_RELAY_URL, read: true, write: true },
    ])
  })

  test('settings round-trip through storage', () => {
    const storage = fakeStorage()
    const next = {
      ...defaultSwapSettings(),
      denomination: 'btc' as const,
      decimalSeparator: ',' as const,
      privacyMode: true,
      relays: [
        { url: 'wss://relay.openagents.com/', read: true, write: false },
        { url: 'wss://relay.example.com/', read: true, write: true },
      ],
    }
    saveSwapSettings(storage, next)
    expect(loadSwapSettings(storage)).toEqual(next)
  })

  test('corrupt or version-mismatched payloads fall back to defaults', () => {
    expect(decodeSwapSettings('not json')).toEqual(defaultSwapSettings())
    expect(decodeSwapSettings('42')).toEqual(defaultSwapSettings())
    expect(decodeSwapSettings(JSON.stringify({ schemaVersion: 999 }))).toEqual(
      defaultSwapSettings(),
    )
    const storage = fakeStorage()
    storage.setItem(SWAP_SETTINGS_STORAGE_KEY, '{broken')
    expect(loadSwapSettings(storage)).toEqual(defaultSwapSettings())
  })

  test('relay URLs must be wss, with ws admitted only on loopback', () => {
    expect(validateSwapRelayUrl('wss://relay.example.com')).toEqual({
      ok: true,
      url: 'wss://relay.example.com/',
    })
    expect(validateSwapRelayUrl('ws://localhost:7777')).toEqual({
      ok: true,
      url: 'ws://localhost:7777/',
    })
    expect(validateSwapRelayUrl('ws://relay.example.com')).toEqual({
      ok: false,
      reason: 'insecure_scheme',
    })
    expect(validateSwapRelayUrl('https://relay.example.com')).toEqual({
      ok: false,
      reason: 'invalid_url',
    })
    expect(validateSwapRelayUrl('not a url')).toEqual({
      ok: false,
      reason: 'invalid_url',
    })
  })
})

describe('local swap log (SWAP-7, #9322)', () => {
  test('append, bound, and export locally', () => {
    const storage = fakeStorage()
    expect(readSwapLog(storage)).toEqual([])
    appendSwapLog(storage, 'settings_changed', { field: 'denomination' })
    appendSwapLog(storage, 'log_exported')
    const entries = readSwapLog(storage)
    expect(entries).toHaveLength(2)
    expect(entries[0]?.event).toBe('settings_changed')
    expect(entries[0]?.detail).toEqual({ field: 'denomination' })

    const bundle = buildSwapLogBundle(storage, () => new Date(0))
    expect(bundle.format).toBe('openagents.swap.log_export.v1')
    expect(bundle.exportedAt).toBe('1970-01-01T00:00:00.000Z')
    expect(bundle.buildCommit).toBe('unknown')
    expect(bundle.entries).toHaveLength(2)
    expect(bundle.settings).toEqual(defaultSwapSettings())
  })

  test('the log is bounded', () => {
    const storage = fakeStorage()
    for (let index = 0; index < SWAP_LOG_MAX_ENTRIES + 25; index += 1) {
      appendSwapLog(storage, `event-${index}`)
    }
    const entries = readSwapLog(storage)
    expect(entries).toHaveLength(SWAP_LOG_MAX_ENTRIES)
    expect(entries[0]?.event).toBe('event-25')
  })
})
