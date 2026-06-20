import { describe, expect, it } from "bun:test"
import {
  existsSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  statSync,
  writeFileSync,
} from "node:fs"
import { tmpdir } from "node:os"
import { join } from "node:path"
import {
  detectExistingPylonIdentity,
  detectedIdentityShortLabel,
  loadIdentityChoice,
  projectIdentityChoiceState,
  saveIdentityChoice,
} from "../src/bun/identity-choice"
import { resolveFirstRunLaunchChoice } from "../src/bun/first-run-launch-choice"

// AO-3 (#5444): identity detection + choice. The seed marker is
// `identity.mnemonic` (matches apps/pylon/src/bootstrap.ts HOME_SEED_MARKER); we
// test presence only and never read it. Tests build a fake user home dir with a
// `.openagents/pylon` and/or `.pylon` home and assert the detect/choice contract.

const makeHomeDir = (): string => mkdtempSync(join(tmpdir(), "ao3-home-"))

// Create a seed-bearing Pylon home: the `identity.mnemonic` marker (content is
// irrelevant — only presence matters) + an `identity.json` public projection.
const seedHome = (
  homeDir: string,
  rel: ".openagents/pylon" | ".pylon",
  identity: { npub?: string; pylonRef?: string; nodeLabel?: string } = {},
): string => {
  const home = join(homeDir, ...rel.split("/"))
  mkdirSync(home, { recursive: true })
  writeFileSync(join(home, "identity.mnemonic"), "DO NOT READ — seed marker\n")
  writeFileSync(join(home, "identity.json"), JSON.stringify(identity))
  return home
}

describe("detectExistingPylonIdentity (AO-3)", () => {
  it("returns null on a fresh machine (no seed anywhere)", () => {
    const homeDir = makeHomeDir()
    try {
      expect(detectExistingPylonIdentity({ homeDir })).toBeNull()
    } finally {
      rmSync(homeDir, { recursive: true, force: true })
    }
  })

  it("detects a seed-bearing ~/.openagents/pylon and reads its public identity", () => {
    const homeDir = makeHomeDir()
    try {
      const home = seedHome(homeDir, ".openagents/pylon", {
        npub: "npub1existing00000000000000000000000000000000000000000000000000",
        pylonRef: "pylon.ab12cd34ef",
        nodeLabel: "studio-mac",
      })
      const detected = detectExistingPylonIdentity({ homeDir })
      expect(detected).not.toBeNull()
      expect(detected?.home).toBe(home)
      expect(detected?.source).toBe("discovered_openagents_pylon")
      expect(detected?.pylonRef).toBe("pylon.ab12cd34ef")
      expect(detectedIdentityShortLabel(detected!)).toBe("pylon.ab12cd34")
    } finally {
      rmSync(homeDir, { recursive: true, force: true })
    }
  })

  it("prefers ~/.openagents/pylon over ~/.pylon when both hold a seed", () => {
    const homeDir = makeHomeDir()
    try {
      seedHome(homeDir, ".pylon", { pylonRef: "pylon.dotpylon" })
      const preferred = seedHome(homeDir, ".openagents/pylon", {
        pylonRef: "pylon.openagents",
      })
      const detected = detectExistingPylonIdentity({ homeDir })
      expect(detected?.home).toBe(preferred)
      expect(detected?.source).toBe("discovered_openagents_pylon")
    } finally {
      rmSync(homeDir, { recursive: true, force: true })
    }
  })

  it("falls back to ~/.pylon when only it holds a seed", () => {
    const homeDir = makeHomeDir()
    try {
      // A ~/.openagents/pylon dir WITHOUT a seed marker must not be adopted.
      mkdirSync(join(homeDir, ".openagents", "pylon"), { recursive: true })
      const dot = seedHome(homeDir, ".pylon", { npub: "npub1dotonly" })
      const detected = detectExistingPylonIdentity({ homeDir })
      expect(detected?.home).toBe(dot)
      expect(detected?.source).toBe("discovered_dot_pylon")
    } finally {
      rmSync(homeDir, { recursive: true, force: true })
    }
  })

  it("never reads the seed — only the marker presence drives detection", () => {
    const homeDir = makeHomeDir()
    try {
      const home = join(homeDir, ".openagents", "pylon")
      mkdirSync(home, { recursive: true })
      // Marker present, but NO identity.json (public projection missing). The
      // home is still authoritative; npub is just null.
      writeFileSync(join(home, "identity.mnemonic"), "seed")
      const detected = detectExistingPylonIdentity({ homeDir })
      expect(detected?.home).toBe(home)
      expect(detected?.npub).toBeNull()
      // Short label falls back to a generic, never the seed.
      expect(detectedIdentityShortLabel(detected!)).toBe("existing Pylon")
    } finally {
      rmSync(homeDir, { recursive: true, force: true })
    }
  })
})

