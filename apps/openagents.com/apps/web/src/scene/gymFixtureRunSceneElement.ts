import { define as defineCustomElement } from 'foldkit/customElement'
import type { Attribute, Html } from 'foldkit/html'
import { html } from 'foldkit/html'
import * as Three from 'three'
import {
  createFlowBeam,
  createResourceBar,
} from '@openagentsinc/three-effect/core'

import type {
  GymLaneRef,
  PublicGymScene,
  PublicGymSceneLaneStatus,
} from '../page/loggedOut/gym/flow'
import { seededUnit, webglCanvas, type AnimationHandle } from './animations/element'

export const GYM_FIXTURE_RUN_SCENE_TAG = 'oa-gym-fixture-run-scene'

export type FixtureSceneLaneGeometry = Readonly<{
  lane: string
  label: string
  status: 'test_passed' | 'skipped_unavailable'
  angle: number
  radius: number
  verdictBeam: boolean
  costFraction: number
}>

export type FixtureSceneGeometry = Readonly<{
  lanes: ReadonlyArray<FixtureSceneLaneGeometry>
  costMeterFraction: number
}>

export const buildGymFixtureSceneGeometry = (
  scene: PublicGymScene,
): FixtureSceneGeometry => {
  const attempted = scene.lanes.reduce(
    (sum, lane) => sum + lane.attemptedCells,
    0,
  )
  const accepted = scene.lanes.reduce((sum, lane) => sum + lane.acceptedCells, 0)

  return {
    costMeterFraction:
      scene.simulatedCostMsat <= 0 ? 0 : Math.min(1, accepted / Math.max(1, attempted)),
    lanes: scene.lanes.map((lane, index) => {
      const count = Math.max(1, scene.lanes.length)
      const angle = -Math.PI / 2 + (Math.PI * 2 * index) / count
      const radius = 1.7 + seededUnit(index, lane.label.length) * 0.25

      return {
        lane: lane.lane,
        label: lane.label,
        status: lane.status,
        angle,
        radius,
        verdictBeam: lane.verdictBeam,
        costFraction:
          lane.attemptedCells === 0
            ? 0
            : Math.min(1, lane.acceptedCells / lane.attemptedCells),
      }
    }),
  }
}

const parseNumber = (value: string | undefined, fallback: number): number => {
  const parsed = Number.parseFloat(value ?? '')
  return Number.isFinite(parsed) ? parsed : fallback
}

const parseLaneRef = (value: string | undefined): GymLaneRef =>
  value === 'pylon-whole-small' ||
  value === 'psionic-shard-wan' ||
  value === 'provider-baseline'
    ? value
    : 'provider-baseline'

const parseLaneStatus = (
  value: string | undefined,
): PublicGymSceneLaneStatus =>
  value === 'skipped_unavailable' ? value : 'test_passed'

const parseScene = (element: HTMLElement): PublicGymScene => {
  const lanes = (element.dataset.lanes ?? '')
    .split(';')
    .filter(Boolean)
    .map(lane => {
      const [id, label, status, attempted, accepted, skipped, beam] =
        lane.split('|')

      return {
        lane: parseLaneRef(id),
        label: label ?? 'Provider baseline',
        status: parseLaneStatus(status),
        attemptedCells: parseNumber(attempted, 0),
        acceptedCells: parseNumber(accepted, 0),
        skippedCells: parseNumber(skipped, 0),
        verdictBeam: beam === '1',
      }
    })

  return {
    schema: 'openagents.gym.fixture_scene.v1',
    durationMs: parseNumber(element.dataset.durationMs, 2400),
    simulatedCostMsat: parseNumber(element.dataset.simulatedCostMsat, 0),
    billedCostMsat: 0,
    lanes,
  }
}

export const encodeGymFixtureSceneLanes = (scene: PublicGymScene): string =>
  scene.lanes
    .map(lane =>
      [
        lane.lane,
        lane.label,
        lane.status,
        lane.attemptedCells,
        lane.acceptedCells,
        lane.skippedCells,
        lane.verdictBeam ? '1' : '0',
      ].join('|'),
    )
    .join(';')

const laneColor = (status: FixtureSceneLaneGeometry['status']): number =>
  status === 'test_passed' ? 0x68d6ff : 0x5b6474

