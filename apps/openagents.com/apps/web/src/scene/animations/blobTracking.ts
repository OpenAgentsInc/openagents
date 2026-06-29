import {
  applyInstanceTransforms,
  createInstancedMesh,
  type InstanceTransform,
} from '@openagentsinc/three-effect/core'
import * as Three from 'three'

import { type AnimationHandle, makeAnimationView, seededUnit, webglCanvas } from './element'

// Data-aesthetics "blob track instancing" — a three-effect port of the
// TouchDesigner-004 component (the-poet-engineer). The original tracks bright
// blobs in a video, instances geometry at each blob centroid, draws the centroid
// coordinates as live [x,y] readouts, and threads spline lines between them over
// a strand-y background. We reproduce the aesthetic synthetically: smoothly
// drifting tracked blobs (instanced rings via three-effect createInstancedMesh),
// pulsing red "hot" cores, crisp [x,y] coordinate sprites, a Catmull-Rom
// connector spline, drifting dust, and faint swaying background strands.

const BLOBS = 11
const STRANDS = 18
const DUST = 320
// Virtual capture resolution (portrait, like the source video) for the readouts.
const VRES_W = 1080
const VRES_H = 1920
// World frame the blobs live in (portrait 9:16).
const WORLD_H = 5
const WORLD_W = (WORLD_H * 9) / 16

type Blob = Readonly<{
  baseX: number
  baseY: number
  ampX: number
  ampY: number
  freqX: number
  freqY: number
  phaseX: number
  phaseY: number
  baseRadius: number
  pulse: number
  hot: boolean
}>

const makeBlobs = (): Blob[] => {
  const blobs: Blob[] = []
  for (let i = 0; i < BLOBS; i += 1) {
    blobs.push({
      ampX: (0.1 + seededUnit(i, 2) * 0.16) * WORLD_W,
      ampY: (0.1 + seededUnit(i, 3) * 0.16) * WORLD_H,
      baseRadius: 0.12 + seededUnit(i, 6) * 0.16,
      baseX: (seededUnit(i, 0) - 0.5) * WORLD_W * 0.82,
      baseY: (seededUnit(i, 1) - 0.5) * WORLD_H * 0.82,
      freqX: 0.12 + seededUnit(i, 4) * 0.22,
      freqY: 0.12 + seededUnit(i, 5) * 0.22,
      hot: seededUnit(i, 7) > 0.72,
      phaseX: seededUnit(i, 8) * Math.PI * 2,
      phaseY: seededUnit(i, 9) * Math.PI * 2,
      pulse: 0.6 + seededUnit(i, 10) * 1.4,
    })
  }
  return blobs
}

const blobPosition = (blob: Blob, t: number): { x: number; y: number } => ({
  x: blob.baseX + Math.sin(t * blob.freqX + blob.phaseX) * blob.ampX,
  y: blob.baseY + Math.cos(t * blob.freqY + blob.phaseY) * blob.ampY,
})

// A crisp [x,y] readout drawn to a 2D canvas, shown as a camera-facing sprite.
type Label = Readonly<{
  sprite: Three.Sprite
  set: (x: number, y: number) => void
  dispose: () => void
}>

const makeLabel = (hot: boolean): Label => {
  const canvas = document.createElement('canvas')
  canvas.width = 256
  canvas.height = 64
  const ctx = canvas.getContext('2d')
  const texture = new Three.CanvasTexture(canvas)
  texture.minFilter = Three.LinearFilter
  const material = new Three.SpriteMaterial({
    depthWrite: false,
    map: texture,
    transparent: true,
  })
  const sprite = new Three.Sprite(material)
  sprite.scale.set(0.95, 0.24, 1)
  let current = ''
  const set = (x: number, y: number): void => {
    const next = `[${x},${y}]`
    if (next === current || ctx === null) return
    current = next
    ctx.clearRect(0, 0, canvas.width, canvas.height)
    ctx.font = '600 30px "SF Mono", ui-monospace, Menlo, monospace'
    ctx.textBaseline = 'middle'
    ctx.fillStyle = hot ? 'rgba(255,176,120,0.95)' : 'rgba(214,246,255,0.82)'
    ctx.fillText(next, 8, 34)
    texture.needsUpdate = true
  }
  return {
    dispose: () => {
      texture.dispose()
      material.dispose()
    },
    set,
    sprite,
  }
}

