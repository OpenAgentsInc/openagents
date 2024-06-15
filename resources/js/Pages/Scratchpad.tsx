import React, { useRef, useState } from 'react';
import * as THREE from 'three';
import { Canvas } from '@react-three/fiber';
import { Html } from '@react-three/drei';
import CustomCard from '../Components/CustomCard';
import { useDrag } from '@use-gesture/react';
import { animated, useSpring } from '@react-spring/web';

export default function Scratchpad() {
  return (
    <Canvas orthographic camera={{ zoom: 80 }}>
      <Node />
    </Canvas>
  );
}

function Node() {
  const [hovered, setHovered] = useState(false);
  const [position, setPosition] = useState({ x: 0, y: 0 });
  const [{ x, y }, api] = useSpring(() => position);

  // Set the drag hook and define component movement based on gesture data
  const bind = useDrag(({ down, offset: [mx, my] }) => {
    if (!down) {
      setPosition({ x: mx, y: my });
    }
    api.start({ x: mx, y: my, immediate: down });
  });

  return (
    <>
      <Html>
        <animated.div {...bind()} style={{ x, y, touchAction: 'none' }}>
          <div
            className="select-none cursor-pointer"
            onPointerOver={() => setHovered(true)}
            onPointerOut={() => setHovered(false)}
          >
            <CustomCard title="Test Node" description={`Test Description. Hovered ${hovered}`} />
          </div>
        </animated.div>
      </Html>
    </>
  );
}
