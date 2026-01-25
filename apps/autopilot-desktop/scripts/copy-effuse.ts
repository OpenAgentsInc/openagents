#!/usr/bin/env bun

import { execSync } from "child_process"
import { readdir, readFile, stat } from "fs/promises"
import { join, relative } from "path"

const ROOT_DIR = process.cwd()

const PROFILE_NAMES = ["adjutant", "effuse"] as const
type ProfileName = (typeof PROFILE_NAMES)[number]

/**
 * Adjutant Flow Relevant Files and Folders
 */
const ADJUTANT_PATHS = [
  // Backend (Rust)
  "src-tauri/src/agent/adjutant",
  "src-tauri/src/agent/acp_agent.rs",
  "src-tauri/src/agent/mod.rs",
  "src-tauri/src/agent/trait_def.rs",
  "src-tauri/src/agent/unified.rs",
  "src-tauri/src/agent/manager.rs",
  "src-tauri/src/agent/commands.rs",
  "src-tauri/src/agent/resolver.rs",
  "src-tauri/src/ai_server",
  "src-tauri/Cargo.toml",

  // Sidecar (AI Server)
  "ai-server/server.ts",
  "ai-server/package.json",

  // Frontend (TypeScript)
  "src/agent/adjutant.ts",
  "src/agent/base.ts",
  "src/agent/registry.ts",
  "src/agent/types.ts",
  "src/components/unified-stream",

  // Documentation
  "docs/adjutant-agent.md",
  "docs/agent-architecture-codex-gemini.md",

  // Scripts
  "scripts/test-adjutant-planning.ts",
  "scripts/copy-effuse.ts",

  // Root Configs
  "package.json",
]

/**
 * Effuse/General Source Snapshot
 */
const EFFUSE_PATHS = [
  "src",
  "ai-server",
  "scripts",
  "package.json",
  "src-tauri/Cargo.toml",
  "README.md",
]

const PROFILES: Record<ProfileName, readonly string[]> = {
  adjutant: ADJUTANT_PATHS,
  effuse: EFFUSE_PATHS,
}

const IGNORE_PATTERNS = [
  "node_modules",
  ".git",
  ".DS_Store",
  "target",
  "dist",
  "bun.lock",
  "bun.lockb",
  "debug-storybook.log",
  ".local",
  ".cache",
]

const shouldIgnore = (path: string): boolean => {
  const parts = path.split(/[/\\\\]/)
  return IGNORE_PATTERNS.some((pattern) => {
    if (pattern.includes("/")) {
      return path === pattern || path.startsWith(`${pattern}/`)
    }
    return parts.includes(pattern)
  })
}

const isBinary = (fileName: string): boolean => {
  const binaryExtensions = [
    ".png", ".jpg", ".jpeg", ".gif", ".ico", ".pdf", ".zip", ".tar", ".gz",
    ".exe", ".dll", ".so", ".dylib", ".bin", ".woff", ".woff2", ".ttf", ".eot"
  ]
  return binaryExtensions.some(ext => fileName.toLowerCase().endsWith(ext))
}

const normalizeProfile = (value: string): ProfileName | null => {
  const normalized = value.trim().toLowerCase()
  return PROFILE_NAMES.includes(normalized as ProfileName)
    ? (normalized as ProfileName)
    : null
}

const printUsage = () => {
  console.log("Usage: bun run scripts/copy-effuse.ts [--profile=adjutant|effuse]")
  console.log("  --profile=adjutant  Copy Adjutant planning flow files (default)")
  console.log("  --profile=effuse    Copy general Effuse/source snapshot")
  console.log("  --adjutant          Alias for --profile=adjutant")
  console.log("  --effuse            Alias for --profile=effuse")
  console.log("  --list              Show available profiles")
}

const printProfiles = () => {
  console.log("Available profiles:")
  PROFILE_NAMES.forEach((name) => console.log(`- ${name}`))
}

