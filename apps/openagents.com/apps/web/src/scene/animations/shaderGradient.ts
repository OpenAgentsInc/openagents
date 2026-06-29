import * as Three from 'three'

import { type AnimationHandle, makeAnimationView, webglCanvas } from './element'

// A full-frame fragment-shader gradient: slow flowing bands in the OpenAgents
// cyan/blue palette. Ported from the react-three-fiber ShaderMaterial demo to a
// plain three.js fullscreen quad on an orthographic camera.

const vertexShader = `
varying vec2 vUv;
void main() {
  vUv = uv;
  gl_Position = vec4(position, 1.0);
}
`

const fragmentShader = `
precision highp float;
varying vec2 vUv;
uniform float uTime;
uniform vec2 uResolution;

// cheap value noise
float hash(vec2 p) { return fract(sin(dot(p, vec2(127.1, 311.7))) * 43758.5453); }
float noise(vec2 p) {
  vec2 i = floor(p); vec2 f = fract(p);
  vec2 u = f * f * (3.0 - 2.0 * f);
  return mix(mix(hash(i), hash(i + vec2(1.0, 0.0)), u.x),
             mix(hash(i + vec2(0.0, 1.0)), hash(i + vec2(1.0, 1.0)), u.x), u.y);
}

void main() {
  vec2 uv = vUv;
  float aspect = uResolution.x / max(uResolution.y, 1.0);
  uv.x *= aspect;
  float t = uTime * 0.08;
  float n = noise(uv * 2.5 + vec2(t, -t * 0.6));
  n += 0.5 * noise(uv * 5.0 - vec2(t * 0.4, t));
  float band = sin((uv.y + uv.x * 0.3) * 6.0 + n * 3.5 + t * 2.0) * 0.5 + 0.5;
  vec3 deep = vec3(0.02, 0.05, 0.10);
  vec3 cyan = vec3(0.42, 0.88, 1.0);
  vec3 blue = vec3(0.16, 0.47, 1.0);
  vec3 col = mix(deep, mix(blue, cyan, band), band * 0.85);
  gl_FragColor = vec4(col, 1.0);
}
`

const mountShaderGradient = (element: HTMLElement): AnimationHandle => {
  const { canvas, size } = webglCanvas(element)
  const renderer = new Three.WebGLRenderer({ alpha: true, antialias: true, canvas })
  renderer.setClearColor(0x000000, 0)

  const scene = new Three.Scene()
  const camera = new Three.OrthographicCamera(-1, 1, 1, -1, 0, 1)

  const uniforms = {
    uResolution: { value: new Three.Vector2(1, 1) },
    uTime: { value: 0 },
  }
  const geometry = new Three.PlaneGeometry(2, 2)
  const material = new Three.ShaderMaterial({ fragmentShader, uniforms, vertexShader })
  scene.add(new Three.Mesh(geometry, material))

  const resize = (): void => {
    const { height, width } = size()
    renderer.setPixelRatio(Math.min(window.devicePixelRatio || 1, 2))
    renderer.setSize(width, height, false)
    uniforms.uResolution.value.set(width, height)
  }

  let disposed = false
  let frame = 0
  const tick = (time: number): void => {
    if (disposed) return
    uniforms.uTime.value = time * 0.001
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
      material.dispose()
      renderer.dispose()
      element.replaceChildren()
    },
  }
}

export const shaderGradientView = makeAnimationView('oa-anim-shader-gradient', mountShaderGradient)
