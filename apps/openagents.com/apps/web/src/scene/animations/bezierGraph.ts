import * as Three from 'three'

import { type AnimationHandle, makeAnimationView, seededUnit, webglCanvas } from './element'

// A WebGL permutation of the live SVG pylon bezier-network: synthetic nodes laid
// out on a ring or golden-angle spiral, quadratic bezier edges bowed into a
// central hub, with bright pulses travelling the edges (work flowing inward).
// Parameterized so /animations can show a few permutations from one mount.

type BezierConfig = Readonly<{
  nodes: number
  layout: 'ring' | 'spiral'
  bow: number
  edgeColor: number
  hubColor: number
  rotateSpeed: number
  segments: number
}>

const nodePosition = (
  index: number,
  total: number,
  layout: 'ring' | 'spiral',
): Three.Vector3 => {
  if (layout === 'ring') {
    const angle = (index / Math.max(1, total)) * Math.PI * 2
    const radius = 2.2
    return new Three.Vector3(Math.cos(angle) * radius, Math.sin(angle) * radius, 0)
  }
  const golden = 2.399963229728653
  const angle = index * golden
  const radius = 0.4 + 2.0 * Math.sqrt((index + 1) / Math.max(1, total))
  return new Three.Vector3(Math.cos(angle) * radius, Math.sin(angle) * radius, 0)
}

const makeBezierGraph =
  (config: BezierConfig) =>
  (element: HTMLElement): AnimationHandle => {
    const { canvas, size } = webglCanvas(element)
    const renderer = new Three.WebGLRenderer({ alpha: true, antialias: true, canvas })
    renderer.setClearColor(0x000000, 0)

    const scene = new Three.Scene()
    const camera = new Three.PerspectiveCamera(45, 1, 0.1, 100)
    camera.position.z = 7

    const group = new Three.Group()
    scene.add(group)

    const hub = new Three.Vector3(0, 0, 0)
    const curves: Three.QuadraticBezierCurve3[] = []
    const disposables: Array<{ dispose: () => void }> = []

    for (let i = 0; i < config.nodes; i += 1) {
      const node = nodePosition(i, config.nodes, config.layout)
      const mid = node.clone().add(hub).multiplyScalar(0.5)
      const dir = hub.clone().sub(node)
      const perpendicular = new Three.Vector3(-dir.y, dir.x, 0).normalize()
      const control = mid.add(perpendicular.multiplyScalar(config.bow))
      const curve = new Three.QuadraticBezierCurve3(node, control, hub)
      curves.push(curve)

      const geometry = new Three.BufferGeometry().setFromPoints(
        curve.getPoints(config.segments),
      )
      const material = new Three.LineBasicMaterial({
        blending: Three.AdditiveBlending,
        color: config.edgeColor,
        opacity: 0.22,
        transparent: true,
      })
      group.add(new Three.Line(geometry, material))
      disposables.push(geometry, material)
    }

    // Static node markers.
    const nodePositions = new Float32Array(config.nodes * 3)
    for (let i = 0; i < config.nodes; i += 1) {
      const node = nodePosition(i, config.nodes, config.layout)
      nodePositions[i * 3] = node.x
      nodePositions[i * 3 + 1] = node.y
      nodePositions[i * 3 + 2] = node.z
    }
    const nodeGeometry = new Three.BufferGeometry()
    nodeGeometry.setAttribute('position', new Three.BufferAttribute(nodePositions, 3))
    const nodeMaterial = new Three.PointsMaterial({
      blending: Three.AdditiveBlending,
      color: config.edgeColor,
      opacity: 0.9,
      size: 0.09,
      transparent: true,
    })
    group.add(new Three.Points(nodeGeometry, nodeMaterial))
    disposables.push(nodeGeometry, nodeMaterial)

    // Hub marker.
    const hubGeometry = new Three.BufferGeometry()
    hubGeometry.setAttribute('position', new Three.BufferAttribute(new Float32Array([0, 0, 0]), 3))
    const hubMaterial = new Three.PointsMaterial({
      blending: Three.AdditiveBlending,
      color: config.hubColor,
      opacity: 1,
      size: 0.3,
      transparent: true,
    })
    group.add(new Three.Points(hubGeometry, hubMaterial))
    disposables.push(hubGeometry, hubMaterial)

    // Pulses travelling each edge toward the hub.
    const pulsePositions = new Float32Array(config.nodes * 3)
    const phases = new Float32Array(config.nodes)
    const speeds = new Float32Array(config.nodes)
    for (let i = 0; i < config.nodes; i += 1) {
      phases[i] = seededUnit(i, 1)
      speeds[i] = 0.12 + seededUnit(i, 2) * 0.22
    }
    const pulseGeometry = new Three.BufferGeometry()
    const pulseAttr = new Three.BufferAttribute(pulsePositions, 3)
    pulseGeometry.setAttribute('position', pulseAttr)
    const pulseMaterial = new Three.PointsMaterial({
      blending: Three.AdditiveBlending,
      color: config.hubColor,
      opacity: 1,
      size: 0.14,
      transparent: true,
    })
    group.add(new Three.Points(pulseGeometry, pulseMaterial))
    disposables.push(pulseGeometry, pulseMaterial)

    const resize = (): void => {
      const { height, width } = size()
      renderer.setPixelRatio(Math.min(window.devicePixelRatio || 1, 2))
      renderer.setSize(width, height, false)
      camera.aspect = width / height
      camera.updateProjectionMatrix()
    }

    let disposed = false
    let frame = 0
    let last = 0
    const tick = (time: number): void => {
      if (disposed) return
      const dt = last === 0 ? 0.016 : Math.min(0.05, (time - last) / 1000)
      last = time
      group.rotation.z = time * config.rotateSpeed
      for (let i = 0; i < config.nodes; i += 1) {
        const curve = curves[i]
        if (curve === undefined) continue
        const phase = (((phases[i] ?? 0) + (speeds[i] ?? 0) * dt) % 1 + 1) % 1
        phases[i] = phase
        const point = curve.getPoint(phase)
        pulsePositions[i * 3] = point.x
        pulsePositions[i * 3 + 1] = point.y
        pulsePositions[i * 3 + 2] = point.z
      }
      pulseAttr.needsUpdate = true
      renderer.render(scene, camera)
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
        for (const item of disposables) item.dispose()
        renderer.dispose()
        element.replaceChildren()
      },
    }
  }

export const bezierRingView = makeAnimationView(
  'oa-anim-bezier-ring',
  makeBezierGraph({
    bow: 1.1,
    edgeColor: 0xd6f6ff,
    hubColor: 0x2979ff,
    layout: 'ring',
    nodes: 18,
    rotateSpeed: 0.00006,
    segments: 40,
  }),
)

export const bezierSpiralView = makeAnimationView(
  'oa-anim-bezier-spiral',
  makeBezierGraph({
    bow: 0.7,
    edgeColor: 0x9fd8ff,
    hubColor: 0x2979ff,
    layout: 'spiral',
    nodes: 64,
    rotateSpeed: 0.00009,
    segments: 32,
  }),
)

export const bezierWebView = makeAnimationView(
  'oa-anim-bezier-web',
  makeBezierGraph({
    bow: 2.0,
    edgeColor: 0x7fffd4,
    hubColor: 0xffffff,
    layout: 'ring',
    nodes: 36,
    rotateSpeed: -0.00005,
    segments: 48,
  }),
)
