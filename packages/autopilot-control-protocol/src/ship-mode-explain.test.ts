import { describe, expect, test } from "bun:test"

import { explainShipMode } from "./ship-mode-explain.js"

describe("ship mode explanation", () => {
  test("explains a missing previous fingerprint as an initial build", () => {
    expect(explainShipMode({
      prev: null,
      next: "expo.fp.0001",
    })).toEqual({
      mode: "initial",
      headline: "Initial build required",
      detail: "No previous Expo Update fingerprint exists, so ship the first native build.",
    })
  })

  test("explains an identical fingerprint as JavaScript-only OTA", () => {
    expect(explainShipMode({
      prev: "expo.fp.0001",
      next: "expo.fp.0001",
    })).toEqual({
      mode: "ota",
      headline: "JavaScript-only OTA eligible",
      detail: "The Expo Update fingerprint is unchanged, so the update can ship over the existing native build.",
    })
  })

  test("explains a changed fingerprint as a native rebuild", () => {
    expect(explainShipMode({
      prev: "expo.fp.0001",
      next: "expo.fp.0002",
    })).toEqual({
      mode: "rebuild",
      headline: "Native rebuild required",
      detail: "The Expo Update fingerprint changed, so a new native build is required before shipping.",
    })
  })

  test("preserves exact fingerprint comparison without trimming whitespace", () => {
    expect(explainShipMode({
      prev: "expo.fp.0001",
      next: " expo.fp.0001 ",
    })).toEqual({
      mode: "rebuild",
      headline: "Native rebuild required",
      detail: "The Expo Update fingerprint changed, so a new native build is required before shipping.",
    })
  })

  test("treats an empty previous fingerprint as a real fingerprint", () => {
    expect(explainShipMode({
      prev: "",
      next: "expo.fp.0001",
    })).toEqual({
      mode: "rebuild",
      headline: "Native rebuild required",
      detail: "The Expo Update fingerprint changed, so a new native build is required before shipping.",
    })
  })

  test("explains matching empty fingerprints as OTA eligible", () => {
    expect(explainShipMode({
      prev: "",
      next: "",
    })).toEqual({
      mode: "ota",
      headline: "JavaScript-only OTA eligible",
      detail: "The Expo Update fingerprint is unchanged, so the update can ship over the existing native build.",
    })
  })
})
