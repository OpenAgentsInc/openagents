import {
  createHudDotGrid,
  createHudLabel,
  createHudMeter,
  createHudStatusLight,
  hudStatusColor,
  type HudStatus,
} from "@openagentsinc/three-effect/core"
import * as Three from "three"

import type {
  HudMeterState,
  HudStatusLightState,
  HudStatusProjection,
} from "../shared/hud-status-projection.js"
import { HUD_SKIN_COLORS } from "../shared/hud-skin.js"

// HUD H7 (#5504): the small live status/meters HUD overlay scene. It composes
// the H2 three-effect HUD kit primitives — `createHudStatusLight` (the node
// LED), `createHudMeter` (sessions + balance gauges), `createHudDotGrid` (faint
// backdrop) and `createHudLabel` (crisp 3D text) — onto one orthographic,
// white-on-black canvas. It does NOT reimplement any primitive; it imports them
// (workspace contract: extend three-effect, don't fork it). The desktop view
// drives it with a `HudStatusProjection` derived from real model state via
// `hud-status-projection.ts`; this module only renders + recolors what it is
// given (no fabricated values — an "unknown" meter renders empty).
//
// Layout is a compact stacked card sized in world units for an orthographic
// camera, intended to sit in a screen corner over the full UI without occluding
// the shell text bar or the hotbar.

export type HudStatusSceneHandle = Readonly<{
  canvas: HTMLCanvasElement
  setProjection: (projection: HudStatusProjection) => void
  resize: () => void
  dispose: () => void
}>

// The kit's `HudStatus` is a superset of our projection tones; our tones are a
// strict subset, so the mapping is identity-safe.
const toneToHudStatus = (tone: HudStatusLightState["tone"]): HudStatus => tone

// World frustum: a narrow portrait card. Width follows aspect on resize.
const WORLD_HEIGHT = 3.2
const CARD_WIDTH = 2.6
const LABEL_X = -CARD_WIDTH / 2 + 0.34

// One meter row: label above a gauge, with the raw value text to the right.
type MeterRowHandle = Readonly<{
  meter: ReturnType<typeof createHudMeter>
  valueLabel: ReturnType<typeof createHudLabel>
  lastValueText: { current: string }
  applyState: (state: HudMeterState) => void
}>

const buildMeterRow = (
  scene: Three.Scene,
  disposables: Array<{ dispose: () => void }>,
  y: number,
  initial: HudMeterState,
): MeterRowHandle => {
  // Static caption (label text never changes for a given row).
  const caption = createHudLabel({
    text: initial.label.toUpperCase(),
    status: "neutral",
    worldHeight: 0.1,
    anchorX: "left",
    position: { x: LABEL_X, y: y + 0.2 },
  })
  scene.add(caption.object3D)
  disposables.push(caption)

  const meter = createHudMeter({ width: CARD_WIDTH - 0.68, height: 0.1, value: 0 })
  meter.group.position.set(LABEL_X + (CARD_WIDTH - 0.68) / 2, y, 0)
  scene.add(meter.group)
  disposables.push(meter)

  const valueLabel = createHudLabel({
    text: initial.valueText,
    status: "line",
    worldHeight: 0.11,
    anchorX: "left",
    position: { x: LABEL_X, y: y - 0.22 },
  })
  scene.add(valueLabel.object3D)
  disposables.push(valueLabel)

  const lastValueText = { current: initial.valueText }

  const applyState = (state: HudMeterState): void => {
    meter.setValue(state.value)
    if (state.valueText !== lastValueText.current) {
      valueLabel.setText(state.valueText)
      lastValueText.current = state.valueText
    }
    // An unknown reading is dimmed to read as "no data", a known one is bright.
    valueLabel.object3D.material.opacity = state.known ? 1 : 0.5
    valueLabel.object3D.material.transparent = true
    valueLabel.object3D.material.color.set(
      hudStatusColor(state.known ? "line" : "neutral"),
    )
  }

  return { meter, valueLabel, lastValueText, applyState }
}

