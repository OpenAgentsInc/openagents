import {
  applyInstanceTransforms,
  createInstancedMesh,
  type InstanceTransform,
} from '@openagentsinc/three-effect/core'
import { define as defineCustomElement } from 'foldkit/customElement'
import type { Attribute, Html } from 'foldkit/html'
import * as Three from 'three'
import { EffectComposer } from 'three/examples/jsm/postprocessing/EffectComposer.js'
import { OutputPass } from 'three/examples/jsm/postprocessing/OutputPass.js'
import { RenderPass } from 'three/examples/jsm/postprocessing/RenderPass.js'
import { UnrealBloomPass } from 'three/examples/jsm/postprocessing/UnrealBloomPass.js'

import type {
  QaSwarmSceneProjection,
  QaSwarmVerdict,
} from '../page/qa-swarm/projection'

export const qaSwarmSceneTagName = 'oa-qa-swarm-scene'

const SCENE_REF_ATTRIBUTE = 'data-scene-ref'
const BLUE = new Three.Color(0x3a7bff)
const CYAN = new Three.Color(0x7cf0ff)
const GOLD = new Three.Color(0xffb400)
const GREEN = new Three.Color(0x00c853)
const RED = new Three.Color(0xd32f2f)
const MUTED = new Three.Color(0x6a7384)

type Disposable = Readonly<{ dispose: () => void }>

export type QaSwarmSceneNodeFrame = Readonly<{
  agentRef: string
  label: string
  position: readonly [number, number, number]
  status: QaSwarmVerdict
}>

export type QaSwarmSceneArcFrame = Readonly<{
  fromAgentRef: string
  lit: boolean
  receiptRef: string
  strength: number
  verdict: QaSwarmVerdict
}>

export type QaSwarmSceneFrame = Readonly<{
  arcs: ReadonlyArray<QaSwarmSceneArcFrame>
  fallbackRef: string
  nodes: ReadonlyArray<QaSwarmSceneNodeFrame>
  targetRef: string
}>

const seededUnit = (a: number, b: number): number => {
  const value = Math.sin((a + 1) * (b * 101 + 997)) * 10000
  return value - Math.floor(value)
}

const verdictColor = (verdict: QaSwarmVerdict): Three.Color => {
  switch (verdict) {
    case 'passed':
      return GREEN
    case 'failed':
      return RED
    case 'warning':
      return GOLD
    case 'inconclusive':
      return MUTED
  }
}

export const buildQaSwarmSceneFrame = (
  projection: QaSwarmSceneProjection,
): QaSwarmSceneFrame => {
  const count = Math.max(1, projection.agents.length)
  return {
    arcs: projection.receiptArcs.map(arc => ({
      fromAgentRef: arc.fromAgentRef,
      lit: arc.receiptRef.length > 0,
      receiptRef: arc.receiptRef,
      strength: Math.max(0, Math.min(1, arc.strength)),
      verdict: arc.verdict,
    })),
    fallbackRef: projection.fallbackRef,
    nodes: projection.agents.map(agent => {
      const ring = (agent.orbitIndex / count) * Math.PI * 2
      const radius = 2.1 + seededUnit(agent.orbitIndex, 2) * 0.85
      return {
        agentRef: agent.agentRef,
        label: agent.label,
        position: [
          Math.cos(ring) * radius,
          Math.sin(ring) * 0.85,
          Math.sin(ring) * radius,
        ],
        status: agent.status,
      }
    }),
    targetRef: projection.targetRef,
  }
}

const sceneProjectionRegistry = new Map<string, QaSwarmSceneProjection>()

export const rememberQaSwarmSceneProjection = (
  projection: QaSwarmSceneProjection,
): void => {
  sceneProjectionRegistry.set(projection.sceneRef, projection)
}

const hostSize = (element: HTMLElement): { height: number; width: number } => {
  const rect = element.getBoundingClientRect()
  return {
    height: Math.max(1, Math.floor(rect.height || element.clientHeight || 360)),
    width: Math.max(1, Math.floor(rect.width || element.clientWidth || 640)),
  }
}

const makeEmissiveMaterial = (
  color: Three.Color,
  strength: number,
  opacity: number,
): Three.MeshBasicMaterial => {
  const material = new Three.MeshBasicMaterial({
    blending: Three.AdditiveBlending,
    color: color.clone().multiplyScalar(strength),
    depthWrite: false,
    opacity,
    transparent: true,
  })
  material.toneMapped = false
  return material
}

