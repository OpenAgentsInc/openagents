import { defaultMokshaAssetUrls } from '@openagentsinc/three-effect/core'
import * as Three from 'three'
import { GLTFLoader } from 'three/examples/jsm/loaders/GLTFLoader.js'

// Self-contained "Pylon" hero scene. All scene logic lives here in the
// openagents app; the only thing pulled from `@openagentsinc/three-effect`
// is the URL of the shared diamond GLB asset, so the (heavy) model stays in
// that package rather than being copied into this repo.
//
// The look mirrors the Moksha diamonds: each diamond is rendered with a
// two-pass backface + refraction shader so the environment refracts through
// the glass. The scene is isolated and ambient: two small diamonds spaced
// apart, rotating slowly in a single direction on a dark field (the centered
// countdown overlay sits in the gap between them).

export type PylonDiamondsOptions = Readonly<{
  backgroundColor?: number
  pixelRatio?: number
  rotationSpeed?: number
  // Used by the Autopilot Desktop network home to composite the exact homepage
  // pylon shader over the existing three-effect network graph.
  transparentBackground?: boolean
}>

export type PylonDiamondsHandle = Readonly<{
  canvas: HTMLCanvasElement
  dispose: () => void
  // #5050: set the live activity level [0,1] that drives the blue glow.
  setActivity: (intensity: number) => void
}>

const DEFAULTS = {
  backgroundColor: 0x0c0f13,
  pixelRatio: 2,
  // radians per millisecond; deliberately slow and ambient
  rotationSpeed: 0.00018,
} as const

// Fixed world height the orthographic camera frames; world width follows the
// host aspect ratio so the scene never distorts on resize.
const WORLD_HEIGHT = 6
// Vertical world height each diamond is normalized to.
const DIAMOND_HEIGHT = 1.1
// Half the distance between the two diamond centers. The diamonds sit a short
// gap apart, with the centered countdown overlay filling the space between.
const STACK_GAP = 1.2
const DIAMOND_LAYER = 1

const backfaceVertexShader = `
  varying vec3 worldNormal;
  void main() {
    vec4 transformedNormal = vec4(normal, 0.0);
    vec4 transformedPosition = vec4(position, 1.0);
    #ifdef USE_INSTANCING
      transformedNormal = instanceMatrix * transformedNormal;
      transformedPosition = instanceMatrix * transformedPosition;
    #endif
    worldNormal = normalize(modelViewMatrix * transformedNormal).xyz;
    gl_Position = projectionMatrix * modelViewMatrix * transformedPosition;
  }
`

const backfaceFragmentShader = `
  varying vec3 worldNormal;
  void main() {
    gl_FragColor = vec4(worldNormal, 1.0);
  }
`

const refractionVertexShader = `
  varying vec3 worldNormal;
  varying vec3 viewDirection;
  void main() {
    vec4 transformedNormal = vec4(normal, 0.0);
    vec4 transformedPosition = vec4(position, 1.0);
    #ifdef USE_INSTANCING
      transformedNormal = instanceMatrix * transformedNormal;
      transformedPosition = instanceMatrix * transformedPosition;
    #endif
    worldNormal = normalize(modelViewMatrix * transformedNormal).xyz;
    viewDirection = normalize((modelMatrix * vec4(position, 1.0)).xyz - cameraPosition);
    gl_Position = projectionMatrix * modelViewMatrix * transformedPosition;
  }
`

const refractionFragmentShader = `
  uniform sampler2D envMap;
  uniform sampler2D backfaceMap;
  uniform vec2 resolution;
  uniform vec3 lightDirection;
  uniform float lightPulse;
  varying vec3 worldNormal;
  varying vec3 viewDirection;
  float fresnelFunc(vec3 viewDirection, vec3 worldNormal) {
    return pow(1.05 + dot(viewDirection, worldNormal), 100.0);
  }
  void main() {
    vec2 uv = gl_FragCoord.xy / resolution;
    vec3 normal = worldNormal * 0.3 - texture2D(backfaceMap, uv).rgb * 0.7;
    vec4 color = texture2D(envMap, uv + refract(viewDirection, normal, 1.0 / 1.5).xy);
    float activity = clamp(lightPulse, 0.0, 1.0);
    vec3 body = mix(color.rgb, vec3(0.025, 0.07, 0.105), 0.52);
    body += vec3(0.015, 0.055, 0.09) * activity;
    gl_FragColor = vec4(body, 1.0);
  }
`

const hostSize = (element: HTMLElement): { height: number; width: number } => {
  const rect = element.getBoundingClientRect()
  const width = Math.max(1, Math.floor(rect.width || element.clientWidth || 320))
  const height = Math.max(
    1,
    Math.floor(rect.height || element.clientHeight || 420),
  )
  return { height, width }
}

