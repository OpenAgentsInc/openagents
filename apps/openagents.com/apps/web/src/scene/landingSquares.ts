import * as Three from 'three'
import { EffectComposer } from 'three/examples/jsm/postprocessing/EffectComposer.js'
import { OutputPass } from 'three/examples/jsm/postprocessing/OutputPass.js'
import { RenderPass } from 'three/examples/jsm/postprocessing/RenderPass.js'
import { UnrealBloomPass } from 'three/examples/jsm/postprocessing/UnrealBloomPass.js'

// Ambient WebGL background for the standalone `/landing` page: a dense grid of
// a few hundred small blue squares on a near-black field. The mood is DARK and
// subtle, not a bright pegboard: most squares read as very dim/near-black, only
// a sparse deterministic subset keeps a faint blue glow, the center is cleared
// (no squares behind the "OpenAgents" wordmark), and a vignette darkens the
// periphery so visibility falls off toward the edges.
//
// The look ports the pylon-network glow-up (three-effect #16 / openagents
// #6068): the glowing squares carry HDR-emissive blue (color above 1.0,
// `toneMapped = false`) and the render loop runs through an EffectComposer ->
// UnrealBloomPass -> OutputPass chain so they read as faint energy in the dark.
// OutputPass owns tone-mapping, so the renderer must NOT tone-map (no double
// tone-map). This mirrors `createEffectComposerResources` from
// `@openagentsinc/three-effect` but stays self-contained so the landing route
// owns a small, lightweight composer for its raw scene.

const WORLD_HEIGHT = 6
const BACKGROUND_COLOR = 0x000000

// A tasteful blue. Driven above 1.0 (HDR) only for the sparse glowing subset so
// those pixels survive the bloom threshold; the base color stays a calm, cool
// blue.
const SQUARE_BLUE = new Three.Color(0x3a7bff)
// HDR strength for the GLOWING subset: emissive = color * strength * intensity.
// Tuned so the lit squares bloom faintly without flooding the field.
const HDR_STRENGTH = 1.55

// Dim squares (the majority) sit well below the bloom threshold so they read as
// very dark texture rather than energy. This is the "way darker" base.
const DIM_INTENSITY = 0.16
// Glowing squares (the sparse subset) ride above 1.0 so they bloom.
const GLOW_INTENSITY = 1.0
// Deterministic fraction of squares that keep a subtle glow (~14%).
const GLOW_FRACTION = 0.14

// Grid layout: ~24 columns x 16 rows = 384 small squares with clear gaps. The
// columns are recentred per-aspect at resize; rows fill WORLD_HEIGHT.
const GRID_COLUMNS = 24
const GRID_ROWS = 16
// Square edge as a fraction of the cell pitch — small, with clear space around.
const SQUARE_FILL = 0.36

// Cleared-center radius as a fraction of the smaller world dimension. The
// wordmark plus a comfortable margin sits on pure black; squares whose center
// falls inside this radius are hidden. Responsive to aspect (wider viewports
// clear a wider horizontal band so the wide wordmark stays clean).
const CENTER_CLEAR_RATIO = 0.52

// Deterministic hash in [0, 1) so the glowing subset and per-square variation
// stay stable frame-to-frame and across reloads (no Math.random flicker).
const hash01 = (row: number, col: number): number => {
  const n = Math.sin(row * 127.1 + col * 311.7) * 43758.5453
  return n - Math.floor(n)
}

