import { useEffect, useState } from 'react'

import { SUPPORTED_LOCALES } from '@openagentsinc/swap-i18n'

import { appendSwapLog, buildSwapLogBundle, swapLogBundleJson } from './log'
import { SwapNotYetAvailable } from './not-yet-available'
import {
  defaultSwapSettings,
  loadSwapSettings,
  saveSwapSettings,
  validateSwapRelayUrl,
  type SwapSettings,
} from './settings'
import { SwapSurfaceShell } from './shell'

const labelClass = 'grid gap-1.5 font-mono text-sm text-khala-text-muted'
const selectClass =
  'min-h-11 w-full max-w-xs border border-khala-border/80 bg-black px-3 py-2 font-mono text-sm text-khala-text outline-none'
const sectionClass =
  'grid gap-4 border border-khala-border/80 bg-khala-surface/40 p-5'
const sectionHeadingClass = 'm-0 text-lg font-semibold text-white'
const noteClass = 'm-0 max-w-[70ch] text-pretty text-xs/5 text-khala-text-faint'

const browserStorage = (): Storage | undefined =>
  typeof window === 'undefined' ? undefined : window.localStorage

/**
 * /swap/settings — exactly the settings MKT-SWP v1 has (SWAP-7, #9322):
 * denomination, decimal separator, privacy mode, relay selection, locale,
 * secret-store management, and log export. Slippage and gas top-up are
 * token-route concepts we do not have. Everything persists locally; nothing
 * is transmitted.
 */
