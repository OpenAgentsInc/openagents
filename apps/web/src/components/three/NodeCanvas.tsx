import { useState, createRef, useEffect } from 'react'
import type * as THREE from 'three'

function WebGLFallback() {
  return (
    <div className="flex h-full w-full items-center justify-center">
      <h1 className="text-4xl font-bold">Welcome to OpenAgents</h1>
    </div>
  )
}

function checkWebGLSupport(): boolean {
  if (typeof window === 'undefined') return false
  try {
    const canvas = document.createElement('canvas')
    const gl = canvas.getContext('webgl') || canvas.getContext('experimental-webgl')
    return !!gl
  } catch {
    return false
  }
}

export function NodeCanvas() {
  const [mounted, setMounted] = useState(false)
  const [hasWebGL, setHasWebGL] = useState(false)
  const [[a, b, c, d, e]] = useState(() =>
    [...Array(5)].map(() => createRef<THREE.Mesh>()),
  )

  useEffect(() => {
    setMounted(true)
    setHasWebGL(checkWebGLSupport())
  }, [])

  if (!mounted) {
    return <div className="flex-1 bg-background" />
  }

  if (!hasWebGL) {
    return <WebGLFallback />
  }

  const { Canvas } = require('@react-three/fiber')
  const { Nodes, Node } = require('./Nodes')

  return (
    <Canvas orthographic camera={{ zoom: 80 }}>
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
