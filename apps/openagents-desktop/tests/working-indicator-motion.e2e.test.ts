import { spawnSync } from "node:child_process"
import path from "node:path"
import { describe, expect, test } from "vite-plus/test"

type AnimationSample = Readonly<{
  animationName: string
  playState: string | null
  currentTime: number | null
}>

type ProbeReceipt = Readonly<{
  first: { label: string | null; animations: ReadonlyArray<AnimationSample> }
  second: { label: string | null; animations: ReadonlyArray<AnimationSample> }
  reduced: { label: string | null; animations: ReadonlyArray<AnimationSample> }
}>

const desktopRoot = path.resolve(import.meta.dirname, "..")
const electron = path.join(desktopRoot, "node_modules", ".bin", "electron")
const probe = path.join(import.meta.dirname, "fixtures", "working-indicator-motion-probe.cjs")

describe("openagents_desktop.working_indicator_continuous_motion.v1", () => {
  test("real Chromium keeps every working bar running and advancing unless reduced motion is requested", () => {
    const env = { ...process.env }
    delete env.ELECTRON_RUN_AS_NODE
    const result = spawnSync(electron, [probe], {
      cwd: desktopRoot,
      encoding: "utf8",
      env,
      timeout: 20_000,
    })
    expect(result.status, result.stderr).toBe(0)
    const receiptLine = result.stdout.trim().split("\n").findLast(line => line.startsWith("{"))
    expect(receiptLine, result.stdout).toBeDefined()
    const receipt = JSON.parse(receiptLine!) as ProbeReceipt

    expect(receipt.first.label).toBe("Codex is working")
    expect(receipt.first.animations).toHaveLength(3)
    for (const [index, first] of receipt.first.animations.entries()) {
      const second = receipt.second.animations[index]!
      expect(first.animationName).toBe("oa-react-working-step")
      expect(first.playState).toBe("running")
      expect(second.playState).toBe("running")
      expect(first.currentTime).not.toBeNull()
      expect(second.currentTime).toBeGreaterThan(first.currentTime!)
    }

    expect(receipt.reduced.animations).toHaveLength(3)
    for (const animation of receipt.reduced.animations) {
      expect(animation.animationName).toBe("none")
      expect(animation.playState).toBeNull()
      expect(animation.currentTime).toBeNull()
    }
  })
})
