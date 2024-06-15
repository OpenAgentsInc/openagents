import React, {
  createContext,
  useMemo,
  useRef,
  useState,
  useContext,
  useLayoutEffect,
  forwardRef,
  useEffect,
} from "react";
import * as THREE from "three";
import { useFrame, useThree } from "@react-three/fiber";
import { Circle, QuadraticBezierLine, Html } from "@react-three/drei";
import { useDrag } from "@use-gesture/react";
import CustomCard from './CustomCard'; // Import your CustomCard component

const context = createContext();

export function Nodes({ children }) {
  const group = useRef();
  const [nodes, set] = useState([]);
  const lines = useMemo(() => {
    const lines = [];
    for (let node of nodes) {
      node.connectedTo
        .map((ref) => [node.position, ref.current.position])
        .forEach(([start, end]) =>
          lines.push({
            start: start.clone().add({ x: 0.35, y: 0, z: 0 }),
            end: end.clone().add({ x: -0.35, y: 0, z: 0 }),
          })
        );
    }
    return lines;
  }, [nodes]);

  useFrame((_, delta) => {
    group.current.children.forEach((group) => {
      group.children[0].material.uniforms.dashOffset.value -= delta * 10;
    });
  });

  return (
    <context.Provider value={set}>
      <group ref={group}>
        {lines.map((line, index) => (
          <group key={index}>
            <QuadraticBezierLine {...line} color="white" dashed dashScale={50} gapSize={20} />
            <QuadraticBezierLine
              {...line}
              color="white"
              lineWidth={0.5}
              transparent
              opacity={0.1}
            />
          </group>
        ))}
      </group>
      {children}
      {lines.map(({ start, end }, index) => (
        <group key={index} position-z={1}>
          {/* Optional: Add visual indicators like circles here if needed */}
        </group>
      ))}
    </context.Provider>
  );
}

export const Node = forwardRef(
  ({ color = "black", name, connectedTo = [], position = [0, 0, 0], description = 'No description', ...props }, ref) => {
    const set = useContext(context);
    const { size, camera, gl } = useThree();
    const [pos, setPos] = useState(() => new THREE.Vector3(...position));
    const state = useMemo(() => ({ position: pos, connectedTo }), [pos, connectedTo]);

    useLayoutEffect(() => {
      set((nodes) => [...nodes, state]);
      return () => set((nodes) => nodes.filter((n) => n !== state));
    }, [state, pos]);

    const [hovered, setHovered] = useState(false);
    useEffect(() => {
      document.body.style.cursor = hovered ? "grab" : "auto";
    }, [hovered]);

    const bind = useDrag(({ down, xy: [x, y] }) => {
      document.body.style.cursor = down ? "grabbing" : "grab";

      // Get the canvas bounding rect
      const rect = gl.domElement.getBoundingClientRect();
      // Correct the coordinates for the offset
      const correctedX = x - rect.left;
      const correctedY = y - rect.top;

      setPos(
        new THREE.Vector3(
          (correctedX / size.width) * 2 - 1,
          -(correctedY / size.height) * 2 + 1,
          0
        )
          .unproject(camera)
          .multiply({ x: 1, y: 1, z: 0 })
          .clone()
      );
    });

    return (
      <group>
        <Circle ref={ref} {...bind()} opacity={0.2} radius={0.5} color={color} position={pos} {...props}>
          {/* Optional: Visual representation */}
        </Circle>
        <Html position={pos.toArray()}>
          <div onPointerOver={() => setHovered(true)} onPointerOut={() => setHovered(false)}>
            <CustomCard title={name} description={description} />
          </div>
        </Html>
      </group>
    );
  }
);
