import {
  Card,
  Link,
  Stack,
  Text,
  makeViewProgramFromState,
  type CardView,
  type LinkView,
  type StackView,
  type TextView,
  type View,
} from '@effect-native/core'
import { makeDomRenderer } from '@effect-native/render-dom'
import { Effect, Exit, Schema, Scope, SubscriptionRef } from '@effect-native/core/effect'
import { useEffect, useRef } from 'react'

import {
  LIVE_VALUE_PENDING,
  fetchKhalaTokensServed,
  formatCount,
} from './-sales-landing-data'
import { stage1EffectNativeTheme } from './-stage1-effect-native-theme'

// EN-4 (#8573) conversion of the interim React `/khala` page
// (`-app-shell-routes.tsx`'s `KhalaInfoPage`) onto the Effect Native DOM
// renderer, following the EN-1 `/stage1` pattern
// (`-stage1-effect-native-page.tsx`): a thin React route shell mounts a
// typed Effect Native `ViewProgram`; all landing content is authored as
// typed views, not JSX. Copy is preserved verbatim from the page it
// replaces. `SceneLayer`/`BackHome` in `-app-shell-routes.tsx` stay in use
// by `-code-page.tsx`; this route no longer depends on them.

class KhalaPublicSnapshotError extends Schema.TaggedErrorClass<KhalaPublicSnapshotError>()(
  'KhalaPublicSnapshotError',
  { message: Schema.String },
) {}

export type KhalaLandingState = Readonly<{
  tokensServed: string
}>

export const initialKhalaLandingState: KhalaLandingState = {
  tokensServed: LIVE_VALUE_PENDING,
}

export const khalaStateFromPublicSnapshot = (
  tokens: Awaited<ReturnType<typeof fetchKhalaTokensServed>>,
): KhalaLandingState => ({
  tokensServed: formatCount(tokens?.tokensServed ?? null),
})

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

const infoCard = (key: string, label: string, value: string): CardView =>
  Card(
    {
      key: `khala-info-${key}`,
      padding: '4',
      radius: 'lg',
      style: {
        backgroundColor: 'surface',
        borderColor: 'border',
        borderWidth: 1,
        flex: 1,
        minWidth: 'sm',
      },
    },
    [
      text(`khala-info-${key}-label`, label, 'caption', 'textMuted'),
      text(`khala-info-${key}-value`, value, 'body', 'textPrimary'),
    ],
  )

const navLink = (key: string, label: string, path: string): LinkView =>
  Link(
    {
      key,
      destination: { kind: 'path', path },
      style: {
        borderColor: 'accent',
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

export const khalaLandingView = (state: KhalaLandingState): View =>
  Stack(
    {
      key: 'khala-root',
      direction: 'column',
      gap: '0',
      style: { backgroundColor: 'background', minHeight: 'full', width: 'full' },
    },
    [
      section('khala-back-row', [
        navLink('khala-back-home', '← OpenAgents', '/'),
      ]),
      section('khala-hero', [
        text('khala-eyebrow', 'OpenAgents inference', 'caption', 'accent'),
        text('khala-title', 'Khala', 'heading'),
        text(
          'khala-copy',
          'Khala is the OpenAgents inference and work rail: an OpenAI-compatible API for public model access, work receipts, and agent-readable evidence. This public page keeps the usable API basics visible without claiming paid capacity is generally live.',
          'body',
          'textMuted',
        ),
      ]),
      section('khala-info', [
        Stack(
          {
            key: 'khala-info-grid',
            direction: 'row',
            gap: '3',
            style: { width: 'full' },
          },
          [
            infoCard('model', 'Model', 'openagents/khala'),
            infoCard('base-url', 'Base URL', 'https://openagents.com/api/v1'),
            infoCard('free-key', 'Free key', 'POST /api/keys/free'),
          ],
        ),
      ]),
      section('khala-counter', [
        Card(
          {
            key: 'khala-counter-card',
            padding: '4',
            radius: 'lg',
            style: {
              backgroundColor: 'surface',
              borderColor: 'border',
              borderWidth: 1,
              width: 'full',
            },
          },
          [
            Stack(
              {
                key: 'khala-counter-row',
                direction: 'row',
                align: 'center',
                gap: '3',
              },
              [
                text('khala-counter-label', 'Tokens Served', 'caption', 'textMuted'),
                text('khala-counter-value', state.tokensServed, 'title', 'textPrimary'),
              ],
            ),
            text(
              'khala-counter-copy',
              'The live counter is hydrated by the production API on the live app. This route preserves the same live projection for the route-by-route migration.',
              'body',
              'textMuted',
            ),
          ],
        ),
      ]),
      section('khala-actions', [
        Stack(
          {
            key: 'khala-actions-row',
            direction: 'row',
            gap: '3',
            style: { width: 'full' },
          },
          [
            navLink('khala-action-overview', 'Read the overview', '/docs/openagents'),
            navLink('khala-action-chat-sync', 'Open web chat sync', '/khala/chat-sync'),
          ],
        ),
      ]),
    ],
  )

const updatePublicSnapshot = (
  state: SubscriptionRef.SubscriptionRef<KhalaLandingState>,
) => {
  const fetchSnapshot = Effect.tryPromise({
    try: async () => khalaStateFromPublicSnapshot(await fetchKhalaTokensServed()),
    catch: (error) =>
      new KhalaPublicSnapshotError({
        message: error instanceof Error ? error.message : String(error),
      }),
  })

  return fetchSnapshot.pipe(Effect.flatMap((next) => SubscriptionRef.set(state, next)))
}

export const mountKhalaEffectNativeSurface = (container: HTMLElement) =>
  Effect.gen(function* () {
    const state = yield* SubscriptionRef.make(initialKhalaLandingState)
    const program = makeViewProgramFromState(state, khalaLandingView)
    const surface = yield* makeDomRenderer({
      theme: stage1EffectNativeTheme,
    }).mount(container, program.viewStream, () => Effect.void)

    yield* updatePublicSnapshot(state).pipe(Effect.catch(() => Effect.void))

    return { state, unmount: surface.unmount }
  })

export function KhalaEffectNativePage() {
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
        return Effect.runPromise(Scope.provide(scope)(mountKhalaEffectNativeSurface(root)))
      })
      .catch(() => undefined)

    return () => {
      disposed = true
      closeScope?.()
    }
  }, [])

  return (
    <main
      aria-label="Khala - OpenAgents inference"
      className="khala-effect-native-host"
      data-route="khala"
      data-khala-effect-native=""
    >
      <div ref={rootRef} data-khala-effect-native-root="" />
    </main>
  )
}
