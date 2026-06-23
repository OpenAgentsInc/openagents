// Playable-in-our-world: the verified Khala-built crossy-road game, running and
// PLAYABLE on a flat screen INSIDE the three-effect Verse (M8 "a three.js game
// inside our three.js world").
//
// APPROACH (first cut — honest about its limits):
//   • The game runs in a SAME-ORIGIN, OFFSCREEN srcdoc <iframe>, with `THREE`
//     injected from the parent's bundled `three` BEFORE the game script runs (so
//     there is NO CDN/network dependency — it works headless + offline). The
//     iframe keeps the game's global `window`/`document`/`requestAnimationFrame`/
//     keydown/resize listeners FULLY ISOLATED from the Verse — which matters
//     because the committed artifact installs `window.addEventListener('keydown')`
//     and would otherwise fight the Verse's own movement keys. A srcdoc iframe is
//     same-origin with the parent, so its <canvas> is NOT tainted and CAN be
//     textured (the cross-document caveat only bites cross-ORIGIN frames).
//   • The iframe's live game <canvas> is registered (by id) with three-effect's
//     game-screen canvas registry, and an in-world `game_screen` world item
//     references that id. The Verse host textures the canvas onto a board near
//     the avatar as a live CanvasTexture and dirties it each frame.
//   • Input is FORWARDED: while the screen is active we re-dispatch the relevant
//     game keys (arrows / WASD / Enter) into the iframe window, and Enter
//     starts/restarts via the game's exposed hooks, so the owner can actually
//     PLAY it in-world.
//
// This is the "game on a screen IN the world" approach, explicitly NOT a deep
// scene-graph merge of the two three.js scenes (a later lane).

import * as Three from "three"
import {
  registerGameScreenCanvas,
  unregisterGameScreenCanvas,
} from "@openagentsinc/three-effect/core"
import type {
  TrainingRunVisualizationOptions,
  TrainingRunWorldItemDefinition,
} from "@openagentsinc/three-effect/core"

import {
  appendVerseVisualization,
  verseSceneWorldToRootLocal,
} from "./verse-scene-helpers.js"
import {
  KHALA_CROSSY_ROAD_ARTIFACT_SHA256,
  KHALA_CROSSY_ROAD_GAME_HTML,
} from "./khala-crossy-road-game.generated.js"

// The stable screen id the world item references and the registry stores the
// game canvas under.
export const VERSE_GAME_SCREEN_ID = "khala:crossy-road"
export const VERSE_GAME_SCREEN_NODE_ID = "verse:game-screen:khala-crossy-road"
export const VERSE_GAME_SCREEN_SOURCE_REF =
  "scripts/khala-demo/artifacts/khala-crossy-road-northstar-passing.v1.html"

// The game's exposed control hooks (set on the iframe window by the artifact).
type CrossyRoadWindow = Window & {
  __openagentsCrossyRoadStart?: () => void
  __openagentsCrossyRoadRestart?: () => void
  __openagentsCrossyRoadState?: () => {
    started: boolean
    progress: number
    loopTicks: number
  }
}

// The keys the game responds to (it reads e.key). We forward ONLY these, so other
// Verse keys are untouched, and we only forward while the screen is active.
const FORWARDED_GAME_KEYS = new Set<string>([
  "ArrowUp",
  "ArrowDown",
  "ArrowLeft",
  "ArrowRight",
  "w",
  "a",
  "s",
  "d",
  "W",
  "A",
  "S",
  "D",
])

export type VerseGameScreenHandle = Readonly<{
  /** The live game canvas (once the iframe game has booted), or null. */
  canvas: () => HTMLCanvasElement | null
  /** Start (or restart) the game via the artifact's exposed hooks. */
  start: () => void
  restart: () => void
  /** Forward a single key to the game (no-op if not a game key). */
  forwardKey: (key: string) => void
  /** A read-only snapshot of the game's own state (started/progress/ticks). */
  state: () => { started: boolean; progress: number; loopTicks: number } | null
  /** Whether the iframe game canvas has been registered with the Verse. */
  ready: () => boolean
  /** Tear down the iframe + unregister the canvas. */
  dispose: () => void
}>

