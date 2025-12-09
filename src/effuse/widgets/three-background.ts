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

      // Create floating particles/geometry
      const particles: THREE.Mesh[] = []
      const particleCount = 50

      for (let i = 0; i < particleCount; i++) {
        const geometry = new THREE.TetrahedronGeometry(
          0.1 + Math.random() * 0.1,
          0
        )
        // Grayscale colors - white to light gray
        const brightness = 0.7 + Math.random() * 0.3 // 0.7 to 1.0 (light gray to white)
        const material = new THREE.MeshStandardMaterial({
          color: new THREE.Color(brightness, brightness, brightness), // Grayscale
          metalness: 0.3,
          roughness: 0.4,
          transparent: true,
          opacity: 0.5 + Math.random() * 0.4,
        })
        const particle = new THREE.Mesh(geometry, material)

        // Random position in 3D space
        particle.position.set(
          (Math.random() - 0.5) * 20,
          (Math.random() - 0.5) * 20,
          (Math.random() - 0.5) * 20
        )

        // Random rotation
        particle.rotation.set(
          Math.random() * Math.PI * 2,
          Math.random() * Math.PI * 2,
          Math.random() * Math.PI * 2
        )

        // Store velocity for animation
        ;(particle as any).velocity = {
          x: (Math.random() - 0.5) * 0.02,
          y: (Math.random() - 0.5) * 0.02,
          z: (Math.random() - 0.5) * 0.02,
        }
        ;(particle as any).rotationSpeed = {
          x: (Math.random() - 0.5) * 0.02,
          y: (Math.random() - 0.5) * 0.02,
          z: (Math.random() - 0.5) * 0.02,
        }

        scene.add(particle)
        particles.push(particle)
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

        // Animate particles
        particles.forEach((particle, i) => {
          const vel = (particle as any).velocity
          const rotSpeed = (particle as any).rotationSpeed

          // Move particles
          particle.position.x += vel.x
          particle.position.y += vel.y
          particle.position.z += vel.z

          // Rotate particles
          particle.rotation.x += rotSpeed.x
          particle.rotation.y += rotSpeed.y
          particle.rotation.z += rotSpeed.z

          // Wrap around edges
          if (Math.abs(particle.position.x) > 10) vel.x *= -1
          if (Math.abs(particle.position.y) > 10) vel.y *= -1
          if (Math.abs(particle.position.z) > 10) vel.z *= -1

          // Subtle pulsing opacity
          const material = particle.material as THREE.MeshStandardMaterial
          material.opacity = 0.4 + Math.sin(time * 2 + i) * 0.3
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
          particles.forEach((particle) => {
            particle.geometry.dispose()
            ;(particle.material as THREE.Material).dispose()
          })
        })
      )
    }),
}
