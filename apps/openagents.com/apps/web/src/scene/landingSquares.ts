import * as Three from 'three'

// A calm, ambient WebGL background for the standalone `/landing` page: a few
// white squares drifting and gently pulsing on a pure-black field. Deliberately
// low-key — slow motion, low opacity, no chrome — so it reads as a quiet
// backdrop rather than a loud animation. Built on the same Three.js stack as
// `@openagentsinc/three-effect` and modeled on the repo's existing ambient
// scenes (see `lightBeams.ts`), kept self-contained for the landing route.

const WORLD_HEIGHT = 6
const BACKGROUND_COLOR = 0x000000
const SQUARE_COLOR = 0xf2f4f8

type Square = Readonly<{
  baseOpacity: number
  drift: number
  mesh: Three.Mesh<Three.PlaneGeometry, Three.MeshBasicMaterial>
  phase: number
  pulseSpeed: number
  size: number
  spin: number
  x: number
  y: number
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

// Fixed, hand-placed layout so the few squares read as a deliberate, balanced
// composition rather than random noise. Values are in world units relative to
// WORLD_HEIGHT; x is recentred per-aspect at resize time.
const squareLayout: ReadonlyArray<
  Readonly<{
    baseOpacity: number
    drift: number
    phase: number
    pulseSpeed: number
    size: number
    spin: number
    x: number
    y: number
  }>
> = [
  { baseOpacity: 0.26, drift: 0.18, phase: 0.0, pulseSpeed: 0.22, size: 0.62, spin: 0.05, x: -1.9, y: 1.3 },
  { baseOpacity: 0.18, drift: 0.12, phase: 0.4, pulseSpeed: 0.17, size: 1.05, spin: -0.04, x: 1.6, y: 0.5 },
  { baseOpacity: 0.32, drift: 0.22, phase: 0.8, pulseSpeed: 0.28, size: 0.4, spin: 0.08, x: 0.2, y: -1.4 },
  { baseOpacity: 0.16, drift: 0.1, phase: 1.5, pulseSpeed: 0.14, size: 0.82, spin: -0.06, x: -1.2, y: -0.9 },
  { baseOpacity: 0.24, drift: 0.16, phase: 2.1, pulseSpeed: 0.2, size: 0.5, spin: 0.07, x: 2.2, y: 1.8 },
]

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
  renderer.setClearColor(BACKGROUND_COLOR, 1)

  const scene = new Three.Scene()
  const camera = new Three.OrthographicCamera(-1, 1, 1, -1, 0.1, 1000)
  camera.position.set(0, 0, 50)
  camera.lookAt(0, 0, 0)

  const geometry = new Three.PlaneGeometry(1, 1)
  const squares: ReadonlyArray<Square> = squareLayout.map(spec => {
    const material = new Three.MeshBasicMaterial({
      color: new Three.Color(SQUARE_COLOR),
      depthTest: false,
      depthWrite: false,
      opacity: spec.baseOpacity,
      transparent: true,
    })
    const mesh = new Three.Mesh(geometry, material)
    mesh.position.set(spec.x, spec.y, -1)
    mesh.scale.set(spec.size, spec.size, 1)
    mesh.renderOrder = -1
    scene.add(mesh)
    return { ...spec, mesh }
  })

  let size = hostSize(element)
  const ratio = (): number => Math.min(window.devicePixelRatio || 1, pixelRatio)

  const resize = (): void => {
    size = hostSize(element)
    const aspect = size.width / size.height
    const worldWidth = WORLD_HEIGHT * aspect
    renderer.setPixelRatio(ratio())
    renderer.setSize(size.width, size.height, false)
    camera.left = -worldWidth / 2
    camera.right = worldWidth / 2
    camera.top = WORLD_HEIGHT / 2
    camera.bottom = -WORLD_HEIGHT / 2
    camera.updateProjectionMatrix()
  }

  let disposed = false
  let frame = 0

  const renderScene = (time: number): void => {
    if (disposed) return
    const seconds = time * 0.001
    squares.forEach(square => {
      const wave = Math.sin(seconds * square.pulseSpeed + square.phase * Math.PI * 2)
      // Very small positional drift so the field feels alive but never busy.
      square.mesh.position.set(
        square.x + Math.sin(seconds * 0.12 + square.phase * Math.PI * 2) * square.drift,
        square.y + Math.cos(seconds * 0.1 + square.phase * Math.PI * 2) * square.drift * 0.7,
        square.mesh.position.z,
      )
      square.mesh.rotation.z = Math.sin(seconds * 0.18 + square.phase) * square.spin
      // Gentle opacity pulse around the base, staying subtle on black.
      square.mesh.material.opacity = Math.max(
        0.02,
        square.baseOpacity + wave * square.baseOpacity * 0.45,
      )
    })

    renderer.render(scene, camera)
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
        square.mesh.material.dispose()
      })
      geometry.dispose()
      renderer.dispose()
      element.replaceChildren()
    },
  }
}
