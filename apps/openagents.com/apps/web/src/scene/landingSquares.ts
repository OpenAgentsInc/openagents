import * as Three from 'three'
import { EffectComposer } from 'three/examples/jsm/postprocessing/EffectComposer.js'
import { OutputPass } from 'three/examples/jsm/postprocessing/OutputPass.js'
import { RenderPass } from 'three/examples/jsm/postprocessing/RenderPass.js'
import { UnrealBloomPass } from 'three/examples/jsm/postprocessing/UnrealBloomPass.js'

// 3D pylon-network scene for the standalone homepage surface. A constellation
// of glowing pylon cores connected by faint energy lines on a near-black field,
// rendered through an EffectComposer -> UnrealBloomPass -> OutputPass chain so
// the HDR-emissive blue (color above 1.0, `toneMapped = false`) blooms in the
// dark. OutputPass owns tone-mapping, so the renderer stays linear/no-tone-map.
//
// The whole point: ONE persistent scene with named CAMERA POSES. Navigating
// `/` <-> `/khala` tweens the camera between vantages — a continuous move
// through the same space — instead of cutting between two pages. The host passes
// the active pose via `setPose`; the render loop eases the camera toward it every
// frame (exponential smoothing), so the transition is smooth and interruptible.
//
// Self-contained + lightweight, mirroring the pylon glow-up recipe
// (three-effect #16 / openagents #6068) without pulling the full Foldkit/three
// scene host for this raw background.

export type LandingPose =
  | 'landing'
  | 'khala'
  | 'tassadar'
  | 'autopilot'
  | 'login'

const BACKGROUND_COLOR = 0x000000
// A tasteful cool blue, driven above 1.0 (HDR) so the cores survive the bloom
// threshold and glow; lines ride a little lower.
const PYLON_BLUE = new Three.Color(0x3a7bff)
const PYLON_COUNT = 18
const HDR_CORE = 2.4 // pylon-core HDR strength (blooms strongly)
const HDR_LINE = 1.15 // connection-line HDR strength (faint energy)
const NEIGHBORS = 2 // connections per pylon

// Camera poses: position + look-at target. `landing` is a wide establishing
// vantage centered on the constellation; `khala` flies in and rotates to a
// different part of the same space, so entering Khala reads as travelling there.
// `tassadar` flies the camera up and across to a third, distinct vantage —
// looking down the network from above-left — so entering /tassadar is another
// continuous move through the same space, not a cut.
// `autopilot` is the onboarding vantage: the camera drops low and pushes in
// close from below-right, banking up into the heart of the constellation so
// the pylons loom overhead. It reads as arriving inside the network (you are
// now flying it), distinct from every other pose — closer and more immersive
// than `landing`, and on the opposite side from `tassadar`'s above-left view.
// `login` is a calm, settled vantage close to `landing` but eased gently to the
// right and a touch lower, so the constellation drifts to frame a centered sign-in
// card; navigating /  <-> /login is a short continuous glide, not a cut.
export const POSES: Record<
  LandingPose,
  { pos: Three.Vector3; target: Three.Vector3 }
> = {
  landing: {
    pos: new Three.Vector3(0, 1.2, 19),
    target: new Three.Vector3(0, 0, 0),
  },
  khala: {
    pos: new Three.Vector3(9.5, -2.6, 8.5),
    target: new Three.Vector3(2.6, -0.6, -2.2),
  },
  tassadar: {
    pos: new Three.Vector3(-8.5, 6.2, 11),
    target: new Three.Vector3(-1.4, 1.1, -2),
  },
  autopilot: {
    pos: new Three.Vector3(5.5, -4.5, 6.0),
    target: new Three.Vector3(0.4, 1.6, -3.2),
  },
  login: {
    pos: new Three.Vector3(3.4, -0.4, 16.5),
    target: new Three.Vector3(-1.1, 0.2, -0.6),
  },
}

// Deterministic hash in [0, 1) so pylon placement + the glow subset stay stable
// across reloads (no Math.random flicker).
const hash01 = (i: number, salt: number): number => {
  const n = Math.sin(i * 127.1 + salt * 311.7) * 43758.5453
  return n - Math.floor(n)
}

// A loose constellation in a flattened volume around the origin.
const pylonPositions = (): Three.Vector3[] => {
  const pts: Three.Vector3[] = []
  const golden = Math.PI * (3 - Math.sqrt(5))
  for (let i = 0; i < PYLON_COUNT; i += 1) {
    const r = 3 + hash01(i, 1) * 7.5
    const a = i * golden
    pts.push(
      new Three.Vector3(
        Math.cos(a) * r,
        (hash01(i, 2) - 0.5) * 7.5,
        Math.sin(a) * r * 0.85 - 2,
      ),
    )
  }
  return pts
}

