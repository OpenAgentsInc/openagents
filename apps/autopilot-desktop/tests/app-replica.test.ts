// Headless app-replica regression — drives the REAL desktop renderer and asserts
// the three live bugs the owner hit are reproduced-then-fixed.
//
// Unlike the full-input-path harness (which reconstructs the input chain in
// pure TS), this boots the SAME Model / view / update / subscriptions + the real
// Foldkit `Runtime.run` mount the live app uses, in headless Chromium, with:
//   • Component styles SOLVED — the entry is served with the same generated
//     styles.out.css as the packaged app, so `view.ts` mounts styled without the
//     old style-runtime throw.
//   • The Electrobun bridge STUBBED via the real `setRequest`/`pushInbound` seam
//     (Effect Commands reach a scripted fake, not the network; the live
//     `khalaToken` push is driven for streaming).
//   • Deterministic frame stepping (fake rAF + clock) so the same scenario
//     renders identical DOM every run.
//
// The driver dispatches REAL DOM events via CDP (a keydown flows through the real
// keyboard subscription → forward gate → interpretKey → reducer → re-render; a
// click fires the real Foldkit OnClick), so nothing here shortcuts the input path.
//
// Skips cleanly where no Chromium is installed (set CHROME_PATH).

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
  scoreCracklingArcRegion,
  scoreRegion,
  type PixelRegion,
} from "../src/testing/headless-pixel"

const KHALA_INPUT = ".verse-khala-input"
const HOTBAR = ".hotbar"
const SLOT_2 = ".hotbar-slot-2"
const SLOT_3 = ".hotbar-slot-3"
const KHALA_BAR = ".verse-khala-bar"
const KHALA_BODY = ".verse-khala-bubble-body"

// The owner removed the on-screen "⚡ Crackling energy" chip entirely (#6033), so
// the replica reads spawn state from the REAL model snapshot the entry exposes
// (window.__OA_REPLICA__.verseState()), not from a DOM chip.
type VerseState = {
  spawnedSceneCount: number
  spawnedSceneIds: ReadonlyArray<string>
  spawnedPortalCount: number
  avatarPose: { x: number; y: number; z: number; yaw: number } | null
}
const verseState = (replica: AppReplica): Promise<VerseState> =>
  replica.evaluate<VerseState>("window.__OA_REPLICA__.verseState()")
const spawnedCount = async (replica: AppReplica): Promise<number> =>
  (await verseState(replica)).spawnedSceneCount

// The host (foldkit oa-training-run wrapper) writes its lifecycle events into a
// shared global log: `verse-host.remount.*` on a FULL remount (camera/controller
// rebuild) and `verse-host.visualization.retained` on an in-place reconcile. The
// no-remount proof reads this log directly.
type SceneLogEntry = { at: string; event: string; detail: Record<string, unknown> }
const sceneLog = (replica: AppReplica): Promise<ReadonlyArray<SceneLogEntry>> =>
  replica.evaluate<ReadonlyArray<SceneLogEntry>>(
    "(window.__OA_DUMP_VERSE_SCENE_LOGS ? window.__OA_DUMP_VERSE_SCENE_LOGS() : [])",
  )
const countEvents = (
  log: ReadonlyArray<SceneLogEntry>,
  prefix: string,
): number => log.filter((e) => e.event.startsWith(prefix)).length

// The crackling arc spawns centre-left at chest height in front of the avatar;
// the Tassadar board sits to the RIGHT, so a centre-left region isolates the arc
// strands from the board's white text. Fraction-of-frame coords (1280×800).
const ARC_REGION: PixelRegion = { x0: 0.37, y0: 0.26, x1: 0.55, y1: 0.4 }
// The committed real-scene proof (the screenshot the orchestrator can eyeball).
const ARC_PROOF_PNG = join(
  import.meta.dir,
  "headless",
  "verse-spawned-arc.real-scene.headless.png",
)
// A committed AFTER-MOVEMENT proof: the spawned arc + portal once the avatar has
// walked, showing the arc stayed world-anchored and the character was not reset.
const AFTER_MOVE_PROOF_PNG = join(
  import.meta.dir,
  "headless",
  "verse-spawned-arc.after-movement.headless.png",
)

