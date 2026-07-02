import { describe, expect, test } from "bun:test"
import { mkdir, mkdtemp, readFile, writeFile } from "node:fs/promises"
import { tmpdir } from "node:os"
import { join, resolve } from "node:path"

import { khalaCodeDesktopBundledSkillContents } from "../src/bun/khala-bundled-skill-content.generated"
import {
  ensureKhalaCodeDesktopBundledSkillsInstalled,
  KHALA_CODE_DESKTOP_BUNDLED_SKILLS_MANAGED_MARKER,
  khalaCodeDesktopBundledSkillsEnabled,
  khalaCodeDesktopUserSkillsRoot,
} from "../src/bun/khala-bundled-skills"

const repoRoot = resolve(import.meta.dir, "../../..")

const khalaFleetSkill = khalaCodeDesktopBundledSkillContents.find(
  skill => skill.name === "khala-fleet",
)

const tempHome = async (): Promise<string> =>
  mkdtemp(join(tmpdir(), "khala-bundled-skills-"))

describe("bundled skill content", () => {
  test("khala-fleet skill is bundled", () => {
    expect(khalaFleetSkill).toBeDefined()
  })

  test("generated content matches the canonical checked-in SKILL.md", async () => {
    for (const skill of khalaCodeDesktopBundledSkillContents) {
      const canonicalPath = resolve(repoRoot, ".agents/skills", skill.name, "SKILL.md")
      const canonical = await readFile(canonicalPath, "utf8")
      expect(skill.markdown).toBe(canonical)
    }
  })

  test("every bundled skill carries frontmatter name/description and the managed marker", () => {
    for (const skill of khalaCodeDesktopBundledSkillContents) {
      expect(skill.markdown.startsWith("---\n")).toBe(true)
      const frontmatterEnd = skill.markdown.indexOf("\n---", 4)
      expect(frontmatterEnd).toBeGreaterThan(0)
      const frontmatter = skill.markdown.slice(4, frontmatterEnd)
      expect(frontmatter).toContain(`name: ${skill.name}`)
      expect(frontmatter).toContain("description:")
      expect(skill.markdown).toContain(KHALA_CODE_DESKTOP_BUNDLED_SKILLS_MANAGED_MARKER)
    }
  })
})

describe("khalaCodeDesktopBundledSkillsEnabled", () => {
  test("defaults to enabled", () => {
    expect(khalaCodeDesktopBundledSkillsEnabled({})).toBe(true)
    expect(khalaCodeDesktopBundledSkillsEnabled({ KHALA_CODE_DESKTOP_BUNDLED_SKILLS: "1" })).toBe(true)
  })

  test("0/false/off disable", () => {
    for (const raw of ["0", "false", "off", " FALSE "]) {
      expect(khalaCodeDesktopBundledSkillsEnabled({ KHALA_CODE_DESKTOP_BUNDLED_SKILLS: raw })).toBe(false)
    }
  })
})

describe("ensureKhalaCodeDesktopBundledSkillsInstalled", () => {
  test("installs into ~/.agents/skills/<name>/SKILL.md when absent", async () => {
    const home = await tempHome()
    const results = await ensureKhalaCodeDesktopBundledSkillsInstalled({
      env: { HOME: home },
    })
    const fleet = results.find(result => result.name === "khala-fleet")
    expect(fleet?.status).toBe("installed")
    const installedPath = join(
      khalaCodeDesktopUserSkillsRoot(home),
      "khala-fleet",
      "SKILL.md",
    )
    expect(fleet?.path).toBe(installedPath)
    const written = await readFile(installedPath, "utf8")
    expect(written).toBe(khalaFleetSkill?.markdown ?? "")
  })

  test("reports unchanged on a second run", async () => {
    const home = await tempHome()
    await ensureKhalaCodeDesktopBundledSkillsInstalled({ env: { HOME: home } })
    const results = await ensureKhalaCodeDesktopBundledSkillsInstalled({
      env: { HOME: home },
    })
    expect(results.every(result => result.status === "unchanged")).toBe(true)
  })

  test("upgrades a stale managed copy in place", async () => {
    const home = await tempHome()
    const skillDir = join(khalaCodeDesktopUserSkillsRoot(home), "khala-fleet")
    await mkdir(skillDir, { recursive: true })
    await writeFile(
      join(skillDir, "SKILL.md"),
      `---\nname: khala-fleet\ndescription: stale\n---\n\n<!-- ${KHALA_CODE_DESKTOP_BUNDLED_SKILLS_MANAGED_MARKER} -->\nold body\n`,
      "utf8",
    )
    const results = await ensureKhalaCodeDesktopBundledSkillsInstalled({
      env: { HOME: home },
    })
    const fleet = results.find(result => result.name === "khala-fleet")
    expect(fleet?.status).toBe("updated")
    const written = await readFile(join(skillDir, "SKILL.md"), "utf8")
    expect(written).toBe(khalaFleetSkill?.markdown ?? "")
  })

  test("never touches a user-owned file without the managed marker", async () => {
    const home = await tempHome()
    const skillDir = join(khalaCodeDesktopUserSkillsRoot(home), "khala-fleet")
    await mkdir(skillDir, { recursive: true })
    const userContent = "---\nname: khala-fleet\ndescription: mine\n---\n\nuser-owned\n"
    await writeFile(join(skillDir, "SKILL.md"), userContent, "utf8")
    const results = await ensureKhalaCodeDesktopBundledSkillsInstalled({
      env: { HOME: home },
    })
    const fleet = results.find(result => result.name === "khala-fleet")
    expect(fleet?.status).toBe("user_owned")
    const untouched = await readFile(join(skillDir, "SKILL.md"), "utf8")
    expect(untouched).toBe(userContent)
  })

  test("disabled flag installs nothing", async () => {
    const home = await tempHome()
    const results = await ensureKhalaCodeDesktopBundledSkillsInstalled({
      env: { HOME: home, KHALA_CODE_DESKTOP_BUNDLED_SKILLS: "0" },
    })
    expect(results.every(result => result.status === "disabled")).toBe(true)
    const skillFile = Bun.file(
      join(khalaCodeDesktopUserSkillsRoot(home), "khala-fleet", "SKILL.md"),
    )
    expect(await skillFile.exists()).toBe(false)
  })

  test("missing HOME reports no_home", async () => {
    const results = await ensureKhalaCodeDesktopBundledSkillsInstalled({ env: {} })
    expect(results.every(result => result.status === "no_home")).toBe(true)
  })
})
