// 3D network scene for the sidebar (@opentui/three): a slowly rotating
// wireframe icosahedron core with orbiting node satellites linked to the
// center - the OpenAgents network of Pylon contributor nodes around the
// market core. Rendered by Three.js on a native WebGPU device and quantized
// to quadrant glyphs by @opentui/three.
//
// This module builds the scene graph and animation; the view component
// (NetworkPane in app.tsx) mounts it as a Solid-managed ThreeRenderable so
// layout and frame-buffer allocation work like any other element. Failure
// is always soft; disable entirely with PYLON_DISABLE_3D=1.

import { openagentsThemeJson, resolveThemeHex } from "./theme"

export interface NetworkSceneParts {
  three: typeof import("@opentui/three")
  scene: import("three").Scene
  camera: import("three").PerspectiveCamera
  tick: (deltaMs: number) => void
  dispose: () => void
}

// Live node state the scene visualizes (issue: make the diagram logical):
// wallet status drives core color and orbital speed, log activity pulses
// satellites, and a balance increase fires a bitcoin-orange payment burst.
export interface NetworkSceneState {
  online: boolean
  balanceSats: number | null
  activityCount: number
}

const NODE_COUNT = 8

export async function buildNetworkScene(
  readState: () => NetworkSceneState = () => ({ online: false, balanceSats: null, activityCount: 0 }),
): Promise<NetworkSceneParts | null> {
  try {
    const [three, THREE] = await Promise.all([import("@opentui/three"), import("three")])

    const coreOnlineColor = new THREE.Color(resolveThemeHex(openagentsThemeJson, "online"))
    const coreOfflineColor = new THREE.Color(resolveThemeHex(openagentsThemeJson, "error"))
    const paymentColor = new THREE.Color(resolveThemeHex(openagentsThemeJson, "payment"))
    const coreColor = new THREE.Color(resolveThemeHex(openagentsThemeJson, "border"))
    const nodeColor = new THREE.Color(resolveThemeHex(openagentsThemeJson, "accent"))
    const linkColor = new THREE.Color(resolveThemeHex(openagentsThemeJson, "online"))

    const scene = new THREE.Scene()
    scene.add(new THREE.AmbientLight(new THREE.Color(0.5, 0.5, 0.5), 1.0))
    const keyLight = new THREE.PointLight(new THREE.Color(1, 1, 1), 60)
    keyLight.position.set(4, 4, 6)
    scene.add(keyLight)

    const group = new THREE.Group()
    scene.add(group)

    // The market core.
    const core = new THREE.Mesh(
      new THREE.IcosahedronGeometry(1.15, 0),
      new THREE.MeshBasicMaterial({ color: coreColor, wireframe: true }),
    )
    group.add(core)

    // Contributor nodes on two inclined orbital rings, each linked to the
    // core by a line updated as it orbits.
    type OrbitingNode = {
      mesh: InstanceType<typeof THREE.Mesh>
      radius: number
      speed: number
      phase: number
      inclination: number
    }
    const nodes: OrbitingNode[] = []
    const nodeGeometry = new THREE.OctahedronGeometry(0.16, 0)
    const nodeMaterial = new THREE.MeshPhongMaterial({ color: nodeColor })
    for (let i = 0; i < NODE_COUNT; i += 1) {
      const mesh = new THREE.Mesh(nodeGeometry, nodeMaterial)
      group.add(mesh)
      nodes.push({
        mesh,
        radius: 2.0 + (i % 2) * 0.55,
        speed: 0.35 + (i % 3) * 0.12,
        phase: (i / NODE_COUNT) * Math.PI * 2,
        inclination: i % 2 === 0 ? 0.45 : -0.6,
      })
    }

    const linkPositions = new Float32Array(NODE_COUNT * 2 * 3)
    const linkGeometry = new THREE.BufferGeometry()
    linkGeometry.setAttribute("position", new THREE.BufferAttribute(linkPositions, 3))
    const links = new THREE.LineSegments(
      linkGeometry,
      new THREE.LineBasicMaterial({ color: linkColor, transparent: true, opacity: 0.55 }),
    )
    group.add(links)

    const camera = new THREE.PerspectiveCamera(42, 1, 0.1, 100)
    camera.position.set(0, 0.8, 7.2)
    camera.lookAt(0, 0, 0)

    let elapsed = 0
    let orbit = 0
    let lastActivityCount = -1
    let lastBalance: number | null = null
    let activityPulse = 0
    let activityNode = 0
    let paymentPulse = 0
    const coreMaterial = core.material as { color: { copy: (c: unknown) => void; lerpColors?: never } }

    const tick = (deltaMs: number) => {
      const dt = deltaMs / 1000
      elapsed += dt

      const state = readState()

      // Wallet status: color the core/links, and slow the network to a
      // crawl while the wallet daemon is offline.
      const speedFactor = state.online ? 1 : 0.3
      orbit += dt * speedFactor

      // Log/network activity: each new feed line pulses the next satellite.
      if (lastActivityCount >= 0 && state.activityCount > lastActivityCount) {
        activityPulse = 1
        activityNode = (activityNode + 1) % NODE_COUNT
      }
      lastActivityCount = state.activityCount

      // Payment: a balance increase fires a bitcoin-orange core burst.
      if (lastBalance !== null && state.balanceSats !== null && state.balanceSats > lastBalance) {
        paymentPulse = 1
      }
      if (state.balanceSats !== null) lastBalance = state.balanceSats

      activityPulse = Math.max(0, activityPulse - dt * 1.8)
      paymentPulse = Math.max(0, paymentPulse - dt * 0.7)

      const statusColor = state.online ? coreOnlineColor : coreOfflineColor
      const blended = statusColor.clone().lerp(paymentColor, paymentPulse)
      coreMaterial.color.copy(blended)
      ;(links.material as { color: { copy: (c: unknown) => void } }).color.copy(blended)

      core.rotation.y += 0.35 * dt * speedFactor
      core.rotation.x += 0.12 * dt * speedFactor
      group.rotation.y += 0.05 * dt * speedFactor
      core.scale.setScalar(1 + Math.sin(elapsed * 1.4) * 0.04 + paymentPulse * 0.3)

      nodes.forEach((node, index) => {
        const angle = node.phase + orbit * node.speed
        const x = Math.cos(angle) * node.radius
        const z = Math.sin(angle) * node.radius
        const y = Math.sin(angle + node.inclination) * node.radius * Math.sin(node.inclination)
        node.mesh.position.set(x, y, z)
        node.mesh.rotation.y += 1.2 * dt
        const pulse = index === activityNode ? 1 + activityPulse * 1.4 : 1
        node.mesh.scale.setScalar(pulse)
        const base = index * 6
        linkPositions[base + 3] = x
        linkPositions[base + 4] = y
        linkPositions[base + 5] = z
      })
      linkGeometry.attributes.position!.needsUpdate = true
    }

    const dispose = () => {
      nodeGeometry.dispose()
      nodeMaterial.dispose()
      linkGeometry.dispose()
      core.geometry.dispose()
      ;(core.material as { dispose: () => void }).dispose()
    }

    return { three, scene, camera, tick, dispose }
  } catch {
    return null
  }
}

