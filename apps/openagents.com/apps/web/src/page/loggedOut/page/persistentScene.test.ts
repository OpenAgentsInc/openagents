import { Option } from 'effect'
import { Scene } from 'foldkit'
import { evo } from 'foldkit/struct'
import { describe, expect, test } from 'vitest'

import { GotLoggedOutMessage } from '../../../message'
import { LoggedOut } from '../../../model'
import {
  HomeRoute,
  KhalaChatRoute,
  KhalaRoute,
  LoginRoute,
  TassadarRoute,
} from '../../../route'
import { update } from '../../../update'
import { view } from '../../../view'
import {
  CompletedFocusKhalaChatComposer,
  CompletedScrollKhalaChatThread,
} from '../message'
import {
  LoadedPublicKhalaTokensServed,
  PublicKhalaTokensServed,
} from '../model'
import {
  FocusKhalaChatComposer,
  ScrollKhalaChatLatestTurnIntoView,
} from '../update'
import {
  PERSISTENT_SCENE_KEY,
  PERSISTENT_SCENE_OVERLAY_PREFIX,
  PERSISTENT_SCENE_SHELL_KEY,
  view as persistentSceneView,
  poseForRoute,
} from './persistentScene'

const githubViewerSession = Option.some({
  userId: 'github:1',
  email: 'viewer@example.com',
  name: 'Octo Viewer',
  avatarUrl: 'https://avatars.githubusercontent.com/u/1?v=4',
})

const monogramViewerSession = Option.some({
  userId: 'github:2',
  email: 'mono@example.com',
  name: 'Mono Gram',
})

type SnabbVNode = {
  readonly sel?: string
  readonly key?: string
  readonly text?: string
  readonly children?: ReadonlyArray<SnabbVNode | string>
}

const walk = (node: SnabbVNode, visit: (n: SnabbVNode) => void): void => {
  visit(node)
  for (const child of node.children ?? []) {
    if (typeof child !== 'string' && child != null) {
      walk(child as SnabbVNode, visit)
    }
  }
}

const findByKey = (root: SnabbVNode, key: string): SnabbVNode | undefined => {
  let found: SnabbVNode | undefined
  walk(root, n => {
    if (found === undefined && n.key === key) {
      found = n
    }
  })
  return found
}

const hasSelector = (root: SnabbVNode, sel: string): boolean => {
  let present = false
  walk(root, n => {
    if (n.sel !== undefined && n.sel.startsWith(sel)) {
      present = true
    }
  })
  return present
}

const allKeys = (root: SnabbVNode): ReadonlyArray<string> => {
  const keys: Array<string> = []
  walk(root, n => {
    if (n.key !== undefined) {
      keys.push(n.key)
    }
  })
  return keys
}

