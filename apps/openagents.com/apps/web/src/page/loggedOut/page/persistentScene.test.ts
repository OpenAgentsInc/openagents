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
  view as persistentSceneView,
} from './persistentScene'

type SnabbVNode = {
  readonly sel?: string
  readonly key?: string
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

  test('renders the landing scene with both glowing CTAs', () => {
    Scene.scene(
      { update, view },
      Scene.with(LoggedOut.init(LandingRoute())),
      Scene.expect(Scene.selector('oa-landing-squares')).toExist(),
      Scene.expect(
        Scene.selector('[data-persistent-scene-shell="landing"]'),
      ).toExist(),
      Scene.expect(Scene.selector('[data-route="landing"]')).toExist(),
      // CTA 1: renamed "What is Khala?" (still navigates to /khala).
      Scene.expect(Scene.selector('[data-landing-cta="khala"]')).toExist(),
      Scene.expect(Scene.selector('[data-landing-cta="khala"]')).toHaveAttr(
        'type',
        'button',
      ),
      Scene.expect(Scene.text('What is Khala?')).toExist(),
      // CTA 2: new "Join the Tassadar training run" (navigates to /tassadar).
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
})