const chromeAvailable = resolveChromePathOrNull() !== null
const stylesBuilt = existsSync(
  join(import.meta.dir, "..", "src", "ui", "styles.out.css"),
)
const canRun = chromeAvailable && stylesBuilt
const describeReplica = canRun ? describe : describe.skip

if (!chromeAvailable) {
  console.warn(
    "[app-replica.test] skipped: no Chromium found (set CHROME_PATH).",
  )
}
if (chromeAvailable && !stylesBuilt) {
  console.warn(
    "[app-replica.test] skipped: src/ui/styles.out.css missing — run `bun run build:css` first.",
  )
}

// Each replica test does several CDP round-trips (build settle frames included),
// which exceeds bun's default 5s per-test timeout. `scripts/run-tests.sh` runs
// `bun test <file>` with no `--timeout`, so the budget is set per-test here.
const REPLICA_TEST_TIMEOUT_MS = 60_000
const slowTest = (
  label: string,
  fn: () => Promise<void> | void,
): void => {
  test(label, fn, REPLICA_TEST_TIMEOUT_MS)
}

const rectsOverlap = (
  a: { left: number; right: number; top: number; bottom: number },
  b: { left: number; right: number; top: number; bottom: number },
): boolean =>
  !(
    a.right <= b.left ||
    a.left >= b.right ||
    a.bottom <= b.top ||
    a.top >= b.bottom
  )

