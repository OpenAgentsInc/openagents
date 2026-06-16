import * as Three from 'three'

import { type AnimationHandle, makeAnimationView, seededUnit, webglCanvas } from './element'

// A drifting point network: nodes wander in a box and edges appear between any
// two within a proximity threshold — a live mesh/peer graph. Ported from the
// react-three-fiber Lines/Pointcloud demos to plain three.js (LineSegments whose
// position buffer is rebuilt each frame).
const COUNT = 70
const LINK_DISTANCE = 1.5
const BOUND = 3

const mountConstellation = (element: HTMLElement): AnimationHandle => {
  const { canvas, size } = webglCanvas(element)
  const renderer = new Three.WebGLRenderer({ alpha: true, antialias: true, canvas })
  renderer.setClearColor(0x000000, 0)

  const scene = new Three.Scene()
  const camera = new Three.PerspectiveCamera(55, 1, 0.1, 100)
  camera.position.z = 7

  const positions = new Float32Array(COUNT * 3)
  const velocities = new Float32Array(COUNT * 3)
  for (let i = 0; i < COUNT; i += 1) {
    positions[i * 3] = (seededUnit(i, 0) - 0.5) * BOUND * 2
    positions[i * 3 + 1] = (seededUnit(i, 1) - 0.5) * BOUND * 2
    positions[i * 3 + 2] = (seededUnit(i, 2) - 0.5) * BOUND * 2
    velocities[i * 3] = (seededUnit(i, 3) - 0.5) * 0.4
    velocities[i * 3 + 1] = (seededUnit(i, 4) - 0.5) * 0.4
    velocities[i * 3 + 2] = (seededUnit(i, 5) - 0.5) * 0.4
  }

  const nodeGeometry = new Three.BufferGeometry()
  const nodeAttr = new Three.BufferAttribute(positions, 3)
  nodeGeometry.setAttribute('position', nodeAttr)
  const nodeMaterial = new Three.PointsMaterial({
    blending: Three.AdditiveBlending,
    color: 0xd6f6ff,
    opacity: 0.95,
    size: 0.1,
    transparent: true,
  })
  scene.add(new Three.Points(nodeGeometry, nodeMaterial))

  const maxEdges = (COUNT * (COUNT - 1)) / 2
  const edgePositions = new Float32Array(maxEdges * 6)
  const edgeGeometry = new Three.BufferGeometry()
  const edgeAttr = new Three.BufferAttribute(edgePositions, 3)
  edgeGeometry.setAttribute('position', edgeAttr)
  const edgeMaterial = new Three.LineBasicMaterial({
    blending: Three.AdditiveBlending,
    color: 0x2979ff,
    opacity: 0.35,
    transparent: true,
  })
  const edges = new Three.LineSegments(edgeGeometry, edgeMaterial)
  scene.add(edges)

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

    for (let i = 0; i < COUNT; i += 1) {
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
    for (let a = 0; a < COUNT; a += 1) {
      const ax = positions[a * 3] ?? 0
      const ay = positions[a * 3 + 1] ?? 0
      const az = positions[a * 3 + 2] ?? 0
      for (let b = a + 1; b < COUNT; b += 1) {
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

    scene.rotation.y = time * 0.00007
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
      nodeGeometry.dispose()
      nodeMaterial.dispose()
      edgeGeometry.dispose()
      edgeMaterial.dispose()
      renderer.dispose()
      element.replaceChildren()
    },
  }
}

export const constellationView = makeAnimationView('oa-anim-constellation', mountConstellation)
