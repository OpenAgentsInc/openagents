import * as THREE from 'three'
import {
  createContext,
  useMemo,
  useRef,
  useState,
  useContext,
  useLayoutEffect,
  forwardRef,
  useEffect,
} from 'react'
import { useFrame, useThree } from '@react-three/fiber'
import { QuadraticBezierLine, Text } from '@react-three/drei'
import { useDrag } from '@use-gesture/react'

interface NodeState {
  position: THREE.Vector3
  connectedTo: React.RefObject<THREE.Mesh | null>[]
}

const context = createContext<React.Dispatch<React.SetStateAction<NodeState[]>> | null>(null)

interface CircleProps {
  children?: React.ReactNode
  opacity?: number
  radius?: number
  segments?: number
  color?: string
  position?: THREE.Vector3 | [number, number, number]
  onPointerOver?: () => void
  onPointerOut?: () => void
}

const Circle = forwardRef<THREE.Mesh, CircleProps>(
  ({ children, opacity = 1, radius = 0.05, segments = 32, color = '#ff1050', ...props }, ref) => (
    <mesh ref={ref} {...props}>
      <circleGeometry args={[radius, segments]} />
      <meshBasicMaterial transparent={opacity < 1} opacity={opacity} color={color} />
      {children}
    </mesh>
  ),
)

interface NodesProps {
  children: React.ReactNode
}

export function Nodes({ children }: NodesProps) {
  const group = useRef<THREE.Group>(null)
  const [nodes, set] = useState<NodeState[]>([])
  const lines = useMemo(() => {
    const lines: { start: THREE.Vector3; end: THREE.Vector3 }[] = []
    for (const node of nodes) {
      node.connectedTo
        .map((ref) => [node.position, ref.current?.position])
        .filter(([, end]) => end)
        .forEach(([start, end]) =>
          lines.push({
            start: (start as THREE.Vector3).clone().add(new THREE.Vector3(0.35, 0, 0)),
            end: (end as THREE.Vector3).clone().add(new THREE.Vector3(-0.35, 0, 0)),
          }),
        )
    }
    return lines
  }, [nodes])

  useFrame((_, delta) => {
    if (group.current) {
      group.current.children.forEach((child) => {
        const mesh = child.children[0] as THREE.Line2 | undefined
        if (mesh?.material && 'uniforms' in mesh.material) {
          const mat = mesh.material as THREE.ShaderMaterial
          if (mat.uniforms.dashOffset) {
            mat.uniforms.dashOffset.value -= delta * 10
          }
        }
      })
    }
  })

  return (
    <context.Provider value={set}>
      <group ref={group}>
        {lines.map((line, index) => (
          <group key={index}>
            <QuadraticBezierLine {...line} color="white" dashed dashScale={50} gapSize={20} />
            <QuadraticBezierLine {...line} color="white" lineWidth={0.5} transparent opacity={0.1} />
          </group>
        ))}
      </group>
      {children}
      {lines.map(({ start, end }, index) => (
        <group key={index} position-z={1}>
          <Circle position={start} />
          <Circle position={end} />
        </group>
      ))}
    </context.Provider>
  )
}

interface NodeProps {
  color?: string
  name: string
  connectedTo?: React.RefObject<THREE.Mesh | null>[]
  position?: [number, number, number]
}

export const Node = forwardRef<THREE.Mesh, NodeProps>(
  ({ color = 'black', name, connectedTo = [], position = [0, 0, 0], ...props }, ref) => {
    const set = useContext(context)
    const { size, camera } = useThree()
    const [pos, setPos] = useState(() => new THREE.Vector3(...position))
    const state = useMemo(() => ({ position: pos, connectedTo }), [pos, connectedTo])

    useLayoutEffect(() => {
      if (set) {
        set((nodes) => [...nodes, state])
        return () => set((nodes) => nodes.filter((n) => n !== state))
      }
    }, [state, set])

    const [hovered, setHovered] = useState(false)
    useEffect(() => {
      document.body.style.cursor = hovered ? 'grab' : 'auto'
    }, [hovered])

    const bind = useDrag(({ down, xy: [x, y] }) => {
      document.body.style.cursor = down ? 'grabbing' : 'grab'
      setPos(
        new THREE.Vector3((x / size.width) * 2 - 1, -(y / size.height) * 2 + 1, 0)
          .unproject(camera as THREE.OrthographicCamera)
          .multiply(new THREE.Vector3(1, 1, 0))
          .clone(),
      )
    })

    return (
      <Circle ref={ref} {...bind()} opacity={0.2} radius={0.5} color={color} position={pos} {...props}>
        <Circle
          radius={0.25}
          position={[0, 0, 0.1]}
          onPointerOver={() => setHovered(true)}
          onPointerOut={() => setHovered(false)}
          color={hovered ? '#ff1050' : color}
        >
          <Text position={[0, 0, 1]} fontSize={0.25} color="white">
            {name}
          </Text>
        </Circle>
      </Circle>
    )
  },
)
