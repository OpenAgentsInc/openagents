import { defaultMokshaAssetUrls } from '@openagentsinc/three-effect/core'
import * as Three from 'three'
import { GLTFLoader } from 'three/examples/jsm/loaders/GLTFLoader.js'

// Self-contained "Pylon" hero scene. All scene logic lives here in the
// openagents app; the only thing pulled from `@openagentsinc/three-effect`
// is the URL of the shared diamond GLB asset, so the (heavy) model stays in
// that package rather than being copied into this repo.
//
// The look mirrors the Moksha diamonds: each diamond is rendered with a
// two-pass backface + refraction shader so the background "Pylon" wordmark
// refracts through the glass. The scene is isolated and ambient: two diamonds
// stacked flat-face-to-flat-face in the middle, rotating slowly in a single
// direction, over white text on a dark field.

export type PylonDiamondsOptions = Readonly<{
  backgroundColor?: number
  pixelRatio?: number
  rotationSpeed?: number
  text?: string
  textColor?: string
}>

export type PylonDiamondsHandle = Readonly<{
  canvas: HTMLCanvasElement
  dispose: () => void
}>

const DEFAULTS = {
  backgroundColor: 0x0c0f13,
  pixelRatio: 2,
  // radians per millisecond; deliberately slow and ambient
  rotationSpeed: 0.00018,
  text: 'Pylon',
  textColor: '#ffffff',
} as const

