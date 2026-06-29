import * as Three from 'three'

import { type AnimationHandle, makeAnimationView, webglCanvas } from './element'

// Drifting point field (a slow rotating "starfield") — three.js Points experiment.
const COUNT = 900
const deterministicUnit = (index: number, axis: number): number => {
  const seed = (index + 1) * (axis * 101 + 997)
  const value = Math.sin(seed) * 10000
  return value - Math.floor(value)
}

const mountParticles = (element: HTMLElement): AnimationHandle => {
  const { canvas, size } = webglCanvas(element)
  const renderer = new Three.WebGLRenderer({
    alpha: true,
    antialias: true,
    canvas,
  })
  renderer.setClearColor(0x000000, 0)

  const scene = new Three.Scene()
  const camera = new Three.PerspectiveCamera(60, 1, 0.1, 100)
  camera.position.z = 4

  const positions = new Float32Array(COUNT * 3)
  for (let i = 0; i < COUNT; i += 1) {
    positions[i * 3] = (deterministicUnit(i, 0) - 0.5) * 8
    positions[i * 3 + 1] = (deterministicUnit(i, 1) - 0.5) * 8
    positions[i * 3 + 2] = (deterministicUnit(i, 2) - 0.5) * 8
  }
  const geometry = new Three.BufferGeometry()
  geometry.setAttribute('position', new Three.BufferAttribute(positions, 3))
  const material = new Three.PointsMaterial({
    color: 0xd6f6ff,
    opacity: 0.7,
    size: 0.025,
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
  const tick = (time: number): void => {
    if (disposed) return
    points.rotation.y = time * 0.00008
    points.rotation.x = Math.sin(time * 0.0001) * 0.15
    renderer.render(scene, camera)
    frame = requestAnimationFrame(tick)
  }

  const observer =
    typeof ResizeObserver === 'undefined'
      ? null
      : new ResizeObserver(() => resize())
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

export const particlesView = makeAnimationView(
  'oa-anim-particles',
  mountParticles,
)
