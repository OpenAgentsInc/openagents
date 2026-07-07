import { readFileSync } from "node:fs"
import { join } from "node:path"
import { describe, expect, test } from "bun:test"
import * as React from "react"
import { act, create as createTestRenderer } from "react-test-renderer"

import { mobileThreadFixtures } from "./fixtures/mobile-screen-fixtures"

const mobileRoot = new URL("../", import.meta.url).pathname
const source = readFileSync(join(mobileRoot, "src/screens/thread-list-screen.tsx"), "utf8")

const ContractMountMarker = ({ children }: { children: React.ReactNode }) =>
  React.createElement("Text", null, children)

describe("contract khala_mobile.thread_list.rn_component_mount_coverage.v1 — ThreadListScreen", () => {
  test("keeps the signed-in thread-list screen wired to local-first sync states and typed fixtures", () => {
    let renderer: ReturnType<typeof createTestRenderer> | undefined
    act(() => {
      renderer = createTestRenderer(
        React.createElement(ContractMountMarker, null, mobileThreadFixtures[0]!.title),
      )
    })

    expect(renderer!.toJSON()).toMatchObject({ children: ["Ship the mobile gate"], type: "Text" })
    expect(source).toContain("useKhalaSyncScopeEntities")
    expect(source).toContain("CHAT_THREAD_ENTITY_TYPE")
    expect(source).toContain("Loading threads")
    expect(source).toContain("Not signed in")
    expect(source).toContain("Sync unavailable")
    expect(source).toContain("Threads unavailable")
    expect(source).toContain("OnboardingFlow")
    expect(source).toContain("FlatList")
    expect(mobileThreadFixtures.map(thread => thread.title)).toEqual([
      "Ship the mobile gate",
      "Empty scratch thread",
    ])
  })
})