let singleton: VerseGameScreenHandle | null = null

// Inject THREE into the iframe window, then boot the game. Stable across the
// basic geometry/material/camera/renderer APIs the artifact uses (r128 → 0.184).
const bootGameInIframe = (
  iframe: HTMLIFrameElement,
  onCanvas: (canvas: HTMLCanvasElement) => void,
): void => {
  const win = iframe.contentWindow as CrossyRoadWindow | null
  const doc = iframe.contentDocument
  if (win === null || doc === null) return
  // Inject the parent's THREE BEFORE the game's inline script runs. We write the
  // document so the game script (which references global THREE) finds it.
  ;(win as unknown as { THREE: typeof Three }).THREE = Three
  doc.open()
  doc.write(KHALA_CROSSY_ROAD_GAME_HTML)
  doc.close()
  // Re-inject after document.write reset the iframe's globals, then poll for the
  // game canvas the artifact appends to #game-container.
  ;(win as unknown as { THREE: typeof Three }).THREE = Three
  let tries = 0
  const findCanvas = (): void => {
    const innerDoc = iframe.contentDocument
    const canvas = innerDoc?.querySelector("canvas") ?? null
    if (canvas instanceof HTMLCanvasElement) {
      onCanvas(canvas)
      return
    }
    tries += 1
    if (tries < 400) setTimeout(findCanvas, 8)
  }
  findCanvas()
}