// Fixed world height the orthographic camera frames; world width follows the
// host aspect ratio so the scene never distorts on resize.
const WORLD_HEIGHT = 6
// Vertical world height each diamond is normalized to.
const DIAMOND_HEIGHT = 2.2
// Half the distance between the two diamond centers. The diamonds sit a bit
// apart with a clear gap between their flat faces, bridged by a golden band
// (Starcraft-Pylon flavored).
const STACK_GAP = 1.5
// Golden connecting band sitting in the gap between the two diamonds.
const BAND_COLOR = 0xe4b24a
const BAND_RADIUS = 0.46
const BAND_HEIGHT = 0.5

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
  varying vec3 worldNormal;
  varying vec3 viewDirection;
  float fresnelFunc(vec3 viewDirection, vec3 worldNormal) {
    return pow(1.05 + dot(viewDirection, worldNormal), 100.0);
  }
  void main() {
    vec2 uv = gl_FragCoord.xy / resolution;
    vec3 normal = worldNormal * 0.3 - texture2D(backfaceMap, uv).rgb * 0.7;
    vec4 color = texture2D(envMap, uv + refract(viewDirection, normal, 1.0 / 1.5).xy);
    gl_FragColor = vec4(mix(color.rgb, vec3(0.4), fresnelFunc(viewDirection, normal)), 1.0);
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

const makeTextTexture = (input: {
  color: string
  text: string
}): { aspect: number; texture: Three.CanvasTexture } => {
  const canvas = document.createElement('canvas')
  const context = canvas.getContext('2d')
  const fontSize = 320
  const padding = fontSize * 0.4
  const font = `600 ${fontSize}px Inter, ui-sans-serif, system-ui, sans-serif`

  let width = 1024
  if (context !== null) {
    context.font = font
    width = Math.ceil(context.measureText(input.text).width) + padding * 2
  }
  const height = Math.ceil(fontSize * 1.4)
  canvas.width = Math.max(2, width)
  canvas.height = Math.max(2, height)

  if (context !== null) {
    context.clearRect(0, 0, canvas.width, canvas.height)
    context.font = font
    context.fillStyle = input.color
    context.textAlign = 'center'
    context.textBaseline = 'middle'
    context.fillText(input.text, canvas.width / 2, canvas.height / 2)
  }

  const texture = new Three.CanvasTexture(canvas)
  texture.colorSpace = Three.SRGBColorSpace
  texture.minFilter = Three.LinearFilter
  return { aspect: canvas.width / canvas.height, texture }
}

const firstMeshGeometry = (
  root: Three.Object3D,
): Three.BufferGeometry | null => {
  let found: Three.BufferGeometry | null = null
  root.traverse(child => {
    if (found !== null) return
    const mesh = child as Three.Object3D & { geometry?: Three.BufferGeometry }
    if (mesh.geometry !== undefined) {
      found = mesh.geometry.clone()
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

  element.replaceChildren()
  element.style.position = 'relative'
  element.style.overflow = 'hidden'
  element.style.background = `#${resolved.backgroundColor
    .toString(16)
    .padStart(6, '0')}`

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
  renderer.setClearColor(resolved.backgroundColor, 1)
  renderer.autoClear = false

  const scene = new Three.Scene()
  const camera = new Three.OrthographicCamera(-1, 1, 1, -1, 0.1, 1000)
  camera.position.set(0, 0, 50)
  camera.lookAt(0, 0, 0)

  // Background "Pylon" wordmark on layer 0 (the refracted environment).
  const { aspect: textAspect, texture: textTexture } = makeTextTexture({
    color: resolved.textColor,
    text: resolved.text,
  })
  const textMaterial = new Three.MeshBasicMaterial({
    depthWrite: false,
    map: textTexture,
    transparent: true,
  })
  const textMesh = new Three.Mesh(new Three.PlaneGeometry(1, 1), textMaterial)
  textMesh.position.set(0, 0, -2)
  textMesh.layers.set(0)
  scene.add(textMesh)

  // Golden band bridging the gap between the two diamonds. Lives on layer 0 so
  // it sits in the environment and refracts through the diamonds like the
  // wordmark. Lit with a couple of lights for a metallic sheen (the diamond
  // and text materials are unlit shaders, so the lights only touch the band).
  const bandGeometry = new Three.CylinderGeometry(
    BAND_RADIUS,
    BAND_RADIUS,
    BAND_HEIGHT,
    48,
    1,
    true,
  )
  const bandMaterial = new Three.MeshStandardMaterial({
    color: BAND_COLOR,
    emissive: new Three.Color(BAND_COLOR).multiplyScalar(0.18),
    metalness: 0.95,
    roughness: 0.28,
    side: Three.DoubleSide,
  })
  const bandMesh = new Three.Mesh(bandGeometry, bandMaterial)
  bandMesh.position.set(0, 0, 0)
  bandMesh.layers.set(0)
  scene.add(bandMesh)

  const ambientLight = new Three.AmbientLight(0xffffff, 1.1)
  const keyLight = new Three.DirectionalLight(0xfff0d0, 2.4)
  keyLight.position.set(2, 3, 4)
  const rimLight = new Three.DirectionalLight(0xffd27a, 1.6)
  rimLight.position.set(-3, -1, 2)
  scene.add(ambientLight, keyLight, rimLight)

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
  diamondMesh.layers.set(1)
  scene.add(diamondMesh)

  const dummy = new Three.Object3D()
  const yAxis = new Three.Vector3(0, 1, 0)
  const flipQuat = new Three.Quaternion().setFromAxisAngle(
    new Three.Vector3(1, 0, 0),
    Math.PI,
  )
  const spinQuat = new Three.Quaternion()

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

    // Size the wordmark relative to the framed world.
    const textWidth = Math.min(worldWidth * 0.62, WORLD_HEIGHT * 1.6)
    textMesh.scale.set(textWidth, textWidth / textAspect, 1)
  }

  let disposed = false
  let frame = 0

  const renderScene = (time: number): void => {
    if (disposed) return
    updateDiamonds(time)

    // Pass 1: render the environment (background + wordmark) into envFbo.
    camera.layers.set(0)
    renderer.setRenderTarget(envFbo)
    renderer.clear()
    renderer.render(scene, camera)
    renderer.clearDepth()

    // Pass 2: render diamond backfaces (world normals) into backfaceFbo.
    camera.layers.set(1)
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
    camera.layers.set(1)
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
    textMesh.geometry.dispose()
    textMaterial.dispose()
    textTexture.dispose()
    bandGeometry.dispose()
    bandMaterial.dispose()
    envFbo.dispose()
    backfaceFbo.dispose()
    renderer.dispose()
    element.replaceChildren()
  }

  return { canvas, dispose }
}
