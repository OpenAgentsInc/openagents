import { Scene } from 'foldkit'
import { describe, expect, test } from 'vitest'

import { LoggedOut } from '../../../model'
import { KhalaRoute, LandingRoute, TassadarRoute } from '../../../route'
import { update } from '../../../update'
import { view } from '../../../view'
import {
  PERSISTENT_SCENE_KEY,
  PERSISTENT_SCENE_OVERLAY_PREFIX,
  PERSISTENT_SCENE_SHELL_KEY,
  poseForRoute,
  view as persistentSceneView,
} from './persistentScene'
import { view as khalaView } from './khala'

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

const textContent = (node: SnabbVNode | string): string => {
  if (typeof node === 'string') {
    return node
  }

  return node.text ?? (node.children ?? []).map(textContent).join('')
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

    const poses = (
      ['Landing', 'Khala', 'Tassadar', 'Autopilot'] as const
    ).map(poseForRoute)
    expect(new Set(poses).size).toBe(poses.length)
    for (const pose of poses) {
      expect(pose.trim().length).toBeGreaterThan(0)
    }
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

  test('renders Khala over the same persistent scene surface', () => {
    Scene.scene(
      { update, view },
      Scene.with(LoggedOut.init(KhalaRoute())),
      Scene.expect(Scene.selector('oa-landing-squares')).toExist(),
      Scene.expect(
        Scene.selector('[data-persistent-scene-overlay="khala"]'),
      ).toExist(),
      Scene.expect(Scene.selector('[data-route="khala"]')).toExist(),
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

  test('keeps public Khala copy inside the promise gate', () => {
    const rendered = textContent(khalaView() as SnabbVNode)

    expect(rendered).toContain('The public Khala catalog uses two model ids')
    expect(rendered).toContain(
      'gateway only serves models whose underlying lane is armed and ready',
    )
    expect(rendered).toContain(
      'verified:true is reserved for an executed acceptance verdict',
    )
    expect(rendered).toContain(
      'Broad self-serve card, Bitcoin, and MPP funding stay behind receipt proof and owner activation',
    )
    expect(rendered).toContain(
      'the receipt records whether executable acceptance actually ran, failed, or remains unverified',
    )
    expect(rendered).not.toContain('Two models are live today')
    expect(rendered).not.toContain('Fund your account with a card')
    expect(rendered).not.toContain('Bitcoin carries a small discount')
    expect(rendered).not.toContain('for example, that tests passed')
  })
})
