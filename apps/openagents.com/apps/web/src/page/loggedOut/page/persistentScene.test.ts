import { Option } from 'effect'
import { Scene } from 'foldkit'
import { describe, expect, test } from 'vitest'

import { LoggedOut } from '../../../model'
import {
  KhalaRoute,
  LandingRoute,
  LoginRoute,
  TassadarRoute,
} from '../../../route'

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
import { update } from '../../../update'
import { view } from '../../../view'
import {
  PERSISTENT_SCENE_KEY,
  PERSISTENT_SCENE_OVERLAY_PREFIX,
  PERSISTENT_SCENE_SHELL_KEY,
  poseForRoute,
  view as persistentSceneView,
} from './persistentScene'

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
    const landing = persistentSceneView('Landing') as SnabbVNode
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

  test('changes only the overlay key between landing and Khala', () => {
    const landing = persistentSceneView('Landing') as SnabbVNode
    const khala = persistentSceneView('Khala') as SnabbVNode
    const landingOverlayKey = `${PERSISTENT_SCENE_OVERLAY_PREFIX}Landing`
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
    const landing = persistentSceneView('Landing') as SnabbVNode
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
    expect(poseForRoute('Landing')).toBe('landing')
    expect(poseForRoute('Khala')).toBe('khala')
    expect(poseForRoute('Tassadar')).toBe('tassadar')
    // The /autopilot onboarding route reuses the SAME persistent scene with
    // its own camera pose (#6125).
    expect(poseForRoute('Autopilot')).toBe('autopilot')
    // /login reuses the SAME persistent scene with its own login vantage.
    expect(poseForRoute('Login')).toBe('login')

    const poses = (
      ['Landing', 'Khala', 'Tassadar', 'Autopilot', 'Login'] as const
    ).map(poseForRoute)
    expect(new Set(poses).size).toBe(poses.length)
    for (const pose of poses) {
      expect(pose.trim().length).toBeGreaterThan(0)
    }
  })

  test('hosts the Login pose on the same persistent canvas (no second scene)', () => {
    const landing = persistentSceneView('Landing') as SnabbVNode
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
    const landing = persistentSceneView('Landing') as SnabbVNode
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

    const hasAttr = (root: SnabbVNode, attr: string, value: string): boolean => {
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

  test('renders the landing scene with the Tassadar CTA (Khala CTA hidden)', () => {
    Scene.scene(
      { update, view },
      Scene.with(LoggedOut.init(LandingRoute())),
      Scene.expect(Scene.selector('oa-landing-squares')).toExist(),
      Scene.expect(
        Scene.selector('[data-persistent-scene-shell="landing"]'),
      ).toExist(),
      Scene.expect(Scene.selector('[data-route="landing"]')).toExist(),
      // The "What is Khala?" CTA is hidden until Khala is fully live.
      Scene.expect(Scene.selector('[data-landing-cta="khala"]')).not.toExist(),
      Scene.expect(Scene.text('What is Khala?')).not.toExist(),
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

  test('renders the Khala chat box over the same persistent scene surface', () => {
    Scene.scene(
      { update, view },
      Scene.with(LoggedOut.init(KhalaRoute())),
      Scene.expect(Scene.selector('oa-landing-squares')).toExist(),
      // The chat box (composer + thread) and the info trigger mount over the
      // dimmed scene; the long-form explainer is gone (condensed into the popup).
      Scene.expect(Scene.selector('[data-khala-chat]')).toExist(),
      Scene.expect(Scene.selector('[data-khala-chat-composer]')).toExist(),
      Scene.expect(Scene.selector('[data-khala-chat-transcript]')).toExist(),
      Scene.expect(Scene.selector('[data-khala-chat-info-trigger]')).toExist(),
      Scene.expect(Scene.text('Ask Khala what it can do.')).toExist(),
      // The removed explainer sections (e.g. "01 What is Khala") are no longer
      // rendered inline on the page.
      Scene.expect(Scene.text('What is Khala')).not.toExist(),
      Scene.expect(
        Scene.text('The public Khala catalog uses two model ids'),
      ).not.toExist(),
    )
  })

  test('opens and closes the "What is Khala?" info popup', () => {
    Scene.scene(
      { update, view },
      Scene.with(LoggedOut.init(KhalaRoute())),
      // The info popup is closed on first paint.
      Scene.expect(Scene.selector('[data-khala-chat-info-dialog]')).not.toExist(),
      // Clicking the info trigger opens the popup.
      Scene.click(Scene.selector('[data-khala-chat-info-trigger]')),
      Scene.expect(Scene.selector('[data-khala-chat-info-dialog]')).toExist(),
      Scene.expect(Scene.text('What is Khala?')).toExist(),
      // The popup carries the condensed, truthful Khala basics.
      Scene.expect(Scene.text('openagents/khala-mini')).toExist(),
      Scene.expect(Scene.text('https://openagents.com/api/v1')).toExist(),
      // Clicking Close dismisses it.
      Scene.click(Scene.selector('[aria-label="Close"]')),
      Scene.expect(Scene.selector('[data-khala-chat-info-dialog]')).not.toExist(),
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
    )
  })

  // Floating signed-in avatar on the chrome-less homepage hero.
  test('keeps the homepage hero clean for a logged-out viewer (no floating avatar)', () => {
    Scene.scene(
      { update, view },
      Scene.with(LoggedOut.init(LandingRoute())),
      // The hero (wordmark + Tassadar CTA) renders, but nothing floats over it.
      Scene.expect(
        Scene.selector('[data-landing-wordmark="openagents"]'),
      ).toExist(),
      Scene.expect(
        Scene.selector('[data-landing-floating-avatar="viewer"]'),
      ).not.toExist(),
      Scene.expect(
        Scene.selector('[data-account-menu-trigger]'),
      ).not.toExist(),
    )
  })

  test('floats the shared avatar menu top-right when signed in, with the same Log out wire', () => {
    Scene.scene(
      { update, view },
      Scene.with(LoggedOut.init(LandingRoute(), githubViewerSession)),
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
      Scene.expect(
        Scene.selector('[data-account-menu-logout]'),
      ).toExist(),
      Scene.expect(Scene.role('menuitem', { name: 'Log out' })).toExist(),
    )
  })

  test('uses the GitHub avatar image when the signed-in viewer has one', () => {
    Scene.scene(
      { update, view },
      Scene.with(LoggedOut.init(LandingRoute(), githubViewerSession)),
      Scene.expect(
        Scene.selector(
          '[src="https://avatars.githubusercontent.com/u/1?v=4"]',
        ),
      ).toExist(),
    )
  })

  test('falls back to the monogram when the signed-in viewer has no avatar', () => {
    Scene.scene(
      { update, view },
      Scene.with(LoggedOut.init(LandingRoute(), monogramViewerSession)),
      Scene.expect(
        Scene.selector('[data-landing-floating-avatar="viewer"]'),
      ).toExist(),
      // No avatar image; the monogram initials stand in (Mono Gram -> MG).
      Scene.expect(Scene.text('MG')).toExist(),
    )
  })

})
