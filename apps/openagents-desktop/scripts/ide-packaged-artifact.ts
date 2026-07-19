import { createHash } from "node:crypto"
import { execFileSync } from "node:child_process"
import {
  lstatSync,
  readFileSync,
  readdirSync,
  readlinkSync,
} from "node:fs"
import path from "node:path"

import { Schema } from "effect"

import {
  IdeBasicIdeArtifactSchema,
  type IdeBasicIdeArtifact,
} from "../src/ide/basic-ide-acceptance-contract.ts"

const appRoot = path.resolve(import.meta.dirname, "..")
const repositoryRoot = path.resolve(appRoot, "../..")

const packagedOutputRoot = path.join(appRoot, "out")

export const resolvePackagedApp = (): string => {
  const directory = readdirSync(packagedOutputRoot, { withFileTypes: true })
    .find(entry => entry.isDirectory() && entry.name.endsWith("-darwin-arm64"))
  if (directory === undefined) throw new Error("darwin-arm64 packaged output is missing")
  const root = path.join(packagedOutputRoot, directory.name)
  const app = readdirSync(root, { withFileTypes: true })
    .find(entry => entry.isDirectory() && entry.name.endsWith(".app"))
  if (app === undefined) throw new Error("packaged application bundle is missing")
  return path.join(root, app.name)
}

export const resolvePackagedBinary = (appPath = resolvePackagedApp()): string => {
  const macOs = path.join(appPath, "Contents", "MacOS")
  const binary = readdirSync(macOs, { withFileTypes: true }).find(entry => entry.isFile())
  if (binary === undefined) throw new Error("packaged application executable is missing")
  return path.join(macOs, binary.name)
}

type TreeDigest = Readonly<{ sha256: string; files: number; bytes: number }>

export const packagedArtifactTreeDigest = (appPath = resolvePackagedApp()): TreeDigest => {
  const entries: string[] = []
  let files = 0
  let bytes = 0
  const visit = (absolute: string, relative: string): void => {
    const stat = lstatSync(absolute)
    if (stat.isDirectory()) {
      entries.push(`directory\0${relative}\0${stat.mode & 0o777}`)
      for (const child of readdirSync(absolute).sort()) visit(path.join(absolute, child), path.join(relative, child))
      return
    }
    files += 1
    if (stat.isSymbolicLink()) {
      const target = readlinkSync(absolute)
      entries.push(`symlink\0${relative}\0${target}\0${stat.mode & 0o777}`)
      return
    }
    const content = readFileSync(absolute)
    bytes += content.byteLength
    entries.push(`file\0${relative}\0${content.byteLength}\0${stat.mode & 0o777}\0${createHash("sha256").update(content).digest("hex")}`)
  }
  visit(appPath, path.basename(appPath))
  return {
    sha256: createHash("sha256").update(entries.join("\n")).digest("hex"),
    files,
    bytes,
  }
}

const command = (executable: string, args: ReadonlyArray<string>): string => {
  try {
    return execFileSync(executable, [...args], { encoding: "utf8" }).trim()
  } catch {
    return "unavailable"
  }
}

export const packagedArtifactReceipt = (
  candidateCommitSha: string,
  appPath = resolvePackagedApp(),
): IdeBasicIdeArtifact => {
  const digest = packagedArtifactTreeDigest(appPath)
  const packageJson = JSON.parse(readFileSync(path.join(appRoot, "package.json"), "utf8")) as { version?: unknown }
  const electronJson = JSON.parse(readFileSync(path.join(repositoryRoot, "node_modules", "electron", "package.json"), "utf8")) as { version?: unknown }
  return Schema.decodeUnknownSync(IdeBasicIdeArtifactSchema)({
    target: "darwin-arm64",
    candidateCommitSha,
    packageVersion: typeof packageJson.version === "string" ? packageJson.version : "unknown",
    electronVersion: typeof electronJson.version === "string" ? electronJson.version : "unknown",
    artifactRef: path.relative(repositoryRoot, appPath).split(path.sep).join("/"),
    artifactTreeSha256: digest.sha256,
    artifactFiles: digest.files,
    artifactBytes: digest.bytes,
    platform: process.platform,
    architecture: process.arch,
    osRelease: `${command("/usr/bin/sw_vers", ["-productVersion"])} (${command("/usr/bin/uname", ["-r"])})`,
    hardwareClass: command("/usr/sbin/sysctl", ["-n", "machdep.cpu.brand_string"]),
    packaged: true,
  })
}
