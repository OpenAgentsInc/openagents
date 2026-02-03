'use client'

import { useState, createRef } from 'react'
import { Canvas } from '@react-three/fiber'
import { Nodes, Node } from './Nodes'
import type * as THREE from 'three'

export function NodeCanvas() {
  const [[a, b, c, d, e]] = useState(() =>
    [...Array(5)].map(() => createRef<THREE.Mesh>()),
  )

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
