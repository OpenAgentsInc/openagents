// Headless app-replica regression — drives the REAL desktop renderer and asserts
// the three live bugs the owner hit are reproduced-then-fixed.
//
// Unlike the full-input-path harness (which reconstructs the input chain in
// pure TS), this boots the SAME Model / view / update / subscriptions + the real
// Foldkit `Runtime.run` mount the live app uses, in headless Chromium, with:
//   • StyleX SOLVED — the entry is compiled with the same @stylexjs/unplugin Bun
//     plugin `build:css` uses for the real `main.ts`, so `view.ts` mounts without
//     throwing (the wall prior agents hit), with the real styles.out.css served.
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

const KHALA_INPUT = ".verse-khala-input"
const HOTBAR = ".hotbar"
const SLOT_2 = ".hotbar-slot-2"
const SLOT_3 = ".hotbar-slot-3"
const KHALA_BAR = ".verse-khala-bar"
const KHALA_BODY = ".verse-khala-bubble-body"
const SCENE_ACTIVE = '[data-verse-spawned-scene="active"]'
const SCENE_TAG = ".verse-spawned-scene-tag"

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

  // ── The replica actually mounts the real view (StyleX solved, no crash) ──────

  slowTest("the real Verse view mounts with real CSS applied (StyleX compiled)", async () => {
    // The real app-shell-verse container is present and laid out full-viewport —
    // proving the real `view.ts` (the same one the app runs) mounted without the
    // StyleX `stylex.attrs` throw, with the real stylesheet applied.
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
    slowTest("click slot 2 spawns the scene and click slot 3 toggles its portal", async () => {
      // From a clean state: ensure no scene is spawned.
      const reset = async (): Promise<void> => {
        if ((await replica.count(SCENE_ACTIVE)) > 0) {
          await replica.click(SLOT_2)
        }
      }
      await reset()
      expect(await replica.count(SCENE_ACTIVE)).toBe(0)

      // Click slot 2 — spawns the crackling scene (works regardless of focus).
      await replica.click(SLOT_2)
      expect(await replica.count(SCENE_ACTIVE)).toBe(1)
      expect(await replica.text(SCENE_TAG)).toContain("Crackling")
      expect(await replica.text(SCENE_TAG)).not.toContain("portal")

      // Click slot 3 — toggles the scene's gateway portal on.
      await replica.click(SLOT_3)
      expect(await replica.text(SCENE_TAG)).toContain("portal")

      // Reset for the next test.
      await replica.click(SLOT_3) // portal off
      await replica.click(SLOT_2) // scene off
      expect(await replica.count(SCENE_ACTIVE)).toBe(0)
    })

    slowTest("clicking slot 2 spawns even while the Ask box is focused", async () => {
      // Ensure clean + focus the Ask box, then click — the slot must still fire.
      if ((await replica.count(SCENE_ACTIVE)) > 0) await replica.click(SLOT_2)
      expect(await replica.count(SCENE_ACTIVE)).toBe(0)
      await replica.focus(KHALA_INPUT)
      await replica.click(SLOT_2)
      expect(await replica.count(SCENE_ACTIVE)).toBe(1)
      await replica.click(SLOT_2) // reset off
      expect(await replica.count(SCENE_ACTIVE)).toBe(0)
    })

    slowTest("bare '2' spawns when NO input is focused", async () => {
      if ((await replica.count(SCENE_ACTIVE)) > 0) await replica.click(SLOT_2)
      expect(await replica.count(SCENE_ACTIVE)).toBe(0)
      // Blur any focused element so the keypress is not "in editable".
      await replica.evaluate(
        `(document.activeElement && document.activeElement.blur && document.activeElement.blur(), true)`,
      )
      await replica.pressKey("2")
      expect(await replica.count(SCENE_ACTIVE)).toBe(1)
      await replica.pressKey("2") // toggle off
      expect(await replica.count(SCENE_ACTIVE)).toBe(0)
    })

    slowTest("FIX: '2' while the Ask box is focused fires the slot and does NOT type a digit", async () => {
      // This is the owner-reported failure: with the Ask box focused, a bare `2`
      // typed a digit into the box and "the hotbar did nothing". Fixed: the digit
      // is swallowed (clean box) and the hotbar slot fires.
      if ((await replica.count(SCENE_ACTIVE)) > 0) await replica.click(SLOT_2)
      expect(await replica.count(SCENE_ACTIVE)).toBe(0)
      await replica.focus(KHALA_INPUT)
      await replica.pressKey("2")
      // The Ask box stays empty — the digit did NOT pollute the prompt.
      const value = await replica.evaluate<string>(
        `document.querySelector("${KHALA_INPUT}").value`,
      )
      expect(value).toBe("")
      // …and the slot fired (the scene spawned).
      expect(await replica.count(SCENE_ACTIVE)).toBe(1)
      await replica.click(SLOT_2) // reset off
      expect(await replica.count(SCENE_ACTIVE)).toBe(0)
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
      if ((await replica.count(SCENE_ACTIVE)) > 0) await replica.click(SLOT_2)
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
})
