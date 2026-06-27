import { describe, expect, test } from "bun:test"
import { mkdtemp, readFile, writeFile } from "node:fs/promises"
import { tmpdir } from "node:os"
import { join } from "node:path"

import { loadOrCreateRuntimeState, resolveStatePaths } from "./state.js"

// #6354: `loadOrCreateRuntimeState` must preserve dynamically-probed
// capabilities (codex / claude) that `provider go-online` wrote into the
// persisted runtime, even when the bootstrap/config base capability set is a
// non-default set (tassadar + nip90 + labor from config.json). The old logic
// overwrote the runtime with the base set on every read-ish command
// (status/heartbeat/assignment), stripping codex — so the standing fleet's
// heartbeats advertised `codex available=0` and genuinely codex-available
// Pylons were 409'd at the Khala coding dispatch gate.
const makePaths = async () => {
  const home = await mkdtemp(join(tmpdir(), "pylon-state-test-"))
  return resolveStatePaths({
    home,
    config: join(home, "config.json"),
    cache: join(home, "cache"),
    releases: join(home, "releases"),
  })
}

const CONFIG_BASE_REFS = [
  "capability.tassadar_poc.numeric_model_executor",
  "capability.public.pylon.nip90.text_inference.v0.3",
  "capability.public.pylon.labor.local_agent.v0.3",
]

describe("loadOrCreateRuntimeState capability preservation (#6354)", () => {
  test("preserves codex/claude from the persisted runtime when the requested base is non-default", async () => {
    const paths = await makePaths()
    await writeFile(
      paths.runtimeState,
      JSON.stringify({
        lifecycle: "online",
        displayName: null,
        resourceMode: "background_20",
        capabilityRefs: [
          ...CONFIG_BASE_REFS,
          "capability.pylon.local_codex",
          "capability.pylon.local_claude_agent",
        ],
        blockerRefs: [],
        updatedAt: "2026-06-27T00:00:00.000Z",
      }),
    )

    const state = await loadOrCreateRuntimeState(paths, {
      capabilityRefs: CONFIG_BASE_REFS,
    })

    expect(state.capabilityRefs).toContain("capability.pylon.local_codex")
    expect(state.capabilityRefs).toContain("capability.pylon.local_claude_agent")
    for (const ref of CONFIG_BASE_REFS) {
      expect(state.capabilityRefs).toContain(ref)
    }

    // The persisted file must also keep codex so the next heartbeat publishes it.
    const persisted = JSON.parse(await readFile(paths.runtimeState, "utf8"))
    expect(persisted.capabilityRefs).toContain("capability.pylon.local_codex")
  })

  test("does not duplicate refs already present in both sets", async () => {
    const paths = await makePaths()
    await writeFile(
      paths.runtimeState,
      JSON.stringify({
        lifecycle: "online",
        displayName: null,
        resourceMode: "background_20",
        capabilityRefs: [...CONFIG_BASE_REFS, "capability.pylon.local_codex"],
        blockerRefs: [],
        updatedAt: "2026-06-27T00:00:00.000Z",
      }),
    )

    const state = await loadOrCreateRuntimeState(paths, {
      capabilityRefs: CONFIG_BASE_REFS,
    })

    const counts = new Map<string, number>()
    for (const ref of state.capabilityRefs) {
      counts.set(ref, (counts.get(ref) ?? 0) + 1)
    }
    for (const [, count] of counts) {
      expect(count).toBe(1)
    }
  })

  test("creates with the requested base when no runtime exists yet", async () => {
    const paths = await makePaths()
    const state = await loadOrCreateRuntimeState(paths, {
      capabilityRefs: CONFIG_BASE_REFS,
    })
    expect(state.capabilityRefs).toEqual(CONFIG_BASE_REFS)
  })
})
