import { describe, expect, test } from "bun:test"

import { classifyByFingerprint } from "./fingerprint-classify.js"

describe("fingerprint ship-mode classifier", () => {
  test("classifies a missing previous fingerprint as the initial ship", () => {
    expect(classifyByFingerprint({
      prev: null,
      next: "expo.fp.0001",
    })).toEqual({
      mode: "initial",
      changed: true,
      reason: "no previous Expo Update fingerprint is available",
    })
  })

  test("classifies an identical fingerprint as OTA without a native change", () => {
    expect(classifyByFingerprint({
      prev: "expo.fp.0001",
      next: "expo.fp.0001",
    })).toEqual({
      mode: "ota",
      changed: false,
      reason: "Expo Update fingerprint is unchanged; JavaScript-only OTA is eligible",
    })
  })

  test("classifies a changed fingerprint as a rebuild", () => {
    expect(classifyByFingerprint({
      prev: "expo.fp.0001",
      next: "expo.fp.0002",
    })).toEqual({
      mode: "rebuild",
      changed: true,
      reason: "Expo Update fingerprint changed; native rebuild is required",
    })
  })

  test("compares fingerprints exactly without trimming whitespace", () => {
    expect(classifyByFingerprint({
      prev: "expo.fp.0001",
      next: " expo.fp.0001 ",
    })).toEqual({
      mode: "rebuild",
      changed: true,
      reason: "Expo Update fingerprint changed; native rebuild is required",
    })
  })

  test("treats an empty previous fingerprint as a real fingerprint", () => {
    expect(classifyByFingerprint({
      prev: "",
      next: "expo.fp.0001",
    })).toEqual({
      mode: "rebuild",
      changed: true,
      reason: "Expo Update fingerprint changed; native rebuild is required",
    })
  })

  test("allows an empty next fingerprint to match an empty previous fingerprint", () => {
    expect(classifyByFingerprint({
      prev: "",
      next: "",
    })).toEqual({
      mode: "ota",
      changed: false,
      reason: "Expo Update fingerprint is unchanged; JavaScript-only OTA is eligible",
    })
  })
})
