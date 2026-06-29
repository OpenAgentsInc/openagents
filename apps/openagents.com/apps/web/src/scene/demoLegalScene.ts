import {
  htmlOverlayStyle,
  isPointBehindCamera,
} from '@openagentsinc/three-effect/core'
import { define as defineCustomElement } from 'foldkit/customElement'
import type { Attribute, Html } from 'foldkit/html'
import * as Three from 'three'

import { seededUnit } from './animations/element'

// Backdrop + html-overlay driver for the /demo/legal landing page.
//
// This is the "drei <Html>" showcase: a subtle three-effect-style constellation
// scene renders behind the page (transparent canvas, pointer-none), and the
// legal MVP cards — real Foldkit DOM siblings of this element, tagged with
// `data-anchor="<id>"` — are anchored to 3D positions and projected to the
// screen every frame with the shared three-effect htmlOverlay primitives
// (projectWorldToScreen / htmlDistanceScale / zIndexRange via htmlOverlayStyle,
// isPointBehindCamera for occlusion). The cards track their anchors as the
// scene slowly drifts.
//
// "Flutter in": each card has a deterministic, seeded entrance offset (opacity +
// translate + scale + slight rotation) driven by the scene clock, so the surface
// assembles itself as the page loads. Determinism uses seededUnit (no Math.random
// / time-of-day), matching scene/animations/element.ts.

export const demoLegalTagName = 'oa-demo-legal-scene'

const NODE_COUNT = 64
const LINK_DISTANCE = 1.7
const BOUND = 3.4

// World-space anchor for each card by its data-anchor id. Spread across the
// frame so the projected screen positions fan out into a workbench layout.
const CARD_ANCHORS: ReadonlyArray<
  Readonly<{ id: string; position: readonly [number, number, number] }>
> = [
  // command-bar lowered (was y=2.1) so it clears the top-center intro headline
  // band; quick-actions follows it down to keep the vertical gap.
  { id: 'command-bar', position: [0, 1.3, 0.4] },
  { id: 'quick-actions', position: [0, 0.2, 0.4] },
  { id: 'nda-draft', position: [-2.7, -0.2, 0.2] },
  { id: 'review-checklist', position: [2.7, -0.1, 0.2] },
  { id: 'time-entry', position: [-2.4, -2.4, 0.0] },
  { id: 'matter-workspace', position: [2.5, -2.3, 0.0] },
  { id: 'daily-brief', position: [0, -2.9, 0.0] },
]

type SceneHandle = Readonly<{ dispose: () => void }>

