import { describe, expect, test } from "bun:test"

import {
  classifyFileAccess,
  isPathInWorkspace,
} from "../src/tas/workspace-boundary"

describe("workspace boundary core", () => {
  const workspaceRoot = "/workspace/project"

  test("allows paths inside the workspace", () => {
    expect(isPathInWorkspace(workspaceRoot, "src/index.ts")).toBe(true)
    expect(
      classifyFileAccess(workspaceRoot, "src/index.ts", "read"),
    ).toEqual({
      allowed: true,
      reason: "inside_workspace",
    })
  })

  test("denies traversal that escapes the workspace", () => {
    expect(isPathInWorkspace(workspaceRoot, "../../etc/passwd")).toBe(false)
    expect(
      classifyFileAccess(workspaceRoot, "../../etc/passwd", "read"),
    ).toEqual({
      allowed: false,
      reason: "outside_workspace",
    })
  })

  test("denies absolute paths outside the workspace", () => {
    expect(isPathInWorkspace(workspaceRoot, "/etc/passwd")).toBe(false)
    expect(classifyFileAccess(workspaceRoot, "/etc/passwd", "read")).toEqual({
      allowed: false,
      reason: "outside_workspace",
    })
  })

  test("denies writes outside the workspace", () => {
    expect(
      classifyFileAccess(workspaceRoot, "/tmp/outside.txt", "write"),
    ).toEqual({
      allowed: false,
      reason: "write_outside_workspace",
    })
  })
})