const parseProfile = (args: string[]): ProfileName => {
  if (args.includes("--help") || args.includes("-h")) {
    printUsage()
    process.exit(0)
  }

  if (args.includes("--list")) {
    printProfiles()
    process.exit(0)
  }

  const profileArg = args.find((arg) => arg.startsWith("--profile="))
  if (profileArg) {
    const value = profileArg.split("=").slice(1).join("=")
    const normalized = normalizeProfile(value)
    if (normalized) {
      return normalized
    }
    throw new Error(`Unknown profile: ${value}`)
  }

  if (args.includes("--effuse")) {
    return "effuse"
  }

  if (args.includes("--adjutant")) {
    return "adjutant"
  }

  return "adjutant"
}

const getAllFiles = async (
  dir: string,
  baseDir: string = ROOT_DIR
): Promise<string[]> => {
  const files: string[] = []

  try {
    const entries = await readdir(dir, { withFileTypes: true })

    for (const entry of entries) {
      const fullPath = join(dir, entry.name)
      const relativePath = relative(baseDir, fullPath)

      if (shouldIgnore(relativePath)) {
        continue
      }

      if (entry.isDirectory()) {
        const subFiles = await getAllFiles(fullPath, baseDir)
        files.push(...subFiles)
      } else if (entry.isFile()) {
        if (!isBinary(entry.name)) {
          files.push(fullPath)
        }
      }
    }
  } catch (error) {
    // Silence errors for missing files in recursion
  }

  return files
}

const gatherFiles = async (paths: readonly string[]): Promise<string[]> => {
  const allFiles: string[] = []

  for (const path of paths) {
    const fullPath = join(ROOT_DIR, path)
    try {
      const stats = await stat(fullPath)
      if (stats.isDirectory()) {
        const files = await getAllFiles(fullPath)
        allFiles.push(...files)
      } else if (stats.isFile()) {
        if (!isBinary(path)) {
          allFiles.push(fullPath)
        }
      }
    } catch {
      // Path doesn't exist, skip
    }
  }

  return [...new Set(allFiles)].sort()
}

const readFileContent = async (filePath: string): Promise<string> => {
  try {
    return await readFile(filePath, "utf-8")
  } catch (error) {
    return `[Error reading file: ${error}]`
  }
}

const buildOutput = async (
  files: string[]
): Promise<{
  output: string
  totalBytes: number
}> => {
  const output: string[] = []
  let totalBytes = 0

  for (const filePath of files) {
    const relativePath = relative(ROOT_DIR, filePath)
    const content = await readFileContent(filePath)

    const header = `--- FILE: ${relativePath} ---\n`
    const suffix = "\n\n"

    output.push(header, content, suffix)
    totalBytes += Buffer.byteLength(header + content + suffix, "utf-8")
  }

  return {
    output: output.join(""),
    totalBytes,
  }
}

const copyToClipboard = (value: string) => {
  if (process.platform === "darwin") {
    execSync("pbcopy", { input: value, encoding: "utf-8" })
  } else if (process.platform === "linux") {
    execSync("xclip -selection clipboard", { input: value, encoding: "utf-8" })
  } else {
    throw new Error(`Unsupported platform: ${process.platform}`)
  }
}

async function main() {
  const profile = parseProfile(process.argv.slice(2))
  const paths = PROFILES[profile]

  console.log(`Collecting files for profile: ${profile}...`)

  const files = await gatherFiles(paths)
  console.log(`\nProcessing ${files.length} files...`)

  const { output, totalBytes } = await buildOutput(files)

  try {
    copyToClipboard(output)
    console.log(`\n✓ Successfully copied ${files.length} ${profile} files to clipboard!`)
    console.log(`Total size: ${(totalBytes / 1024).toFixed(2)} KB`)
  } catch (error) {
    console.error("Error copying to clipboard:", error)
    process.exit(1)
  }
}

main().catch((error) => {
  console.error("Error:", error)
  process.exit(1)
})
