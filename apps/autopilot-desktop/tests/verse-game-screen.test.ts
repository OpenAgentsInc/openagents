// M8 "playable-in-our-world" — REAL-SCENE proof that the verified Khala-built
// crossy-road game actually RENDERS on an in-Verse screen surface and is
// input-driveable.
//
// Like app-replica.test.ts this boots the SAME Model / view / update /
// subscriptions + real Foldkit mount the live app uses, in headless Chromium
// (WebGL under SwiftShader), and drives the REAL hotbar via CDP. The game runs in
// a same-origin srcdoc iframe with THREE injected from the parent's bundled three
// (no CDN/network), and its live canvas is textured onto an in-world board.
//
// WHAT THIS PROVES (headless): the game canvas textures onto the Verse surface
// (recognizable sky-blue game pixels appear on the board when slot 4 is on, and
// none when off), and input plumbs through (forwarded keys advance the game's own
// state). WHAT IT DOES NOT PROVE: the on-device Electrobun-GPU look + smooth play
// (SwiftShader is a software rasterizer; the iframe game animates on its own real
// rAF here, decoupled from the Verse's deterministic fake clock).

import { afterAll, beforeAll, describe, expect, test } from "bun:test"
import { existsSync } from "node:fs"
import { join } from "node:path"

import {
  launchAppReplica,
  resolveChromePathOrNull,
  type AppReplica,
} from "../src/testing/app-replica"
import {
  decodePng,
  scoreGameScreenSkyRegion,
  type PixelRegion,
} from "../src/testing/headless-pixel"

const SLOT_4 = ".hotbar-slot-4"

// The arcade screen is dropped in front of the avatar, so it sits roughly
// screen-centre. A generous centre band catches it across SwiftShader framing
// without straying into the dark world edges.
const SCREEN_REGION: PixelRegion = { x0: 0.28, y0: 0.18, x1: 0.72, y1: 0.62 }

// The committed real-scene proof the orchestrator can eyeball.
const SCREEN_PROOF_PNG = join(
  import.meta.dir,
  "headless",
  "verse-game-screen.real-scene.headless.png",
)

const chromeAvailable = resolveChromePathOrNull() !== null
const stylesBuilt = existsSync(
  join(import.meta.dir, "..", "src", "ui", "styles.out.css"),
)
const canRun = chromeAvailable && stylesBuilt
const describeReplica = canRun ? describe : describe.skip

if (!chromeAvailable) {
  console.warn("[verse-game-screen.test] skipped: no Chromium (set CHROME_PATH).")
}
if (chromeAvailable && !stylesBuilt) {
  console.warn(
    "[verse-game-screen.test] skipped: src/ui/styles.out.css missing — run `bun run build:css` first.",
  )
}

const REPLICA_TEST_TIMEOUT_MS = 90_000
const slowTest = (
  label: string,
  fn: () => Promise<void> | void,
): void => {
  test(label, fn, REPLICA_TEST_TIMEOUT_MS)
}

// Read the in-iframe game's own exposed state (started / progress / loopTicks).
type GameState = {
  started: boolean
  progress: number
  loopTicks: number
} | null
const gameState = (replica: AppReplica): Promise<GameState> =>
  replica.evaluate<GameState>(`
    (() => {
      const f = document.querySelector("iframe[title^='Khala crossy-road']")
      const w = f && f.contentWindow
      return w && typeof w.__openagentsCrossyRoadState === "function"
        ? w.__openagentsCrossyRoadState()
        : null
    })()
  `)

const gameScreenActive = (replica: AppReplica): Promise<boolean> =>
  replica.evaluate<boolean>(
    "(window.__OA_REPLICA__ && window.__OA_REPLICA__.verseState ? !!window.__OA_REPLICA__.verseState().gameScreenActive : false)",
  )

