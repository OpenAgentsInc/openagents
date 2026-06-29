import * as Three from 'three'

// The white diagonal "light beams" from the homepage pylon scene, extracted as
// a standalone, transparent, beams-only WebGL background — no diamonds, no
// refraction passes. Same beam shader + motion as the hero so it reads
// identically, but self-contained so it can sit behind any surface (e.g. the
// login screen). Deliberately a new function (not mountPylonDiamonds).

const WORLD_HEIGHT = 6
const STACK_GAP = 1.2
const LIGHT_BEAM_COUNT = 7

const lightBeamVertexShader = `
  varying vec2 beamUv;
  void main() {
    beamUv = uv;
    gl_Position = projectionMatrix * modelViewMatrix * vec4(position, 1.0);
  }
`

const lightBeamFragmentShader = `
  uniform vec3 color;
  uniform float opacity;
  varying vec2 beamUv;
  void main() {
    float center = 1.0 - smoothstep(0.0, 0.5, abs(beamUv.y - 0.5));
    float head = smoothstep(0.0, 0.22, beamUv.x);
    float tail = smoothstep(1.0, 0.72, beamUv.x);
    float alpha = opacity * pow(center, 1.7) * head * tail;
    gl_FragColor = vec4(color * (1.0 + center * 0.45), alpha);
  }
`

const createLightBeamMaterial = (opacity: number): Three.ShaderMaterial =>
  new Three.ShaderMaterial({
    blending: Three.AdditiveBlending,
    depthTest: false,
    depthWrite: false,
    fragmentShader: lightBeamFragmentShader,
    transparent: true,
    uniforms: {
      color: { value: new Three.Color(0xd6f6ff) },
      opacity: { value: opacity },
    },
    vertexShader: lightBeamVertexShader,
  })

type LightBeam = Readonly<{
  angle: number
  length: number
  mesh: Three.Mesh<Three.PlaneGeometry, Three.ShaderMaterial>
  phase: number
  speed: number
  thickness: number
  verticalOffset: number
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

export type LightBeamsOptions = Readonly<{
  pixelRatio?: number
}>

export type LightBeamsHandle = Readonly<{ dispose: () => void }>

export const mountLightBeams = (
  element: HTMLElement,
  options: LightBeamsOptions = {},
): LightBeamsHandle => {
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

  const renderer = new Three.WebGLRenderer({ alpha: true, antialias: true, canvas })
  renderer.outputColorSpace = Three.SRGBColorSpace
  renderer.setClearColor(0x000000, 0)

  const scene = new Three.Scene()
  const camera = new Three.OrthographicCamera(-1, 1, 1, -1, 0.1, 1000)
  camera.position.set(0, 0, 50)
  camera.lookAt(0, 0, 0)

  const geometry = new Three.PlaneGeometry(1, 1)
  const beams: ReadonlyArray<LightBeam> = Array.from(
    { length: LIGHT_BEAM_COUNT },
    (_, index) => {
      const mesh = new Three.Mesh(
        geometry,
        createLightBeamMaterial(0.2 + (index % 3) * 0.05),
      )
      mesh.position.z = -1 - index * 0.01
      mesh.renderOrder = -2
      scene.add(mesh)
      return {
        angle: index % 2 === 0 ? -0.74 : 0.74,
        length: 2.8 + (index % 4) * 0.35,
        mesh,
        phase: index / LIGHT_BEAM_COUNT,
        speed: 0.075 + index * 0.008,
        thickness: 0.035 + (index % 3) * 0.012,
        verticalOffset: index % 2 === 0 ? -STACK_GAP : STACK_GAP,
      }
    },
  )

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
    beams.forEach(beam => {
      const sweep = (((seconds * beam.speed + beam.phase) % 1) - 0.5) * 5.8
      const wave = Math.sin(seconds * 1.45 + beam.phase * Math.PI * 2)
      beam.mesh.position.set(
        sweep + Math.sin(seconds * 0.6 + beam.phase * Math.PI * 2) * 0.24,
        beam.verticalOffset + wave * 0.14,
        beam.mesh.position.z,
      )
      beam.mesh.rotation.z =
        beam.angle + Math.sin(seconds * 0.8 + beam.phase * Math.PI * 2) * 0.12
      beam.mesh.scale.set(
        beam.length,
        beam.thickness * (0.75 + Math.abs(wave) * 0.45),
        1,
      )
      const opacityUniform = beam.mesh.material.uniforms.opacity
      if (opacityUniform !== undefined) {
        opacityUniform.value =
          0.11 +
          (Math.sin(seconds * 1.9 + beam.phase * Math.PI * 2) * 0.5 + 0.5) * 0.18
      }
    })

    renderer.setRenderTarget(null)
    renderer.clear()
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
      beams.forEach(beam => {
        scene.remove(beam.mesh)
        beam.mesh.material.dispose()
      })
      geometry.dispose()
      renderer.dispose()
      element.replaceChildren()
    },
  }
}
