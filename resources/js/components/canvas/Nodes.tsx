import React, {
  createContext, forwardRef, useContext, useEffect, useMemo, useRef, useState
} from "react"
import * as THREE from "three"
import { Line, Text } from "@react-three/drei"
import { useFrame, useThree } from "@react-three/fiber"
import { useDrag } from "@use-gesture/react"

interface ContextType {
  nodes: any[]
  setNodes: React.Dispatch<React.SetStateAction<any[]>>
  updateNodePosition: (id: string, newPosition: THREE.Vector3) => void
  canvasDimensions: any
}

const NodeContext = createContext<ContextType | undefined>(undefined)

interface CircleProps {
  children?: React.ReactNode
  opacity?: number
  radius?: number
  segments?: number
  color?: string
  [key: string]: any
}

const Circle = forwardRef<THREE.Mesh, CircleProps>(({ children, opacity = 1, radius = 0.05, segments = 32, color = '#ff1050', ...props }, ref) => (
  <mesh ref={ref} {...props}>
    <circleGeometry args={[radius, segments]} />
    <meshBasicMaterial transparent={opacity < 1} opacity={opacity} color={color} />
    {children}
  </mesh>
))

interface NodesProps {
  children: React.ReactNode
  canvasDimensions: any
}

interface LineType {
  points: THREE.Vector3[]
  key: string
}

export function Nodes({ children, canvasDimensions }: NodesProps) {
  const group = useRef<THREE.Group>(null)
  const [nodes, setNodes] = useState<any[]>([])

  const lines = useMemo<LineType[]>(() => {
    const lines: LineType[] = []
    for (let node of nodes) {
      node.connectedTo.forEach((connectedNodeId: string) => {
        const connectedNode = nodes.find(n => n.id === connectedNodeId)
        if (connectedNode && node.position && connectedNode.position) {
          lines.push({
            points: [
              node.position,
              connectedNode.position
            ],
            key: `line-${node.id}-${connectedNodeId}`
          })
        }
      })
    }
    return lines
  }, [nodes])

  useFrame((_, delta) => {
    if (group.current) {
      group.current.children.forEach((child) => {
        if ((child as any).material?.uniforms?.dashOffset) {
          (child as any).material.uniforms.dashOffset.value -= delta * 10
        }
      })
    }
  })

  const updateNodePosition = (id: string, newPosition: THREE.Vector3) => {
    setNodes(prevNodes =>
      prevNodes.map(node =>
        node.id === id ? { ...node, position: newPosition } : node
      )
    )
  }

  const contextValue: ContextType = { nodes, setNodes, updateNodePosition, canvasDimensions }

  return (
    <NodeContext.Provider value={contextValue}>
      <group ref={group}>
        {lines.map((line) => (
          <group key={line.key}>
            <Line points={line.points} color="white" dashed dashScale={50} gapSize={20} />
            <Line points={line.points} color="white" lineWidth={0.5} transparent opacity={0.1} />
          </group>
        ))}
      </group>
      {children}
    </NodeContext.Provider>
  )
}

interface NodeProps {
  id: string
  color?: string
  name: string
  type: string
  borderColor: string
  connectedTo?: string[]
  position?: [number, number, number]
}

export const Node = forwardRef<THREE.Mesh, NodeProps>(({ id, color = 'black', name, type, borderColor, connectedTo = [], position = [0, 0, 0], ...props }, ref) => {
  const nodeContext = useContext(NodeContext)
  if (!nodeContext) {
    throw new Error("Node must be used within a Nodes component")
  }
  const { nodes, setNodes, updateNodePosition, canvasDimensions } = nodeContext
  const { camera } = useThree()
  const [pos, setPos] = useState(() => new THREE.Vector3(...position))

  useEffect(() => {
    setNodes(prevNodes => [...prevNodes, { id, position: pos, connectedTo }])
    return () => setNodes(prevNodes => prevNodes.filter(node => node.id !== id))
  }, [])

  useEffect(() => {
    updateNodePosition(id, pos)
  }, [pos])

  const [hovered, setHovered] = useState(false)
  useEffect(() => void (document.body.style.cursor = hovered ? 'grab' : 'auto'), [hovered])

  const bind = useDrag(({ down, xy: [x, y] }) => {
    document.body.style.cursor = down ? 'grabbing' : 'grab'
    const { width, height, top, left } = canvasDimensions
    const normalizedX = ((x - left) / width) * 2 - 1
    const normalizedY = -((y - top) / height) * 2 + 1
    const newPos = new THREE.Vector3(normalizedX, normalizedY, 0).unproject(camera).multiply({ x: 1, y: 1, z: 0 })
    if (!isNaN(newPos.x) && !isNaN(newPos.y) && !isNaN(newPos.z)) {
      setPos(newPos)
    } else {
      console.error('Invalid position:', newPos)
    }
  })

  if (isNaN(pos.x) || isNaN(pos.y) || isNaN(pos.z)) {
    console.error('Invalid node position:', pos)
    return null
  }

  const circleRadius = 0.1

  return (
    <group>
      <Text
        position={[pos.x, pos.y + 0.25, pos.z]}
        fontSize={0.16}
        color="white"
        font="/fonts/JetBrainsMono-Regular.ttf"
        anchorX="center"
        anchorY="bottom"
      >
        {name}
      </Text>
      <Circle ref={ref} {...bind()} opacity={1} radius={circleRadius + 0.01} color={borderColor} position={pos} {...props}>
        <Circle
          radius={circleRadius}
          position={[0, 0, 0.1]}
          onPointerOver={() => setHovered(true)}
          onPointerOut={() => setHovered(false)}
          color={color}
        />
      </Circle>
    </group>
  )
})