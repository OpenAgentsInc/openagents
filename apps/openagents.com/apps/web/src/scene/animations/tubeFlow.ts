import * as Three from 'three'

import { type AnimationHandle, makeAnimationView, webglCanvas } from './element'

// A closed tube following a smooth 3D curve with bright pulses streaming along
// it — a data/work pipeline. Ported from the react-three-fiber curve/tube demos
// to plain three.js (TubeGeometry on a CatmullRomCurve3 + travelling Points).
const PULSES = 14
const SEGMENTS = 220

const mountTubeFlow = (element: HTMLElement): AnimationHandle => {
  const { canvas, size } = webglCanvas(element)
  const renderer = new Three.WebGLRenderer({ alpha: true, antialias: true, canvas })
  renderer.setClearColor(0x000000, 0)

  const scene = new Three.Scene()
  const camera = new Three.PerspectiveCamera(55, 1, 0.1, 100)
  camera.position.z = 5.5

  const group = new Three.Group()
  scene.add(group)

  const controlPoints: Three.Vector3[] = []
  const knots = 8
  for (let i = 0; i < knots; i += 1) {
    const a = (i / knots) * Math.PI * 2
    controlPoints.push(
      new Three.Vector3(
        Math.cos(a) * 2 + Math.cos(a * 3) * 0.6,
        Math.sin(a * 2) * 1.4,
        Math.sin(a) * 2 + Math.sin(a * 3) * 0.6,
      ),
    )
  }
  const curve = new Three.CatmullRomCurve3(controlPoints, true, 'catmullrom', 0.5)

  const tubeGeometry = new Three.TubeGeometry(curve, SEGMENTS, 0.06, 12, true)
  const tubeMaterial = new Three.MeshBasicMaterial({
    color: 0x2979ff,
    opacity: 0.45,
    transparent: true,
    wireframe: true,
  })
  group.add(new Three.Mesh(tubeGeometry, tubeMaterial))

  const pulsePositions = new Float32Array(PULSES * 3)
  const pulseGeometry = new Three.BufferGeometry()
  const pulseAttr = new Three.BufferAttribute(pulsePositions, 3)
  pulseGeometry.setAttribute('position', pulseAttr)
  const pulseMaterial = new Three.PointsMaterial({
    blending: Three.AdditiveBlending,
    color: 0xd6f6ff,
    depthWrite: false,
    opacity: 1,
    size: 0.18,
    transparent: true,
  })
  group.add(new Three.Points(pulseGeometry, pulseMaterial))

  const resize = (): void => {
    const { height, width } = size()
    renderer.setPixelRatio(Math.min(window.devicePixelRatio || 1, 2))
    renderer.setSize(width, height, false)
    camera.aspect = width / height
    camera.updateProjectionMatrix()
  }

  let disposed = false
  let frame = 0
  const tick = (time: number): void => {
    if (disposed) return
    const t = time * 0.00006
    for (let i = 0; i < PULSES; i += 1) {
      const phase = (t + i / PULSES) % 1
      const point = curve.getPointAt(phase)
      pulsePositions[i * 3] = point.x
      pulsePositions[i * 3 + 1] = point.y
      pulsePositions[i * 3 + 2] = point.z
    }
    pulseAttr.needsUpdate = true
    group.rotation.y = time * 0.00012
    group.rotation.x = Math.sin(time * 0.0001) * 0.25
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
      tubeGeometry.dispose()
      tubeMaterial.dispose()
      pulseGeometry.dispose()
      pulseMaterial.dispose()
      renderer.dispose()
      element.replaceChildren()
    },
  }
}

export const tubeFlowView = makeAnimationView('oa-anim-tube-flow', mountTubeFlow)
