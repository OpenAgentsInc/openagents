import * as Three from 'three'

import { type AnimationHandle, makeAnimationView, seededUnit, webglCanvas } from './element'

// Points advected through a sinusoidal flow field (a curl-noise approximation) —
// streams of work circulating. Ported from the react-three-fiber Pointcloud demo
// to plain three.js Points whose positions integrate a velocity field each frame.
const COUNT = 1600
const BOUND = 4

const field = (x: number, y: number, z: number, t: number): [number, number, number] => {
  const fx = Math.sin(y * 0.8 + t) + Math.cos(z * 0.6 - t * 0.5)
  const fy = Math.sin(z * 0.7 + t * 0.8) + Math.cos(x * 0.5 + t)
  const fz = Math.sin(x * 0.6 - t) + Math.cos(y * 0.9 + t * 0.6)
  return [fx, fy, fz]
}

const mountFlowField = (element: HTMLElement): AnimationHandle => {
  const { canvas, size } = webglCanvas(element)
  const renderer = new Three.WebGLRenderer({ alpha: true, antialias: true, canvas })
  renderer.setClearColor(0x000000, 0)

  const scene = new Three.Scene()
  const camera = new Three.PerspectiveCamera(60, 1, 0.1, 100)
  camera.position.z = 8

  const positions = new Float32Array(COUNT * 3)
  for (let i = 0; i < COUNT; i += 1) {
    positions[i * 3] = (seededUnit(i, 0) - 0.5) * BOUND * 2
    positions[i * 3 + 1] = (seededUnit(i, 1) - 0.5) * BOUND * 2
    positions[i * 3 + 2] = (seededUnit(i, 2) - 0.5) * BOUND * 2
  }
  const geometry = new Three.BufferGeometry()
  const positionAttr = new Three.BufferAttribute(positions, 3)
  geometry.setAttribute('position', positionAttr)
  const material = new Three.PointsMaterial({
    blending: Three.AdditiveBlending,
    color: 0x9fe8ff,
    depthWrite: false,
    opacity: 0.7,
    size: 0.045,
    transparent: true,
  })
  const points = new Three.Points(geometry, material)
  scene.add(points)

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
    const t = time * 0.0004
    for (let i = 0; i < COUNT; i += 1) {
      const k = i * 3
      const px = positions[k] ?? 0
      const py = positions[k + 1] ?? 0
      const pz = positions[k + 2] ?? 0
      const [fx, fy, fz] = field(px, py, pz, t)
      const components = [px + fx * dt * 0.5, py + fy * dt * 0.5, pz + fz * dt * 0.5]
      for (let axis = 0; axis < 3; axis += 1) {
        let value = components[axis] ?? 0
        if (value > BOUND) value = -BOUND
        else if (value < -BOUND) value = BOUND
        positions[k + axis] = value
      }
    }
    positionAttr.needsUpdate = true
    points.rotation.y = time * 0.00005
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
      geometry.dispose()
      material.dispose()
      renderer.dispose()
      element.replaceChildren()
    },
  }
}

export const flowFieldView = makeAnimationView('oa-anim-flow-field', mountFlowField)