// HDR-emissive blue: color * strength, additive, no tone-map on the material so
// the OutputPass tone-mapper at the end of the chain is the only one.
const makeEmissiveMaterial = (strength: number): Three.MeshBasicMaterial => {
  const material = new Three.MeshBasicMaterial({
    blending: Three.AdditiveBlending,
    color: PYLON_BLUE.clone().multiplyScalar(strength),
    depthWrite: false,
    transparent: true,
  })
  material.toneMapped = false
  return material
}

type Pylon = Readonly<{
  baseStrength: number
  core: Three.Mesh<Three.SphereGeometry, Three.MeshBasicMaterial>
  phase: number
  pulseSpeed: number
}>

const hostSize = (element: HTMLElement): { height: number; width: number } => {
  const rect = element.getBoundingClientRect()
  const width = Math.max(1, Math.floor(rect.width || element.clientWidth || 320))
  const height = Math.max(
    1,
    Math.floor(rect.height || element.clientHeight || 420),
  )
  return { height, width }
}

export type LandingSquaresOptions = Readonly<{
  pixelRatio?: number
  pose?: LandingPose
}>

export type LandingSquaresHandle = Readonly<{
  dispose: () => void
  setPose: (pose: LandingPose) => void
}>

export const mountLandingSquares = (
  element: HTMLElement,
  options: LandingSquaresOptions = {},
): LandingSquaresHandle => {
  const pixelRatio = options.pixelRatio ?? 2

  element.replaceChildren()
  element.style.position = 'absolute'
  element.style.inset = '0'
  element.style.overflow = 'hidden'
  element.style.background = '#000'

  const canvas = document.createElement('canvas')
  canvas.style.display = 'block'
  canvas.style.inset = '0'
  canvas.style.position = 'absolute'
  canvas.style.height = '100%'
  canvas.style.width = '100%'
  element.append(canvas)

  // Vignette overlay: radial gradient above the canvas (transparent center ->
  // near-black edges) for a moody falloff. Pointer-inert; the wordmark sits
  // above it.
  const vignette = document.createElement('div')
  vignette.style.position = 'absolute'
  vignette.style.inset = '0'
  vignette.style.pointerEvents = 'none'
  vignette.style.background =
    'radial-gradient(ellipse 85% 85% at 50% 50%,' +
    ' rgba(0,0,0,0) 0%,' +
    ' rgba(0,0,0,0) 38%,' +
    ' rgba(0,0,0,0.30) 62%,' +
    ' rgba(0,0,0,0.72) 85%,' +
    ' rgba(0,0,0,0.95) 100%)'
  element.append(vignette)

  const renderer = new Three.WebGLRenderer({
    alpha: false,
    antialias: true,
    canvas,
  })
  renderer.outputColorSpace = Three.SRGBColorSpace
  renderer.toneMapping = Three.NoToneMapping
  renderer.setClearColor(BACKGROUND_COLOR, 1)

  const scene = new Three.Scene()
  const camera = new Three.PerspectiveCamera(52, 1, 0.1, 200)

  // The constellation lives in a group so the whole network drifts slowly
  // (ambient life) independent of the camera move.
  const group = new Three.Group()
  scene.add(group)

  const positions = pylonPositions()

  // Pylon cores: small octahedra with HDR-emissive blue that pulse gently.
  const coreGeometry = new Three.SphereGeometry(0.32, 20, 20)
  const pylons: Pylon[] = positions.map((p, i) => {
    const baseStrength = HDR_CORE * (0.75 + hash01(i, 3) * 0.6)
    const material = makeEmissiveMaterial(baseStrength)
    const core = new Three.Mesh(coreGeometry, material)
    core.position.copy(p)
    group.add(core)
    return {
      baseStrength,
      core,
      phase: hash01(i, 4),
      pulseSpeed: 0.4 + hash01(i, 5) * 0.5,
    }
  })

  // Connection lines: each pylon links to its nearest neighbors. Deduped into a
  // single additive HDR LineSegments so the network reads as faint energy that
  // brightens through the bloom pass.
  const seen = new Set<string>()
  const linePoints: number[] = []
  positions.forEach((p, i) => {
    positions
      .map((q, j) => ({ d: p.distanceTo(q), j }))
      .filter(o => o.j !== i)
      .sort((a, b) => a.d - b.d)
      .slice(0, NEIGHBORS)
      .forEach(({ j }) => {
        const key = i < j ? `${i}:${j}` : `${j}:${i}`
        if (seen.has(key)) return
        seen.add(key)
        const q = positions[j]
        if (q === undefined) return
        linePoints.push(p.x, p.y, p.z, q.x, q.y, q.z)
      })
  })
  const lineGeometry = new Three.BufferGeometry()
  lineGeometry.setAttribute(
    'position',
    new Three.Float32BufferAttribute(linePoints, 3),
  )
  const lineMaterial = new Three.LineBasicMaterial({
    blending: Three.AdditiveBlending,
    color: PYLON_BLUE.clone().multiplyScalar(HDR_LINE),
    depthWrite: false,
    transparent: true,
  })
  lineMaterial.toneMapped = false
  const lines = new Three.LineSegments(lineGeometry, lineMaterial)
  group.add(lines)

  // Sparse drifting sparks for depth/energy (additive HDR points).
  const SPARKS = 140
  const sparkPositions = new Float32Array(SPARKS * 3)
  for (let i = 0; i < SPARKS; i += 1) {
    sparkPositions[i * 3] = (hash01(i, 6) - 0.5) * 34
    sparkPositions[i * 3 + 1] = (hash01(i, 7) - 0.5) * 20
    sparkPositions[i * 3 + 2] = (hash01(i, 8) - 0.5) * 30 - 4
  }
  const sparkGeometry = new Three.BufferGeometry()
  sparkGeometry.setAttribute(
    'position',
    new Three.Float32BufferAttribute(sparkPositions, 3),
  )
  const sparkMaterial = new Three.PointsMaterial({
    blending: Three.AdditiveBlending,
    color: PYLON_BLUE.clone().multiplyScalar(1.4),
    depthWrite: false,
    size: 0.06,
    sizeAttenuation: true,
    transparent: true,
  })
  sparkMaterial.toneMapped = false
  const sparks = new Three.Points(sparkGeometry, sparkMaterial)
  group.add(sparks)

  // EffectComposer: RenderPass -> UnrealBloomPass (HDR glow) -> OutputPass.
  // Threshold sits below the HDR cores (color > 1.0) so only the blue energy
  // blooms while the near-black field stays dark.
  const composer = new EffectComposer(renderer)
  composer.addPass(new RenderPass(scene, camera))
  const bloomPass = new UnrealBloomPass(
    new Three.Vector2(1, 1),
    /* strength */ 0.9,
    /* radius */ 0.6,
    /* threshold */ 0.85,
  )
  composer.addPass(bloomPass)
  composer.addPass(new OutputPass())

  // Camera pose state: the camera eases toward `targetPos`/`targetLook` every
  // frame, so changing the pose produces a continuous flight rather than a cut.
  const initialPose: LandingPose = options.pose ?? 'landing'
  const camPos = POSES[initialPose].pos.clone()
  const camLook = POSES[initialPose].target.clone()
  const targetPos = POSES[initialPose].pos.clone()
  const targetLook = POSES[initialPose].target.clone()
  camera.position.copy(camPos)
  camera.lookAt(camLook)

  const setPose = (pose: LandingPose): void => {
    const next = POSES[pose] ?? POSES.landing
    targetPos.copy(next.pos)
    targetLook.copy(next.target)
  }

  let size = hostSize(element)
  const ratio = (): number => Math.min(window.devicePixelRatio || 1, pixelRatio)

  const resize = (): void => {
    size = hostSize(element)
    const r = ratio()
    renderer.setPixelRatio(r)
    renderer.setSize(size.width, size.height, false)
    composer.setPixelRatio(r)
    composer.setSize(size.width, size.height)
    bloomPass.setSize(size.width * r, size.height * r)
    camera.aspect = size.width / size.height
    camera.updateProjectionMatrix()
  }

  let disposed = false
  let frame = 0
  const colorScratch = new Three.Color()

  const renderScene = (time: number): void => {
    if (disposed) return
    const seconds = time * 0.001

    // Ease the camera toward the active pose (continuous flight on nav).
    camPos.lerp(targetPos, 0.045)
    camLook.lerp(targetLook, 0.045)
    camera.position.copy(camPos)
    camera.lookAt(camLook)

    // Ambient life: the whole constellation drifts slowly.
    group.rotation.y = seconds * 0.04
    group.rotation.x = Math.sin(seconds * 0.13) * 0.05

    // Each core breathes around its base HDR strength.
    pylons.forEach(pylon => {
      const wave = Math.sin(seconds * pylon.pulseSpeed + pylon.phase * Math.PI * 2)
      const strength = pylon.baseStrength * (1 + wave * 0.28)
      colorScratch.copy(PYLON_BLUE).multiplyScalar(Math.max(0.2, strength))
      pylon.core.material.color.copy(colorScratch)
    })

    composer.render()
    frame = requestAnimationFrame(renderScene)
  }

  const observer =
    typeof ResizeObserver === 'undefined'
      ? null
      : new ResizeObserver(() => resize())

  resize()
  observer?.observe(element)
  frame = requestAnimationFrame(renderScene)

  return {
    dispose: () => {
      if (disposed) return
      disposed = true
      cancelAnimationFrame(frame)
      observer?.disconnect()
      pylons.forEach(pylon => {
        group.remove(pylon.core)
        pylon.core.material.dispose()
      })
      coreGeometry.dispose()
      lineGeometry.dispose()
      lineMaterial.dispose()
      sparkGeometry.dispose()
      sparkMaterial.dispose()
      bloomPass.dispose()
      composer.dispose()
      renderer.dispose()
      element.replaceChildren()
    },
    setPose,
  }
}