const mountBlobTracking = (element: HTMLElement): AnimationHandle => {
  const { canvas, size } = webglCanvas(element)
  const renderer = new Three.WebGLRenderer({ alpha: true, antialias: true, canvas })
  renderer.setClearColor(0x000000, 0)

  const scene = new Three.Scene()
  const camera = new Three.PerspectiveCamera(45, 0.5625, 0.1, 100)
  camera.position.z = 6.4

  const disposables: Array<{ dispose: () => void }> = []
  const blobs = makeBlobs()

  // Background strands (the grass-like vertical lines from the source video).
  const strandGroup = new Three.Group()
  const strandLines: Array<{ line: Three.Line; baseX: number; sway: number; phase: number }> = []
  for (let i = 0; i < STRANDS; i += 1) {
    const segments = 32
    const positions = new Float32Array((segments + 1) * 3)
    const baseX = (seededUnit(i, 20) - 0.5) * WORLD_W * 1.4
    for (let s = 0; s <= segments; s += 1) {
      positions[s * 3] = baseX
      positions[s * 3 + 1] = (s / segments - 0.5) * WORLD_H * 1.2
      positions[s * 3 + 2] = -1.5 - seededUnit(i, 21) * 1.5
    }
    const geometry = new Three.BufferGeometry()
    geometry.setAttribute('position', new Three.BufferAttribute(positions, 3))
    const material = new Three.LineBasicMaterial({
      color: 0x1f5a3a,
      opacity: 0.22,
      transparent: true,
    })
    const line = new Three.Line(geometry, material)
    strandGroup.add(line)
    strandLines.push({
      baseX,
      line,
      phase: seededUnit(i, 22) * Math.PI * 2,
      sway: 0.05 + seededUnit(i, 23) * 0.12,
    })
    disposables.push(geometry, material)
  }
  scene.add(strandGroup)

  // Drifting dust.
  const dustPositions = new Float32Array(DUST * 3)
  for (let i = 0; i < DUST; i += 1) {
    dustPositions[i * 3] = (seededUnit(i, 30) - 0.5) * WORLD_W * 1.4
    dustPositions[i * 3 + 1] = (seededUnit(i, 31) - 0.5) * WORLD_H * 1.2
    dustPositions[i * 3 + 2] = -0.5 - seededUnit(i, 32) * 1.5
  }
  const dustGeometry = new Three.BufferGeometry()
  const dustAttr = new Three.BufferAttribute(dustPositions, 3)
  dustGeometry.setAttribute('position', dustAttr)
  const dustMaterial = new Three.PointsMaterial({
    blending: Three.AdditiveBlending,
    color: 0xbfe8d8,
    depthWrite: false,
    opacity: 0.5,
    size: 0.02,
    transparent: true,
  })
  scene.add(new Three.Points(dustGeometry, dustMaterial))
  disposables.push(dustGeometry, dustMaterial)

  // Blob tracking rings — instanced via three-effect.
  const ringGeometry = new Three.RingGeometry(0.86, 1, 40)
  const ringMaterial = new Three.MeshBasicMaterial({
    transparent: true,
    vertexColors: true,
  })
  const initialTransforms: InstanceTransform[] = blobs.map(() => ({ scale: 0.0001 }))
  const ringMesh = createInstancedMesh(ringGeometry, ringMaterial, initialTransforms)
  scene.add(ringMesh)
  disposables.push(ringGeometry, ringMaterial, ringMesh)

  // Filled cores (bright for "hot" blobs, faint for the rest).
  const coreGeometry = new Three.CircleGeometry(1, 28)
  const coreMaterial = new Three.MeshBasicMaterial({
    blending: Three.AdditiveBlending,
    depthWrite: false,
    transparent: true,
    vertexColors: true,
  })
  const coreMesh = createInstancedMesh(coreGeometry, coreMaterial, initialTransforms)
  scene.add(coreMesh)
  disposables.push(coreGeometry, coreMaterial, coreMesh)

  // Connector spline threading the tracked centroids.
  const connectorGeometry = new Three.BufferGeometry()
  const connectorMaterial = new Three.LineBasicMaterial({
    blending: Three.AdditiveBlending,
    color: 0xd6f6ff,
    opacity: 0.2,
    transparent: true,
  })
  const connector = new Three.Line(connectorGeometry, connectorMaterial)
  scene.add(connector)
  disposables.push(connectorGeometry, connectorMaterial)

  // Coordinate readouts.
  const labels = blobs.map(blob => {
    const label = makeLabel(blob.hot)
    scene.add(label.sprite)
    disposables.push(label)
    return label
  })

  const ringColor = new Three.Color(0xd6f6ff)
  const hotColor = new Three.Color(0xff7a3c)
  const coldCore = new Three.Color(0x0a1a22)

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
    const t = time * 0.001

    const ringTransforms: InstanceTransform[] = []
    const coreTransforms: InstanceTransform[] = []
    const centroids: Three.Vector3[] = []

    for (let i = 0; i < blobs.length; i += 1) {
      const blob = blobs[i]
      if (blob === undefined) continue
      const { x, y } = blobPosition(blob, t)
      const radius = blob.baseRadius * (1 + Math.sin(t * blob.pulse + blob.phaseX) * 0.18)
      centroids.push(new Three.Vector3(x, y, 0))

      ringTransforms.push({
        color: blob.hot ? hotColor : ringColor,
        position: [x, y, 0],
        scale: radius,
      })
      coreTransforms.push({
        color: blob.hot ? hotColor : coldCore,
        position: [x, y, -0.01],
        scale: radius * (blob.hot ? 0.55 : 0.32),
      })

      const label = labels[i]
      if (label !== undefined) {
        label.sprite.position.set(x + radius + 0.55, y + radius + 0.18, 0.1)
        const vx = Math.round((x / WORLD_W + 0.5) * VRES_W)
        const vy = Math.round((0.5 - y / WORLD_H) * VRES_H)
        label.set(vx, vy)
      }
    }

    applyInstanceTransforms(ringMesh, ringTransforms)
    applyInstanceTransforms(coreMesh, coreTransforms)

    if (centroids.length >= 2) {
      const curve = new Three.CatmullRomCurve3(centroids, false, 'catmullrom', 0.6)
      connectorGeometry.setFromPoints(curve.getPoints(160))
    }

    // Sway the strands and drift dust.
    for (const strand of strandLines) {
      const attr = strand.line.geometry.getAttribute('position')
      if (attr instanceof Three.BufferAttribute) {
        for (let s = 0; s < attr.count; s += 1) {
          const yy = attr.getY(s)
          attr.setX(s, strand.baseX + Math.sin(t * 0.4 + yy * 0.6 + strand.phase) * strand.sway)
        }
        attr.needsUpdate = true
      }
    }
    for (let i = 0; i < DUST; i += 1) {
      const k = i * 3 + 1
      const next = (dustPositions[k] ?? 0) + 0.0008 + (i % 5) * 0.0002
      dustPositions[k] = next > WORLD_H * 0.6 ? -WORLD_H * 0.6 : next
    }
    dustAttr.needsUpdate = true

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

export const blobTrackingView = makeAnimationView('oa-anim-blob-tracking', mountBlobTracking)
