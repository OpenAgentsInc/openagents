import { describe, expect, test } from "vite-plus/test"

import { desktopLaunchWorkspaceRoot } from "./desktop-launch-workspace.ts"

describe("desktopLaunchWorkspaceRoot", () => {
  test("prioritizes the directory explicitly captured by the desktop launcher", () => {
    expect(desktopLaunchWorkspaceRoot({
      explicitRoot: " /work/owner-project ",
      processWorkingDirectory: "/work/.oa-launch/apps/openagents-desktop",
      homeRoot: "/Users/owner",
      isDirectory: candidate => ["/work/owner-project", "/work/.oa-launch/apps/openagents-desktop"].includes(candidate),
    })).toBe("/work/owner-project")
  })

  test("uses the process launch directory when no launcher override is present", () => {
    expect(desktopLaunchWorkspaceRoot({
      explicitRoot: undefined,
      processWorkingDirectory: "/work/direct-launch",
      homeRoot: "/Users/owner",
      isDirectory: candidate => candidate === "/work/direct-launch",
    })).toBe("/work/direct-launch")
  })

  test("falls back safely when a captured directory has disappeared", () => {
    expect(desktopLaunchWorkspaceRoot({
      explicitRoot: "/work/deleted",
      processWorkingDirectory: "/work/direct-launch",
      homeRoot: "/Users/owner",
      isDirectory: candidate => candidate === "/work/direct-launch",
    })).toBe("/work/direct-launch")
    expect(desktopLaunchWorkspaceRoot({
      explicitRoot: "/work/deleted",
      processWorkingDirectory: "/work/also-deleted",
      homeRoot: "/Users/owner",
      isDirectory: () => false,
    })).toBe("/Users/owner")
  })

  test("never returns the filesystem root — a Finder/Dock launch (cwd '/') falls back to home", () => {
    // `/` is a directory, but returning it opens the app against the whole
    // filesystem and triggers the macOS permission-prompt storm (#9156/#9157).
    expect(desktopLaunchWorkspaceRoot({
      explicitRoot: undefined,
      processWorkingDirectory: "/",
      homeRoot: "/Users/owner",
      isDirectory: () => true,
    })).toBe("/Users/owner")
  })

  test("an explicit root of '/' is also rejected in favor of home", () => {
    expect(desktopLaunchWorkspaceRoot({
      explicitRoot: "/",
      processWorkingDirectory: "/",
      homeRoot: "/Users/owner",
      isDirectory: () => true,
    })).toBe("/Users/owner")
  })
})