export function SwapSettingsPage() {
  const [settings, setSettings] = useState<SwapSettings>(defaultSwapSettings)
  const [relayDraft, setRelayDraft] = useState('')
  const [relayError, setRelayError] = useState<string | undefined>(undefined)

  useEffect(() => {
    const storage = browserStorage()
    if (storage !== undefined) setSettings(loadSwapSettings(storage))
  }, [])

  const update = (next: SwapSettings, changedField: string) => {
    setSettings(next)
    const storage = browserStorage()
    if (storage !== undefined) {
      saveSwapSettings(storage, next)
      appendSwapLog(storage, 'settings_changed', { field: changedField })
    }
  }

  const addRelay = () => {
    const validated = validateSwapRelayUrl(relayDraft)
    if (!validated.ok) {
      setRelayError(
        validated.reason === 'insecure_scheme'
          ? 'Plain ws: is admitted only for loopback development relays. Use wss:// for anything reachable over a network.'
          : 'Enter a well-formed wss:// relay URL.',
      )
      return
    }
    if (settings.relays.some(relay => relay.url === validated.url)) {
      setRelayError('That relay is already in the set.')
      return
    }
    setRelayError(undefined)
    setRelayDraft('')
    update(
      {
        ...settings,
        relays: [
          ...settings.relays,
          { url: validated.url, read: true, write: true },
        ],
      },
      'relays',
    )
  }

  const exportLogs = () => {
    const storage = browserStorage()
    if (storage === undefined) return
    appendSwapLog(storage, 'log_exported')
    const json = swapLogBundleJson(buildSwapLogBundle(storage))
    const blob = new Blob([json], { type: 'application/json' })
    const url = URL.createObjectURL(blob)
    const anchor = document.createElement('a')
    anchor.href = url
    anchor.download = 'openagents-swap-log.json'
    anchor.click()
    URL.revokeObjectURL(url)
  }

  return (
    <SwapSurfaceShell active="settings">
      <header className="grid gap-2" data-route="swap-settings">
        <h1 className="m-0 text-3xl font-semibold tracking-tight">Settings</h1>
        <p className="m-0 max-w-[70ch] text-pretty text-sm/6 text-khala-text-muted">
          Everything here persists in this browser only. Nothing is
          transmitted to a relay, a provider, or openagents.com.
        </p>
      </header>

      <section className={sectionClass} data-swap-settings-display="">
        <h2 className={sectionHeadingClass}>Display</h2>
        <label className={labelClass}>
          <span>Denomination</span>
          <select
            className={selectClass}
            onChange={event =>
              update(
                {
                  ...settings,
                  denomination: event.target.value === 'btc' ? 'btc' : 'sats',
                },
                'denomination',
              )
            }
            value={settings.denomination}
          >
            <option value="sats">sats</option>
            <option value="btc">BTC</option>
          </select>
        </label>
        <label className={labelClass}>
          <span>Decimal separator</span>
          <select
            className={selectClass}
            onChange={event =>
              update(
                {
                  ...settings,
                  decimalSeparator: event.target.value === ',' ? ',' : '.',
                },
                'decimalSeparator',
              )
            }
            value={settings.decimalSeparator}
          >
            <option value=".">. (point)</option>
            <option value=",">, (comma)</option>
          </select>
        </label>
        <label className="flex items-center gap-2 font-mono text-sm text-khala-text-muted">
          <input
            checked={settings.privacyMode}
            onChange={event =>
              update(
                { ...settings, privacyMode: event.target.checked },
                'privacyMode',
              )
            }
            type="checkbox"
          />
          <span>Privacy mode — hide amounts on swap surfaces</span>
        </label>
        <label className={labelClass}>
          <span>Language</span>
          <select
            className={selectClass}
            disabled={SUPPORTED_LOCALES.length < 2}
            onChange={() => undefined}
            value={settings.locale}
          >
            {SUPPORTED_LOCALES.map(locale => (
              <option key={locale} value={locale}>
                {locale}
              </option>
            ))}
          </select>
          <span className={noteClass}>
            English only for now. More languages are a data change on the
            typed message table (openagents#9323).
          </span>
        </label>
      </section>

      <section className={sectionClass} data-swap-settings-relays="">
        <h2 className={sectionHeadingClass}>Relays</h2>
        <p className="m-0 max-w-[70ch] text-pretty text-sm/6 text-khala-text-muted">
          A market on relays means you pick which relays you read offers from
          and write events to.
        </p>
        <ul className="m-0 grid list-none gap-2 p-0">
          {settings.relays.map(relay => (
            <li
              className="flex flex-wrap items-center gap-3 border border-khala-border/60 bg-black px-3 py-2 font-mono text-sm"
              data-swap-relay={relay.url}
              key={relay.url}
            >
              <span className="min-w-0 flex-1 break-all text-khala-text">
                {relay.url}
              </span>
              <label className="flex items-center gap-1 text-khala-text-muted">
                <input
                  checked={relay.read}
                  onChange={event =>
                    update(
                      {
                        ...settings,
                        relays: settings.relays.map(entry =>
                          entry.url === relay.url
                            ? { ...entry, read: event.target.checked }
                            : entry,
                        ),
                      },
                      'relays',
                    )
                  }
                  type="checkbox"
                />
                read
              </label>
              <label className="flex items-center gap-1 text-khala-text-muted">
                <input
                  checked={relay.write}
                  onChange={event =>
                    update(
                      {
                        ...settings,
                        relays: settings.relays.map(entry =>
                          entry.url === relay.url
                            ? { ...entry, write: event.target.checked }
                            : entry,
                        ),
                      },
                      'relays',
                    )
                  }
                  type="checkbox"
                />
                write
              </label>
              <button
                className="border border-khala-border/70 px-2 py-1 text-xs text-khala-text-muted hover:text-white disabled:opacity-40"
                disabled={settings.relays.length === 1}
                onClick={() =>
                  update(
                    {
                      ...settings,
                      relays: settings.relays.filter(
                        entry => entry.url !== relay.url,
                      ),
                    },
                    'relays',
                  )
                }
                type="button"
              >
                Remove
              </button>
            </li>
          ))}
        </ul>
        <div className="flex flex-wrap items-start gap-2">
          <label className="grid flex-1 gap-1.5 font-mono text-sm text-khala-text-muted">
            <span className="sr-only">Relay URL</span>
            <input
              className={`${selectClass} max-w-md`}
              onChange={event => setRelayDraft(event.target.value)}
              placeholder="wss://relay.example.com"
              value={relayDraft}
            />
          </label>
          <button
            className="min-h-11 border border-khala-border bg-khala-surface px-4 py-2 font-mono text-sm text-white hover:bg-khala-surface-muted"
            onClick={addRelay}
            type="button"
          >
            Add relay
          </button>
        </div>
        {relayError === undefined ? null : (
          <p
            className="m-0 font-mono text-xs text-red-400"
            data-swap-relay-error=""
          >
            {relayError}
          </p>
        )}
      </section>

      <SwapNotYetAvailable
        marker="secret-store"
        heading="Secret store"
        body="No secret store exists in this browser yet. The secret store, the rescue key, and the verified backup ceremony land with the Rescue surface, and management controls appear here when they are real."
        issue="SWAP-4 (openagents#9319)"
      />

      <section className={sectionClass} data-swap-settings-logs="">
        <h2 className={sectionHeadingClass}>Logs</h2>
        <p className="m-0 max-w-[70ch] text-pretty text-sm/6 text-khala-text-muted">
          The swap surface keeps a small local diagnostic log. Export it as a
          file you can read before sharing. There is no upload path and no
          third-party support widget on this surface.
        </p>
        <button
          className="w-fit min-h-11 border border-khala-border bg-khala-surface px-4 py-2 font-mono text-sm text-white hover:bg-khala-surface-muted"
          data-swap-log-export=""
          onClick={exportLogs}
          type="button"
        >
          Export logs
        </button>
      </section>
    </SwapSurfaceShell>
  )
}
