import React, { createRef, useState } from 'react'
import { Canvas } from '@react-three/fiber'
import { Node, Nodes } from '../Components/Nodes'

export default function PluginMap() {
  const [[a, b, c, d, e]] = useState(() => [...Array(5)].map(createRef))
  return (
    <Canvas orthographic camera={{ zoom: 80 }}>
      <Nodes>
        <Node ref={a} position={[-6, 3, 0]} name="Plugin A" description="Here's an example" color="#525458" connectedTo={[b, c, e]} />
        <Node ref={b} position={[4, 3.4, 0]} name="Plugin B" description="Hello a test description" color="#525458" connectedTo={[d, a]} />
        <Node ref={c} position={[-3.35, 1.2, 0]} name="Plugin C" color="#525458" />
        <Node ref={d} position={[0.5, -0.75, 0]} name="Plugin D" color="#525458" />
        <Node ref={e} position={[-3.5, -2, 0]} name="Plugin E" color="#525458" />
      </Nodes>
    </Canvas>
  )
}
