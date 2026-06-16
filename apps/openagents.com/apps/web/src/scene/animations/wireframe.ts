import * as Three from 'three'

import { type AnimationHandle, makeAnimationView, webglCanvas } from './element'

// Slowly tumbling wireframe icosahedron — a minimal three.js experiment.
const mountWireframe = (element: HTMLElement): AnimationHandle => {
  const { canvas, size } = webglCanvas(element)
  const renderer = new Three.WebGLRenderer({ alpha: true, antialias: true, canvas })
  renderer.setClearColor(0x000000, 0)

  const scene = new Three.Scene()
  const camera = new Three.PerspectiveCamera(50, 1, 0.1, 100)
  camera.position.z = 3.2

  const geometry = new Three.IcosahedronGeometry(1, 1)
  const material = new Three.MeshBasicMaterial({
    color: 0xd6f6ff,
    opacity: 0.55,
    transparent: true,
    wireframe: true,
  })
  const mesh = new Three.Mesh(geometry, material)
  scene.add(mesh)

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
    mesh.rotation.x = time * 0.0003
    mesh.rotation.y = time * 0.0005
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

export const wireframeView = makeAnimationView('oa-anim-wireframe', mountWireframe)
