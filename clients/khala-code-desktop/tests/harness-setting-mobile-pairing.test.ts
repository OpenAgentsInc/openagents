import { afterEach, describe, expect, test } from "bun:test"
import { mkdtemp, readFile, rm } from "node:fs/promises"
import { tmpdir } from "node:os"
import { join } from "node:path"

import {
  readKhalaCodeDesktopKhalaSyncOwnerUserId,
  resolveKhalaCodeDesktopMobilePairingCredentials,
  writeKhalaCodeDesktopOpenAgentsAgentToken,
} from "../src/bun/harness-setting"

const tempDirs: string[] = []

afterEach(async () => {
  await Promise.all(tempDirs.splice(0).map(dir => rm(dir, { force: true, recursive: true })))
})

const tempSettingsPath = async (): Promise<string> => {
  const dir = await mkdtemp(join(tmpdir(), "khala-mobile-pairing-"))
  tempDirs.push(dir)
  return join(dir, "desktop-settings.json")
}

describe("resolveKhalaCodeDesktopMobilePairingCredentials (MC-6)", () => {
  test("is null when neither a token nor an owner user id is stored", async () => {
    const settingsPath = await tempSettingsPath()
    const credentials = await resolveKhalaCodeDesktopMobilePairingCredentials({
      KHALA_CODE_DESKTOP_HARNESS_SETTING_PATH: settingsPath,
    })
    expect(credentials).toBeNull()
  })

  test("is null when a token is persisted but no owner user id is known", async () => {
    const settingsPath = await tempSettingsPath()
    await writeKhalaCodeDesktopOpenAgentsAgentToken(
      "oa_agent_token_without_owner",
      { KHALA_CODE_DESKTOP_HARNESS_SETTING_PATH: settingsPath },
    )
    const credentials = await resolveKhalaCodeDesktopMobilePairingCredentials({
      KHALA_CODE_DESKTOP_HARNESS_SETTING_PATH: settingsPath,
    })
    expect(credentials).toBeNull()
  })

  test("returns both fields once the token and owner user id are persisted together", async () => {
    const settingsPath = await tempSettingsPath()
    await writeKhalaCodeDesktopOpenAgentsAgentToken(
      "oa_agent_paired_token",
      { KHALA_CODE_DESKTOP_HARNESS_SETTING_PATH: settingsPath },
      "user_paired_owner",
    )
    const env = { KHALA_CODE_DESKTOP_HARNESS_SETTING_PATH: settingsPath }
    const credentials = await resolveKhalaCodeDesktopMobilePairingCredentials(env)
    expect(credentials).toEqual({ ownerUserId: "user_paired_owner", token: "oa_agent_paired_token" })

    // Never persist the credential pair in a form that would print the raw
    // token anywhere but the dedicated field.
    const raw = await readFile(settingsPath, "utf8")
    expect(raw).toContain("oa_agent_paired_token")
    expect(JSON.parse(raw).khalaSyncOwnerUserId).toBe("user_paired_owner")
  })

  test("falls back to KHALA_SYNC_CHAT_OWNER_USER_ID when nothing is persisted yet (dev/manual setups)", async () => {
    const settingsPath = await tempSettingsPath()
    const env = {
      KHALA_CODE_DESKTOP_HARNESS_SETTING_PATH: settingsPath,
      KHALA_SYNC_CHAT_OWNER_USER_ID: "user_env_owner",
      OPENAGENTS_AGENT_TOKEN: "oa_agent_env_token",
    }
    expect(await readKhalaCodeDesktopKhalaSyncOwnerUserId(env)).toBe("user_env_owner")
    const credentials = await resolveKhalaCodeDesktopMobilePairingCredentials(env)
    expect(credentials).toEqual({ ownerUserId: "user_env_owner", token: "oa_agent_env_token" })
  })

  test("a persisted owner user id takes priority over the env fallback", async () => {
    const settingsPath = await tempSettingsPath()
    await writeKhalaCodeDesktopOpenAgentsAgentToken(
      "oa_agent_persisted_wins",
      { KHALA_CODE_DESKTOP_HARNESS_SETTING_PATH: settingsPath },
      "user_persisted",
    )
    const env = {
      KHALA_CODE_DESKTOP_HARNESS_SETTING_PATH: settingsPath,
      KHALA_SYNC_CHAT_OWNER_USER_ID: "user_should_be_ignored",
    }
    expect(await readKhalaCodeDesktopKhalaSyncOwnerUserId(env)).toBe("user_persisted")
  })
})
