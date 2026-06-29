import * as Three from 'three'

import { type AnimationHandle, makeAnimationView, webglCanvas } from './element'

// A torus knot with a soft additive glow shell (a bloom-free approximation: a
// larger back-faced copy with a fresnel emissive shader). Inspired by the
// react-three-fiber postprocessing/MeshDistortMaterial demos but kept
// self-contained so it deploys without the postprocessing addon.

const glowVertexShader = `
varying vec3 vNormal;
varying vec3 vView;
void main() {
  vNormal = normalize(normalMatrix * normal);
  vec4 mv = modelViewMatrix * vec4(position, 1.0);
  vView = normalize(-mv.xyz);
  gl_Position = projectionMatrix * mv;
}
`

const glowFragmentShader = `
varying vec3 vNormal;
varying vec3 vView;
uniform vec3 uColor;
uniform float uPower;
void main() {
  float fresnel = pow(1.0 - max(dot(vNormal, vView), 0.0), uPower);
  gl_FragColor = vec4(uColor, fresnel);
}
`

const mountGlowKnot = (element: HTMLElement): AnimationHandle => {
  const { canvas, size } = webglCanvas(element)
  const renderer = new Three.WebGLRenderer({ alpha: true, antialias: true, canvas })
  renderer.setClearColor(0x000000, 0)

  const scene = new Three.Scene()
  const camera = new Three.PerspectiveCamera(50, 1, 0.1, 100)
  camera.position.z = 4.5

  const group = new Three.Group()
  scene.add(group)

  const geometry = new Three.TorusKnotGeometry(1, 0.32, 160, 24)
  const coreMaterial = new Three.MeshBasicMaterial({
    color: 0x6fe0ff,
    opacity: 0.85,
    transparent: true,
    wireframe: true,
  })
  group.add(new Three.Mesh(geometry, coreMaterial))

  const glowMaterial = new Three.ShaderMaterial({
    blending: Three.AdditiveBlending,
    depthWrite: false,
    fragmentShader: glowFragmentShader,
    side: Three.BackSide,
    transparent: true,
    uniforms: {
      uColor: { value: new Three.Color(0x2979ff) },
      uPower: { value: 2.4 },
    },
    vertexShader: glowVertexShader,
  })
  const glow = new Three.Mesh(geometry, glowMaterial)
  glow.scale.setScalar(1.25)
  group.add(glow)

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
    group.rotation.x = time * 0.0003
    group.rotation.y = time * 0.0005
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
      geometry.dispose()
      coreMaterial.dispose()
      glowMaterial.dispose()
      renderer.dispose()
      element.replaceChildren()
    },
  }
}

export const glowKnotView = makeAnimationView('oa-anim-glow-knot', mountGlowKnot)
