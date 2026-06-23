import { Scene } from 'foldkit'
import { describe, expect, test } from 'vitest'

import { LoggedOut } from '../../../model'
import { KhalaRoute, LandingRoute } from '../../../route'
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

  test('renders the landing scene with a client-side Khala link', () => {
    Scene.scene(
      { update, view },
      Scene.with(LoggedOut.init(LandingRoute())),
      Scene.expect(Scene.selector('oa-landing-squares')).toExist(),
      Scene.expect(
        Scene.selector('[data-persistent-scene-shell="landing"]'),
      ).toExist(),
      Scene.expect(Scene.selector('[data-route="landing"]')).toExist(),
      Scene.expect(Scene.selector('[data-landing-cta="khala"]')).toExist(),
      Scene.expect(Scene.selector('[data-landing-cta="khala"]')).toHaveAttr(
        'type',
        'button',
      ),
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
})