describe('persistent landing and Khala scene', () => {
  test('keeps the canvas wrapper key stable across landing and Khala', () => {
    const landing = persistentSceneView('Home') as SnabbVNode
    const khala = persistentSceneView('Khala') as SnabbVNode

    expect(findByKey(landing, PERSISTENT_SCENE_SHELL_KEY)).toBeDefined()
    expect(findByKey(khala, PERSISTENT_SCENE_SHELL_KEY)).toBeDefined()

    const landingCanvas = findByKey(landing, PERSISTENT_SCENE_KEY)
    const khalaCanvas = findByKey(khala, PERSISTENT_SCENE_KEY)

    expect(landingCanvas).toBeDefined()
    expect(khalaCanvas).toBeDefined()
    expect(landingCanvas?.sel).toBe(khalaCanvas?.sel)
    expect(landingCanvas?.key).toBe(khalaCanvas?.key)
    expect(hasSelector(landingCanvas as SnabbVNode, 'oa-landing-squares')).toBe(
      true,
    )
    expect(hasSelector(khalaCanvas as SnabbVNode, 'oa-landing-squares')).toBe(
      true,
    )
  })

  test('keeps the Chat route on the same canvas at the Khala pose', () => {
    const khala = persistentSceneView('Khala') as SnabbVNode
    const chat = persistentSceneView('KhalaChat') as SnabbVNode

    const khalaCanvas = findByKey(khala, PERSISTENT_SCENE_KEY)
    const chatCanvas = findByKey(chat, PERSISTENT_SCENE_KEY)

    expect(khalaCanvas).toBeDefined()
    expect(chatCanvas).toBeDefined()
    expect(khalaCanvas?.sel).toBe(chatCanvas?.sel)
    expect(khalaCanvas?.key).toBe(chatCanvas?.key)
    expect(poseForRoute('KhalaChat')).toBe('khala')
  })

  test('changes only the overlay key between landing and Khala', () => {
    const landing = persistentSceneView('Home') as SnabbVNode
    const khala = persistentSceneView('Khala') as SnabbVNode
    const landingOverlayKey = `${PERSISTENT_SCENE_OVERLAY_PREFIX}Home`
    const khalaOverlayKey = `${PERSISTENT_SCENE_OVERLAY_PREFIX}Khala`
    const stableKeys = (keys: ReadonlyArray<string>): ReadonlyArray<string> =>
      keys.filter(k => !k.startsWith(PERSISTENT_SCENE_OVERLAY_PREFIX))

    expect(findByKey(landing, landingOverlayKey)).toBeDefined()
    expect(findByKey(khala, khalaOverlayKey)).toBeDefined()
    expect(findByKey(landing, khalaOverlayKey)).toBeUndefined()
    expect(findByKey(khala, landingOverlayKey)).toBeUndefined()
    expect(stableKeys(allKeys(landing))).toEqual(stableKeys(allKeys(khala)))
  })

  test('keeps the canvas wrapper key stable for the Tassadar pose too', () => {
    const landing = persistentSceneView('Home') as SnabbVNode
    const tassadar = persistentSceneView('Tassadar') as SnabbVNode

    const landingCanvas = findByKey(landing, PERSISTENT_SCENE_KEY)
    const tassadarCanvas = findByKey(tassadar, PERSISTENT_SCENE_KEY)

    expect(landingCanvas).toBeDefined()
    expect(tassadarCanvas).toBeDefined()
    expect(landingCanvas?.sel).toBe(tassadarCanvas?.sel)
    expect(landingCanvas?.key).toBe(tassadarCanvas?.key)
    expect(
      hasSelector(tassadarCanvas as SnabbVNode, 'oa-landing-squares'),
    ).toBe(true)
  })

  test('maps each route to its distinct, non-blank camera pose', () => {
    expect(poseForRoute('Home')).toBe('landing')
    expect(poseForRoute('Khala')).toBe('khala')
    expect(poseForRoute('KhalaChat')).toBe('khala')
    expect(poseForRoute('Tassadar')).toBe('tassadar')
    // The /autopilot onboarding route reuses the SAME persistent scene with
    // its own camera pose (#6125).
    expect(poseForRoute('Autopilot')).toBe('autopilot')
    // /login reuses the SAME persistent scene with its own login vantage.
    expect(poseForRoute('Login')).toBe('login')

    const poses = (
      ['Home', 'Khala', 'Tassadar', 'Autopilot', 'Login'] as const
    ).map(poseForRoute)
    expect(new Set(poses).size).toBe(poses.length)
    for (const pose of poses) {
      expect(pose.trim().length).toBeGreaterThan(0)
    }
  })

  test('hosts the Login pose on the same persistent canvas (no second scene)', () => {
    const landing = persistentSceneView('Home') as SnabbVNode
    const login = persistentSceneView('Login') as SnabbVNode

    const landingCanvas = findByKey(landing, PERSISTENT_SCENE_KEY)
    const loginCanvas = findByKey(login, PERSISTENT_SCENE_KEY)

    expect(landingCanvas).toBeDefined()
    expect(loginCanvas).toBeDefined()
    // Same keyed canvas wrapper => one persistent scene instance, not two.
    expect(landingCanvas?.sel).toBe(loginCanvas?.sel)
    expect(landingCanvas?.key).toBe(loginCanvas?.key)
    expect(hasSelector(loginCanvas as SnabbVNode, 'oa-landing-squares')).toBe(
      true,
    )

    // Only the overlay key differs between routes; every other key is stable, so
    // navigating /  <-> /login eases the camera through ONE scene (no recreate).
    const stableKeys = (keys: ReadonlyArray<string>): ReadonlyArray<string> =>
      keys.filter(k => !k.startsWith(PERSISTENT_SCENE_OVERLAY_PREFIX))
    expect(stableKeys(allKeys(landing))).toEqual(stableKeys(allKeys(login)))
  })

  test('hosts the Autopilot pose on the same persistent canvas (no second scene)', () => {
    const landing = persistentSceneView('Home') as SnabbVNode
    const autopilot = persistentSceneView('Autopilot') as SnabbVNode

    const landingCanvas = findByKey(landing, PERSISTENT_SCENE_KEY)
    const autopilotCanvas = findByKey(autopilot, PERSISTENT_SCENE_KEY)

    expect(landingCanvas).toBeDefined()
    expect(autopilotCanvas).toBeDefined()
    // Same keyed canvas wrapper => one persistent scene instance, not two.
    expect(landingCanvas?.sel).toBe(autopilotCanvas?.sel)
    expect(landingCanvas?.key).toBe(autopilotCanvas?.key)
    expect(
      hasSelector(autopilotCanvas as SnabbVNode, 'oa-landing-squares'),
    ).toBe(true)

    // Only the overlay key differs between routes; every other key is stable.
    const stableKeys = (keys: ReadonlyArray<string>): ReadonlyArray<string> =>
      keys.filter(k => !k.startsWith(PERSISTENT_SCENE_OVERLAY_PREFIX))
    expect(stableKeys(allKeys(landing))).toEqual(stableKeys(allKeys(autopilot)))
  })

  test('passes the autopilot pose + overlay through the shared shell', () => {
    // The /autopilot route wiring lands in #6124/#6129; here we verify the
    // persistentScene mapping SUPPORTS the Autopilot pose directly off the view.
    const autopilot = persistentSceneView('Autopilot') as SnabbVNode

    const hasAttr = (
      root: SnabbVNode,
      attr: string,
      value: string,
    ): boolean => {
      let present = false
      walk(root, n => {
        const data = (n as { data?: { attrs?: Record<string, unknown> } }).data
        if (data?.attrs?.[attr] === value) {
          present = true
        }
      })
      return present
    }

    expect(hasSelector(autopilot, 'oa-landing-squares')).toBe(true)
    expect(hasAttr(autopilot, 'data-pose', 'autopilot')).toBe(true)
    expect(hasAttr(autopilot, 'data-route', 'autopilot')).toBe(true)
    expect(
      hasAttr(autopilot, 'data-persistent-scene-overlay', 'autopilot'),
    ).toBe(true)
  })

  test('renders the landing scene with the Khala + Tassadar CTAs', () => {
    Scene.scene(
      { update, view },
      Scene.with(LoggedOut.init(HomeRoute())),
      Scene.expect(Scene.selector('oa-landing-squares')).toExist(),
      Scene.expect(
        Scene.selector('[data-persistent-scene-shell="landing"]'),
      ).toExist(),
      Scene.expect(Scene.selector('[data-route="landing"]')).toExist(),
      // CTA: "What is Khala?" (navigates to /khala — the flagship, re-enabled).
      Scene.expect(Scene.selector('[data-landing-cta="khala"]')).toExist(),
      Scene.expect(Scene.selector('[data-landing-cta="khala"]')).toHaveAttr(
        'type',
        'button',
      ),
      Scene.expect(Scene.text('What is Khala?')).toExist(),
      // CTA: "Join the Tassadar training run" (navigates to /tassadar).
      Scene.expect(Scene.selector('[data-landing-cta="tassadar"]')).toExist(),
      Scene.expect(Scene.selector('[data-landing-cta="tassadar"]')).toHaveAttr(
        'type',
        'button',
      ),
      Scene.expect(Scene.text('Join the Tassadar training run')).toExist(),
      // The old neutral copy is gone.
      Scene.expect(Scene.text('Enter Khala')).not.toExist(),
    )
  })

  test('renders the Khala API-instructions panel over the same persistent scene', () => {
    Scene.scene(
      { update, view },
      Scene.with(LoggedOut.init(KhalaRoute())),
      Scene.expect(Scene.selector('oa-landing-squares')).toExist(),
      // The instructions panel mounts over the dimmed scene. The generic chat box
      // is intentionally NOT shown yet (not ready).
      Scene.expect(Scene.selector('[data-khala-instructions]')).toExist(),
      Scene.expect(Scene.selector('[data-khala-chat]')).not.toExist(),
      Scene.expect(Scene.selector('[data-khala-chat-composer]')).not.toExist(),
      // The panel carries the API basics: single model + base URL + free token.
      Scene.expect(Scene.text('openagents/khala')).toExist(),
      Scene.expect(Scene.text('https://openagents.com/api/v1')).toExist(),
      Scene.expect(Scene.text('POST /api/keys/free')).toExist(),
      // ...and the live "Tokens Served" counter (#6227).
      Scene.expect(
        Scene.selector('[data-counter="khala-tokens-served"]'),
      ).toExist(),
      Scene.expect(Scene.text('Tokens Served')).toExist(),
      // Back button mirrors /tassadar: same "← OpenAgents" home control.
      Scene.expect(Scene.selector('[data-khala-back="home"]')).toExist(),
      Scene.expect(
        Scene.role('button', { name: 'Back to OpenAgents home' }),
      ).toExist(),
    )
  })

  test('renders /chat as only the bottom Khala chat box over the persistent scene', () => {
    Scene.scene(
      { update, view },
      Scene.with(LoggedOut.init(KhalaChatRoute())),
      Scene.expect(Scene.selector('oa-landing-squares')).toExist(),
      Scene.expect(Scene.selector('[data-pose="khala"]')).toExist(),
      Scene.expect(
        Scene.selector('[data-persistent-scene-overlay="chat"]'),
      ).toExist(),
      Scene.expect(Scene.selector('[data-khala-chat]')).toExist(),
      Scene.expect(Scene.selector('[data-khala-chat-composer]')).toExist(),
      Scene.expect(
        Scene.selector('[data-khala-chat-info-trigger]'),
      ).not.toExist(),
      Scene.expect(Scene.selector('[data-khala-instructions]')).not.toExist(),
      Scene.expect(Scene.selector('[data-khala-back="home"]')).not.toExist(),
      Scene.expect(Scene.text('Ask Khala what it can do.')).not.toExist(),
    )
  })

  test('submits the /chat composer on Enter while leaving Shift+Enter to the textarea', () => {
    Scene.scene(
      { update, view },
      Scene.with(LoggedOut.init(KhalaChatRoute())),
      Scene.type(Scene.label('Message Khala'), 'hello'),
      Scene.keydown(Scene.label('Message Khala'), 'Enter'),
      Scene.expect(Scene.text('hello')).toExist(),
      Scene.expect(Scene.label('Message Khala')).toHaveValue(''),
      Scene.Command.resolveAll(
        [
          ScrollKhalaChatLatestTurnIntoView,
          CompletedScrollKhalaChatThread(),
          message => GotLoggedOutMessage({ message }),
        ],
        [
          FocusKhalaChatComposer,
          CompletedFocusKhalaChatComposer(),
          message => GotLoggedOutMessage({ message }),
        ],
      ),
    )

    Scene.scene(
      { update, view },
      Scene.with(LoggedOut.init(KhalaChatRoute())),
      Scene.type(Scene.label('Message Khala'), 'Line one'),
      Scene.keydown(Scene.label('Message Khala'), 'Enter', {
        shiftKey: true,
      }),
      Scene.expect(
        Scene.selector('[data-khala-chat-transcript]'),
      ).not.toExist(),
      Scene.expect(Scene.label('Message Khala')).toHaveValue('Line one'),
    )
  })

  test('expands the /chat composer to ten rows before scrolling', () => {
    const tallPrompt = [
      'Line one',
      'Line two',
      'Line three',
      'Line four',
      'Line five',
      'Line six',
      'Line seven',
      'Line eight',
      'Line nine',
      'Line ten',
      'Line eleven',
    ].join('\n')

    Scene.scene(
      { update, view },
      Scene.with(LoggedOut.init(KhalaChatRoute())),
      Scene.type(Scene.label('Message Khala'), tallPrompt),
      Scene.expect(Scene.label('Message Khala')).toHaveAttr('rows', '10'),
    )
  })

  test('renders /login as the sign-in card over the same persistent scene', () => {
    Scene.scene(
      { update, view },
      Scene.with(LoggedOut.init(LoginRoute())),
      // The ONE persistent scene canvas is present (not a second/isolated one).
      Scene.expect(Scene.selector('oa-landing-squares')).toExist(),
      // The login card mounts as this route's overlay, at the login pose.
      Scene.expect(
        Scene.selector('[data-persistent-scene-overlay="login"]'),
      ).toExist(),
      Scene.expect(Scene.selector('[data-pose="login"]')).toExist(),
      // The flush public header (no separate isolated scene) is present.
      Scene.expect(Scene.role('link', { name: 'Homepage' })).toExist(),
      // The login card content + form are preserved verbatim.
      Scene.expect(
        Scene.role('heading', { name: 'Log in to OpenAgents' }),
      ).toExist(),
      Scene.expect(
        Scene.text(
          'Enter your email and we’ll send a one-time sign-in code, or continue with GitHub.',
        ),
      ).toExist(),
      Scene.expect(Scene.text('Email me a code')).toExist(),
      Scene.expect(
        Scene.role('link', { name: 'Continue with GitHub' }),
      ).toHaveAttr('href', '/login/github'),
    )
  })

  test('renders Tassadar as a third pose over the same persistent scene', () => {
    Scene.scene(
      { update, view },
      Scene.with(LoggedOut.init(TassadarRoute())),
      Scene.expect(Scene.selector('oa-landing-squares')).toExist(),
      Scene.expect(
        Scene.selector('[data-persistent-scene-overlay="tassadar"]'),
      ).toExist(),
      Scene.expect(Scene.selector('[data-route="tassadar"]')).toExist(),
      // The continuous-flight camera pose is passed to the canvas.
      Scene.expect(Scene.selector('[data-pose="tassadar"]')).toExist(),
      // The public Tassadar info content + the Copy Agent Instructions button.
      Scene.expect(Scene.text('Tassadar')).toExist(),
      Scene.expect(
        Scene.selector('[data-tassadar-copy="agent-instructions"]'),
      ).toExist(),
      Scene.expect(Scene.text('Copy Agent Instructions')).toExist(),
      // Back button: the shared "← OpenAgents" home control.
      Scene.expect(Scene.selector('[data-tassadar-back="home"]')).toExist(),
      Scene.expect(
        Scene.role('button', { name: 'Back to OpenAgents home' }),
      ).toExist(),
    )
  })

  // Floating signed-in avatar on the chrome-less homepage hero.
  test('keeps the homepage hero clean for a logged-out viewer (no floating avatar)', () => {
    Scene.scene(
      { update, view },
      Scene.with(LoggedOut.init(HomeRoute())),
      // The hero (wordmark + Tassadar CTA) renders, but nothing floats over it.
      Scene.expect(
        Scene.selector('[data-landing-wordmark="openagents"]'),
      ).toExist(),
      Scene.expect(
        Scene.selector('[data-landing-floating-avatar="viewer"]'),
      ).not.toExist(),
      Scene.expect(Scene.selector('[data-account-menu-trigger]')).not.toExist(),
    )
  })

  test('floats the shared avatar menu top-right when signed in, with the same Log out wire', () => {
    Scene.scene(
      { update, view },
      Scene.with(LoggedOut.init(HomeRoute(), githubViewerSession)),
      // The floating control mounts over the hero (no header bar / nav).
      Scene.expect(
        Scene.selector('[data-landing-floating-avatar="viewer"]'),
      ).toExist(),
      Scene.expect(Scene.selector('[data-account-menu-trigger]')).toExist(),
      // No header bar / global nav on the chrome-less hero.
      Scene.expect(Scene.role('link', { name: 'Homepage' })).not.toExist(),
      // The native <details> dropdown carries the same menu the header has:
      // identity, the same links, and the same Log out menuitem.
      Scene.expect(Scene.selector('[data-account-menu]')).toExist(),
      Scene.expect(Scene.text('Octo Viewer')).toExist(),
      Scene.expect(Scene.text('viewer@example.com')).toExist(),
      Scene.expect(Scene.role('menuitem', { name: 'Workroom' })).toExist(),
      Scene.expect(Scene.role('menuitem', { name: 'Settings' })).toExist(),
      Scene.expect(Scene.selector('[data-account-menu-logout]')).toExist(),
      Scene.expect(Scene.role('menuitem', { name: 'Log out' })).toExist(),
    )
  })

  test('uses the GitHub avatar image when the signed-in viewer has one', () => {
    Scene.scene(
      { update, view },
      Scene.with(LoggedOut.init(HomeRoute(), githubViewerSession)),
      Scene.expect(
        Scene.selector('[src="https://avatars.githubusercontent.com/u/1?v=4"]'),
      ).toExist(),
    )
  })

  test('falls back to the monogram when the signed-in viewer has no avatar', () => {
    Scene.scene(
      { update, view },
      Scene.with(LoggedOut.init(HomeRoute(), monogramViewerSession)),
      Scene.expect(
        Scene.selector('[data-landing-floating-avatar="viewer"]'),
      ).toExist(),
      // No avatar image; the monogram initials stand in (Mono Gram -> MG).
      Scene.expect(Scene.text('MG')).toExist(),
    )
  })

  // The live "Tokens Served" pill occupies the top-left slot on the
  // homepage (#6273 follow-up). It reads the SAME live tokens-served model that
  // powers the /khala counter, mirrors the back-button styling, and links to
  // /stats. The back button (the slot's child-route occupant) is absent on /.
  const landingWithTokens = (tokensServed: number) =>
    evo(LoggedOut.init(HomeRoute()), {
      publicKhalaTokensServed: () =>
        LoadedPublicKhalaTokensServed({
          served: PublicKhalaTokensServed.make({
            tokensServed,
            generatedAt: '2026-06-24T12:00:00.000Z',
          }),
        }),
    })

  test('shows the live "Tokens Served" pill in the top-left slot on the homepage', () => {
    Scene.scene(
      { update, view },
      Scene.with(landingWithTokens(1_250_000)),
      // The homepage hero is present (the pill mounts over the same scene).
      Scene.expect(
        Scene.selector('[data-landing-wordmark="openagents"]'),
      ).toExist(),
      // The pill is in the top-left slot, reading the fixed label + live total
      // with thousands separators (the same formatter the /khala counter uses).
      Scene.expect(
        Scene.selector('[data-landing-khala-tokens-pill="home"]'),
      ).toExist(),
      Scene.expect(Scene.text('Tokens Served:')).toExist(),
      Scene.expect(Scene.text('1,250,000')).toExist(),
      // It is an accessible, keyboard-activatable link to /stats, with the same
      // pointer cursor affordance as the center homepage buttons.
      Scene.expect(
        Scene.role('link', { name: 'Tokens served — open stats' }),
      ).toHaveAttr('href', '/stats'),
      Scene.expect(
        Scene.role('link', { name: 'Tokens served — open stats' }),
      ).toHaveClass('cursor-pointer'),
      // The back button (the child-route occupant of the same slot) is NOT on /.
      Scene.expect(Scene.selector('[data-khala-back="home"]')).not.toExist(),
      Scene.expect(Scene.selector('[data-tassadar-back="home"]')).not.toExist(),
    )
  })

  test('renders the em-dash placeholder in the pill before the live total loads', () => {
    Scene.scene(
      { update, view },
      Scene.with(LoggedOut.init(HomeRoute())),
      Scene.expect(
        Scene.selector('[data-landing-khala-tokens-pill="home"]'),
      ).toExist(),
      Scene.expect(Scene.text('Tokens Served:')).toExist(),
      Scene.expect(Scene.text('—')).toExist(),
    )
  })

  test('child routes (/khala, /tassadar) show the back button in the slot, not the pill', () => {
    Scene.scene(
      { update, view },
      Scene.with(LoggedOut.init(KhalaRoute())),
      Scene.expect(Scene.selector('[data-khala-back="home"]')).toExist(),
      Scene.expect(
        Scene.selector('[data-landing-khala-tokens-pill="home"]'),
      ).not.toExist(),
    )
    Scene.scene(
      { update, view },
      Scene.with(LoggedOut.init(TassadarRoute())),
      Scene.expect(Scene.selector('[data-tassadar-back="home"]')).toExist(),
      Scene.expect(
        Scene.selector('[data-landing-khala-tokens-pill="home"]'),
      ).not.toExist(),
    )
  })

})
