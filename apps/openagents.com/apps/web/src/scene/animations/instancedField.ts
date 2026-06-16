import * as Three from 'three'

import { type AnimationHandle, makeAnimationView, webglCanvas } from './element'

// A grid of instanced cells rising and falling in a travelling wave — a "compute
// fleet" field. Ported from the react-three-fiber InstancedMesh pattern
// (example/src/demos) to plain three.js InstancedMesh with per-instance color.
const GRID = 22
const COUNT = GRID * GRID
const SPACING = 0.42

const mountInstancedField = (element: HTMLElement): AnimationHandle => {
  const { canvas, size } = webglCanvas(element)
  const renderer = new Three.WebGLRenderer({ alpha: true, antialias: true, canvas })
  renderer.setClearColor(0x000000, 0)

  const scene = new Three.Scene()
  const camera = new Three.PerspectiveCamera(45, 1, 0.1, 100)
  camera.position.set(0, 7, 9)
  camera.lookAt(0, 0, 0)

  scene.add(new Three.AmbientLight(0x335577, 1.4))
  const key = new Three.DirectionalLight(0xd6f6ff, 2.2)
  key.position.set(4, 8, 6)
  scene.add(key)

  const geometry = new Three.BoxGeometry(0.3, 1, 0.3)
  const material = new Three.MeshStandardMaterial({
    metalness: 0.1,
    roughness: 0.4,
    vertexColors: true,
  })
  const mesh = new Three.InstancedMesh(geometry, material, COUNT)

  const dummy = new Three.Object3D()
  const colorLow = new Three.Color(0x123047)
  const colorHigh = new Three.Color(0x6fe0ff)
  const color = new Three.Color()
  const half = (GRID - 1) / 2
  for (let x = 0; x < GRID; x += 1) {
    for (let z = 0; z < GRID; z += 1) {
      const i = x * GRID + z
      dummy.position.set((x - half) * SPACING, 0, (z - half) * SPACING)
      dummy.updateMatrix()
      mesh.setMatrixAt(i, dummy.matrix)
      mesh.setColorAt(i, colorLow)
    }
  }
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
    const t = time * 0.0012
    for (let x = 0; x < GRID; x += 1) {
      for (let z = 0; z < GRID; z += 1) {
        const i = x * GRID + z
        const wave = Math.sin((x - half) * 0.55 + t) + Math.cos((z - half) * 0.55 + t)
        const height = 0.4 + (wave + 2) * 0.9
        dummy.position.set((x - half) * SPACING, height / 2 - 0.5, (z - half) * SPACING)
        dummy.scale.set(1, height, 1)
        dummy.updateMatrix()
        mesh.setMatrixAt(i, dummy.matrix)
        color.copy(colorLow).lerp(colorHigh, (wave + 2) / 4)
        mesh.setColorAt(i, color)
      }
    }
    mesh.instanceMatrix.needsUpdate = true
    if (mesh.instanceColor !== null) mesh.instanceColor.needsUpdate = true
    scene.rotation.y = time * 0.00008
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
      mesh.dispose()
      renderer.dispose()
      element.replaceChildren()
    },
  }
}

export const instancedFieldView = makeAnimationView('oa-anim-instanced-field', mountInstancedField)