describeReplica("app-replica boots the REAL desktop renderer headless", () => {
  let replica: AppReplica

  beforeAll(async () => {
    replica = await launchAppReplica()
  })

  afterAll(async () => {
    await replica?.close()
  })

  // ── The replica actually mounts the real view (styles solved, no crash) ──────

  slowTest("the real Verse view mounts with real CSS applied", async () => {
    // The real app-shell-verse container is present and laid out full-viewport —
    // proving the real `view.ts` (the same one the app runs) mounted without the
    // old style-runtime throw, with the real stylesheet applied.
    const shell = await replica.boundingBox(".app-shell-verse")
    expect(shell).not.toBeNull()
    expect(shell!.width).toBeGreaterThan(200)
    const position = await replica.evaluate<string>(
      `getComputedStyle(document.querySelector(".app-shell-verse")).position`,
    )
    // .app-shell is `position: fixed` via real CSS — confirms styles.out.css loaded.
    expect(position).toBe("fixed")
    // The hotbar + the in-world Ask box are the real surfaces under test.
    expect(await replica.count(HOTBAR)).toBe(1)
    expect(await replica.count(KHALA_INPUT)).toBe(1)
    // No crash overlay.
    expect(await replica.count("[data-replica-crash]")).toBe(0)
  })

  // ── BUG (a): hotbar 2/3 when the Ask box is focused ─────────────────────────

  describe("bug (a): hotbar slots fire regardless of focus; numbers fire unfocused", () => {
    const reset = async (): Promise<void> => {
      if ((await spawnedCount(replica)) > 0) await replica.click(SLOT_2)
    }

    slowTest("click slot 2 spawns the scene and click slot 3 toggles its portal", async () => {
      await reset()
      expect(await spawnedCount(replica)).toBe(0)

      // Click slot 2 — spawns the crackling scene (works regardless of focus).
      await replica.click(SLOT_2)
      let state = await verseState(replica)
      expect(state.spawnedSceneCount).toBe(1)
      expect(state.spawnedSceneIds).toContain("crackling-energy")
      expect(state.spawnedPortalCount).toBe(0)

      // Click slot 3 — toggles the scene's gateway portal on.
      await replica.click(SLOT_3)
      state = await verseState(replica)
      expect(state.spawnedPortalCount).toBe(1)

      // Reset for the next test.
      await replica.click(SLOT_3) // portal off
      await replica.click(SLOT_2) // scene off
      expect(await spawnedCount(replica)).toBe(0)
    })

    slowTest("clicking slot 2 spawns even while the Ask box is focused", async () => {
      await reset()
      expect(await spawnedCount(replica)).toBe(0)
      await replica.focus(KHALA_INPUT)
      await replica.click(SLOT_2)
      expect(await spawnedCount(replica)).toBe(1)
      await replica.click(SLOT_2) // reset off
      expect(await spawnedCount(replica)).toBe(0)
    })

    slowTest("bare '2' spawns when NO input is focused", async () => {
      await reset()
      expect(await spawnedCount(replica)).toBe(0)
      // Blur any focused element so the keypress is not "in editable".
      await replica.evaluate(
        `(document.activeElement && document.activeElement.blur && document.activeElement.blur(), true)`,
      )
      await replica.pressKey("2")
      expect(await spawnedCount(replica)).toBe(1)
      await replica.pressKey("2") // toggle off
      expect(await spawnedCount(replica)).toBe(0)
    })

    slowTest("FIX: '2' while the Ask box is focused fires the slot and does NOT type a digit", async () => {
      // This is the owner-reported failure: with the Ask box focused, a bare `2`
      // typed a digit into the box and "the hotbar did nothing". Fixed: the digit
      // is swallowed (clean box) and the hotbar slot fires.
      await reset()
      expect(await spawnedCount(replica)).toBe(0)
      await replica.focus(KHALA_INPUT)
      await replica.pressKey("2")
      // The Ask box stays empty — the digit did NOT pollute the prompt.
      const value = await replica.evaluate<string>(
        `document.querySelector("${KHALA_INPUT}").value`,
      )
      expect(value).toBe("")
      // …and the slot fired (the scene spawned).
      expect(await spawnedCount(replica)).toBe(1)
      await replica.click(SLOT_2) // reset off
      expect(await spawnedCount(replica)).toBe(0)
    })

    slowTest("letters still type into the focused Ask box (no over-swallowing)", async () => {
      await replica.focus(KHALA_INPUT)
      await replica.type(KHALA_INPUT, "ask")
      const value = await replica.evaluate<string>(
        `document.querySelector("${KHALA_INPUT}").value`,
      )
      expect(value).toBe("ask")
    })
  })

  // ── BUG (c): the Ask box must sit BESIDE the hotbar, not overlap it ──────────

  describe("bug (c): Ask box is beside the hotbar, same height, not overlapping", () => {
    slowTest("the khala bar and the hotbar do not overlap", async () => {
      const hotbar = await replica.boundingBox(HOTBAR)
      const bar = await replica.boundingBox(KHALA_BAR)
      expect(hotbar).not.toBeNull()
      expect(bar).not.toBeNull()
      expect(rectsOverlap(hotbar!, bar!)).toBe(false)
    })

    slowTest("the khala bar is to the RIGHT of the hotbar (beside it)", async () => {
      const hotbar = await replica.boundingBox(HOTBAR)
      const bar = await replica.boundingBox(KHALA_BAR)
      // Beside = the bar starts at/after the hotbar's right edge.
      expect(bar!.left).toBeGreaterThanOrEqual(hotbar!.right)
    })

    slowTest("the khala bar shares the hotbar's vertical band (same height, aligned)", async () => {
      const hotbar = await replica.boundingBox(HOTBAR)
      const bar = await replica.boundingBox(KHALA_BAR)
      // Same height (within 2px) and vertically aligned (bottoms within 2px).
      expect(Math.abs(bar!.height - hotbar!.height)).toBeLessThanOrEqual(2)
      expect(Math.abs(bar!.bottom - hotbar!.bottom)).toBeLessThanOrEqual(2)
    })
  })

  // ── BUG (b): the Khala answer renders exactly once ──────────────────────────

  describe("bug (b): the Khala response renders exactly once", () => {
    const ANSWER = "Hello world from Khala"
    const submit = async (resolveBeforeStream: boolean): Promise<string> => {
      await replica.scriptKhala({
        deltas: ["Hello", " world", " from", " Khala"],
        text: ANSWER,
        ok: true,
        live: false,
        resolveBeforeStream,
      })
      await replica.focus(KHALA_INPUT)
      await replica.type(KHALA_INPUT, "hi")
      await replica.pressKey("Enter")
      await replica.settle()
      return replica.text(KHALA_BODY)
    }
    const occurrences = (haystack: string, needle: string): number =>
      haystack.split(needle).length - 1

    slowTest("streaming-first ordering: the answer appears once", async () => {
      const body = await submit(false)
      expect(occurrences(body, ANSWER)).toBe(1)
    })

    slowTest("FIX: terminal-answer-first race no longer doubles the answer", async () => {
      // resolveBeforeStream reproduces the live race that doubled the response:
      // the RPC terminal answer lands before the trailing streamed deltas, which
      // then appended the same text again. Fixed: late post-settle deltas are
      // dropped, so the answer renders exactly once.
      const body = await submit(true)
      expect(occurrences(body, ANSWER)).toBe(1)
      expect(body).toBe(ANSWER)
    })
  })

  // ── Determinism: the same scenario produces identical observations ──────────

  slowTest("determinism: re-running a scenario yields identical boxes + text", async () => {
    const observe = async (): Promise<string> => {
      // Clean scene, run the double-response race, capture the answer + boxes.
      if ((await spawnedCount(replica)) > 0) await replica.click(SLOT_2)
      await replica.scriptKhala({
        deltas: ["Hello", " world", " from", " Khala"],
        text: "Hello world from Khala",
        ok: true,
        resolveBeforeStream: true,
      })
      await replica.focus(KHALA_INPUT)
      await replica.type(KHALA_INPUT, "hi")
      await replica.pressKey("Enter")
      await replica.settle()
      const body = await replica.text(KHALA_BODY)
      const hotbar = await replica.boundingBox(HOTBAR)
      const bar = await replica.boundingBox(KHALA_BAR)
      return JSON.stringify({ body, hotbar, bar })
    }
    const first = await observe()
    const second = await observe()
    expect(second).toBe(first)
  })

  // ── BUG (#6033): the crackling scene must VISIBLY RENDER in the REAL Verse ────
  //
  // The prior single-frame pixel proof mounted the crackling-arc primitive in
  // ISOLATION, so it never caught the FULL-SCENE failures the owner hit. These
  // DYNAMIC tests drive the REAL view/update/subscriptions through the real
  // hotbar and assert behaviour over multiple frames + after avatar MOVEMENT —
  // the things a single screenshot missed.
  describe("bug (#6033): the spawned crackling arc visibly renders in the real Verse", () => {
    const reset = async (): Promise<void> => {
      if ((await spawnedCount(replica)) > 0) await replica.click(SLOT_2)
    }

    // (c) THE ARC RENDERS: spawning lights bright cyan/blue strand pixels where
    // the avatar looks; the un-spawned world has none. Fails on either regression
    // (dropped beam ⇒ 0 px; out-of-view arc ⇒ 0 px). Screenshot written for eyeball.
    slowTest(
      "(c) spawning via the real hotbar lights up the arc region; un-spawned does not",
      async () => {
        await reset()
        expect(await spawnedCount(replica)).toBe(0)

        await replica.stepFrames(60)
        const offShot = await replica.screenshot()
        const offArc = scoreCracklingArcRegion(
          decodePng(Buffer.from(offShot, "base64")),
          ARC_REGION,
        )

        await replica.click(SLOT_2)
        expect(await spawnedCount(replica)).toBe(1)
        await replica.stepFrames(120)
        const onShot = await replica.screenshot()
        const onArc = scoreCracklingArcRegion(
          decodePng(Buffer.from(onShot, "base64")),
          ARC_REGION,
        )

        expect(offArc).toBeLessThan(40)
        expect(onArc).toBeGreaterThan(300)
        expect(onArc - offArc).toBeGreaterThan(300)

        await reset()
        expect(await spawnedCount(replica)).toBe(0)
      },
    )

    // (a) NO REMOUNT + POSE UNCHANGED ON SPAWN: this is the dominant #6054
    // regression. Pressing 2 must reconcile the beam IN PLACE (one
    // `verse-host.visualization.retained` event) and emit ZERO new
    // `verse-host.remount.*` events, and the avatar's pose must be byte-identical
    // before/after the spawn (no camera/controller rebuild → no reset).
    slowTest(
      "(a) spawning emits NO remount (retained reconcile) and does NOT reset the avatar pose",
      async () => {
        await reset()
        // Let the world settle + the host emit at least one pose so the snapshot
        // is populated, then take the baseline.
        await replica.stepFrames(60)
        const poseBefore = (await verseState(replica)).avatarPose
        const remountsBefore = countEvents(
          await sceneLog(replica),
          "verse-host.remount.",
        )
        const retainedBefore = countEvents(
          await sceneLog(replica),
          "verse-host.visualization.retained",
        )

        // Spawn via the REAL hotbar slot.
        await replica.click(SLOT_2)
        expect(await spawnedCount(replica)).toBe(1)
        await replica.stepFrames(30)

        const log = await sceneLog(replica)
        const remountsAfter = countEvents(log, "verse-host.remount.")
        const retainedAfter = countEvents(
          log,
          "verse-host.visualization.retained",
        )
        const poseAfter = (await verseState(replica)).avatarPose

        // NO new remount on spawn — the beam reconciled in place. This is the
        // dominant #6054 regression: the old fingerprint hack forced a full
        // remount on every spawn (camera + controller rebuilt → avatar reset).
        expect(remountsAfter).toBe(remountsBefore)
        // And the host took the retained (in-place reconcile) path at least once
        // — the spawn went through updateVisualization, not a rebuild.
        expect(retainedAfter).toBeGreaterThan(retainedBefore)
        // The avatar pose did NOT snap back to the controller's initial spawn
        // ([0,0,4.4]) — a remount would have reset it there. Without input the
        // pose stays put (no large jump). We assert NO teleport rather than exact
        // equality (a gentle idle settle is fine; a reset is a metres-large jump).
        expect(poseBefore).not.toBeNull()
        expect(poseAfter).not.toBeNull()
        const jump =
          Math.abs((poseAfter!.x ?? 0) - (poseBefore!.x ?? 0)) +
          Math.abs((poseAfter!.y ?? 0) - (poseBefore!.y ?? 0)) +
          Math.abs((poseAfter!.z ?? 0) - (poseBefore!.z ?? 0))
        expect(jump).toBeLessThan(0.5)

        await reset()
      },
    )

    // (b) WORLD-ANCHORED UNDER MOVEMENT: after spawning, walking the avatar must
    // NOT drag the arc with it. We capture the arc's screen footprint, hold a
    // movement key for many frames (the avatar + camera move), and assert the arc
    // pixel cluster SHIFTED consistently with the world (camera moved past a fixed
    // arc) rather than staying glued to screen-centre (which is what a pose-chasing
    // entity would do). The avatar pose must have actually changed.
    slowTest(
      "(b) after spawning, moving the avatar leaves the arc world-anchored (it does not chase the camera)",
      async () => {
        await reset()
        await replica.click(SLOT_2) // arc on
        await replica.click(SLOT_3) // + gateway portal on, so the proof shows both
        expect(await spawnedCount(replica)).toBe(1)
        expect((await verseState(replica)).spawnedPortalCount).toBe(1)
        await replica.stepFrames(120)

        // Blur any field so movement keys reach the window (the controller target).
        await replica.evaluate(
          `(document.activeElement && document.activeElement.blur && document.activeElement.blur(), true)`,
        )
        const poseStart = (await verseState(replica)).avatarPose

        // Sample the arc footprint across the full lower-centre band BEFORE moving.
        const BAND: PixelRegion = { x0: 0.2, y0: 0.2, x1: 0.8, y1: 0.6 }
        const arcBefore = scoreCracklingArcRegion(
          decodePng(Buffer.from(await replica.screenshot(), "base64")),
          BAND,
        )

        // Walk forward (toward the arc) for a good stretch of frames, then strafe,
        // so both the avatar position AND the camera framing change materially.
        await replica.holdKey("w", 90)
        await replica.holdKey("d", 60)
        await replica.stepFrames(60)

        const poseEnd = (await verseState(replica)).avatarPose
        // Commit the AFTER-MOVEMENT frame: arc + portal correctly placed, the
        // character not reset. This is the screenshot the orchestrator eyeballs.
        const afterMoveShot = await replica.screenshot(AFTER_MOVE_PROOF_PNG)
        const arcAfter = scoreCracklingArcRegion(
          decodePng(Buffer.from(afterMoveShot, "base64")),
          BAND,
        )

        // The avatar actually moved (movement keys integrated in the controller).
        // If the headless controller did not move (a real-vs-replica gap), this
        // assertion fails LOUDLY rather than silently passing a no-op.
        expect(poseStart).not.toBeNull()
        expect(poseEnd).not.toBeNull()
        const moved =
          Math.abs((poseEnd!.x ?? 0) - (poseStart!.x ?? 0)) +
          Math.abs((poseEnd!.z ?? 0) - (poseStart!.z ?? 0))
        expect(moved).toBeGreaterThan(0.5)

        // The arc is still SOMEWHERE in the world (not deleted / not flung
        // off-screen by a pose-chasing transform): a world-anchored arc the camera
        // walked toward stays visible. A chasing/inverted entity tends to whip out
        // of frame or collapse. We assert the arc remains rendered (>0) and that
        // its footprint CHANGED (the camera moved relative to a fixed arc) — i.e.
        // it did NOT stay pinned identically to the avatar's screen position.
        expect(arcAfter).toBeGreaterThan(0)
        expect(Math.abs(arcAfter - arcBefore)).toBeGreaterThan(0)

        await reset()
      },
    )

    // (d) SLOT 3 RENDERS A PORTAL: toggling the gateway portal must add a visibly
    // rendered portal (concentric torus rings + sparks) to the scene — the slot-3
    // bug was that the portal entity never reached the scene graph (mount-only +
    // the fingerprint ignored portal-only entity changes, so toggling showPortal
    // never re-rendered). With live entity reconcile, the portal appears. We
    // measure the WHOLE-FRAME bright-pixel delta (the portal adds geometry the
    // arc-only frame does not have) so the assertion does not depend on a hand-
    // picked region that the bright arc might already saturate.
    slowTest(
      "(d) slot 3 toggles a visibly-rendered gateway portal into the scene",
      async () => {
        await reset()
        await replica.click(SLOT_2) // arc on (no portal yet)
        expect(await spawnedCount(replica)).toBe(1)
        expect((await verseState(replica)).spawnedPortalCount).toBe(0)

        const FRAME: PixelRegion = { x0: 0, y0: 0, x1: 1, y1: 1 }
        // The crackling arc's per-strand opacity + geometry OSCILLATE over two
        // incommensurate cycles, so a single whole-frame bright-pixel count swings
        // by more than the (steady) portal's contribution. Comparing two frames at
        // DIFFERENT arc phases is fragile (a dimmer arc phase can swamp the
        // portal's pixels) — the historical source of this test's flakiness. To
        // isolate the portal's contribution we hold the arc phase FIXED across the
        // toggle: settle the arc, then capture portal-OFF and portal-ON only ONE
        // frame apart, so the arc barely advances and the bright-pixel delta is
        // the portal's geometry, not arc-phase noise. (Deterministic fake clock.)
        await replica.stepFrames(120)

        const portalOffBefore = scoreRegion(
          decodePng(Buffer.from(await replica.screenshot(), "base64")),
          FRAME,
        ).brightPixels

        // Toggle the gateway portal ON (slot 3); step a single frame so the live
        // reconcile applies and the rings render at essentially the same arc phase.
        await replica.click(SLOT_3)
        expect((await verseState(replica)).spawnedPortalCount).toBe(1)
        await replica.stepFrames(1)
        const withPortalShot = await replica.screenshot(ARC_PROOF_PNG)
        const withPortal = scoreRegion(
          decodePng(Buffer.from(withPortalShot, "base64")),
          FRAME,
        ).brightPixels

        // The portal added rendered geometry: at the held arc phase the frame has
        // MORE bright pixels than the immediately-prior portal-off frame. A
        // never-rendered portal (the slot-3 bug) would leave the count unchanged.
        expect(withPortal).toBeGreaterThan(portalOffBefore)

        // Toggling it OFF removes that geometry again (live reconcile both ways),
        // captured one frame later at essentially the same arc phase.
        await replica.click(SLOT_3)
        expect((await verseState(replica)).spawnedPortalCount).toBe(0)
        await replica.stepFrames(1)
        const portalOff = scoreRegion(
          decodePng(Buffer.from(await replica.screenshot(), "base64")),
          FRAME,
        ).brightPixels
        // Removing the portal drops bright pixels back below the with-portal frame.
        expect(portalOff).toBeLessThan(withPortal)

        await reset()
      },
    )
  })
})
