import React, { useState } from 'react'
import { Canvas } from '@react-three/fiber'
import { Html } from '@react-three/drei'
import CustomCard from '../Components/CustomCard';

export default function Scratchpad() {
  return (
    <Canvas orthographic camera={{ zoom: 80 }}>
      <Node />
    </Canvas>
  )
}

function Node() {
  const [hovered, setHovered] = useState(false);
  return (
    <Html>
      <div onPointerOver={() => setHovered(true)} onPointerOut={() => setHovered(false)}>
        <CustomCard title="Test Node" description={`Test Description. Hovered ${hovered}`} />
      </div>
    </Html>
  )
}
