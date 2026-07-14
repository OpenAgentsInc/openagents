import { readFile } from "node:fs/promises"
import { expect, test } from "vite-plus/test"

test("live barge smoke sends the frozen zero-based AUDIO-1 sequence", async () => {
  const source = await readFile(
    new URL("../scripts/live-barge-smoke.ts", import.meta.url),
  , "utf8")

  expect(source).toContain("let sequence = -1")
  expect(source).toContain("mediaFrame(++sequence")
})
