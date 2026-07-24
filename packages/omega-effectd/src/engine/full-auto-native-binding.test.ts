import { describe, expect, test } from "vite-plus/test"
import { mkdtempSync, rmSync } from "node:fs"
import { tmpdir } from "node:os"
import { join } from "node:path"

import {
  assessFullAutoNativeBoundary,
  buildFullAutoNativeBinding,
  openFullAutoNativeBindingStore,
} from "./full-auto-native-binding.ts"

describe("full-auto-native-binding", () => {
  test("assesses mismatch, rebase-unsafe, and missing binding", () => {
    expect(
      assessFullAutoNativeBoundary({
        binding: null,
        expectedWorkspaceRef: "workspace.omega.supervised",
      }).ok,
    ).toBe(false)

    const binding = buildFullAutoNativeBinding({
      runRef: "run.1",
      workspaceRef: "workspace.other",
      projectRef: "project.1",
      worktreeRef: "worktree.1",
      worktreeAbsolutePath: "/tmp/demo",
    })
    const mismatch = assessFullAutoNativeBoundary({
      binding,
      expectedWorkspaceRef: "workspace.omega.supervised",
    })
    expect(mismatch.ok).toBe(false)
    if (!mismatch.ok) expect(mismatch.reason).toBe("workspace_mismatch")

    const unsafe = assessFullAutoNativeBoundary({
      binding: { ...binding, workspaceRef: "workspace.omega.supervised", rebaseUnsafe: true },
      expectedWorkspaceRef: "workspace.omega.supervised",
    })
    expect(unsafe.ok).toBe(false)
    if (!unsafe.ok) expect(unsafe.reason).toBe("rebase_unsafe")
  })

  test("persists bindings for a completed-run evidence join", () => {
    const root = mkdtempSync(join(tmpdir(), "oa-native-bind-"))
    try {
      const store = openFullAutoNativeBindingStore(join(root, "native-bindings.json"))
      const binding = buildFullAutoNativeBinding({
        runRef: "run.full-auto.1",
        workspaceRef: "workspace.omega.supervised",
        projectRef: "project.42",
        worktreeRef: "worktree.7",
        worktreeAbsolutePath: "/Users/demo/proj",
        gitHead: "abc123",
      })
      store.put(binding)
      const loaded = openFullAutoNativeBindingStore(join(root, "native-bindings.json"))
      const again = loaded.get("run.full-auto.1")
      expect(again?.projectRef).toBe("project.42")
      expect(again?.worktreePathDigest).toMatch(/^[a-f0-9]{64}$/)
      expect(again?.gitHead).toBe("abc123")
      const ok = assessFullAutoNativeBoundary({
        binding: again,
        expectedWorkspaceRef: "workspace.omega.supervised",
        currentWorktreePathDigest: again?.worktreePathDigest ?? undefined,
      })
      expect(ok.ok).toBe(true)
    } finally {
      rmSync(root, { recursive: true, force: true })
    }
  })
})
