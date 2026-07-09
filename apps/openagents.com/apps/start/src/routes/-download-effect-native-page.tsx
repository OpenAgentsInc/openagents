import {
  Card,
  Link,
  Stack,
  Text,
  makeIntentRegistry,
  makeNavigationIntentHandlers,
  makeViewProgramFromState,
  navigationIntentDefinitions,
  resolveIntentRef,
  type CardView,
  type IntentReporter,
  type LinkView,
  type StackView,
  type TextView,
  type View,
} from '@effect-native/core'
import {
  makeDomNavigationHandler,
  makeDomRenderer,
} from '@effect-native/render-dom'
import { Effect, Exit, Scope, SubscriptionRef } from '@effect-native/core/effect'
import { useEffect, useRef } from 'react'

import { stage1EffectNativeTheme } from './-stage1-effect-native-theme'

// EN-4 (#8573) conversion of the interim React `/download` page
// (`-download-page.tsx`) onto the Effect Native DOM renderer, following the
// EN-1 `/stage1` and EN-4 `/khala`/`/tassadar` patterns: a thin React route
// shell mounts a typed Effect Native `ViewProgram`; all page content is
// authored as typed views, not JSX. Copy is preserved verbatim.

export const DOWNLOAD_ONE_CLICK_READY = false

export const AUTOPILOT_DESKTOP_DMG_URL =
  'https://github.com/OpenAgentsInc/openagents/releases/download/autopilot-desktop-v1.0.0-rc.3/AutopilotDesktop-1.0.0-rc.3-macos-arm64.dmg'

export const AUTOPILOT_DESKTOP_RELEASE_URL =
  'https://github.com/OpenAgentsInc/openagents/releases/tag/autopilot-desktop-v1.0.0-rc.3'

export const PYLON_INSTALL_COMMAND = 'npx @openagentsinc/pylon'

export type DownloadLandingState = Readonly<{
  oneClickReady: boolean
}>

export const initialDownloadLandingState: DownloadLandingState = {
  oneClickReady: DOWNLOAD_ONE_CLICK_READY,
}

export const downloadStatusFromState = (
  state: DownloadLandingState,
): 'live' | 'gated' => (state.oneClickReady ? 'live' : 'gated')

const text = (
  key: string,
  content: string,
  variant: TextView['variant'] = 'body',
  color: TextView['color'] = 'textPrimary',
): TextView =>
  Text({
    key,
    content,
    variant,
    color,
    style: { width: 'full' },
  })

const section = (key: string, children: ReadonlyArray<View>): StackView =>
  Stack(
    {
      key,
      direction: 'column',
      gap: '6',
      padding: '6',
      style: { width: 'full', maxWidth: 960, alignSelf: 'center' },
    },
    children,
  )

const externalLink = (
  key: string,
  label: string,
  href: string,
  variant: 'primary' | 'secondary' = 'secondary',
): LinkView =>
  Link(
    {
      key,
      destination: { kind: 'url', href },
      style:
        variant === 'primary'
          ? {
              backgroundColor: 'surface',
              borderColor: 'accent',
              borderWidth: 1,
              borderRadius: 'md',
              paddingTop: '3',
              paddingRight: '4',
              paddingBottom: '3',
              paddingLeft: '4',
            }
          : {
              borderColor: 'border',
              borderWidth: 1,
              borderRadius: 'md',
              paddingTop: '3',
              paddingRight: '4',
              paddingBottom: '3',
              paddingLeft: '4',
            },
    },
    [
      Text({
        key: `${key}-label`,
        content: label,
        variant: 'label',
        color: 'accent',
      }),
    ],
  )

const platformRow = (
  key: string,
  platform: string,
  status: string,
): StackView =>
  Stack(
    {
      key: `download-platform-row-${key}`,
      direction: 'row',
      align: 'center',
      gap: '3',
      style: { width: 'full' },
    },
    [
      text(`download-platform-${key}-name`, platform, 'label', 'textPrimary'),
      text(`download-platform-${key}-status`, status, 'caption', 'textMuted'),
    ],
  )

const infoCard = (
  key: string,
  children: ReadonlyArray<View>,
): CardView =>
  Card(
    {
      key: `download-card-${key}`,
      padding: '4',
      radius: 'lg',
      style: {
        backgroundColor: 'surface',
        borderColor: 'border',
        borderWidth: 1,
        width: 'full',
      },
    },
    children,
  )