const firstMeshGeometry = (
  root: Three.Object3D,
): Three.BufferGeometry | null => {
  let found: Three.BufferGeometry | null = null
  root.traverse(child => {
    if (found !== null) return
    if (child instanceof Three.Mesh) {
      found = child.geometry.clone()
    }
  })
  if (found === null) return null
  const geometry: Three.BufferGeometry = found
  geometry.center()
  return geometry
}

// Normalize a centered geometry so its vertical extent equals DIAMOND_HEIGHT.
const normalizeDiamondGeometry = (geometry: Three.BufferGeometry): number => {
  geometry.computeBoundingBox()
  const box = geometry.boundingBox
  if (box === null) return 1
  const size = new Three.Vector3()
  box.getSize(size)
  const extent = Math.max(size.y, 1e-6)
  return DIAMOND_HEIGHT / extent
}

export const mountPylonDiamonds = (
  element: HTMLElement,
  options: PylonDiamondsOptions = {},
): PylonDiamondsHandle => {
  const resolved = { ...DEFAULTS, ...options }
  const transparentBackground = resolved.transparentBackground === true

  // #5050: live activity level [0,1] driving the glow; updated via setActivity.
  let activityIntensity = 0

  element.replaceChildren()
  element.style.position = 'relative'
  element.style.overflow = 'hidden'
  element.style.background = transparentBackground
    ? 'transparent'
    : `#${resolved.backgroundColor.toString(16).padStart(6, '0')}`

  const canvas = document.createElement('canvas')
  canvas.style.display = 'block'
  canvas.style.inset = '0'
  canvas.style.position = 'absolute'
  canvas.style.height = '100%'
  canvas.style.width = '100%'
  element.append(canvas)

  const renderer = new Three.WebGLRenderer({
    alpha: transparentBackground,
    antialias: true,
    canvas,
  })
  renderer.outputColorSpace = Three.SRGBColorSpace
  renderer.setClearColor(resolved.backgroundColor, transparentBackground ? 0 : 1)
  renderer.autoClear = false

  const scene = new Three.Scene()
  const camera = new Three.OrthographicCamera(-1, 1, 1, -1, 0.1, 1000)
  camera.position.set(0, 0, 50)
  camera.lookAt(0, 0, 0)

  // Two diamonds on layer 1, rendered with the refraction material.
  const fallbackGeometry = new Three.OctahedronGeometry(1, 0)
  let modelScale = normalizeDiamondGeometry(fallbackGeometry)

  let size = hostSize(element)
  const ratio = (): number =>
    Math.min(window.devicePixelRatio || 1, resolved.pixelRatio)
  const targetSize = (): { height: number; width: number } => ({
    height: Math.max(1, Math.floor(size.height * ratio())),
    width: Math.max(1, Math.floor(size.width * ratio())),
  })

  const initialTarget = targetSize()
  const envFbo = new Three.WebGLRenderTarget(
    initialTarget.width,
    initialTarget.height,
  )
  const backfaceFbo = new Three.WebGLRenderTarget(
    initialTarget.width,
    initialTarget.height,
  )

  const backfaceMaterial = new Three.ShaderMaterial({
    fragmentShader: backfaceFragmentShader,
    side: Three.BackSide,
    vertexShader: backfaceVertexShader,
  })
  const refractionMaterial = new Three.ShaderMaterial({
    fragmentShader: refractionFragmentShader,
    uniforms: {
      backfaceMap: { value: backfaceFbo.texture },
      envMap: { value: envFbo.texture },
      lightDirection: {
        value: new Three.Vector3(0.45, 0.22, 0.86).normalize(),
      },
      lightPulse: { value: 0.6 },
      resolution: {
        value: new Three.Vector2(initialTarget.width, initialTarget.height),
      },
    },
    vertexShader: refractionVertexShader,
  })

  const diamondMesh = new Three.InstancedMesh<
    Three.BufferGeometry,
    Three.Material
  >(fallbackGeometry, refractionMaterial, 2)
  diamondMesh.layers.set(DIAMOND_LAYER)
  scene.add(diamondMesh)

  const dummy = new Three.Object3D()
  const yAxis = new Three.Vector3(0, 1, 0)
  const flipQuat = new Three.Quaternion().setFromAxisAngle(
    new Three.Vector3(1, 0, 0),
    Math.PI,
  )
  const spinQuat = new Three.Quaternion()
  const lightDirection = new Three.Vector3()

  const updateLight = (time: number): void => {
    const seconds = time * 0.001
    lightDirection
      .set(
        Math.sin(seconds * 0.78) * 0.6,
        Math.cos(seconds * 0.52) * 0.35,
        0.8,
      )
      .normalize()
    const lightUniform = refractionMaterial.uniforms.lightDirection?.value
    if (lightUniform instanceof Three.Vector3) {
      lightUniform.copy(lightDirection)
    }
    const pulseUniform = refractionMaterial.uniforms.lightPulse
    if (pulseUniform !== undefined) {
      // #5050: the blue glow tracks live network activity. Idle breathes slow +
      // dim; an active network (work/compute flowing) pulses brighter + faster.
      // `activityIntensity` in [0,1] is driven by setActivity() from live stats.
      const base = 0.30 + activityIntensity * 0.30
      const span = 0.18 + activityIntensity * 0.40
      const rate = 1.0 + activityIntensity * 1.6
      pulseUniform.value = base + (Math.sin(seconds * rate) * 0.5 + 0.5) * span
    }
  }

  const updateDiamonds = (time: number): void => {
    const spin = time * resolved.rotationSpeed
    spinQuat.setFromAxisAngle(yAxis, spin)

    // Bottom diamond: identity orientation (flat face up, toward center).
    dummy.position.set(0, -STACK_GAP, 0)
    dummy.quaternion.copy(spinQuat)
    dummy.scale.setScalar(modelScale)
    dummy.updateMatrix()
    diamondMesh.setMatrixAt(0, dummy.matrix)

    // Top diamond: flipped about X so its flat face points down to meet the
    // bottom diamond in the middle. Same world-Y spin direction.
    dummy.position.set(0, STACK_GAP, 0)
    dummy.quaternion.copy(spinQuat).multiply(flipQuat)
    dummy.scale.setScalar(modelScale)
    dummy.updateMatrix()
    diamondMesh.setMatrixAt(1, dummy.matrix)

    diamondMesh.instanceMatrix.needsUpdate = true
  }

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

    const fbo = targetSize()
    envFbo.setSize(fbo.width, fbo.height)
    backfaceFbo.setSize(fbo.width, fbo.height)
    const resolution = refractionMaterial.uniforms.resolution?.value
    if (resolution instanceof Three.Vector2) {
      resolution.set(fbo.width, fbo.height)
    }
  }

  let disposed = false
  let frame = 0

  const renderScene = (time: number): void => {
    if (disposed) return
    updateLight(time)
    updateDiamonds(time)

    // Pass 1: render the environment (background + wordmark) into envFbo.
    camera.layers.set(0)
    renderer.setRenderTarget(envFbo)
    renderer.clear()
    renderer.render(scene, camera)
    renderer.clearDepth()

    // Pass 2: render diamond backfaces (world normals) into backfaceFbo.
    camera.layers.set(DIAMOND_LAYER)
    diamondMesh.material = backfaceMaterial
    renderer.setRenderTarget(backfaceFbo)
    renderer.clearDepth()
    renderer.render(scene, camera)

    // Pass 3: draw the environment to the screen.
    camera.layers.set(0)
    renderer.setRenderTarget(null)
    renderer.clear()
    renderer.render(scene, camera)
    renderer.clearDepth()

    // Pass 4: draw the refracting diamonds over the environment.
    camera.layers.set(DIAMOND_LAYER)
    diamondMesh.material = refractionMaterial
    renderer.render(scene, camera)

    camera.layers.set(0)

    frame = requestAnimationFrame(renderScene)
  }

  const observer =
    typeof ResizeObserver === 'undefined'
      ? null
      : new ResizeObserver(() => resize())

  const gltfLoader = new GLTFLoader()
  gltfLoader.load(
    defaultMokshaAssetUrls.diamondModelUrl,
    gltf => {
      if (disposed) return
      const geometry = firstMeshGeometry(gltf.scene)
      if (geometry === null) return
      modelScale = normalizeDiamondGeometry(geometry)
      diamondMesh.geometry.dispose()
      diamondMesh.geometry = geometry
    },
    undefined,
    () => {},
  )

  resize()
  observer?.observe(element)
  frame = requestAnimationFrame(renderScene)

  const dispose = (): void => {
    if (disposed) return
    disposed = true
    cancelAnimationFrame(frame)
    observer?.disconnect()
    diamondMesh.geometry.dispose()
    fallbackGeometry.dispose()
    backfaceMaterial.dispose()
    refractionMaterial.dispose()
    envFbo.dispose()
    backfaceFbo.dispose()
    renderer.dispose()
    element.replaceChildren()
  }

  const setActivity = (intensity: number): void => {
    activityIntensity = Number.isFinite(intensity)
      ? Math.max(0, Math.min(1, intensity))
      : 0
  }

  return { canvas, dispose, setActivity }
}
