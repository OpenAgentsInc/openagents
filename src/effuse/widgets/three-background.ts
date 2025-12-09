/**
 * Three.js Background Scene Widget
 *
 * Creates a cool animated Three.js scene behind the intro card.
 * Transparent background so the dot grid shows through.
 */

import { Effect, Ref, Stream, pipe } from "effect"
import { html } from "../template/html.js"
import type { Widget } from "../widget/types.js"
import * as THREE from "three"

// ============================================================================
// Types
// ============================================================================

export interface ThreeBackgroundState {
  // No state needed for static background scene
}

export type ThreeBackgroundEvent = never

// ============================================================================
// Widget Definition
// ============================================================================

export const ThreeBackgroundWidget: Widget<ThreeBackgroundState, ThreeBackgroundEvent> = {
  id: "three-background",

  initialState: () => ({}),

  render: (ctx) =>
    Effect.gen(function* () {
      // Render canvas container (positioned behind intro card)
      return html`
        <div
          class="three-background-container"
          style="position: fixed; inset: 0; z-index: 0; pointer-events: none;"
        >
          <canvas
            id="${ctx.container.id}-three-bg-canvas"
            style="display: block; width: 100%; height: 100%;"
          ></canvas>
        </div>
      `
    }),

  setupEvents: (ctx) =>
    Effect.gen(function* () {
      // Get canvas element
      const canvas = yield* ctx.dom.queryId<HTMLCanvasElement>(
        `${ctx.container.id}-three-bg-canvas`
      )

      // Set canvas size
      const resizeCanvas = () => {
        canvas.width = window.innerWidth
        canvas.height = window.innerHeight
      }
      resizeCanvas()
      window.addEventListener("resize", resizeCanvas)

      // Create Three.js scene with transparent background
      const scene = new THREE.Scene()
      const camera = new THREE.PerspectiveCamera(
        75,
        canvas.width / canvas.height,
        0.1,
        1000
      )
      const renderer = new THREE.WebGLRenderer({
        canvas,
        alpha: true, // Transparent background
        antialias: true,
      })
      renderer.setClearColor(0x000000, 0) // Transparent black
      renderer.setSize(canvas.width, canvas.height)
      renderer.setPixelRatio(window.devicePixelRatio)

      // Create Factorio-inspired factory structures
      const factoryObjects: THREE.Mesh[] = []

      // Create factory machines (assemblers) - grid-aligned
      const machineCount = 12
      const gridSpacing = 3
      const gridOffset = -15

      for (let i = 0; i < machineCount; i++) {
        const row = Math.floor(i / 4)
        const col = i % 4

        // Machine base (rectangular, like Factorio assemblers)
        const machineGeometry = new THREE.BoxGeometry(1.2, 0.3, 1.2)
        const machineMaterial = new THREE.MeshStandardMaterial({
          color: new THREE.Color(0.85, 0.85, 0.85), // Light gray
          metalness: 0.5,
          roughness: 0.3,
          transparent: true,
          opacity: 0.7,
        })
        const machine = new THREE.Mesh(machineGeometry, machineMaterial)

        // Position on grid
        machine.position.set(
          gridOffset + col * gridSpacing,
          -2 + row * gridSpacing * 0.8,
          -5 + row * 0.5
        )

        // Machine top (status indicator)
        const topGeometry = new THREE.BoxGeometry(1.0, 0.2, 1.0)
        const topMaterial = new THREE.MeshStandardMaterial({
          color: new THREE.Color(0.6, 0.6, 0.6), // Darker gray
          metalness: 0.6,
          roughness: 0.2,
          transparent: true,
          opacity: 0.8,
        })
        const top = new THREE.Mesh(topGeometry, topMaterial)
        top.position.set(0, 0.25, 0)
        machine.add(top)

        // Status light (green for active)
        const lightGeometry = new THREE.SphereGeometry(0.1, 8, 8)
        const lightMaterial = new THREE.MeshStandardMaterial({
          color: new THREE.Color(0.2, 0.8, 0.2), // Green
          emissive: new THREE.Color(0.1, 0.4, 0.1),
          transparent: true,
          opacity: 0.9,
        })
        const statusLight = new THREE.Mesh(lightGeometry, lightMaterial)
        statusLight.position.set(0.4, 0.35, 0.4)
        machine.add(statusLight)

        ;(machine as any).statusLight = statusLight
        ;(machine as any).pulsePhase = Math.random() * Math.PI * 2

        scene.add(machine)
        factoryObjects.push(machine)
      }

      // Create conveyor belts (horizontal lines connecting machines)
      const beltCount = 8
      for (let i = 0; i < beltCount; i++) {
        const row = Math.floor(i / 2)
        const isHorizontal = i % 2 === 0

        const beltGeometry = isHorizontal
          ? new THREE.BoxGeometry(2.5, 0.1, 0.3)
          : new THREE.BoxGeometry(0.3, 0.1, 2.5)
        const beltMaterial = new THREE.MeshStandardMaterial({
          color: new THREE.Color(0.5, 0.5, 0.5), // Medium gray
          metalness: 0.4,
          roughness: 0.5,
          transparent: true,
          opacity: 0.6,
        })
        const belt = new THREE.Mesh(beltGeometry, beltMaterial)

        if (isHorizontal) {
          belt.position.set(
            gridOffset + 1.5 * gridSpacing,
            -1.9 + row * gridSpacing * 0.8,
            -5 + row * 0.5
          )
        } else {
          belt.position.set(
            gridOffset + 0.5 * gridSpacing,
            -1.9 + row * gridSpacing * 0.8,
            -5 + row * 0.5
          )
        }

        ;(belt as any).offset = Math.random() * 2
        scene.add(belt)
        factoryObjects.push(belt)
      }

      // Create power poles (vertical structures)
      const poleCount = 6
      for (let i = 0; i < poleCount; i++) {
        const poleGeometry = new THREE.BoxGeometry(0.15, 2, 0.15)
        const poleMaterial = new THREE.MeshStandardMaterial({
          color: new THREE.Color(0.4, 0.4, 0.4), // Dark gray
          metalness: 0.7,
          roughness: 0.3,
          transparent: true,
          opacity: 0.7,
        })
        const pole = new THREE.Mesh(poleGeometry, poleMaterial)

        pole.position.set(
          gridOffset + (i % 3) * gridSpacing * 1.5,
          -1,
          -5 + Math.floor(i / 3) * gridSpacing
        )

        scene.add(pole)
        factoryObjects.push(pole)
      }

      // Add dramatic lighting for grayscale scene
      const ambientLight = new THREE.AmbientLight(0xffffff, 0.4)
      scene.add(ambientLight)

      // Main directional light (key light)
      const keyLight = new THREE.DirectionalLight(0xffffff, 1.2)
      keyLight.position.set(5, 8, 5)
      keyLight.castShadow = false
      scene.add(keyLight)

      // Fill light (softer, from opposite side)
      const fillLight = new THREE.DirectionalLight(0xffffff, 0.5)
      fillLight.position.set(-5, 3, -5)
      scene.add(fillLight)

      // Rim light (backlight for edge definition)
      const rimLight = new THREE.DirectionalLight(0xffffff, 0.8)
      rimLight.position.set(0, 0, -10)
      scene.add(rimLight)

      // Point light for extra depth
      const pointLight = new THREE.PointLight(0xffffff, 0.6, 20)
      pointLight.position.set(0, 0, 10)
      scene.add(pointLight)

      // Position camera
      camera.position.z = 10
      camera.position.y = 2
      camera.lookAt(0, 0, 0)

      // Animation loop
      let animationId: number | null = null
      let time = 0

      const animate = () => {
        animationId = requestAnimationFrame(animate)
        time += 0.01

        // Animate factory objects
        factoryObjects.forEach((obj, i) => {
          // Pulse status lights on machines
          if ((obj as any).statusLight) {
            const light = (obj as any).statusLight as THREE.Mesh
            const phase = (obj as any).pulsePhase + time * 2
            const intensity = 0.5 + Math.sin(phase) * 0.3
            const material = light.material as THREE.MeshStandardMaterial
            material.emissive.setRGB(0.1 * intensity, 0.4 * intensity, 0.1 * intensity)
          }

          // Animate conveyor belts (subtle movement)
          if ((obj as any).offset !== undefined) {
            const belt = obj as THREE.Mesh
            const offset = (obj as any).offset + time * 0.5
            ;(obj as any).offset = offset % 1

            // Subtle texture offset effect (visual only, no actual texture)
            const material = belt.material as THREE.MeshStandardMaterial
            material.opacity = 0.5 + Math.sin(offset * Math.PI * 2) * 0.1
          }
        })

        // Gentle camera movement
        camera.position.x = Math.sin(time * 0.3) * 2
        camera.position.y = Math.cos(time * 0.2) * 1.5
        camera.lookAt(0, 0, 0)

        renderer.render(scene, camera)
      }

      animate()

      // Handle window resize
      const handleResize = () => {
        resizeCanvas()
        camera.aspect = canvas.width / canvas.height
        camera.updateProjectionMatrix()
        renderer.setSize(canvas.width, canvas.height)
      }
      window.addEventListener("resize", handleResize)

      // Cleanup
      yield* Effect.addFinalizer(() =>
        Effect.sync(() => {
          if (animationId !== null) {
            cancelAnimationFrame(animationId)
          }
          window.removeEventListener("resize", resizeCanvas)
          window.removeEventListener("resize", handleResize)
          renderer.dispose()
          factoryObjects.forEach((obj) => {
            obj.traverse((child) => {
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
        })
      )
    }),
}
