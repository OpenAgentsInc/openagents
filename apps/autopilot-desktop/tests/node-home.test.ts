import { describe, expect, test } from "bun:test"
import {
  canonicalPylonHomes,
  createControlTokenResolver,
  discoverPylonHome,
  pylonHomeCandidates,
  resolveAcceptedControlToken,
} from "../src/bun/node-home"

describe("CL-45 discoverPylonHome", () => {
  const cwd = "/work/openagents/apps/autopilot-desktop"
  const homeDir = "/home/op"

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
    const cands = pylonHomeCandidates({ env: "/custom/home", cwd, homeDir })
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

  // CL-45b: the canonical standalone-pylon homes the running control server
  // actually authenticates against (`apps/pylon/src/bootstrap.ts`).
  test("candidate list includes the canonical pylon homes, last, in order", () => {
    const cands = pylonHomeCandidates({ cwd, homeDir })
    const openagentsPylon = "/home/op/.openagents/pylon"
    const dotPylon = "/home/op/.pylon"
    expect(canonicalPylonHomes(homeDir)).toEqual([openagentsPylon, dotPylon])
    expect(cands).toContain(openagentsPylon)
    expect(cands).toContain(dotPylon)
    // nested ~/.openagents/pylon is preferred over bare ~/.pylon
    expect(cands.indexOf(openagentsPylon)).toBeLessThan(cands.indexOf(dotPylon))
    // canonical homes come AFTER the in-tree discovered homes
    expect(cands.indexOf("/work/openagents/.pylon-tailnet")).toBeLessThan(
      cands.indexOf(openagentsPylon),
    )
    // they are the last two entries
    expect(cands[cands.length - 2]).toBe(openagentsPylon)
    expect(cands[cands.length - 1]).toBe(dotPylon)
  })

  // CL-45b: discovery still falls through to the canonical home when only it
  // holds a readable token (the live machine: stale in-tree homes absent).
  test("discovery falls through to the canonical ~/.openagents/pylon home", () => {
    const home = discoverPylonHome({
      cwd,
      homeDir,
      readToken: (h) => (h === "/home/op/.openagents/pylon" ? "tok" : null),
    })
    expect(home).toBe("/home/op/.openagents/pylon")
  })
})

describe("CL-45b resolveAcceptedControlToken — no stale-token dead-end", () => {
  const cwd = "/work/openagents/apps/autopilot-desktop"
  const homeDir = "/home/op"
  const staleHome = "/work/openagents/.pylon-tailnet"
  const canonicalHome = "/home/op/.openagents/pylon"

  // The live-machine bug: a stale token in an earlier candidate home is REJECTED
  // by the server (401); the canonical home's token is ACCEPTED. Resolution must
  // fall through past the stale token to the accepted one instead of dead-ending.
  test("falls through a server-rejected stale token to the accepted canonical token", async () => {
    const probed: string[] = []
    const accepted = await resolveAcceptedControlToken({
      cwd,
      homeDir,
      readToken: (h) =>
        h === staleHome ? "stale-tok" : h === canonicalHome ? "good-tok" : null,
      probe: async (token) => {
        probed.push(token)
        // 401 then 200: the stale token is rejected, the canonical one accepted.
        return token === "good-tok"
      },
    })
    expect(accepted).not.toBeNull()
    expect(accepted?.home).toBe(canonicalHome)
    expect(accepted?.token).toBe("good-tok")
    // it probed the stale token first (rejected), then the canonical one.
    expect(probed).toEqual(["stale-tok", "good-tok"])
  })

  test("uses the first accepted token and stops probing", async () => {
    let probeCount = 0
    const accepted = await resolveAcceptedControlToken({
      cwd,
      homeDir,
      readToken: (h) => (h === staleHome || h === canonicalHome ? "tok" : null),
      probe: async () => {
        probeCount += 1
        return true
      },
    })
    expect(accepted?.home).toBe(staleHome)
    // the two homes share the SAME token string; probed once, never re-probed.
    expect(probeCount).toBe(1)
  })

  test("returns null when NO candidate token is accepted (all 401)", async () => {
    const accepted = await resolveAcceptedControlToken({
      cwd,
      homeDir,
      readToken: (h) =>
        h === staleHome ? "stale-a" : h === canonicalHome ? "stale-b" : null,
      probe: async () => false,
    })
    expect(accepted).toBeNull()
  })

  test("returns null when no candidate home holds any token", async () => {
    let probed = false
    const accepted = await resolveAcceptedControlToken({
      cwd,
      homeDir,
      readToken: () => null,
      probe: async () => {
        probed = true
        return true
      },
    })
    expect(accepted).toBeNull()
    // nothing to probe → the server is never contacted.
    expect(probed).toBe(false)
  })
})

describe("CL-45b createControlTokenResolver — caching + re-validation", () => {
  const cwd = "/work/openagents/apps/autopilot-desktop"
  const homeDir = "/home/op"
  const canonicalHome = "/home/op/.openagents/pylon"

  test("caches the resolved token and only re-validates (no full re-resolve) on reuse", async () => {
    let probeCount = 0
    const resolver = createControlTokenResolver(() => ({
      cwd,
      homeDir,
      readToken: (h) => (h === canonicalHome ? "good-tok" : null),
      probe: async () => {
        probeCount += 1
        return true
      },
    }))
    const first = await resolver.resolve()
    expect(first?.token).toBe("good-tok")
    expect(probeCount).toBe(1)
    // Second call reuses the cache: a single re-validation probe, no re-walk.
    const second = await resolver.resolve()
    expect(second?.token).toBe("good-tok")
    expect(probeCount).toBe(2)
  })

  test("re-resolves from scratch when the cached token stops being accepted", async () => {
    let accept = true
    let resolveWalks = 0
    const resolver = createControlTokenResolver(() => ({
      cwd,
      homeDir,
      readToken: (h) => {
        if (h === canonicalHome) {
          resolveWalks += 1
          return "tok"
        }
        return null
      },
      probe: async () => accept,
    }))
    expect((await resolver.resolve())?.token).toBe("tok")
    // The cached token is now rejected (rotated/expired) → re-resolve, find none.
    accept = false
    expect(await resolver.resolve()).toBeNull()
    expect(resolveWalks).toBeGreaterThan(1)
  })
})
