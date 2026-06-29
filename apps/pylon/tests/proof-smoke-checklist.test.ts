import { describe, expect, test } from "bun:test"

import { evaluateProofSmoke } from "../src/coordinator/proof-smoke-checklist"

describe("proof smoke checklist", () => {
  test("marks ready when every lane ran and passed and every surface is verified", () => {
    expect(
      evaluateProofSmoke({
        lanes: [
          { name: "codex", ran: true, passed: true },
          { name: "claude", ran: true, passed: true },
        ],
        surfaces: [
          { name: "autopilot", verified: true },
          { name: "forum", verified: true },
        ],
      }),
    ).toEqual({
      ready: true,
      missing: [],
      summary: "Proof smoke ready: 2 lane(s), 2 surface(s).",
    })
  })

  test("blocks when a lane did not run", () => {
    expect(
      evaluateProofSmoke({
        lanes: [{ name: "codex", ran: false, passed: true }],
        surfaces: [{ name: "autopilot", verified: true }],
      }),
    ).toEqual({
      ready: false,
      missing: ["lane:codex:ran"],
      summary: "Proof smoke blocked: 1 missing check(s).",
    })
  })

  test("blocks when a lane did not pass", () => {
    expect(
      evaluateProofSmoke({
        lanes: [{ name: "claude", ran: true, passed: false }],
        surfaces: [{ name: "forum", verified: true }],
      }),
    ).toEqual({
      ready: false,
      missing: ["lane:claude:passed"],
      summary: "Proof smoke blocked: 1 missing check(s).",
    })
  })

  test("reports both lane checks when a lane neither ran nor passed", () => {
    expect(
      evaluateProofSmoke({
        lanes: [{ name: "overnight", ran: false, passed: false }],
        surfaces: [{ name: "public-proof", verified: true }],
      }).missing,
    ).toEqual(["lane:overnight:ran", "lane:overnight:passed"])
  })

  test("blocks when a surface is not verified", () => {
    expect(
      evaluateProofSmoke({
        lanes: [{ name: "codex", ran: true, passed: true }],
        surfaces: [{ name: "sites", verified: false }],
      }),
    ).toEqual({
      ready: false,
      missing: ["surface:sites:verified"],
      summary: "Proof smoke blocked: 1 missing check(s).",
    })
  })

  test("preserves missing checklist order across lanes then surfaces", () => {
    expect(
      evaluateProofSmoke({
        lanes: [
          { name: "lane-a", ran: false, passed: false },
          { name: "lane-b", ran: true, passed: false },
        ],
        surfaces: [
          { name: "surface-a", verified: false },
          { name: "surface-b", verified: true },
          { name: "surface-c", verified: false },
        ],
      }),
    ).toEqual({
      ready: false,
      missing: [
        "lane:lane-a:ran",
        "lane:lane-a:passed",
        "lane:lane-b:passed",
        "surface:surface-a:verified",
        "surface:surface-c:verified",
      ],
      summary: "Proof smoke blocked: 5 missing check(s).",
    })
  })
})
