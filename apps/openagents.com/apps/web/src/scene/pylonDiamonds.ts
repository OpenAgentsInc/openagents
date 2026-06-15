import {
  createConditionalLineSegments,
  defaultMokshaAssetUrls,
  type ConditionalLineSegmentsHandle,
} from '@openagentsinc/three-effect/core'
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
}>

export type PylonDiamondsHandle = Readonly<{
  canvas: HTMLCanvasElement
  dispose: () => void
}>

type DiamondEdgeView = Readonly<{
  handle: ConditionalLineSegmentsHandle
}>

type LightBeam = Readonly<{
  angle: number
  length: number
  mesh: Three.Mesh<Three.PlaneGeometry, Three.ShaderMaterial>
  phase: number
  speed: number
  thickness: number
  verticalOffset: number
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
const DIAMOND_EDGE_LAYER = 2
const LIGHT_BEAM_COUNT = 7

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
    float glint = pow(max(dot(normalize(normal), normalize(lightDirection)), 0.0), 4.5) * lightPulse;
    vec3 caustic = vec3(0.28, 0.62, 0.92) * glint;
    gl_FragColor = vec4(mix(color.rgb + caustic, vec3(0.42), fresnelFunc(viewDirection, normal)), 1.0);
  }
`

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

  const lightBeamGeometry = new Three.PlaneGeometry(1, 1)
  const lightBeams: ReadonlyArray<LightBeam> = Array.from(
    { length: LIGHT_BEAM_COUNT },
    (_, index) => {
      const phase = index / LIGHT_BEAM_COUNT
      const mesh = new Three.Mesh(
        lightBeamGeometry,
        createLightBeamMaterial(0.2 + (index % 3) * 0.05),
      )
      mesh.position.z = -1 - index * 0.01
      mesh.renderOrder = -2
      scene.add(mesh)

      return {
        angle: index % 2 === 0 ? -0.74 : 0.74,
        length: 2.8 + (index % 4) * 0.35,
        mesh,
        phase,
        speed: 0.075 + index * 0.008,
        thickness: 0.035 + (index % 3) * 0.012,
        verticalOffset: index % 2 === 0 ? -STACK_GAP : STACK_GAP,
      }
    },
  )

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

  const diamondEdgeGroup = new Three.Group()
  scene.add(diamondEdgeGroup)

  let diamondEdgeViews: ReadonlyArray<DiamondEdgeView> = []

  const disposeDiamondEdges = (): void => {
    for (const view of diamondEdgeViews) {
      diamondEdgeGroup.remove(view.handle.line)
      view.handle.dispose()
    }
    diamondEdgeViews = []
  }

  const createDiamondEdges = (
    geometry: Three.BufferGeometry,
  ): ReadonlyArray<DiamondEdgeView> =>
    Array.from({ length: 2 }, () => {
      const handle = createConditionalLineSegments(geometry, {
        color: 0xd8f4ff,
        depthTest: false,
        linewidth: 1.25,
        opacity: 0.82,
        resolution: [initialTarget.width, initialTarget.height],
        transparent: true,
      })
      handle.line.layers.set(DIAMOND_EDGE_LAYER)
      handle.line.renderOrder = 4
      diamondEdgeGroup.add(handle.line)
      return { handle }
    })

  const replaceDiamondEdges = (geometry: Three.BufferGeometry): void => {
    disposeDiamondEdges()
    diamondEdgeViews = createDiamondEdges(geometry)
  }

  replaceDiamondEdges(fallbackGeometry)

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
      pulseUniform.value =
        0.42 + (Math.sin(seconds * 1.3) * 0.5 + 0.5) * 0.42
    }

    lightBeams.forEach(beam => {
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
          (Math.sin(seconds * 1.9 + beam.phase * Math.PI * 2) * 0.5 + 0.5) *
            0.18
      }
    })
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
    diamondEdgeViews[0]?.handle.line.position.copy(dummy.position)
    diamondEdgeViews[0]?.handle.line.quaternion.copy(dummy.quaternion)
    diamondEdgeViews[0]?.handle.line.scale.copy(dummy.scale)

    // Top diamond: flipped about X so its flat face points down to meet the
    // bottom diamond in the middle. Same world-Y spin direction.
    dummy.position.set(0, STACK_GAP, 0)
    dummy.quaternion.copy(spinQuat).multiply(flipQuat)
    dummy.scale.setScalar(modelScale)
    dummy.updateMatrix()
    diamondMesh.setMatrixAt(1, dummy.matrix)
    diamondEdgeViews[1]?.handle.line.position.copy(dummy.position)
    diamondEdgeViews[1]?.handle.line.quaternion.copy(dummy.quaternion)
    diamondEdgeViews[1]?.handle.line.scale.copy(dummy.scale)

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
    diamondEdgeViews.forEach(view =>
      view.handle.setResolution(fbo.width, fbo.height),
    )
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

    camera.layers.set(DIAMOND_EDGE_LAYER)
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
      replaceDiamondEdges(geometry)
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
    disposeDiamondEdges()
    lightBeams.forEach(beam => {
      scene.remove(beam.mesh)
      beam.mesh.material.dispose()
    })
    lightBeamGeometry.dispose()
    diamondMesh.geometry.dispose()
    fallbackGeometry.dispose()
    backfaceMaterial.dispose()
    refractionMaterial.dispose()
    envFbo.dispose()
    backfaceFbo.dispose()
    renderer.dispose()
    element.replaceChildren()
  }

  return { canvas, dispose }
}