describe("saveIdentityChoice / loadIdentityChoice (AO-3)", () => {
  it("create_new persists the chosen display name (0600) and is loadable", () => {
    const homeDir = makeHomeDir()
    try {
      const result = saveIdentityChoice(
        { kind: "create_new", displayName: "My Studio Agent" },
        { homeDir },
      )
      expect(result.ok).toBe(true)
      const loaded = loadIdentityChoice({ homeDir })
      expect(loaded?.kind).toBe("create_new")
      expect(loaded?.displayName).toBe("My Studio Agent")
      expect(loaded?.home).toBeNull()
      const path = join(
        homeDir,
        ".openagents",
        "autopilot-desktop",
        "identity-choice.json",
      )
      expect((statSync(path).mode & 0o777)).toBe(0o600)
    } finally {
      rmSync(homeDir, { recursive: true, force: true })
    }
  })

  it("use_existing persists the detected home and re-verifies the seed marker", () => {
    const homeDir = makeHomeDir()
    try {
      const home = seedHome(homeDir, ".openagents/pylon", {
        pylonRef: "pylon.keepme",
      })
      const result = saveIdentityChoice({ kind: "use_existing", home }, { homeDir })
      expect(result.ok).toBe(true)
      const loaded = loadIdentityChoice({ homeDir })
      expect(loaded?.kind).toBe("use_existing")
      expect(loaded?.home).toBe(home)
    } finally {
      rmSync(homeDir, { recursive: true, force: true })
    }
  })

  it("use_existing REFUSES a home without a seed marker (never adopt the wrong home)", () => {
    const homeDir = makeHomeDir()
    try {
      const seedless = join(homeDir, "fake-home")
      mkdirSync(seedless, { recursive: true })
      const result = saveIdentityChoice(
        { kind: "use_existing", home: seedless },
        { homeDir },
      )
      expect(result.ok).toBe(false)
      // Nothing persisted.
      expect(loadIdentityChoice({ homeDir })).toBeNull()
    } finally {
      rmSync(homeDir, { recursive: true, force: true })
    }
  })

  it("never writes into a seed-bearing Pylon home (no overwrite of an identity)", () => {
    const homeDir = makeHomeDir()
    try {
      const home = seedHome(homeDir, ".openagents/pylon", {
        pylonRef: "pylon.untouched",
      })
      const before = readFileSync(join(home, "identity.json"), "utf8")
      saveIdentityChoice({ kind: "use_existing", home }, { homeDir })
      // The seed home's identity.json is byte-for-byte unchanged, and no choice
      // file was written inside the seed home.
      expect(readFileSync(join(home, "identity.json"), "utf8")).toBe(before)
      expect(existsSync(join(home, "identity-choice.json"))).toBe(false)
    } finally {
      rmSync(homeDir, { recursive: true, force: true })
    }
  })

  it("returns null for a missing or malformed choice file", () => {
    const homeDir = makeHomeDir()
    try {
      expect(loadIdentityChoice({ homeDir })).toBeNull()
      const dir = join(homeDir, ".openagents", "autopilot-desktop")
      mkdirSync(dir, { recursive: true })
      writeFileSync(join(dir, "identity-choice.json"), "{not json")
      expect(loadIdentityChoice({ homeDir })).toBeNull()
      writeFileSync(
        join(dir, "identity-choice.json"),
        JSON.stringify({ kind: "bogus" }),
      )
      expect(loadIdentityChoice({ homeDir })).toBeNull()
    } finally {
      rmSync(homeDir, { recursive: true, force: true })
    }
  })
})

