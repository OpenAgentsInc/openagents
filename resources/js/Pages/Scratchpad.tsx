import React, { useRef, useState } from 'react';
import * as THREE from 'three';
import { Canvas } from '@react-three/fiber';
import { Html } from '@react-three/drei';
import CustomCard from '../Components/CustomCard';
import { useDrag } from '@use-gesture/react';
import { animated, useSpring } from '@react-spring/web';
import { Bloom, EffectComposer, ToneMapping } from '@react-three/postprocessing'
import { ToneMappingMode } from 'postprocessing'

export default function Scratchpad() {
  return (
    <Canvas orthographic camera={{ zoom: 80 }}>
      <Node />
      <EffectComposer>
        <ToneMapping mode={ToneMappingMode.ACES_FILMIC} />
        <Bloom />
      </EffectComposer>
    </Canvas>
  );
}

function Node() {
  const [hovered, setHovered] = useState(false);
  const [position, setPosition] = useState({ x: 0, y: 0 });
  const [{ x, y }, api] = useSpring(() => position);

  const glowRef = useRef();

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

export const GlowShader = {
  uniforms: {
    'c': { type: 'f', value: 1.0 },
    'p': { type: 'f', value: 1.4 },
    glowColor: { type: 'c', value: new THREE.Color(0xffffff) },
    viewVector: { type: 'v3', value: new THREE.Vector3(0, 0, 1) },
  },
  vertexShader: `
    uniform vec3 viewVector;
    uniform float c;
    uniform float p;
    varying float intensity;
    void main() {
      vec3 vNormal = normalize( normalMatrix * normal );
      vec3 vNormel = normalize( normalMatrix * viewVector );
      intensity = pow( c - dot(vNormal, vNormel), p );
      gl_Position = projectionMatrix * modelViewMatrix * vec4( position, 1.0 );
    }
  `,
  fragmentShader: `
    uniform vec3 glowColor;
    varying float intensity;
    void main() {
      vec3 glow = glowColor * intensity;
      gl_FragColor = vec4( glow, 1.0 );
    }
  `
};
