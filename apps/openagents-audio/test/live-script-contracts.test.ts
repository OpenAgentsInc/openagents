import { expect, test } from "bun:test"

test("live barge smoke sends the frozen zero-based AUDIO-1 sequence", async () => {
  const source = await Bun.file(
    new URL("../scripts/live-barge-smoke.ts", import.meta.url),
  ).text()

  expect(source).toContain("let sequence = -1")
  expect(source).toContain("mediaFrame(++sequence")
})