describe("projectIdentityChoiceState (AO-3)", () => {
  it("fresh machine: choice needed, no detection, create-new always available", () => {
    const homeDir = makeHomeDir()
    try {
      const state = projectIdentityChoiceState({ homeDir })
      expect(state.choiceNeeded).toBe(true)
      expect(state.detected.present).toBe(false)
      expect(state.detected.pylonRef).toBeNull()
      expect(state.chosen).toBeNull()
      // The default on a fresh machine is create-new, and it is always offered.
      expect(state.createNewAvailable).toBe(true)
    } finally {
      rmSync(homeDir, { recursive: true, force: true })
    }
  })

  it("existing detected: BOTH options available (use-existing + create-new)", () => {
    const homeDir = makeHomeDir()
    try {
      seedHome(homeDir, ".openagents/pylon", { pylonRef: "pylon.bothopts" })
      const state = projectIdentityChoiceState({ homeDir })
      expect(state.choiceNeeded).toBe(true)
      // "Use existing" is offered (detected present)…
      expect(state.detected.present).toBe(true)
      expect(state.detected.shortLabel).toBe("pylon.bothopts")
      expect(state.detected.pylonRef).toBe("pylon.bothopts")
      // …and "create new" is STILL available even with an existing Pylon.
      expect(state.createNewAvailable).toBe(true)
    } finally {
      rmSync(homeDir, { recursive: true, force: true })
    }
  })

  it("after a choice is persisted, the choice is no longer needed", () => {
    const homeDir = makeHomeDir()
    try {
      saveIdentityChoice(
        { kind: "create_new", displayName: "Named One" },
        { homeDir },
      )
      const state = projectIdentityChoiceState({ homeDir })
      expect(state.choiceNeeded).toBe(false)
      expect(state.chosen?.kind).toBe("create_new")
      expect(state.chosen?.displayName).toBe("Named One")
    } finally {
      rmSync(homeDir, { recursive: true, force: true })
    }
  })
})

describe("resolveFirstRunLaunchChoice (AO-3)", () => {
  it("does not auto-onboard before the user chooses an identity", () => {
    const homeDir = makeHomeDir()
    try {
      expect(resolveFirstRunLaunchChoice({ homeDir })).toEqual({
        choiceMade: false,
        chosenExistingHome: null,
        chosenDisplayName: null,
      })
    } finally {
      rmSync(homeDir, { recursive: true, force: true })
    }
  })

  it("threads a create-new display name into launcher startup", () => {
    const homeDir = makeHomeDir()
    try {
      saveIdentityChoice(
        { kind: "create_new", displayName: "Studio Mac" },
        { homeDir },
      )
      expect(resolveFirstRunLaunchChoice({ homeDir })).toEqual({
        choiceMade: true,
        chosenExistingHome: null,
        chosenDisplayName: "Studio Mac",
      })
    } finally {
      rmSync(homeDir, { recursive: true, force: true })
    }
  })

  it("refuses a stale use-existing choice when the detected seed home changed", () => {
    const homeDir = makeHomeDir()
    try {
      const chosen = seedHome(homeDir, ".pylon", { pylonRef: "pylon.old" })
      saveIdentityChoice({ kind: "use_existing", home: chosen }, { homeDir })
      // A later ~/.openagents/pylon seed now wins detection, so the old choice
      // must not be auto-adopted.
      seedHome(homeDir, ".openagents/pylon", { pylonRef: "pylon.new" })
      expect(resolveFirstRunLaunchChoice({ homeDir })).toEqual({
        choiceMade: false,
        chosenExistingHome: null,
        chosenDisplayName: null,
      })
    } finally {
      rmSync(homeDir, { recursive: true, force: true })
    }
  })
})
