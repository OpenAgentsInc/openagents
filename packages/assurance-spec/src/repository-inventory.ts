import { createHash } from "node:crypto"
import { realpathSync } from "node:fs"
import { basename } from "node:path"
import { spawnSync } from "node:child_process"

import { canonicalJson } from "./serializer.ts"
import type { RepositoryDeclaredScript, RepositoryInventory } from "./schema.ts"

const MAX_CANDIDATE_REFS = 400
const MAX_PACKAGE_MANIFESTS = 250
const MAX_DECLARED_SCRIPTS = 2_000
const MAX_MANIFEST_BYTES = 512_000
const MAX_GIT_OUTPUT_BYTES = 16 * 1024 * 1024

type GitResult = Readonly<{ ok: true; stdout: Buffer }> | Readonly<{ ok: false }>

const gitEnvironment = (): NodeJS.ProcessEnv => ({
  PATH: process.env.PATH ?? "/usr/bin:/bin:/usr/sbin:/sbin",
  HOME: process.env.HOME ?? "",
  GIT_OPTIONAL_LOCKS: "0",
  GIT_TERMINAL_PROMPT: "0",
  LC_ALL: "C",
})

const git = (root: string, args: ReadonlyArray<string>, configOverrides: ReadonlyArray<string> = []): GitResult => {
  const result = spawnSync("git", ["-C", root, ...configOverrides, ...args], {
    env: gitEnvironment(),
    maxBuffer: MAX_GIT_OUTPUT_BYTES,
    stdio: ["ignore", "pipe", "ignore"],
  })
  return result.status === 0 && result.stdout instanceof Buffer
    ? { ok: true, stdout: result.stdout }
    : { ok: false }
}

const sha256 = (value: string): string =>
  `sha256:${createHash("sha256").update(value, "utf8").digest("hex")}`

const candidateArtifact = (path: string): boolean => {
  const normalized = path.toLowerCase()
  const file = normalized.split("/").at(-1) ?? normalized
  const inTestDirectory = normalized.split("/").some((part) => ["test", "tests", "spec", "specs", "e2e"].includes(part))
  const testSuffix = /(?:^|\.)(?:test|spec|e2e)\.(?:[cm]?[jt]sx?|py|rs|go|swift|kt|java|sh)$/.test(file)
  const exactConfig = /^(?:playwright|vitest|jest|cypress|maestro|pytest|tox)\.(?:config\.)?.+$/.test(file)
  return inTestDirectory || testSuffix || exactConfig
}

const inventoryDigest = (value: Omit<RepositoryInventory, "inventory_digest">): string =>
  sha256(canonicalJson(value))

export const absentRepositoryInventory = (): RepositoryInventory => {
  const base: Omit<RepositoryInventory, "inventory_digest"> = {
    state: "absent",
    repository_label: "not-supplied",
    tracked_file_count: 0,
    candidate_artifact_refs: [],
    declared_scripts: [],
    truncated: false,
    diagnostics: ["repository_not_supplied"],
  }
  return { ...base, inventory_digest: inventoryDigest(base) }
}

const unavailableInventory = (
  state: "not_git" | "unavailable",
  label: string,
  diagnostic: string,
): RepositoryInventory => {
  const base: Omit<RepositoryInventory, "inventory_digest"> = {
    state,
    repository_label: label || "repository",
    tracked_file_count: 0,
    candidate_artifact_refs: [],
    declared_scripts: [],
    truncated: false,
    diagnostics: [diagnostic],
  }
  return { ...base, inventory_digest: inventoryDigest(base) }
}

const text = (result: Extract<GitResult, { ok: true }>): string => result.stdout.toString("utf8").trim()