type Square = Readonly<{
  baseIntensity: number
  col: number
  glowing: boolean
  material: Three.MeshBasicMaterial
  mesh: Three.Mesh<Three.PlaneGeometry, Three.MeshBasicMaterial>
  phase: number
  pulseSpeed: number
  row: number
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

// HDR-emissive blue: color * strength, additive, no tone-map on the material so
// the OutputPass tone-mapper at the end of the chain is the only one. This is
// the same "glow in the dark" recipe the pylon cores use.
const makeEmissiveBlueMaterial = (
  intensity: number,
): Three.MeshBasicMaterial => {
  const material = new Three.MeshBasicMaterial({
    blending: Three.AdditiveBlending,
    color: SQUARE_BLUE.clone().multiplyScalar(HDR_STRENGTH * intensity),
    depthTest: false,
    depthWrite: false,
    transparent: true,
  })
  material.toneMapped = false
  return material
}

export type LandingSquaresOptions = Readonly<{
  pixelRatio?: number
}>

export type LandingSquaresHandle = Readonly<{ dispose: () => void }>

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

  // Vignette overlay: a radial gradient layered above the canvas (transparent
  // center -> near-black edges) so brightness falls off toward the periphery
  // for a moody look. Pointer-inert; the wordmark in the page sits above it.
  const vignette = document.createElement('div')
  vignette.style.position = 'absolute'
  vignette.style.inset = '0'
  vignette.style.pointerEvents = 'none'
  vignette.style.background =
    'radial-gradient(ellipse 80% 80% at 50% 50%,' +
    ' rgba(0,0,0,0) 0%,' +
    ' rgba(0,0,0,0) 30%,' +
    ' rgba(0,0,0,0.35) 55%,' +
    ' rgba(0,0,0,0.78) 82%,' +
    ' rgba(0,0,0,0.97) 100%)'
  element.append(vignette)

  const renderer = new Three.WebGLRenderer({
    alpha: false,
    antialias: true,
    canvas,
  })
  renderer.outputColorSpace = Three.SRGBColorSpace
  // OutputPass owns tone-mapping at the end of the composer chain; the renderer
  // must stay linear/no-tone-map so we do not tone-map twice.
  renderer.toneMapping = Three.NoToneMapping
  renderer.setClearColor(BACKGROUND_COLOR, 1)

  const scene = new Three.Scene()
  const camera = new Three.OrthographicCamera(-1, 1, 1, -1, 0.1, 1000)
  camera.position.set(0, 0, 50)
  camera.lookAt(0, 0, 0)

  // One shared geometry; each square gets its own emissive material so the
  // gentle per-square pulse can ride the HDR intensity independently.
  const geometry = new Three.PlaneGeometry(1, 1)
  const squares: Square[] = []
  for (let row = 0; row < GRID_ROWS; row += 1) {
    for (let col = 0; col < GRID_COLUMNS; col += 1) {
      // A deterministic subset glows; the rest stay very dim. The glowing base
      // also varies a little so the lit squares are not all identical.
      const glowing = hash01(row, col) < GLOW_FRACTION
      const baseIntensity = glowing
        ? GLOW_INTENSITY * (0.85 + hash01(row + 3, col + 7) * 0.3)
        : DIM_INTENSITY * (0.7 + hash01(row + 5, col + 11) * 0.6)
      const phase = ((row * 31 + col * 17) % 360) / 360
      const pulseSpeed = 0.16 + ((row + col) % 5) * 0.015
      const material = makeEmissiveBlueMaterial(baseIntensity)
      const mesh = new Three.Mesh(geometry, material)
      mesh.position.set(0, 0, -1)
      mesh.renderOrder = -1
      scene.add(mesh)
      squares.push({
        baseIntensity,
        col,
        glowing,
        material,
        mesh,
        phase,
        pulseSpeed,
        row,
      })
    }
  }

  // Self-contained EffectComposer mirroring `createEffectComposerResources`:
  // RenderPass -> UnrealBloomPass (HDR glow) -> OutputPass (tone-map). The
  // threshold sits below the HDR squares (which are color > 1.0) so only the
  // blue energy blooms while the near-black field stays dark.
  const composer = new EffectComposer(renderer)
  composer.addPass(new RenderPass(scene, camera))
  // Dialed way down from the old bright field: lower strength + higher
  // threshold so only the sparse HDR (>1.0) squares bloom, and faintly.
  const bloomPass = new UnrealBloomPass(
    new Three.Vector2(1, 1),
    /* strength */ 0.45,
    /* radius */ 0.55,
    /* threshold */ 0.95,
  )
  composer.addPass(bloomPass)
  composer.addPass(new OutputPass())

  let size = hostSize(element)
  const ratio = (): number => Math.min(window.devicePixelRatio || 1, pixelRatio)

  // Place the grid so it fills the viewport with even spacing and clear gaps.
  // Recomputed on resize because column spacing follows the world aspect.
  const layoutGrid = (): void => {
    const aspect = size.width / size.height
    const worldWidth = WORLD_HEIGHT * aspect
    const cellW = worldWidth / GRID_COLUMNS
    const cellH = WORLD_HEIGHT / GRID_ROWS
    const squareEdge = Math.min(cellW, cellH) * SQUARE_FILL
    // Cleared center: hide squares whose center sits inside an ellipse around
    // the middle so the wordmark sits on pure black. The ellipse follows the
    // world aspect (wider band on wide viewports for the wide wordmark) and the
    // base radius scales with the smaller world dimension.
    const clearRadius = (Math.min(worldWidth, WORLD_HEIGHT) / 2) * CENTER_CLEAR_RATIO
    const clearX = clearRadius * Math.max(1, Math.min(aspect, 2.2))
    const clearY = clearRadius
    squares.forEach(square => {
      const x = (square.col - (GRID_COLUMNS - 1) / 2) * cellW
      const y = (square.row - (GRID_ROWS - 1) / 2) * cellH
      const nx = x / clearX
      const ny = y / clearY
      square.mesh.visible = nx * nx + ny * ny >= 1
      square.mesh.position.set(x, y, -1)
      square.mesh.scale.set(squareEdge, squareEdge, 1)
    })
  }

  const resize = (): void => {
    size = hostSize(element)
    const aspect = size.width / size.height
    const worldWidth = WORLD_HEIGHT * aspect
    const r = ratio()
    renderer.setPixelRatio(r)
    renderer.setSize(size.width, size.height, false)
    composer.setPixelRatio(r)
    composer.setSize(size.width, size.height)
    bloomPass.setSize(size.width * r, size.height * r)
    camera.left = -worldWidth / 2
    camera.right = worldWidth / 2
    camera.top = WORLD_HEIGHT / 2
    camera.bottom = -WORLD_HEIGHT / 2
    camera.updateProjectionMatrix()
    layoutGrid()
  }

  let disposed = false
  let frame = 0
  const blueScratch = new Three.Color()

  const renderScene = (time: number): void => {
    if (disposed) return
    const seconds = time * 0.001
    // Calm ambient pulse: each square's HDR intensity breathes gently around
    // its base so the grid shimmers slowly without ever feeling busy.
    squares.forEach(square => {
      if (!square.mesh.visible) return
      // Only the glowing subset breathes; the dim majority stays steady-dark so
      // the field reads calm and mostly unlit.
      const amplitude = square.glowing ? 0.22 : 0.06
      const wave = Math.sin(
        seconds * square.pulseSpeed + square.phase * Math.PI * 2,
      )
      const intensity = square.baseIntensity * (1 + wave * amplitude)
      blueScratch
        .copy(SQUARE_BLUE)
        .multiplyScalar(HDR_STRENGTH * Math.max(0.05, intensity))
      square.material.color.copy(blueScratch)
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
      squares.forEach(square => {
        scene.remove(square.mesh)
        square.material.dispose()
      })
      geometry.dispose()
      bloomPass.dispose()
      composer.dispose()
      renderer.dispose()
      element.replaceChildren()
    },
  }
}
