// Installs the bundled Khala skills into the user-scope agent skills root
// (`~/.agents/skills/<name>/SKILL.md`) so the Codex harness — and any other
// agent that reads the shared `.agents/skills` convention — discovers them
// without manual setup. Default-on; disable with
// KHALA_CODE_DESKTOP_BUNDLED_SKILLS=0.
//
// Overwrite policy: a file we previously wrote carries a managed-by marker
// and is upgraded in place when the bundled content changes. A file without
// the marker belongs to the user and is never touched.
import { mkdir, readFile, writeFile } from "node:fs/promises"
import { resolve } from "node:path"

import { khalaCodeDesktopBundledSkillContents } from "./khala-bundled-skill-content.generated.js"

export const KHALA_CODE_DESKTOP_BUNDLED_SKILLS_MANAGED_MARKER = "managed-by: khala-code"

export type KhalaCodeDesktopBundledSkillInstallStatus =
  | "disabled"
  | "installed"
  | "no_home"
  | "unchanged"
  | "updated"
  | "user_owned"
  | "write_failed"

export type KhalaCodeDesktopBundledSkillInstallResult = Readonly<{
  name: string
  path: string | null
  status: KhalaCodeDesktopBundledSkillInstallStatus
}>

export const khalaCodeDesktopBundledSkillsEnabled = (
  env: Readonly<Record<string, string | undefined>>,
): boolean => {
  const raw = env.KHALA_CODE_DESKTOP_BUNDLED_SKILLS?.trim().toLowerCase()
  return raw !== "0" && raw !== "false" && raw !== "off"
}

export const khalaCodeDesktopUserSkillsRoot = (homeDir: string): string =>
  resolve(homeDir, ".agents", "skills")

const readExisting = async (path: string): Promise<string | null> => {
  try {
    return await readFile(path, "utf8")
  } catch {
    return null
  }
}

export const ensureKhalaCodeDesktopBundledSkillsInstalled = async (options: {
  readonly env: Readonly<Record<string, string | undefined>>
  readonly homeDir?: string
}): Promise<readonly KhalaCodeDesktopBundledSkillInstallResult[]> => {
  const { env } = options
  if (!khalaCodeDesktopBundledSkillsEnabled(env)) {
    return khalaCodeDesktopBundledSkillContents.map(skill => ({
      name: skill.name,
      path: null,
      status: "disabled" as const,
    }))
  }

  const homeDir = options.homeDir?.trim() || env.HOME?.trim()
  if (homeDir === undefined || homeDir.length === 0) {
    return khalaCodeDesktopBundledSkillContents.map(skill => ({
      name: skill.name,
      path: null,
      status: "no_home" as const,
    }))
  }

  const skillsRoot = khalaCodeDesktopUserSkillsRoot(homeDir)
  const results: KhalaCodeDesktopBundledSkillInstallResult[] = []
  for (const skill of khalaCodeDesktopBundledSkillContents) {
    const skillDir = resolve(skillsRoot, skill.name)
    const skillPath = resolve(skillDir, "SKILL.md")
    const existing = await readExisting(skillPath)

    if (existing === skill.markdown) {
      results.push({ name: skill.name, path: skillPath, status: "unchanged" })
      continue
    }
    if (
      existing !== null &&
      !existing.includes(KHALA_CODE_DESKTOP_BUNDLED_SKILLS_MANAGED_MARKER)
    ) {
      results.push({ name: skill.name, path: skillPath, status: "user_owned" })
      continue
    }

    try {
      await mkdir(skillDir, { recursive: true })
      await writeFile(skillPath, skill.markdown, "utf8")
      results.push({
        name: skill.name,
        path: skillPath,
        status: existing === null ? "installed" : "updated",
      })
    } catch {
      results.push({ name: skill.name, path: skillPath, status: "write_failed" })
    }
  }
  return results
}