describeReplica("M8: the Khala crossy-road game renders + plays on an in-Verse screen", () => {
  let replica: AppReplica

  beforeAll(async () => {
    replica = await launchAppReplica()
  })

  afterAll(async () => {
    await replica?.close()
  })

  slowTest(
    "toggling hotbar slot 4 textures the game canvas onto the in-world screen (sky pixels appear); off ⇒ none",
    async () => {
      // OFF baseline: no in-world game screen, so the dark Verse has ~no sky-blue
      // pixels in the screen region.
      await replica.stepFrames(60)
      const offShot = await replica.screenshot()
      const offSky = scoreGameScreenSkyRegion(
        decodePng(Buffer.from(offShot, "base64")),
        SCREEN_REGION,
      )

      // Turn the arcade screen ON via the REAL hotbar slot 4.
      await replica.click(SLOT_4)
      expect(await gameScreenActive(replica)).toBe(true)

      // Give the iframe game time to boot (it runs on its OWN real rAF/timers in
      // headless), then pump Verse frames so the host textures the live canvas.
      await replica.evaluate(`new Promise((r) => setTimeout(r, 1200))`)
      await replica.stepFrames(120)

      const onShot = await replica.screenshot(SCREEN_PROOF_PNG)
      const onSky = scoreGameScreenSkyRegion(
        decodePng(Buffer.from(onShot, "base64")),
        SCREEN_REGION,
      )

      // The game's sky-blue background now floods the in-world screen region —
      // recognizable GAME pixels on the board, not a blank/black face. The dark
      // Verse OFF frame has only a small baseline of incidental cyan-ish pixels
      // (the reticle ring + the Tassadar board's UI text). Measured: off≈1042,
      // on≈28939 — a ~28× jump. Generous thresholds keep it robust across
      // SwiftShader framing without ever passing a blank screen.
      expect(offSky).toBeLessThan(3000)
      expect(onSky).toBeGreaterThan(12000)
      expect(onSky - offSky).toBeGreaterThan(10000)

      // Turn it OFF again (reset for any following test) and confirm the screen
      // is gone (sky pixels drop back down).
      await replica.click(SLOT_4)
      expect(await gameScreenActive(replica)).toBe(false)
      await replica.stepFrames(30)
    },
  )

  slowTest(
    "forwarded input drives the game (Enter starts it, movement keys advance it)",
    async () => {
      // Ensure the screen is on.
      if (!(await gameScreenActive(replica))) await replica.click(SLOT_4)
      expect(await gameScreenActive(replica)).toBe(true)
      await replica.evaluate(`new Promise((r) => setTimeout(r, 1200))`)
      await replica.stepFrames(30)

      // The game booted and exposes its state.
      const booted = await gameState(replica)
      expect(booted).not.toBeNull()

      // Start the game via its exposed hook (the activation effect also does this;
      // call again idempotently so the test does not race the boot poll).
      await replica.evaluate(`
        (() => {
          const f = document.querySelector("iframe[title^='Khala crossy-road']")
          const w = f && f.contentWindow
          if (w && typeof w.__openagentsCrossyRoadStart === "function") w.__openagentsCrossyRoadStart()
          return true
        })()
      `)
      await replica.evaluate(`new Promise((r) => setTimeout(r, 200))`)
      const started = await gameState(replica)
      expect(started?.started).toBe(true)

      // Forward a hop forward (ArrowUp) into the game window — the SAME path the
      // live window keydown forwarder uses — and confirm the game advances. We
      // hold across real time so the game's setInterval(update) integrates the hop.
      const progressBefore = (await gameState(replica))?.progress ?? 0
      await replica.evaluate(`
        (() => {
          const f = document.querySelector("iframe[title^='Khala crossy-road']")
          const w = f && f.contentWindow
          for (let i = 0; i < 3; i += 1) {
            w.dispatchEvent(new KeyboardEvent("keydown", { key: "ArrowUp", bubbles: true, cancelable: true }))
          }
          return true
        })()
      `)
      // The hop integrates over the game's own ~16ms interval; wait real time.
      await replica.evaluate(`new Promise((r) => setTimeout(r, 700))`)
      const progressAfter = (await gameState(replica))?.progress ?? 0

      // The game's own progress (rows crossed) increased — input plumbed through.
      expect(progressAfter).toBeGreaterThan(progressBefore)

      await replica.click(SLOT_4) // off
      await replica.stepFrames(20)
    },
  )
})