// Mounts the scene as an absolutely-positioned overlay added directly to
// renderer.root - the layout path verified to allocate the renderable's
// frame buffer. The Solid sidebar reserves the cells underneath; this
// overlay paints them. Repositions on terminal resize.
export interface NetworkOverlayHandle {
  dispose: () => void
}

export async function mountNetworkOverlay(cli: {
  root: { add: (r: unknown) => void; remove?: (id: string) => void }
  width: number
  height: number
  setFrameCallback: (cb: (dt: number) => Promise<void>) => void
  removeFrameCallback: (cb: (dt: number) => Promise<void>) => void
  start: () => void
  on: (event: string, cb: () => void) => void
  off?: (event: string, cb: () => void) => void
}, readState?: () => NetworkSceneState): Promise<NetworkOverlayHandle | null> {
  const parts = await buildNetworkScene(readState)
  if (!parts) return null
  try {
    const { ThreeRenderable } = parts.three
    const { BoxRenderable, RGBA } = await import("@opentui/core")

    const PANE_WIDTH = 33
    const PANE_HEIGHT = 10
    const position = () => ({
      left: Math.max(0, cli.width - PANE_WIDTH - 1),
      top: Math.max(0, cli.height - PANE_HEIGHT - 7),
    })

    const box = new BoxRenderable(cli as never, {
      id: "network-overlay",
      position: "absolute",
      ...position(),
      width: PANE_WIDTH,
      height: PANE_HEIGHT,
      zIndex: 100,
    })
    const view = new ThreeRenderable(cli as never, {
      id: "network-overlay-view",
      width: PANE_WIDTH,
      height: PANE_HEIGHT,
      scene: parts.scene,
      camera: parts.camera,
      renderer: { alpha: true, backgroundColor: RGBA.fromValues(0, 0, 0, 0) },
    })
    box.add(view)
    cli.root.add(box)

    const MIN_ROWS = 32
    const reposition = () => {
      const next = position()
      ;(box as { left?: number; top?: number }).left = next.left
      ;(box as { top?: number }).top = next.top
      // Hide when the terminal is too short for the sidebar to reserve the
      // cells (NetworkPane collapses below the same threshold).
      ;(box as { visible?: boolean }).visible = cli.height >= MIN_ROWS && cli.width >= 60
    }
    reposition()
    cli.on("resize", reposition)

    const tick = async (deltaMs: number) => {
      parts.tick(deltaMs)
    }
    cli.setFrameCallback(tick)
    cli.start()

    return {
      dispose: () => {
        cli.removeFrameCallback(tick)
        cli.off?.("resize", reposition)
        try {
          ;(view as { destroy: () => void }).destroy()
          ;(box as { destroy: () => void }).destroy()
        } catch {
          // teardown is best-effort during shutdown
        }
        parts.dispose()
      },
    }
  } catch {
    parts.dispose()
    return null
  }
}