const mountDemoLegalScene = (
  host: HTMLElement,
  mount: HTMLElement,
): SceneHandle => {
  mount.style.position = 'absolute'
  mount.style.inset = '0'
  mount.style.overflow = 'hidden'

  const canvas = document.createElement('canvas')
  canvas.style.cssText =
    'position:absolute;inset:0;width:100%;height:100%;display:block'
  mount.append(canvas)

  const renderer = new Three.WebGLRenderer({
    alpha: true,
    antialias: true,
    canvas,
  })
  renderer.setClearColor(0x000000, 0)

  const scene = new Three.Scene()
  const camera = new Three.PerspectiveCamera(52, 1, 0.1, 100)
  camera.position.z = 8.5

  // Drifting node/edge constellation (constellation.ts aesthetic).
  const positions = new Float32Array(NODE_COUNT * 3)
  const velocities = new Float32Array(NODE_COUNT * 3)
  for (let i = 0; i < NODE_COUNT; i += 1) {
    positions[i * 3] = (seededUnit(i, 0) - 0.5) * BOUND * 2
    positions[i * 3 + 1] = (seededUnit(i, 1) - 0.5) * BOUND * 2
    positions[i * 3 + 2] = (seededUnit(i, 2) - 0.5) * BOUND * 2
    velocities[i * 3] = (seededUnit(i, 3) - 0.5) * 0.32
    velocities[i * 3 + 1] = (seededUnit(i, 4) - 0.5) * 0.32
    velocities[i * 3 + 2] = (seededUnit(i, 5) - 0.5) * 0.32
  }

  const nodeGeometry = new Three.BufferGeometry()
  const nodeAttr = new Three.BufferAttribute(positions, 3)
  nodeGeometry.setAttribute('position', nodeAttr)
  const nodeMaterial = new Three.PointsMaterial({
    blending: Three.AdditiveBlending,
    color: 0xbfe3ff,
    opacity: 0.85,
    size: 0.085,
    transparent: true,
  })
  const group = new Three.Group()
  group.add(new Three.Points(nodeGeometry, nodeMaterial))

  const maxEdges = (NODE_COUNT * (NODE_COUNT - 1)) / 2
  const edgePositions = new Float32Array(maxEdges * 6)
  const edgeGeometry = new Three.BufferGeometry()
  const edgeAttr = new Three.BufferAttribute(edgePositions, 3)
  edgeGeometry.setAttribute('position', edgeAttr)
  const edgeMaterial = new Three.LineBasicMaterial({
    blending: Three.AdditiveBlending,
    color: 0x2f6dd0,
    opacity: 0.28,
    transparent: true,
  })
  group.add(new Three.LineSegments(edgeGeometry, edgeMaterial))
  scene.add(group)

  const size = (): { height: number; width: number } => {
    const rect = mount.getBoundingClientRect()
    return {
      height: Math.max(1, Math.floor(rect.height || mount.clientHeight || 600)),
      width: Math.max(1, Math.floor(rect.width || mount.clientWidth || 960)),
    }
  }

  const resize = (): void => {
    const { height, width } = size()
    renderer.setPixelRatio(Math.min(window.devicePixelRatio || 1, 2))
    renderer.setSize(width, height, false)
    camera.aspect = width / height
    camera.updateProjectionMatrix()
  }

  // Resolve the card DOM nodes once; they are light-DOM siblings inside the
  // shared positioned overlay container.
  type AnchoredCard = Readonly<{
    element: HTMLElement
    target: Three.Vector3
    flutter: Readonly<{ delay: number; dx: number; dy: number; rot: number }>
  }>

  const resolveCards = (): ReadonlyArray<AnchoredCard> => {
    const overlay = host.parentElement
    if (overlay === null) return []
    return CARD_ANCHORS.flatMap((anchor, index) => {
      const element = overlay.querySelector<HTMLElement>(
        `[data-anchor="${anchor.id}"]`,
      )
      if (element === null) return []
      // Seeded, deterministic flutter-in parameters per card.
      return [
        {
          element,
          target: new Three.Vector3(
            anchor.position[0],
            anchor.position[1],
            anchor.position[2],
          ),
          flutter: {
            delay: 0.12 + seededUnit(index, 11) * 0.9,
            dx: (seededUnit(index, 12) - 0.5) * 46,
            dy: 28 + seededUnit(index, 13) * 40,
            rot: (seededUnit(index, 14) - 0.5) * 10,
          },
        },
      ]
    })
  }

  let cards = resolveCards()

  const easeOutCubic = (t: number): number => 1 - (1 - t) * (1 - t) * (1 - t)

  const positionCards = (elapsed: number): void => {
    const dims = size()
    for (const card of cards) {
      // Flutter-in progress driven by the scene clock (deterministic).
      const local = elapsed - card.flutter.delay
      const raw = local <= 0 ? 0 : local / 0.95
      const progress = raw >= 1 ? 1 : easeOutCubic(raw)
      const entranceOpacity = progress
      const offsetX = card.flutter.dx * (1 - progress)
      const offsetY = card.flutter.dy * (1 - progress)
      const rot = card.flutter.rot * (1 - progress)
      const entranceScale = 0.94 + 0.06 * progress

      const behind = isPointBehindCamera(card.target, camera)
      const base = htmlOverlayStyle(card.target, camera, dims, {
        center: true,
        distanceFactor: 9,
        zIndexRange: [40, 10],
      })

      // Compose the projected anchor transform with the flutter-in offset.
      const projectedTransform = base.transform ?? 'translate3d(0,0,0)'
      card.element.style.position = 'absolute'
      card.element.style.top = base.top ?? '0px'
      card.element.style.left = base.left ?? '0px'
      card.element.style.transformOrigin = '50% 50%'
      card.element.style.transform = `${projectedTransform} translate(${offsetX}px, ${offsetY}px) rotate(${rot}deg) scale(${entranceScale})`
      card.element.style.zIndex = base.zIndex ?? '20'
      card.element.style.opacity = behind ? '0' : String(entranceOpacity)
      card.element.style.pointerEvents =
        behind || progress < 0.35 ? 'none' : 'auto'
      card.element.style.display = base.display === 'none' ? 'none' : 'block'
    }
  }

  let disposed = false
  let frame = 0
  let start = 0
  let last = 0
  let recheckedCards = false

  const tick = (time: number): void => {
    if (disposed) return
    if (start === 0) start = time
    const elapsed = (time - start) / 1000
    const dt = last === 0 ? 0.016 : Math.min(0.05, (time - last) / 1000)
    last = time

    // Cards may mount a frame after the canvas; re-resolve once if empty.
    if (!recheckedCards && cards.length < CARD_ANCHORS.length) {
      cards = resolveCards()
      if (cards.length >= CARD_ANCHORS.length) recheckedCards = true
    }

    for (let i = 0; i < NODE_COUNT; i += 1) {
      for (let axis = 0; axis < 3; axis += 1) {
        const k = i * 3 + axis
        const v = velocities[k] ?? 0
        const next = (positions[k] ?? 0) + v * dt
        positions[k] = next
        if (next > BOUND || next < -BOUND) velocities[k] = -v
      }
    }
    nodeAttr.needsUpdate = true

    let edgeCount = 0
    for (let a = 0; a < NODE_COUNT; a += 1) {
      const ax = positions[a * 3] ?? 0
      const ay = positions[a * 3 + 1] ?? 0
      const az = positions[a * 3 + 2] ?? 0
      for (let b = a + 1; b < NODE_COUNT; b += 1) {
        const bx = positions[b * 3] ?? 0
        const by = positions[b * 3 + 1] ?? 0
        const bz = positions[b * 3 + 2] ?? 0
        const dx = ax - bx
        const dy = ay - by
        const dz = az - bz
        if (dx * dx + dy * dy + dz * dz < LINK_DISTANCE * LINK_DISTANCE) {
          const o = edgeCount * 6
          edgePositions[o] = ax
          edgePositions[o + 1] = ay
          edgePositions[o + 2] = az
          edgePositions[o + 3] = bx
          edgePositions[o + 4] = by
          edgePositions[o + 5] = bz
          edgeCount += 1
        }
      }
    }
    edgeGeometry.setDrawRange(0, edgeCount * 2)
    edgeAttr.needsUpdate = true

    // Gentle parallax drift so the anchored cards visibly track the scene.
    group.rotation.y = Math.sin(elapsed * 0.12) * 0.16
    group.rotation.x = Math.sin(elapsed * 0.08) * 0.06
    camera.position.x = Math.sin(elapsed * 0.1) * 0.5
    camera.lookAt(0, 0, 0)

    renderer.render(scene, camera)
    positionCards(elapsed)
    frame = requestAnimationFrame(tick)
  }

  const observer =
    typeof ResizeObserver === 'undefined'
      ? null
      : new ResizeObserver(() => resize())
  resize()
  observer?.observe(mount)
  frame = requestAnimationFrame(tick)

  return {
    dispose: () => {
      if (disposed) return
      disposed = true
      cancelAnimationFrame(frame)
      observer?.disconnect()
      nodeGeometry.dispose()
      nodeMaterial.dispose()
      edgeGeometry.dispose()
      edgeMaterial.dispose()
      renderer.dispose()
      mount.replaceChildren()
    },
  }
}

