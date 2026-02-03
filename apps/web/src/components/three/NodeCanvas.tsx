import { useState, createRef, useEffect, useMemo } from 'react'
import { Canvas } from '@react-three/fiber'
import { Nodes, Node } from './Nodes'
import type * as THREE from 'three'

export function NodeCanvas() {
  const [mounted, setMounted] = useState(false)
  const [webglReady, setWebglReady] = useState(false)
  const [[a, b, c, d, e]] = useState(() =>
    [...Array(5)].map(() => createRef<THREE.Mesh>()),
  )

  const glConfig = useMemo(
    () => ({
      antialias: false,
      alpha: false,
      depth: true,
      stencil: false,
      premultipliedAlpha: false,
      preserveDrawingBuffer: false,
      powerPreference: 'high-performance' as const,
      failIfMajorPerformanceCaveat: false,
    }),
    [],
  )

  useEffect(() => {
    setMounted(true)
    const supported = checkWebGLSupport(glConfig)
    if (!supported) {
      console.warn('[three] WebGL not available; falling back to static hero')
    } else {
      console.info('[three] WebGL available; mounting canvas')
    }
    setWebglReady(supported)
  }, [glConfig])

  const fallback = useMemo(
    () => (
      <div className="flex h-full w-full items-center justify-center">
        <h1 className="text-4xl font-bold">Welcome to OpenAgents</h1>
      </div>
    ),
    [],
  )

  if (!mounted) return <div className="flex-1 bg-background" />
  if (!webglReady) return fallback

  return (
    <Canvas
      orthographic
      camera={{ zoom: 80 }}
      gl={glConfig}
      fallback={fallback}
    >
      <Nodes>
        <Node ref={a} name="a" color="#204090" position={[-2, 2, 0]} connectedTo={[b, c, e]} />
        <Node ref={b} name="b" color="#904020" position={[2, -3, 0]} connectedTo={[d, a]} />
        <Node ref={c} name="c" color="#209040" position={[-0.25, 0, 0]} />
        <Node ref={d} name="d" color="#204090" position={[0.5, -0.75, 0]} />
        <Node ref={e} name="e" color="#204090" position={[-0.5, -1, 0]} />
      </Nodes>
    </Canvas>
  )
}

function checkWebGLSupport(contextAttributes: WebGLContextAttributes) {
  if (typeof document === 'undefined') return false
  const canvas = document.createElement('canvas')
  try {
    const webgl2 = canvas.getContext('webgl2', contextAttributes)
    if (webgl2) {
      console.info('[three] WebGL2 context created')
      return true
    }
    const webgl1 = canvas.getContext('webgl', contextAttributes)
    if (webgl1) {
      console.info('[three] WebGL1 context created')
      return true
    }
    console.warn('[three] Failed to create WebGL context', { contextAttributes })
    return false
  } catch {
    console.warn('[three] Exception while creating WebGL context', { contextAttributes })
    return false
  }
}
