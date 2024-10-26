import React, { useEffect, useRef, useState } from "react"
import * as THREE from "three"
import { useThree } from "@react-three/fiber"
import { EdgeLabel } from "./EdgeLabel"
import { Node, Nodes } from "./Nodes"

interface SceneProps {
  canvasDimensions: {
    width: number
    height: number
    top: number
    left: number
  }
  nodes: Array<{
    id: string
    name: string
    type: string
  }>
  edges: Array<{
    source: string
    target: string
    type: string
  }>
}

export function Scene({ canvasDimensions, nodes, edges }: SceneProps) {
  const nodeRefs = useRef(nodes.map(() => React.createRef<THREE.Mesh>()))
  const [opacity, setOpacity] = useState(0)
  const { scene } = useThree()

  // Increase the radius to spread nodes further apart
  const radius = Math.sqrt(nodes.length) * 2

  useEffect(() => {
    scene.background = new THREE.Color(0x000000)
  }, [scene])

  return (
    <group>
      <mesh>
        <planeGeometry args={[100, 100]} />
        <meshBasicMaterial transparent opacity={opacity} />
      </mesh>
      <Nodes canvasDimensions={canvasDimensions}>
        {nodes.map((node, index) => (
          <Node
            key={`node-${node.id}`}
            ref={nodeRefs.current[index]}
            id={node.id}
            name={node.name}
            type={node.type}
            color="black"
            borderColor="white"
            position={[
              Math.cos((-index / nodes.length) * Math.PI * 2) * radius,
              Math.sin((-index / nodes.length) * Math.PI * 2) * radius,
              0
            ]}
            connectedTo={edges.filter(edge => edge.source === node.id).map(edge => edge.target)}
          />
        ))}
        {edges.map((edge, index) => {
          const startNodeIndex = nodes.findIndex(node => node.id === edge.source)
          const endNodeIndex = nodes.findIndex(node => node.id === edge.target)
          return (
            <EdgeLabel
              key={`edge-${edge.source}-${edge.target}-${index}`}
              startNodeRef={nodeRefs.current[startNodeIndex]}
              endNodeRef={nodeRefs.current[endNodeIndex]}
              label={edge.type}
            />
          )
        })}
      </Nodes>
    </group>
  )
}
