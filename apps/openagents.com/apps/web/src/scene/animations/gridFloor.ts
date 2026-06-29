import * as Three from 'three'

import { type AnimationHandle, makeAnimationView, webglCanvas } from './element'

// An infinite shader grid scrolling toward the viewer over a tilted plane —
// spatial context for a compute surface. Ported from the react-three-fiber
// shaderMaterial/grid demos to a self-contained ShaderMaterial plane.

const vertexShader = `
varying vec2 vUv;
varying float vDist;
void main() {
  vUv = uv;
  vec4 mv = modelViewMatrix * vec4(position, 1.0);
  vDist = -mv.z;
  gl_Position = projectionMatrix * mv;
}
`

const fragmentShader = `
precision highp float;
varying vec2 vUv;
varying float vDist;
uniform float uTime;
uniform vec3 uColor;

float gridLine(vec2 uv, float scale) {
  vec2 g = abs(fract(uv * scale - 0.5) - 0.5) / fwidth(uv * scale);
  return 1.0 - min(min(g.x, g.y), 1.0);
}

void main() {
  vec2 uv = vUv * vec2(1.0, 1.0);
  uv.y += uTime * 0.06;
  float fine = gridLine(uv, 40.0) * 0.5;
  float coarse = gridLine(uv, 8.0);
  float line = clamp(fine + coarse, 0.0, 1.0);
  float fade = smoothstep(28.0, 2.0, vDist);
  float alpha = line * fade * 0.85;
  gl_FragColor = vec4(uColor, alpha);
}
`

const mountGridFloor = (element: HTMLElement): AnimationHandle => {
  const { canvas, size } = webglCanvas(element)
  const renderer = new Three.WebGLRenderer({ alpha: true, antialias: true, canvas })
  renderer.setClearColor(0x000000, 0)

  const scene = new Three.Scene()
  const camera = new Three.PerspectiveCamera(60, 1, 0.1, 100)
  camera.position.set(0, 1.4, 3.2)
  camera.lookAt(0, 0, -6)

  const uniforms = {
    uColor: { value: new Three.Color(0x4fb8ff) },
    uTime: { value: 0 },
  }
  const geometry = new Three.PlaneGeometry(60, 60, 1, 1)
  geometry.rotateX(-Math.PI / 2)
  const material = new Three.ShaderMaterial({
    blending: Three.AdditiveBlending,
    depthWrite: false,
    fragmentShader,
    transparent: true,
    uniforms,
    vertexShader,
  })
  scene.add(new Three.Mesh(geometry, material))

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

export const gridFloorView = makeAnimationView('oa-anim-grid-floor', mountGridFloor)