// Create (once) the offscreen iframe game host and register its canvas with the
// Verse. Idempotent: repeated calls return the same handle.
export const ensureVerseGameScreen = (): VerseGameScreenHandle => {
  if (singleton !== null) return singleton
  if (typeof document === "undefined") {
    // Headless without a DOM (pure unit context): an inert handle.
    const inert: VerseGameScreenHandle = {
      canvas: () => null,
      start: () => {},
      restart: () => {},
      forwardKey: () => {},
      state: () => null,
      ready: () => false,
      dispose: () => {},
    }
    singleton = inert
    return inert
  }

  const iframe = document.createElement("iframe")
  iframe.title = "Khala crossy-road (in-Verse game source)"
  // Offscreen but RENDERED (display:none would not paint the WebGL canvas), so
  // the game canvas actually draws frames we can texture. Pull it far off-screen.
  iframe.setAttribute(
    "style",
    [
      "position:fixed",
      "left:-10000px",
      "top:0",
      "width:480px",
      "height:320px",
      "border:0",
      "pointer-events:none",
      "opacity:0",
    ].join(";"),
  )
  iframe.setAttribute("aria-hidden", "true")
  // srcdoc placeholder so contentWindow exists; we document.write the real game.
  iframe.srcdoc = "<!doctype html><title>khala</title>"
  document.body.appendChild(iframe)

  let canvas: HTMLCanvasElement | null = null
  let registered = false
  let blitRaf: number | null = null

  // We register a 2D MIRROR canvas (not the raw iframe WebGL canvas) with the
  // Verse, and blit the game canvas into it each frame. WHY: texturing a WebGL
  // canvas from a SEPARATE renderer/document is fragile (the drawing buffer is
  // cleared post-composite, and cross-document WebGL-canvas reads are unreliable
  // under software rasterizers). A plain 2D canvas textures reliably, and
  // `drawImage(webglCanvas, …)` copies the last drawn game frame into it. So the
  // Verse always samples a fresh, stable 2D mirror of the game.
  const mirror =
    typeof document === "undefined" ? null : document.createElement("canvas")
  const mirrorCtx = mirror?.getContext("2d") ?? null

  const startBlit = (gameCanvas: HTMLCanvasElement): void => {
    if (mirror === null || mirrorCtx === null) return
    if (mirror.width !== gameCanvas.width || mirror.height !== gameCanvas.height) {
      mirror.width = gameCanvas.width || 480
      mirror.height = gameCanvas.height || 320
    }
    const tick = (): void => {
      if (mirror.width !== gameCanvas.width && gameCanvas.width > 0) {
        mirror.width = gameCanvas.width
        mirror.height = gameCanvas.height
      }
      try {
        mirrorCtx.drawImage(gameCanvas, 0, 0, mirror.width, mirror.height)
      } catch {
        // A transient cross-document read hiccup — keep looping.
      }
      blitRaf =
        typeof requestAnimationFrame === "function"
          ? requestAnimationFrame(tick)
          : null
    }
    tick()
  }

  const onCanvas = (found: HTMLCanvasElement): void => {
    canvas = found
    if (mirror !== null) {
      registerGameScreenCanvas(VERSE_GAME_SCREEN_ID, mirror)
      startBlit(found)
    } else {
      registerGameScreenCanvas(VERSE_GAME_SCREEN_ID, found)
    }
    registered = true
  }

  const onLoad = (): void => {
    bootGameInIframe(iframe, onCanvas)
  }
  // The placeholder srcdoc fires load; boot the game then.
  if (iframe.contentDocument?.readyState === "complete") onLoad()
  else iframe.addEventListener("load", onLoad, { once: true })

  const gameWindow = (): CrossyRoadWindow | null =>
    (iframe.contentWindow as CrossyRoadWindow | null) ?? null

  const handle: VerseGameScreenHandle = {
    canvas: () => canvas,
    start: () => {
      gameWindow()?.__openagentsCrossyRoadStart?.()
    },
    restart: () => {
      gameWindow()?.__openagentsCrossyRoadRestart?.()
    },
    forwardKey: (key: string) => {
      if (!FORWARDED_GAME_KEYS.has(key)) return
      const win = gameWindow()
      if (win === null || typeof KeyboardEvent === "undefined") return
      // Dispatch a real keydown into the iframe window — the game's own
      // window.addEventListener('keydown', onKeyDown) handles it. The parent's
      // KeyboardEvent constructor dispatches fine into a same-origin frame.
      win.dispatchEvent(
        new KeyboardEvent("keydown", { key, bubbles: true, cancelable: true }),
      )
    },
    state: () => gameWindow()?.__openagentsCrossyRoadState?.() ?? null,
    ready: () => registered,
    dispose: () => {
      if (blitRaf !== null && typeof cancelAnimationFrame === "function") {
        cancelAnimationFrame(blitRaf)
        blitRaf = null
      }
      unregisterGameScreenCanvas(VERSE_GAME_SCREEN_ID)
      registered = false
      canvas = null
      iframe.remove()
      if (singleton === handle) singleton = null
    },
  }
  singleton = handle
  return handle
}

export const verseGameScreenHandleOrNull = (): VerseGameScreenHandle | null =>
  singleton

// ── Lifecycle: activate / deactivate the in-world game screen ──────────────────
//
// Activating: ensure the offscreen iframe game host exists, start the game, and
// install a single window keydown forwarder that re-dispatches the game keys
// (arrows / WASD) into the iframe so the owner can PLAY it in-world while moving
// through the Verse. The forwarder does NOT preventDefault — the Verse's own
// third-person controller still reads WASD for avatar movement, so the same keys
// drive both (acceptable for a first cut; a focus/active-screen gate is a later
// refinement). Returns a teardown that removes the forwarder + disposes the host.
export const activateVerseGameScreen = (): (() => void) => {
  const handle = ensureVerseGameScreen()
  // Start the game once its canvas is registered (poll briefly post-boot).
  let startTries = 0
  const tryStart = (): void => {
    if (handle.ready()) {
      handle.start()
      return
    }
    startTries += 1
    if (startTries < 400 && typeof setTimeout === "function") {
      setTimeout(tryStart, 16)
    }
  }
  tryStart()

  const forwarder =
    typeof window === "undefined"
      ? null
      : (event: KeyboardEvent): void => {
          if (singleton === null) return
          singleton.forwardKey(event.key)
        }
  if (forwarder !== null && typeof window !== "undefined") {
    window.addEventListener("keydown", forwarder)
  }

  return () => {
    if (forwarder !== null && typeof window !== "undefined") {
      window.removeEventListener("keydown", forwarder)
    }
    verseGameScreenHandleOrNull()?.dispose()
  }
}