const demoLegalElement = defineCustomElement({
  events: {},
  properties: {},
  tag: demoLegalTagName,
})

const makeDemoLegalElement = (): CustomElementConstructor =>
  class DemoLegalSceneElement extends HTMLElement {
    #handle: SceneHandle | null = null

    connectedCallback(): void {
      if (this.#handle !== null) return
      const shadow = this.shadowRoot ?? this.attachShadow({ mode: 'open' })
      shadow.replaceChildren()
      const style = document.createElement('style')
      style.textContent =
        ':host{position:absolute;inset:0;display:block;pointer-events:none}.mount{width:100%;height:100%}'
      const mount = document.createElement('div')
      mount.className = 'mount'
      shadow.append(style, mount)
      this.#handle = mountDemoLegalScene(this, mount)
    }

    disconnectedCallback(): void {
      if (this.#handle === null) return
      this.#handle.dispose()
      this.#handle = null
    }
  }

export const registerDemoLegalElement = (): void => {
  if (typeof customElements === 'undefined') return
  if (typeof HTMLElement === 'undefined') return
  if (customElements.get(demoLegalTagName) !== undefined) return
  customElements.define(demoLegalTagName, makeDemoLegalElement())
}

export const demoLegalSceneView = <Message>(
  attributes: ReadonlyArray<Attribute<Message>> = [],
): Html => {
  registerDemoLegalElement()
  const element = demoLegalElement.withMessage<Message>()
  return element(attributes, [])
}
