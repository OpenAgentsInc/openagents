/**
 * Three.js Scene Example
 *
 * Simple example showing how to use Three.js with the no-build ESM setup.
 * Import statements are preserved and resolved via the import map.
 */

import * as THREE from "three"

/**
 * Create and start a simple Three.js scene
 */
export function createScene(canvas: HTMLCanvasElement): void {
  // Scene setup
  const scene = new THREE.Scene()
  const camera = new THREE.PerspectiveCamera(
    75,
    canvas.width / canvas.height,
    0.1,
    1000
  )
  const renderer = new THREE.WebGLRenderer({ canvas, antialias: true })
  renderer.setSize(canvas.width, canvas.height)

  // Add a cube
  const geometry = new THREE.BoxGeometry(1, 1, 1)
  const material = new THREE.MeshStandardMaterial({ color: 0x00ff00 })
  const cube = new THREE.Mesh(geometry, material)
  scene.add(cube)

  // Add lighting
  const light = new THREE.DirectionalLight(0xffffff, 1)
  light.position.set(5, 5, 5)
  scene.add(light)
  scene.add(new THREE.AmbientLight(0x404040))

  // Position camera
  camera.position.z = 5

  // Animation loop
  function animate() {
    requestAnimationFrame(animate)
    cube.rotation.x += 0.01
    cube.rotation.y += 0.01
    renderer.render(scene, camera)
  }

  animate()
}
