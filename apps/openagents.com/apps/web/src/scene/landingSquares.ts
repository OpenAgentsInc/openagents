import * as Three from 'three'
import { EffectComposer } from 'three/examples/jsm/postprocessing/EffectComposer.js'
import { OutputPass } from 'three/examples/jsm/postprocessing/OutputPass.js'
import { RenderPass } from 'three/examples/jsm/postprocessing/RenderPass.js'
import { UnrealBloomPass } from 'three/examples/jsm/postprocessing/UnrealBloomPass.js'

// Ambient WebGL background for the standalone `/landing` page: a dense grid of
// a few hundred small blue squares, evenly spaced on a near-black field, that
// glow with a soft bloom. The look ports the pylon-network glow-up
// (three-effect #16 / openagents #6068): the squares carry HDR-emissive blue
// (color above 1.0, `toneMapped = false`) and the render loop runs through an
// EffectComposer -> UnrealBloomPass -> OutputPass chain so they read as energy
// in the dark. OutputPass owns tone-mapping, so the renderer must NOT tone-map
// (no double tone-map). This mirrors `createEffectComposerResources` from
// `@openagentsinc/three-effect` but stays self-contained so the landing route
// owns a small, lightweight composer for its raw scene.

const WORLD_HEIGHT = 6
const BACKGROUND_COLOR = 0x000000

// A tasteful blue. Driven above 1.0 (HDR) so it survives the bloom threshold
// and blooms in the dark; the base color stays a calm, cool blue.
const SQUARE_BLUE = new Three.Color(0x3a7bff)
// HDR strength: emissive = color * strength. Above 1.0 the pixels exceed the
// bloom threshold and glow; the grid stays calm because each square is small.
const HDR_STRENGTH = 1.85

// Grid layout: ~24 columns x 16 rows = 384 small squares with clear gaps. The
// columns are recentred per-aspect at resize; rows fill WORLD_HEIGHT.
const GRID_COLUMNS = 24
const GRID_ROWS = 16
// Square edge as a fraction of the cell pitch — small, with clear space around.
const SQUARE_FILL = 0.36

type Square = Readonly<{
  baseIntensity: number
  col: number
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

  const canvas = document.createElement('canvas')
  canvas.style.display = 'block'
  canvas.style.inset = '0'
  canvas.style.position = 'absolute'
  canvas.style.height = '100%'
  canvas.style.width = '100%'
  element.append(canvas)

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
      // A calm, low-amplitude per-square base so the grid has texture without
      // looking noisy; the phase staggers the pulse across the field.
      const baseIntensity = 0.78 + ((row * 7 + col * 13) % 11) * 0.02
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
  const bloomPass = new UnrealBloomPass(
    new Three.Vector2(1, 1),
    /* strength */ 0.85,
    /* radius */ 0.6,
    /* threshold */ 0.85,
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
    squares.forEach(square => {
      const x = (square.col - (GRID_COLUMNS - 1) / 2) * cellW
      const y = (square.row - (GRID_ROWS - 1) / 2) * cellH
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
      const wave = Math.sin(
        seconds * square.pulseSpeed + square.phase * Math.PI * 2,
      )
      const intensity = square.baseIntensity * (1 + wave * 0.22)
      blueScratch
        .copy(SQUARE_BLUE)
        .multiplyScalar(HDR_STRENGTH * Math.max(0.2, intensity))
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