export const inventoryRepository = (requestedRoot: string): RepositoryInventory => {
  let requestedRealRoot: string
  try {
    requestedRealRoot = realpathSync(requestedRoot)
  } catch {
    return unavailableInventory("unavailable", basename(requestedRoot), "repository_unavailable")
  }

  // `-c core.bare=false` neutralizes a shared worktree-hub `.git/config`
  // whose common core.bare=true would otherwise leak into this worktree's
  // root resolution (see issue #8984). Inventory only ever runs against a
  // real working tree, so the override is always correct here.
  const rootResult = git(
    requestedRealRoot,
    ["rev-parse", "--path-format=absolute", "--show-toplevel"],
    ["-c", "core.bare=false"],
  )
  if (!rootResult.ok || text(rootResult).length === 0) {
    return unavailableInventory("not_git", basename(requestedRealRoot), "repository_not_git")
  }

  let root: string
  try {
    root = realpathSync(text(rootResult))
  } catch {
    return unavailableInventory("unavailable", basename(requestedRealRoot), "repository_root_unavailable")
  }

  const headResult = git(root, ["rev-parse", "--verify", "HEAD"])
  const treeResult = git(root, ["rev-parse", "HEAD^{tree}"])
  const filesResult = git(root, ["ls-tree", "-rz", "--name-only", "--full-tree", "HEAD"])
  const statusResult = git(root, ["status", "--porcelain=v2", "-z", "--untracked-files=no"])
  if (!headResult.ok || !treeResult.ok || !filesResult.ok || !statusResult.ok) {
    return unavailableInventory("unavailable", basename(root), "repository_snapshot_unavailable")
  }

  const diagnostics: string[] = []
  const allFiles = filesResult.stdout.toString("utf8").split("\0").filter(Boolean).sort()
  let truncated = false
  const candidates = allFiles.filter(candidateArtifact)
  if (candidates.length > MAX_CANDIDATE_REFS) {
    candidates.length = MAX_CANDIDATE_REFS
    truncated = true
    diagnostics.push("candidate_artifact_inventory_truncated")
  }

  const manifests = allFiles.filter((path) => path === "package.json" || path.endsWith("/package.json"))
  if (manifests.length > MAX_PACKAGE_MANIFESTS) {
    manifests.length = MAX_PACKAGE_MANIFESTS
    truncated = true
    diagnostics.push("package_manifest_inventory_truncated")
  }
  const declaredScripts: RepositoryDeclaredScript[] = []
  for (const manifestPath of manifests) {
    if (declaredScripts.length >= MAX_DECLARED_SCRIPTS) {
      truncated = true
      diagnostics.push("declared_script_inventory_truncated")
      break
    }
    const manifestResult = git(root, ["show", `HEAD:${manifestPath}`])
    if (!manifestResult.ok || manifestResult.stdout.byteLength > MAX_MANIFEST_BYTES) continue
    try {
      const manifest = JSON.parse(manifestResult.stdout.toString("utf8")) as { scripts?: unknown }
      if (typeof manifest.scripts !== "object" || manifest.scripts === null) continue
      for (const [name, command] of Object.entries(manifest.scripts)) {
        if (typeof command !== "string" || command.trim() === "") continue
        if (declaredScripts.length >= MAX_DECLARED_SCRIPTS) {
          truncated = true
          diagnostics.push("declared_script_inventory_truncated")
          break
        }
        declaredScripts.push({ manifest_path: manifestPath, name, command })
      }
    } catch {
      diagnostics.push("package_manifest_invalid_json")
    }
  }
  declaredScripts.sort((left, right) =>
    left.manifest_path.localeCompare(right.manifest_path) || left.name.localeCompare(right.name))

  const dirty = statusResult.stdout.byteLength > 0
  if (dirty) diagnostics.push("repository_dirty")
  if (candidates.length > 0 || declaredScripts.length > 0) diagnostics.push("repository_candidates_unmapped")

  const base: Omit<RepositoryInventory, "inventory_digest"> = {
    state: dirty ? "dirty" : "clean",
    repository_label: basename(root),
    head: text(headResult),
    tree: text(treeResult),
    tracked_file_count: allFiles.length,
    candidate_artifact_refs: candidates,
    declared_scripts: declaredScripts,
    truncated,
    diagnostics: [...new Set(diagnostics)].sort(),
  }
  return { ...base, inventory_digest: inventoryDigest(base) }
}
