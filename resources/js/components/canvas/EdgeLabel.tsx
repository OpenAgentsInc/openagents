import React, { useRef, useState } from "react"
import * as THREE from "three"
import { Text } from "@react-three/drei"
import { useFrame } from "@react-three/fiber"

interface EdgeLabelProps {
  startNodeRef: React.RefObject<THREE.Object3D>
  endNodeRef: React.RefObject<THREE.Object3D>
  label: string
}

export function EdgeLabel({ startNodeRef, endNodeRef, label }: EdgeLabelProps) {
  const ref = useRef<THREE.Mesh>(null)
  const [pos, setPos] = useState(new THREE.Vector3())
  const [rotation, setRotation] = useState(0)

  useFrame(() => {
    if (startNodeRef.current && endNodeRef.current) {
      const startPos = startNodeRef.current.position
      const endPos = endNodeRef.current.position
      const midPoint = new THREE.Vector3().addVectors(startPos, endPos).multiplyScalar(0.5)
      setPos(midPoint)

      // Calculate the angle between start and end points
      let angle = Math.atan2(endPos.y - startPos.y, endPos.x - startPos.x)

      // Adjust the angle to keep text upright
      if (angle < -Math.PI / 2 || angle > Math.PI / 2) {
        angle += Math.PI
      }

      setRotation(angle)
    }
  })

  return (
    <Text
      ref={ref}
      position={pos}
      rotation={[0, 0, rotation]}
      fontSize={0.16}
      color="white"
      font="/fonts/JetBrainsMono-Regular.ttf"
      anchorX="center"
      anchorY="middle"
    >
      {label}
    </Text>
  )
}
