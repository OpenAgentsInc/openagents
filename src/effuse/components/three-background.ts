/**
 * Three.js Background Scene Component
 *
 * Creates a cool animated Three.js scene behind the intro card.
 * Transparent background so the dot grid shows through.
 */

import { Effect } from "effect"
import type { Component, ComponentContext } from "../component/types.js"
import { html } from "../template/html.js"
import * as THREE from "three"

// ============================================================================
// Types
// ============================================================================

export interface ThreeBackgroundState {
  // No state needed for static background scene
}

export type ThreeBackgroundEvent = never

// ============================================================================
// Component Definition
// ============================================================================

export const ThreeBackgroundComponent: Component<ThreeBackgroundState, ThreeBackgroundEvent> = {
  id: "three-background",

  initialState: () => ({}),

  render: (ctx) =>
    Effect.gen(function* () {
      // Render canvas container (positioned behind intro card)
      return html`
        <div
          class="three-background-container"
          style="position: fixed; inset: 0; z-index: 0;"
        >
          <canvas
            id="${ctx.container.id}-three-bg-canvas"
            style="display: block; width: 100%; height: 100%; pointer-events: auto;"
          ></canvas>
        </div>
      `
    }),

  setupEvents: (ctx: ComponentContext<ThreeBackgroundState, ThreeBackgroundEvent>) =>
    Effect.gen(function* () {
      // Get canvas element - catch and ignore errors to match Component interface
      const canvasResult = yield* ctx.dom.queryId<HTMLCanvasElement>(
        `${ctx.container.id}-three-bg-canvas`
      ).pipe(Effect.either)

      if (canvasResult._tag === "Left") {
        // Canvas not found, return void (component will work without Three.js background)
        return
      }

      const canvas = canvasResult.right

      // Set canvas size
      const resizeCanvas = () => {
        canvas.width = window.innerWidth
        canvas.height = window.innerHeight
      }
      resizeCanvas()
      window.addEventListener("resize", resizeCanvas)

      // Create Three.js scene with transparent background
      const scene = new THREE.Scene()

      // Top-down orthographic camera (like Factorio map view)
      const viewSize = 20
      const camera = new THREE.OrthographicCamera(
        -viewSize * (canvas.width / canvas.height),
        viewSize * (canvas.width / canvas.height),
        viewSize,
        -viewSize,
        0.1,
        1000
      )
      camera.position.set(0, 15, 0) // Top-down view
      camera.lookAt(0, 0, 0)

      const renderer = new THREE.WebGLRenderer({
        canvas,
        alpha: true, // Transparent background
        antialias: true,
      })
      renderer.setClearColor(0x000000, 0) // Transparent black
      renderer.setSize(canvas.width, canvas.height)
      renderer.setPixelRatio(window.devicePixelRatio)

      // Raycaster for mouse interaction
      const raycaster = new THREE.Raycaster()
      const mouse = new THREE.Vector2()

      // Create ATIF type nodes with connections
      const nodes: THREE.Mesh[] = []
      const connections: THREE.Line[] = []
      const connectionData: Array<{
        line: THREE.Line
        from: THREE.Mesh
        to: THREE.Mesh
        progress: number
      }> = []

      // Create label container for HTML text overlays
      const labelContainer = document.createElement("div")
      labelContainer.style.position = "absolute"
      labelContainer.style.top = "0"
      labelContainer.style.left = "0"
      labelContainer.style.width = "100%"
      labelContainer.style.height = "100%"
      labelContainer.style.pointerEvents = "none"
      labelContainer.style.zIndex = "1"
      ctx.container.appendChild(labelContainer)

      // ATIF types with positions and labels (further from center)
      const atifTypes = [
        { type: "Trajectory", label: "Trajectory", x: -9, z: -9 },
        { type: "Step", label: "Step", x: 0, z: -9 },
        { type: "Agent", label: "Agent", x: 9, z: -9 },
        { type: "ToolCall", label: "ToolCall", x: -9, z: 0 },
        { type: "Observation", label: "Observation", x: 9, z: 0 },
        { type: "Metrics", label: "Metrics", x: -9, z: 9 },
        { type: "Checkpoint", label: "Checkpoint", x: 0, z: 9 },
        { type: "SubagentRef", label: "SubagentRef", x: 9, z: 9 },
      ]

      // Create connections (dotted lines)
      const connectionPairs = [
        [0, 1], // Trajectory -> Step
        [0, 2], // Trajectory -> Agent
        [1, 3], // Step -> ToolCall
        [1, 4], // Step -> Observation
        [1, 5], // Step -> Metrics
        [0, 6], // Trajectory -> Checkpoint
        [4, 7], // Observation -> SubagentRef
      ]

      // Create nodes
      for (let i = 0; i < atifTypes.length; i++) {
        const atifType = atifTypes[i]

        // Node base (square, top-down view) - dark with white border
        const nodeGeometry = new THREE.BoxGeometry(1.8, 0.2, 1.8)
        const nodeMaterial = new THREE.MeshStandardMaterial({
          color: new THREE.Color(0.05, 0.05, 0.05), // Very dark/black
          metalness: 0.1,
          roughness: 0.8,
          transparent: true,
          opacity: 0.7,
        })
        const node = new THREE.Mesh(nodeGeometry, nodeMaterial)

        // Position
        node.position.set(atifType.x, 0, atifType.z)

        // White border wireframe
        const borderGeometry = new THREE.BoxGeometry(1.85, 0.21, 1.85)
        const wireframe = new THREE.WireframeGeometry(borderGeometry)
        const lineMaterial = new THREE.LineBasicMaterial({
          color: new THREE.Color(1.0, 1.0, 1.0), // White
          transparent: true,
          opacity: 0.9,
        })
        const borderLines = new THREE.LineSegments(wireframe, lineMaterial)
        borderLines.position.y = 0.1
        node.add(borderLines)
        ;(node as any).borderLines = borderLines

        // Store node metadata
        ;(node as any).pulsePhase = Math.random() * Math.PI * 2
        ;(node as any).isHovered = false
        ;(node as any).nodeId = i
        ;(node as any).atifType = atifType.type
        ;(node as any).label = atifType.label
        ;(node as any).screenPosition = { x: 0, y: 0 } // Will be updated

        scene.add(node)
        nodes.push(node)

        // Create HTML label for this node (inside the node)
        const label = document.createElement("div")
        label.textContent = atifType.label
        label.style.position = "absolute"
        label.style.color = "#ffffff"
        label.style.fontSize = "11px"
        label.style.fontFamily = "'Berkeley Mono', monospace"
        label.style.textAlign = "center"
        label.style.whiteSpace = "nowrap"
        label.style.pointerEvents = "none"
        label.style.fontWeight = "500"
        label.style.opacity = "0.95"
        label.id = `atif-label-${i}`
        labelContainer.appendChild(label)
        ;(node as any).labelElement = label
      }

      // Create dotted connection lines with animation
      connectionPairs.forEach(([fromIdx, toIdx]) => {
        const fromNode = nodes[fromIdx]
        const toNode = nodes[toIdx]

        // Create dotted line using points
        const points: THREE.Vector3[] = []
        const segments = 30
        for (let i = 0; i <= segments; i++) {
          const t = i / segments
          const x = fromNode.position.x + (toNode.position.x - fromNode.position.x) * t
          const z = fromNode.position.z + (toNode.position.z - fromNode.position.z) * t
          points.push(new THREE.Vector3(x, 0.15, z))
        }

        const geometry = new THREE.BufferGeometry().setFromPoints(points)

        // Dotted line material (grayscale) - will animate dash offset
        const material = new THREE.LineDashedMaterial({
          color: new THREE.Color(0.6, 0.6, 0.6), // Gray
          dashSize: 0.4,
          gapSize: 0.3,
          transparent: true,
          opacity: 0.6,
        })

        const line = new THREE.Line(geometry, material)
        line.computeLineDistances() // Required for LineDashedMaterial
        scene.add(line)
        connections.push(line)

        // Store connection data for animation
        connectionData.push({
          line,
          from: fromNode,
          to: toNode,
          progress: Math.random(), // Random starting position
        })
      })

      // Simple top-down lighting
      const ambientLight = new THREE.AmbientLight(0xffffff, 0.6)
      scene.add(ambientLight)

      // Directional light from above (top-down view)
      const topLight = new THREE.DirectionalLight(0xffffff, 0.8)
      topLight.position.set(0, 10, 0)
      scene.add(topLight)

      // Animation loop
      let animationId: number | null = null
      let time = 0

      const animate = () => {
        animationId = requestAnimationFrame(animate)
        time += 0.01

        // Animate node borders (subtle pulse)
        nodes.forEach((node) => {
          // No indicator animation needed - using border instead

          // Update label position (project 3D to screen coordinates)
          const labelElement = (node as any).labelElement as HTMLElement
          if (labelElement) {
            const vector = new THREE.Vector3()
            node.getWorldPosition(vector)

            // Project 3D position to normalized device coordinates (-1 to 1)
            vector.project(camera)

            // Convert NDC to viewport pixels
            // NDC: x and y are -1 to 1, with (0,0) at center, y up
            // Viewport: x and y are 0 to width/height, with (0,0) at top-left, y down
            // Since labelContainer is fixed with inset:0, use window dimensions
            const x = ((vector.x + 1) / 2) * window.innerWidth
            const y = ((-vector.y + 1) / 2) * window.innerHeight

            labelElement.style.left = `${x}px`
            labelElement.style.top = `${y}px`
            labelElement.style.transform = "translate(-50%, -50%)"
          }
        })

        // Animate connection lines (moving dashes)
        connectionData.forEach((conn) => {
          const material = conn.line.material as THREE.LineDashedMaterial
          // Animate dash offset to create flowing effect
          conn.progress += 0.01 // Speed of animation
          if (conn.progress > 1) conn.progress = 0

          // Calculate dash offset based on progress
          // LineDashedMaterial has dashOffset but TypeScript types may not include it
          const offset = -conn.progress * (material.dashSize + material.gapSize) * 10
          if ("dashOffset" in material) {
            (material as THREE.LineDashedMaterial & { dashOffset: number }).dashOffset = offset
          }
        })

        renderer.render(scene, camera)
      }

      animate()

      // Handle window resize
      const handleResize = () => {
        resizeCanvas()
        const aspect = canvas.width / canvas.height
        camera.left = -viewSize * aspect
        camera.right = viewSize * aspect
        camera.updateProjectionMatrix()
        renderer.setSize(canvas.width, canvas.height)
      }
      window.addEventListener("resize", handleResize)

      // Mouse interaction for clickable nodes
      const onMouseMove = (event: MouseEvent) => {
        mouse.x = (event.clientX / canvas.width) * 2 - 1
        mouse.y = -(event.clientY / canvas.height) * 2 + 1

        raycaster.setFromCamera(mouse, camera)
        const intersects = raycaster.intersectObjects(nodes)

        // Reset all nodes
        nodes.forEach((node) => {
          const material = node.material as THREE.MeshStandardMaterial
          material.color.setRGB(0.05, 0.05, 0.05) // Dark/black
          material.opacity = 0.7
          ;(node as any).isHovered = false
        })

        // Highlight hovered node (brighter with more opacity)
        if (intersects.length > 0) {
          const hoveredNode = intersects[0].object as THREE.Mesh
          const material = hoveredNode.material as THREE.MeshStandardMaterial
          material.color.setRGB(0.15, 0.15, 0.15) // Slightly brighter
          material.opacity = 0.9 // More opaque
          ;(hoveredNode as any).isHovered = true
          canvas.style.cursor = "pointer"
        } else {
          canvas.style.cursor = "default"
        }
      }

      const onMouseClick = (event: MouseEvent) => {
        mouse.x = (event.clientX / canvas.width) * 2 - 1
        mouse.y = -(event.clientY / canvas.height) * 2 + 1

        raycaster.setFromCamera(mouse, camera)
        const intersects = raycaster.intersectObjects(nodes)

        if (intersects.length > 0) {
          const clickedNode = intersects[0].object as THREE.Mesh & {
            atifType?: string
            label?: string
          }
          const atifType = clickedNode.atifType
          const label = clickedNode.label
          if (atifType && label) {
            console.log(`[Three Background] ATIF ${atifType} (${label}) clicked`)
            // TODO: Emit event or trigger action
          }
        }
      }

      canvas.addEventListener("mousemove", onMouseMove)
      canvas.addEventListener("click", onMouseClick)

      // Store cleanup function - mount system will handle calling it via scope cleanup
      const cleanup = () => {
        if (animationId !== null) {
          cancelAnimationFrame(animationId)
        }
        window.removeEventListener("resize", resizeCanvas)
        window.removeEventListener("resize", handleResize)
        canvas.removeEventListener("mousemove", onMouseMove)
        canvas.removeEventListener("click", onMouseClick)
        if (labelContainer.parentNode) {
          labelContainer.parentNode.removeChild(labelContainer)
        }
        renderer.dispose()
        nodes.forEach((node) => {
          node.traverse((child) => {
            if (child instanceof THREE.Mesh) {
              child.geometry.dispose()
              if (Array.isArray(child.material)) {
                child.material.forEach((mat) => mat.dispose())
              } else {
                child.material.dispose()
              }
            }
          })
        })
        connections.forEach((line) => {
          line.geometry.dispose()
          if (Array.isArray(line.material)) {
            line.material.forEach((mat) => mat.dispose())
          } else {
            line.material.dispose()
          }
        })
      }
      // Store cleanup on container for mount system to access
      ;(ctx.container as any).__threeBackgroundCleanup = cleanup
    }).pipe(Effect.catchAll(() => Effect.void)),
}