export const mountGymFixtureRunScene = (
  element: HTMLElement,
  scene: PublicGymScene,
): AnimationHandle => {
  const { canvas, size } = webglCanvas(element)
  const renderer = new Three.WebGLRenderer({ alpha: true, antialias: true, canvas })
  renderer.setClearColor(0x000000, 0)

  const world = new Three.Scene()
  const camera = new Three.PerspectiveCamera(45, 1, 0.1, 100)
  camera.position.set(0, 0.5, 5.2)

  const geometry = buildGymFixtureSceneGeometry(scene)
  const hub = new Three.Mesh(
    new Three.IcosahedronGeometry(0.35, 1),
    new Three.MeshBasicMaterial({ color: 0xb8d4ff, wireframe: true }),
  )
  world.add(hub)

  const disposables: Array<{ dispose: () => void }> = [
    hub.geometry,
    hub.material as Three.Material,
  ]
  const animated: Array<{ update: (deltaSeconds: number) => void }> = []

  geometry.lanes.forEach(lane => {
    const x = Math.cos(lane.angle) * lane.radius
    const y = Math.sin(lane.angle) * lane.radius * 0.72
    const nodeMaterial = new Three.MeshBasicMaterial({
      color: laneColor(lane.status),
      opacity: lane.status === 'test_passed' ? 0.9 : 0.35,
      transparent: true,
      wireframe: lane.status === 'skipped_unavailable',
    })
    const nodeGeometry = new Three.SphereGeometry(0.12, 16, 12)
    const node = new Three.Mesh(nodeGeometry, nodeMaterial)
    node.position.set(x, y, 0)
    world.add(node)
    disposables.push(nodeGeometry, nodeMaterial)

    const arc = createFlowBeam({
      from: [0, 0, 0],
      to: [x, y, 0],
      color: laneColor(lane.status),
      rate: lane.status === 'test_passed' ? 0.32 : 0.08,
      pulseCount: lane.status === 'test_passed' ? 3 : 1,
      radius: lane.status === 'test_passed' ? 0.009 : 0.004,
      pulseRadius: lane.status === 'test_passed' ? 0.035 : 0.018,
      bend: lane.verdictBeam ? 0.34 : 0.08,
      opacity: lane.status === 'test_passed' ? 0.36 : 0.14,
    })
    world.add(arc.object3D)
    disposables.push(arc)
    animated.push(arc)

    if (lane.verdictBeam) {
      const verdict = createFlowBeam({
        from: [x, y, 0.08],
        to: [0, 0, 0.16],
        color: 0x9bf59b,
        rate: 0.46,
        pulseCount: 2,
        radius: 0.006,
        pulseRadius: 0.028,
        bend: -0.2,
        opacity: 0.34,
      })
      world.add(verdict.object3D)
      disposables.push(verdict)
      animated.push(verdict)
    }
  })

  const meter = createResourceBar({
    backgroundColor: 0x111827,
    borderColor: 0x3a82ff,
    depthTest: false,
    fillColor: 0x7cf0ff,
    height: 0.12,
    kind: 'earnings',
    position: [0, -1.55, 0.2],
    value: 0,
    width: 2.2,
  })
  world.add(meter.group)
  disposables.push(meter)

  const resize = (): void => {
    const { height, width } = size()
    renderer.setPixelRatio(Math.min(window.devicePixelRatio || 1, 2))
    renderer.setSize(width, height, false)
    camera.aspect = width / height
    camera.updateProjectionMatrix()
  }

  let disposed = false
  let frame = 0
  let lastTime = 0
  const tick = (time: number): void => {
    if (disposed) return
    const phase = Math.min(1, (time % scene.durationMs) / scene.durationMs)
    const deltaSeconds = lastTime === 0 ? 0 : Math.max(0, (time - lastTime) / 1000)
    lastTime = time
    hub.rotation.x = phase * Math.PI * 2
    hub.rotation.y = phase * Math.PI
    for (const item of animated) {
      item.update(deltaSeconds)
    }
    meter.setValue(geometry.costMeterFraction * phase)
    meter.faceCamera(camera)
    renderer.render(world, camera)
    frame = requestAnimationFrame(tick)
  }

  const observer =
    typeof ResizeObserver === 'undefined' ? null : new ResizeObserver(() => resize())
  resize()
  observer?.observe(element)
  frame = requestAnimationFrame(tick)

  return {
    dispose: () => {
      if (disposed) return
      disposed = true
      cancelAnimationFrame(frame)
      observer?.disconnect()
      for (const disposable of disposables) {
        disposable.dispose()
      }
      renderer.dispose()
      element.replaceChildren()
    },
  }
}

const element = defineCustomElement({
  events: {},
  properties: {},
  tag: GYM_FIXTURE_RUN_SCENE_TAG,
})

const makeElement = (): CustomElementConstructor =>
  class GymFixtureRunSceneElement extends HTMLElement {
    #handle: AnimationHandle | null = null

    connectedCallback(): void {
      if (this.#handle !== null) return
      const shadow = this.shadowRoot ?? this.attachShadow({ mode: 'open' })
      shadow.replaceChildren()
      const style = document.createElement('style')
      style.textContent =
        ':host{display:block;min-height:260px}.mount{position:relative;min-height:260px;overflow:hidden;border:1px solid rgba(127,176,255,.18);background:#020409}'
      const mount = document.createElement('div')
      mount.className = 'mount'
      shadow.append(style, mount)
      this.#handle = mountGymFixtureRunScene(mount, parseScene(this))
    }

    disconnectedCallback(): void {
      if (this.#handle === null) return
      this.#handle.dispose()
      this.#handle = null
    }
  }

export const registerGymFixtureRunSceneElement = (): void => {
  if (typeof customElements === 'undefined') return
  if (typeof HTMLElement === 'undefined') return
  if (customElements.get(GYM_FIXTURE_RUN_SCENE_TAG) !== undefined) return
  customElements.define(GYM_FIXTURE_RUN_SCENE_TAG, makeElement())
}

export const gymFixtureRunSceneView = <Message>(
  scene: PublicGymScene,
  attributes: ReadonlyArray<Attribute<Message>> = [],
): Html => {
  registerGymFixtureRunSceneElement()
  const h = html<Message>()

  return element.withMessage<Message>()(
    [
      ...attributes,
      h.DataAttribute('three-effect-scene', 'gym-fixture-run'),
      h.DataAttribute('lanes', encodeGymFixtureSceneLanes(scene)),
      h.DataAttribute('duration-ms', String(scene.durationMs)),
      h.DataAttribute(
        'simulated-cost-msat',
        String(scene.simulatedCostMsat),
      ),
    ],
    [],
  )
}