const makeLabel = (text: string, color: string): Three.Sprite => {
  const canvas = document.createElement('canvas')
  canvas.width = 512
  canvas.height = 128
  const context = canvas.getContext('2d')
  if (context !== null) {
    context.font = '700 38px ui-monospace, SFMono-Regular, Menlo, monospace'
    context.textAlign = 'center'
    context.textBaseline = 'middle'
    context.fillStyle = color
    context.fillText(text, canvas.width / 2, canvas.height / 2)
  }
  const texture = new Three.CanvasTexture(canvas)
  const material = new Three.SpriteMaterial({
    map: texture,
    opacity: 0.88,
    transparent: true,
  })
  const sprite = new Three.Sprite(material)
  sprite.scale.set(1.85, 0.46, 1)
  sprite.userData = {
    dispose: () => {
      texture.dispose()
      material.dispose()
    },
  }
  return sprite
}

export type QaSwarmSceneHandle = Readonly<{ dispose: () => void }>

export const mountQaSwarmScene = (
  element: HTMLElement,
  projection: QaSwarmSceneProjection,
): QaSwarmSceneHandle => {
  const frame = buildQaSwarmSceneFrame(projection)
  element.replaceChildren()
  element.style.position = 'absolute'
  element.style.inset = '0'
  element.style.overflow = 'hidden'

  const canvas = document.createElement('canvas')
  canvas.setAttribute('data-qa-swarm-webgl', projection.sceneRef)
  canvas.style.cssText =
    'position:absolute;inset:0;width:100%;height:100%;display:block'
  element.append(canvas)

  const renderer = new Three.WebGLRenderer({
    alpha: true,
    antialias: true,
    canvas,
    preserveDrawingBuffer: true,
  })
  renderer.outputColorSpace = Three.SRGBColorSpace
  renderer.toneMapping = Three.NoToneMapping
  renderer.setClearColor(0x000000, 0)

  const scene = new Three.Scene()
  const camera = new Three.PerspectiveCamera(48, 1, 0.1, 100)
  camera.position.set(0, 2.2, 7.2)
  camera.lookAt(0, 0, 0)

  const root = new Three.Group()
  scene.add(root)

  const disposables: Array<Disposable> = []
  const targetMaterial = makeEmissiveMaterial(BLUE, 2.7, 0.9)
  const targetGeometry = new Three.IcosahedronGeometry(0.58, 2)
  const target = new Three.Mesh(targetGeometry, targetMaterial)
  root.add(target)
  disposables.push(targetGeometry, targetMaterial)

  const haloMaterial = makeEmissiveMaterial(CYAN, 1.25, 0.34)
  const haloGeometry = new Three.TorusGeometry(0.98, 0.012, 12, 128)
  const halo = new Three.Mesh(haloGeometry, haloMaterial)
  halo.rotation.x = Math.PI * 0.5
  root.add(halo)
  disposables.push(haloGeometry, haloMaterial)

  const agentGeometry = new Three.SphereGeometry(1, 28, 16)
  const agentMaterial = new Three.MeshBasicMaterial({
    blending: Three.AdditiveBlending,
    depthWrite: false,
    transparent: true,
    vertexColors: true,
  })
  const agentMesh = createInstancedMesh(
    agentGeometry,
    agentMaterial,
    frame.nodes.map(node => ({
      color: verdictColor(node.status),
      position: node.position,
      scale: 0.18,
    })),
  )
  root.add(agentMesh)
  disposables.push(agentGeometry, agentMaterial, agentMesh)

  const labels = frame.nodes.map(node => {
    const label = makeLabel(node.label, '#d6f6ff')
    label.position.set(
      node.position[0],
      node.position[1] - 0.42,
      node.position[2],
    )
    root.add(label)
    return label
  })

  const arcObjects = frame.arcs.flatMap(arc => {
    const node = frame.nodes.find(item => item.agentRef === arc.fromAgentRef)
    if (node === undefined) {
      return []
    }
    const start = new Three.Vector3(...node.position)
    const middle = start.clone().multiplyScalar(0.45)
    middle.y += 0.78 + arc.strength * 0.72
    const curve = new Three.QuadraticBezierCurve3(
      start,
      middle,
      new Three.Vector3(0, 0, 0),
    )
    const geometry = new Three.BufferGeometry().setFromPoints(
      curve.getPoints(96),
    )
    const material = new Three.LineBasicMaterial({
      blending: Three.AdditiveBlending,
      color: verdictColor(arc.verdict)
        .clone()
        .multiplyScalar(arc.lit ? 1.35 : 0.24),
      depthWrite: false,
      opacity: arc.lit ? 0.44 + arc.strength * 0.34 : 0.12,
      transparent: true,
    })
    material.toneMapped = false
    const line = new Three.Line(geometry, material)
    root.add(line)

    const pulseGeometry = new Three.SphereGeometry(
      0.075 + arc.strength * 0.045,
      16,
      8,
    )
    const pulseMaterial = makeEmissiveMaterial(
      verdictColor(arc.verdict),
      arc.lit ? 2.2 : 0.35,
      arc.lit ? 0.98 : 0.2,
    )
    const pulse = new Three.Mesh(pulseGeometry, pulseMaterial)
    root.add(pulse)
    disposables.push(geometry, material, pulseGeometry, pulseMaterial)
    return [{ arc, curve, pulse }]
  })

  const landingGeometry = new Three.RingGeometry(0.26, 0.34, 48)
  const landingMaterial = new Three.MeshBasicMaterial({
    blending: Three.AdditiveBlending,
    depthWrite: false,
    transparent: true,
    vertexColors: true,
  })
  const landingMesh = createInstancedMesh(
    landingGeometry,
    landingMaterial,
    projection.verdictLandings.map((landing, index) => ({
      color: verdictColor(landing.verdict),
      position: [
        Math.cos(
          (index / Math.max(1, projection.verdictLandings.length)) *
            Math.PI *
            2,
        ) * 0.82,
        0.03,
        Math.sin(
          (index / Math.max(1, projection.verdictLandings.length)) *
            Math.PI *
            2,
        ) * 0.82,
      ],
      scale: Math.max(0.35, landing.burst),
    })),
  )
  root.add(landingMesh)
  disposables.push(landingGeometry, landingMaterial, landingMesh)

  const sparkCount = 180
  const sparkPositions = new Float32Array(sparkCount * 3)
  for (let i = 0; i < sparkCount; i += 1) {
    const radius = 0.8 + seededUnit(i, 4) * 3.1
    const angle = seededUnit(i, 5) * Math.PI * 2
    sparkPositions[i * 3] = Math.cos(angle) * radius
    sparkPositions[i * 3 + 1] = (seededUnit(i, 6) - 0.5) * 2.8
    sparkPositions[i * 3 + 2] = Math.sin(angle) * radius
  }
  const sparkGeometry = new Three.BufferGeometry()
  sparkGeometry.setAttribute(
    'position',
    new Three.Float32BufferAttribute(sparkPositions, 3),
  )
  const sparkMaterial = new Three.PointsMaterial({
    blending: Three.AdditiveBlending,
    color: CYAN.clone().multiplyScalar(1.35),
    depthWrite: false,
    opacity: 0.58,
    size: 0.035,
    transparent: true,
  })
  sparkMaterial.toneMapped = false
  const sparks = new Three.Points(sparkGeometry, sparkMaterial)
  root.add(sparks)
  disposables.push(sparkGeometry, sparkMaterial)

  const composer = new EffectComposer(renderer)
  composer.addPass(new RenderPass(scene, camera))
  const bloomPass = new UnrealBloomPass(
    new Three.Vector2(1, 1),
    1.05,
    0.72,
    0.82,
  )
  composer.addPass(bloomPass)
  composer.addPass(new OutputPass())

  const resize = (): void => {
    const size = hostSize(element)
    const ratio = Math.min(window.devicePixelRatio || 1, 2)
    renderer.setPixelRatio(ratio)
    renderer.setSize(size.width, size.height, false)
    composer.setPixelRatio(ratio)
    composer.setSize(size.width, size.height)
    bloomPass.setSize(size.width * ratio, size.height * ratio)
    camera.aspect = size.width / size.height
    camera.updateProjectionMatrix()
  }

  let disposed = false
  let frameId = 0
  const render = (time: number): void => {
    if (disposed) {
      return
    }
    const seconds = time * 0.001
    root.rotation.y = seconds * 0.12
    target.rotation.y = seconds * 0.42
    target.rotation.x = Math.sin(seconds * 0.7) * 0.24
    halo.rotation.z = seconds * 0.36

    const agentTransforms: Array<InstanceTransform> = frame.nodes.map(
      (node, index) => {
        const position = new Three.Vector3(...node.position)
        const breathe = 1 + Math.sin(seconds * 1.8 + index * 0.8) * 0.18
        position.y += Math.sin(seconds * 0.65 + index) * 0.08
        return {
          color: verdictColor(node.status),
          position: [position.x, position.y, position.z],
          scale: 0.18 * breathe,
        }
      },
    )
    applyInstanceTransforms(agentMesh, agentTransforms)

    arcObjects.forEach((item, index) => {
      const phase =
        (seconds * (0.22 + item.arc.strength * 0.22) + index * 0.21) % 1
      const point = item.curve.getPoint(phase)
      item.pulse.position.copy(point)
    })

    const landingTransforms: Array<InstanceTransform> = projection.verdictLandings.map(
      (landing, index) => {
        const angle =
          (index / Math.max(1, projection.verdictLandings.length)) * Math.PI * 2
        const spark = 1 + Math.sin(seconds * 2.3 + index) * 0.2
        return {
          color: verdictColor(landing.verdict),
          position: [Math.cos(angle) * 0.82, 0.03, Math.sin(angle) * 0.82],
          scale: Math.max(0.35, landing.burst) * spark,
        }
      },
    )
    applyInstanceTransforms(landingMesh, landingTransforms)

    composer.render()
    frameId = requestAnimationFrame(render)
  }

  const observer =
    typeof ResizeObserver === 'undefined'
      ? null
      : new ResizeObserver(() => resize())
  resize()
  observer?.observe(element)
  frameId = requestAnimationFrame(render)

  return {
    dispose: () => {
      if (disposed) {
        return
      }
      disposed = true
      cancelAnimationFrame(frameId)
      observer?.disconnect()
      labels.forEach(label => {
        const dispose = label.userData.dispose
        if (typeof dispose === 'function') {
          dispose()
        }
      })
      disposables.forEach(item => item.dispose())
      bloomPass.dispose()
      composer.dispose()
      renderer.dispose()
      element.replaceChildren()
    },
  }
}

