import { describe, expect, test } from "vite-plus/test"
import { desktopRuntimeWorkspaceRoot } from "./desktop-runtime-workspace.ts"

describe("desktopRuntimeWorkspaceRoot", () => {
  test("binds real provider turns to the selected workspace", () => {
    expect(desktopRuntimeWorkspaceRoot({
      fixtureMode: false,
      userDataPath: "/tmp/profile",
      selectedWorkspaceRoot: "/work/openagents",
      launchFallbackRoot: "/Users/owner",
    })).toBe("/work/openagents")
  })

  test("keeps bounded fixtures and the no-selection fallback explicit", () => {
    expect(desktopRuntimeWorkspaceRoot({
      fixtureMode: true,
      userDataPath: "/tmp/profile",
      selectedWorkspaceRoot: "/work/openagents",
      launchFallbackRoot: "/Users/owner",
    })).toBe("/tmp/profile/fable-local/fixture-workspace")
    expect(desktopRuntimeWorkspaceRoot({
      fixtureMode: false,
      userDataPath: "/tmp/profile",
      selectedWorkspaceRoot: null,
      launchFallbackRoot: "/Users/owner",
    })).toBe("/Users/owner")
  })
})
