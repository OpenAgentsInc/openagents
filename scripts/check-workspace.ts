import { glob, readFile } from "node:fs/promises"
import { dirname, relative, resolve } from "node:path"
import { Runtime } from "@openagentsinc/runtime-platform"

import {
  completionTargets,
  componentExclusions,
  componentOverrides,
  fastPolicyTargets,
  rootComponentTargets,
  rootTestTargets,
  type CheckComponent,
  type CheckTarget,
} from "./check-manifest"

interface PackageJson {
  readonly name?: string
  readonly scripts?: Readonly<Record<string, string>>
  readonly workspaces?: readonly string[] | { readonly packages?: readonly string[] }
}

export interface WorkspaceTarget extends CheckTarget {
  readonly directory: string
  readonly component: CheckComponent
}

const normalize = (value: string): string => value.replaceAll("\\", "/").replace(/^\.\//, "")

export const workspacePatterns = (manifest: PackageJson): readonly string[] =>
  Array.isArray(manifest.workspaces) ? manifest.workspaces : manifest.workspaces?.packages ?? []

export const discoverWorkspaceDirectories = async (root: string): Promise<readonly string[]> => {
  const rootManifest = JSON.parse(await readFile(resolve(root, "package.json"), "utf8")) as PackageJson
  const directories = new Set<string>()

  for (const pattern of workspacePatterns(rootManifest)) {
    for await (const match of glob(`${normalize(pattern)}/package.json`, { cwd: root })) {
      directories.add(normalize(dirname(match)))
    }
  }

  return [...directories].sort()
}

export const discoverWorkspaceTargets = async (
  root: string,
  component: CheckComponent,
): Promise<readonly WorkspaceTarget[]> => {
  const excluded = new Set(componentExclusions[component] ?? [])
  const targets: WorkspaceTarget[] = []

  for (const directory of await discoverWorkspaceDirectories(root)) {
    if (excluded.has(directory)) continue
    const manifest = JSON.parse(await readFile(resolve(root, directory, "package.json"), "utf8")) as PackageJson
    const script = componentOverrides[directory]?.[component] ??
      (component === "fmt" ? (manifest.scripts?.fmt ? "fmt" : manifest.scripts?.format ? "format" : undefined) : manifest.scripts?.[component] ? component : undefined)
    if (!script) continue
    targets.push({
      name: `${manifest.name ?? directory}:${script}`,
      command: ["vp", "run", "--filter", `./${directory}`, script],
      directory,
      component,
    })
  }

  return targets
}

const runTarget = async (root: string, target: CheckTarget): Promise<void> => {
  const started = performance.now()
  console.error(`\n[check] ${target.name}`)
  const child = Runtime.spawn(target.command, { cwd: root, stdin: "inherit", stdout: "inherit", stderr: "inherit" })
  const exitCode = await child.exited
  const elapsed = ((performance.now() - started) / 1_000).toFixed(1)
  if (exitCode !== 0) throw new Error(`${target.name} failed with exit ${exitCode} after ${elapsed}s`)
  console.error(`[check] ${target.name} green (${elapsed}s)`)
}

export const runTargets = async (root: string, targets: readonly CheckTarget[]): Promise<void> => {
  for (const target of targets) await runTarget(root, target)
}

export const runComponent = async (root: string, component: CheckComponent): Promise<void> => {
  const rootTargets = component === "test" ? rootTestTargets : rootComponentTargets[component] ?? []
  const workspaceTargets = await discoverWorkspaceTargets(root, component)
  console.error(`[check] ${component}: ${rootTargets.length} root + ${workspaceTargets.length} workspace targets`)
  await runTargets(root, [...rootTargets, ...workspaceTargets])
}

export const runCheck = async (root: string, mode: string): Promise<void> => {
  if (mode === "fast") {
    await runTargets(root, fastPolicyTargets)
    return
  }
  if (["test", "typecheck", "lint", "fmt"].includes(mode)) {
    await runComponent(root, mode as CheckComponent)
    return
  }
  if (mode !== "check") throw new Error(`unknown check mode: ${mode}`)

  await runTargets(root, fastPolicyTargets)
  await runComponent(root, "typecheck")
  await runComponent(root, "lint")
  await runComponent(root, "test")
  await runTargets(root, completionTargets)
}

export const repositoryRootFrom = (cwd: string): string => resolve(cwd)

export const relativeWorkspacePath = (root: string, packageJsonPath: string): string =>
  normalize(relative(root, dirname(packageJsonPath)))