const qaSwarmSceneElement = defineCustomElement({
  events: {},
  properties: {},
  tag: qaSwarmSceneTagName,
})

const makeQaSwarmSceneElement = (): CustomElementConstructor =>
  class QaSwarmSceneElement extends HTMLElement {
    #handle: QaSwarmSceneHandle | null = null

    static get observedAttributes(): ReadonlyArray<string> {
      return [SCENE_REF_ATTRIBUTE]
    }

    connectedCallback(): void {
      this.#mount()
    }

    attributeChangedCallback(): void {
      this.#mount()
    }

    disconnectedCallback(): void {
      this.#handle?.dispose()
      this.#handle = null
    }

    #mount(): void {
      if (!this.isConnected) {
        return
      }
      this.#handle?.dispose()
      this.#handle = null

      const sceneRef = this.getAttribute(SCENE_REF_ATTRIBUTE)
      const projection =
        sceneRef === null ? undefined : sceneProjectionRegistry.get(sceneRef)
      const reducedMotion =
        typeof window !== 'undefined' &&
        typeof window.matchMedia === 'function' &&
        window.matchMedia('(prefers-reduced-motion: reduce)').matches
      if (projection === undefined || reducedMotion) {
        return
      }
      this.#handle = mountQaSwarmScene(this, projection)
    }
  }

export const registerQaSwarmSceneElement = (): void => {
  if (typeof customElements === 'undefined') {
    return
  }
  if (typeof HTMLElement === 'undefined') {
    return
  }
  if (customElements.get(qaSwarmSceneTagName) !== undefined) {
    return
  }
  customElements.define(qaSwarmSceneTagName, makeQaSwarmSceneElement())
}

export const qaSwarmSceneView = <Message>(
  attributes: ReadonlyArray<Attribute<Message>> = [],
): Html => {
  registerQaSwarmSceneElement()
  const element = qaSwarmSceneElement.withMessage<Message>()
  return element(attributes, [])
}
