import React, { useEffect, useRef, useState } from "react"
import { OrbitControls, OrthographicCamera } from "@react-three/drei"
import { Canvas } from "@react-three/fiber"
import { demoEdges, demoNodes } from "./demoData"
import { Scene } from "./Scene"

export default function CanvasLayout({ enablePan = true }) {
  const canvasRef = useRef<HTMLDivElement>(null)
  const [canvasDimensions, setCanvasDimensions] = useState({ width: 0, height: 0, top: 0, left: 0 })

  useEffect(() => {
    const updateDimensions = () => {
      if (canvasRef.current) {
        const rect = canvasRef.current.getBoundingClientRect()
        setCanvasDimensions({
          width: rect.width,
          height: rect.height,
          top: rect.top,
          left: rect.left
        })
      }
    }

    updateDimensions()
    window.addEventListener('resize', updateDimensions)
    return () => window.removeEventListener('resize', updateDimensions)
  }, [])

  return (
    <div className="w-full h-full bg-black relative">
      <div id="canvas-container" ref={canvasRef} className="w-full h-full">
        <Canvas>
          <OrthographicCamera
            makeDefault
            zoom={80}
            position={[0, 0, 100]}
          />
          <OrbitControls
            enableRotate={false}
            enablePan={false}
            enableZoom={false}
            zoomSpeed={0.5}
            panSpeed={0.5}
            minZoom={50}
            maxZoom={200}
          />
          <Scene canvasDimensions={canvasDimensions} nodes={demoNodes} edges={demoEdges} />
        </Canvas>
      </div>
      <div className="absolute inset-0 bg-black pointer-events-none animate-fade-out"></div>
    </div>
  )
}
