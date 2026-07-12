import { expect, test } from "bun:test"
import golden from "../../../fixtures/audio-contract/media-v1.json"
import { decodeMediaHeader } from "./index"
test("Effect matches the normative Rust/Effect media corpus", () => {
  for (const item of golden.cases) {
    let accepted = true
    try { decodeMediaHeader(item.header) } catch { accepted = false }
    expect(accepted, item.name).toBe(item.accept)
  }
})