export const downloadLandingView = (state: DownloadLandingState): View => {
  const status = downloadStatusFromState(state)

  return Stack(
    {
      key: 'download-root',
      direction: 'column',
      gap: '0',
      style: { backgroundColor: 'background', minHeight: 'full', width: 'full' },
    },
    [
      section('download-hero', [
        text('download-eyebrow', 'Autopilot Desktop', 'caption', 'accent'),
        text('download-title', 'Download Autopilot for Mac', 'heading'),
        text(
          'download-copy',
          'The signed macOS app is available now. The one-click auto-onboarding experience is shipping in the next signed build - see the status note below before you install.',
          'body',
          'textMuted',
        ),
      ]),
      section('download-status', [
        infoCard('status', [
          text(
            'download-status-label',
            'Status: auto-onboarding not in this build yet',
            'label',
            'warning',
          ),
          text(
            'download-status-body',
            'The currently published .dmg is the latest signed release, but it predates the new auto-onboarding flow. If you install it today it boots an isolated node - it does not yet self-register or join the run for you on its own. The next signed build will. If you want to start contributing right now, run a Pylon node instead.',
            'body',
            'textMuted',
          ),
          text(
            'download-status-gate',
            `download-status:${status}`,
            'caption',
            'textMuted',
          ),
        ]),
      ]),
      section('download-macos', [
        infoCard('macos', [
          text('download-macos-eyebrow', 'macOS · Apple Silicon', 'caption', 'accent'),
          text('download-macos-title', 'Signed + notarized .dmg', 'title'),
          text(
            'download-macos-body',
            'Built with an Apple Developer ID and notarized, so macOS Gatekeeper opens it without warnings. Apple Silicon (M-series) Macs.',
            'body',
            'textMuted',
          ),
          Stack(
            {
              key: 'download-macos-actions',
              direction: 'column',
              gap: '3',
              style: { width: 'full' },
            },
            [
              externalLink(
                'download-cta-autopilot',
                'Download for Mac (Apple Silicon)',
                AUTOPILOT_DESKTOP_DMG_URL,
                'primary',
              ),
              externalLink(
                'download-cta-release',
                'View the release on GitHub',
                AUTOPILOT_DESKTOP_RELEASE_URL,
                'secondary',
              ),
            ],
          ),
        ]),
      ]),
      section('download-pylon', [
        infoCard('pylon', [
          text(
            'download-pylon-eyebrow',
            'For agents + operators',
            'caption',
            'accent',
          ),
          text(
            'download-pylon-title',
            'Run a Pylon node from the terminal',
            'title',
          ),
          text(
            'download-pylon-body',
            'The contributor path that works today. Paste this to your coding agent or run it yourself.',
            'body',
            'textMuted',
          ),
          text(
            'download-pylon-command',
            PYLON_INSTALL_COMMAND,
            'label',
            'success',
          ),
        ]),
      ]),
      section('download-platforms', [
        infoCard('platforms', [
          text(
            'download-platforms-eyebrow',
            'Platform availability',
            'caption',
            'accent',
          ),
          platformRow(
            'macos-arm',
            'macOS · Apple Silicon',
            'Available now (signed + notarized)',
          ),
          platformRow('macos-intel', 'macOS · Intel', 'Not published yet'),
          platformRow(
            'windows',
            'Windows',
            'Pending the Authenticode signing certificate',
          ),
          platformRow('linux', 'Linux', 'Not published yet'),
        ]),
      ]),
      section('download-owner', [
        infoCard('owner', [
          text(
            'download-owner-title',
            'For the owner - to make one-click live',
            'caption',
            'textMuted',
          ),
          text(
            'download-owner-body',
            'Build + sign + notarize a fresh DMG from current main (with AO-1..AO-4), publish it, update AUTOPILOT_DESKTOP_DMG_URL in page/download.ts, then set DOWNLOAD_ONE_CLICK_READY = true. See docs/launch/2026-06-18-autopilot-desktop-availability-audit.md section 4.',
            'caption',
            'textMuted',
          ),
        ]),
      ]),
    ],
  )
}

export const mountDownloadEffectNativeSurface = (container: HTMLElement) =>
  Effect.gen(function* () {
    const state = yield* SubscriptionRef.make(initialDownloadLandingState)
    const program = makeViewProgramFromState(state, downloadLandingView)
    const registry = yield* makeIntentRegistry(
      navigationIntentDefinitions,
      makeNavigationIntentHandlers(makeDomNavigationHandler()),
    )
    const report: IntentReporter = (ref, runtimeValue) =>
      registry.dispatch(resolveIntentRef(ref, runtimeValue))
    const surface = yield* makeDomRenderer({
      theme: stage1EffectNativeTheme,
    }).mount(container, program.viewStream, report)

    return { state, unmount: surface.unmount }
  })

export function DownloadEffectNativePage() {
  const rootRef = useRef<HTMLDivElement | null>(null)

  useEffect(() => {
    const root = rootRef.current
    if (root === null) {
      return undefined
    }

    let disposed = false
    let closeScope: (() => void) | undefined

    void Effect.runPromise(Scope.make())
      .then((scope) => {
        const close = () => {
          void Effect.runPromise(Scope.close(scope, Exit.void))
        }
        closeScope = close
        if (disposed) {
          close()
          return undefined
        }
        return Effect.runPromise(
          Scope.provide(scope)(mountDownloadEffectNativeSurface(root)),
        )
      })
      .catch(() => undefined)

    return () => {
      disposed = true
      closeScope?.()
    }
  }, [])

  return (
    <main
      aria-label="Download Autopilot for Mac"
      className="download-effect-native-host"
      data-route="download"
      data-download-effect-native=""
    >
      <div ref={rootRef} data-download-effect-native-root="" />
    </main>
  )
}
