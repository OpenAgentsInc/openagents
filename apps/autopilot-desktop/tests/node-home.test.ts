import { describe, expect, test } from "bun:test"
import { discoverPylonHome, pylonHomeCandidates } from "../src/bun/node-home"

describe("CL-45 discoverPylonHome", () => {
  const cwd = "/work/openagents/apps/autopilot-desktop"

  test("an explicit env home wins over discovery", () => {
    const home = discoverPylonHome({
      env: "/custom/home",
      cwd,
      readToken: (h) => (h === "/custom/home" ? "tok" : null),
    })
    expect(home).toBe("/custom/home")
  })

  test("prefers .pylon-tailnet over .pylon-local at the same ancestor", () => {
    // Both exist at the repo root; tailnet is listed first.
    const home = discoverPylonHome({
      cwd,
      readToken: (h) =>
        h === "/work/openagents/.pylon-tailnet" || h === "/work/openagents/.pylon-local" ? "tok" : null,
    })
    expect(home).toBe("/work/openagents/.pylon-tailnet")
  })

  test("walks up ancestors until a readable token is found", () => {
    const home = discoverPylonHome({
      cwd,
      readToken: (h) => (h === "/work/.pylon-tailnet" ? "tok" : null),
    })
    expect(home).toBe("/work/.pylon-tailnet")
  })

  test("returns null when no home has a readable token", () => {
    expect(discoverPylonHome({ cwd, readToken: () => null })).toBeNull()
  })

  test("treats an empty token as not present", () => {
    expect(discoverPylonHome({ cwd, readToken: () => "" })).toBeNull()
  })

  test("candidate list is ordered env → tailnet/local per ancestor, de-duped", () => {
    const cands = pylonHomeCandidates({ env: "/custom/home", cwd })
    expect(cands[0]).toBe("/custom/home")
    expect(cands).toContain("/work/openagents/.pylon-tailnet")
    expect(cands).toContain("/work/openagents/.pylon-local")
    // tailnet for an ancestor comes before local for the same ancestor
    const t = cands.indexOf("/work/openagents/.pylon-tailnet")
    const l = cands.indexOf("/work/openagents/.pylon-local")
    expect(t).toBeLessThan(l)
    // no duplicates
    expect(new Set(cands).size).toBe(cands.length)
  })
})