export const mountHudStatusScene = (
  mount: HTMLElement,
  initial: HudStatusProjection,
): HudStatusSceneHandle => {
  const canvas = document.createElement("canvas")
  canvas.style.display = "block"
  canvas.style.width = "100%"
  canvas.style.height = "100%"
  mount.replaceChildren(canvas)

  const renderer = new Three.WebGLRenderer({
    canvas,
    antialias: true,
    alpha: true,
  })
  // Transparent clear so the overlay floats over the app shell rather than
  // painting its own black block.
  renderer.setClearColor(HUD_SKIN_COLORS.background, 0)
  renderer.setPixelRatio(Math.min(window.devicePixelRatio || 1, 2))

  const scene = new Three.Scene()
  const camera = new Three.OrthographicCamera(-1, 1, 1, -1, 0.1, 100)
  camera.position.set(0, 0, 10)
  camera.lookAt(0, 0, 0)

  const disposables: Array<{ dispose: () => void }> = []

  // Faint dot-grid backdrop confined to the card.
  const dots = createHudDotGrid({
    width: CARD_WIDTH,
    height: WORLD_HEIGHT - 0.2,
    spacing: 0.22,
    opacity: 0.16,
    color: HUD_SKIN_COLORS.primary,
    z: -0.2,
  })
  scene.add(dots.points)
  disposables.push(dots)

  // Title.
  const title = createHudLabel({
    text: "AUTOPILOT // STATUS",
    status: "primary",
    worldHeight: 0.12,
    anchorX: "left",
    position: { x: LABEL_X, y: WORLD_HEIGHT / 2 - 0.42 },
  })
  scene.add(title.object3D)
  disposables.push(title)

  // Node status LED + its honest one-line label.
  const nodeLight = createHudStatusLight({
    status: toneToHudStatus(initial.nodeLight.tone),
    radius: 0.075,
    pulseHz: initial.nodeLight.pulse ? 1.4 : 0,
    position: { x: LABEL_X, y: WORLD_HEIGHT / 2 - 0.86 },
  })
  scene.add(nodeLight.group)
  disposables.push(nodeLight)

  // The node label is recreated on text change (the text primitive rasterizes
  // text at build time, so we swap it via setText and recolor the material).
  const nodeLabel = createHudLabel({
    text: initial.nodeLight.label.toUpperCase(),
    status: "line",
    worldHeight: 0.11,
    anchorX: "left",
    position: { x: LABEL_X + 0.22, y: WORLD_HEIGHT / 2 - 0.86 },
  })
  scene.add(nodeLabel.object3D)
  disposables.push(nodeLabel)

  let lastNodeLabelText = initial.nodeLight.label
  let nodePulsing = initial.nodeLight.pulse

  // Two meter rows below the node line.
  const sessionsRow = buildMeterRow(
    scene,
    disposables,
    WORLD_HEIGHT / 2 - 1.42,
    initial.sessionsMeter,
  )
  const balanceRow = buildMeterRow(
    scene,
    disposables,
    WORLD_HEIGHT / 2 - 2.12,
    initial.balanceMeter,
  )

  const applyNodeLight = (state: HudStatusLightState): void => {
    nodeLight.setStatus(toneToHudStatus(state.tone))
    nodePulsing = state.pulse
    if (state.label !== lastNodeLabelText) {
      nodeLabel.setText(state.label.toUpperCase())
      lastNodeLabelText = state.label
    }
    nodeLabel.object3D.material.color.set(hudStatusColor(state.tone))
  }

  const setProjection = (projection: HudStatusProjection): void => {
    applyNodeLight(projection.nodeLight)
    sessionsRow.applyState(projection.sessionsMeter)
    balanceRow.applyState(projection.balanceMeter)
  }

  // Apply the initial projection (recolors meters from their values, sets the
  // value-label dimming, etc.).
  setProjection(initial)

  const hostSize = (): { width: number; height: number } => {
    const rect = mount.getBoundingClientRect()
    const width = Math.max(1, Math.floor(rect.width || mount.clientWidth || 220))
    const height = Math.max(
      1,
      Math.floor(rect.height || mount.clientHeight || 280),
    )
    return { width, height }
  }

  const resize = (): void => {
    const { width, height } = hostSize()
    renderer.setSize(width, height, false)
    const aspect = width / height
    const halfH = WORLD_HEIGHT / 2
    const halfW = halfH * aspect
    camera.left = -halfW
    camera.right = halfW
    camera.top = halfH
    camera.bottom = -halfH
    camera.updateProjectionMatrix()
  }

  let disposed = false
  let frameId = 0
  const start =
    typeof performance === "undefined" ? Date.now() : performance.now()

  const render = (now: number): void => {
    if (disposed) return
    const elapsed = (now - start) / 1000
    nodeLight.update(nodePulsing ? elapsed : 0)
    renderer.render(scene, camera)
    frameId = requestAnimationFrame(render)
  }

  const observer =
    typeof ResizeObserver === "undefined"
      ? null
      : new ResizeObserver(() => resize())

  resize()
  observer?.observe(mount)
  frameId = requestAnimationFrame(render)

  const dispose = (): void => {
    if (disposed) return
    disposed = true
    cancelAnimationFrame(frameId)
    observer?.disconnect()
    for (const d of disposables) d.dispose()
    renderer.dispose()
    canvas.remove()
  }

  return { canvas, setProjection, resize, dispose }
}