// ── The in-world game-screen world item ───────────────────────────────────────

// Where (scene-world) the screen cabinet stands relative to the avatar: a couple
// of units in front, at standing height. Mirrors the spawned-scene station frame.
const SCREEN_FORWARD = 3.4
const SCREEN_HEIGHT = 0.0

export type VerseGameScreenAnchor = Readonly<{
  x: number
  y: number
  z: number
  yaw?: number
}>

const finite = (v: number): boolean => Number.isFinite(v) && !Number.isNaN(v)

// Place the screen cabinet in front of the avatar (scene-world), then convert to
// the renderer's root-local frame. Falls back to a fixed spot in front of the
// default spawn when there is no pose yet.
const screenWorldItem = (
  anchor: VerseGameScreenAnchor | null,
): TrainingRunWorldItemDefinition => {
  const a =
    anchor !== null && finite(anchor.x) && finite(anchor.y) && finite(anchor.z)
      ? anchor
      : { x: 0, y: 0, z: 4.4, yaw: 0 }
  const yaw = finite(a.yaw ?? Number.NaN) ? (a.yaw as number) : 0
  // Scene-world: forward is -Z (avatar faces -Z at yaw 0), rotated by yaw.
  const cos = Math.cos(yaw)
  const sin = Math.sin(yaw)
  const worldX = a.x + SCREEN_FORWARD * sin
  const worldZ = a.z - SCREEN_FORWARD * cos
  const worldY = a.y + SCREEN_HEIGHT
  const position = verseSceneWorldToRootLocal([worldX, worldY, worldZ])
  return {
    id: VERSE_GAME_SCREEN_NODE_ID,
    kind: "game_screen",
    label: "Khala arcade — crossy road",
    detail: `${VERSE_GAME_SCREEN_SOURCE_REF} · sha256:${KHALA_CROSSY_ROAD_ARTIFACT_SHA256.slice(0, 12)}`,
    position,
    // The board uses the SAME orientation convention as the Tassadar bulletin
    // board (whose readable text faces the avatar): a root-local Z-rotation of ~0
    // makes the face (group-local -Y, which the -90°X root maps toward the camera)
    // point back at the avatar standing in front of it. We track the avatar's yaw
    // so the cabinet turns with where the player is looking, but the base
    // orientation that faces the viewer is the bulletin-board's yaw≈0 frame.
    yaw: -yaw,
    status: "active",
    screenCanvasId: VERSE_GAME_SCREEN_ID,
    screenWidth: 3.0,
    screenHeight: 2.0,
    sourceRefs: [VERSE_GAME_SCREEN_SOURCE_REF],
  }
}

// Overlay the in-world game screen onto the base visualization when active. When
// inactive the Verse is byte-identical (no world item added). Pure: it only adds
// a `game_screen` world item referencing the registry canvas id; the iframe game
// host is created/torn down separately (the view effect), not here.
export const withVerseGameScreenLayer = (
  base: TrainingRunVisualizationOptions,
  input: Readonly<{ active: boolean; anchor: VerseGameScreenAnchor | null }>,
): TrainingRunVisualizationOptions => {
  if (!input.active) return base
  return appendVerseVisualization(base, {
    worldItems: [screenWorldItem(input.anchor)],
  })
}
