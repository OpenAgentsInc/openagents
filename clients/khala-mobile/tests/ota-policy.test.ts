import { describe, expect, test } from "bun:test"

import appConfig from "../app.json"
import packageJson from "../package.json"
import {
  KHALA_MOBILE_OTA_CONTRACT,
  KHALA_MOBILE_UPDATES_URL,
  forbiddenHostedExpoCommands
} from "../src/config/updates"

describe("Khala mobile OTA and build policy", () => {
  test("embeds the OpenAgents Updates manifest URL", () => {
    expect(appConfig.expo.updates.url).toBe(KHALA_MOBILE_UPDATES_URL)
    expect(KHALA_MOBILE_OTA_CONTRACT.url).toContain("/khala-mobile/manifest")
  })

  test("keeps EAS commands out of package scripts", () => {
    const scripts = Object.values(packageJson.scripts).join("\n")
    for (const command of forbiddenHostedExpoCommands) {
      expect(scripts).not.toContain(command)
    }
  })

  test("publish script no longer points at retired AutopilotRemoteControl", async () => {
    const script = await Bun.file(
      new URL("../../../apps/oa-updates/scripts/publish-ota.sh", import.meta.url),
    ).text()

    expect(script).toContain("clients/khala-mobile")
    expect(script).toContain("OA_MOBILE_PLATFORM")
    expect(script).not.toContain("AutopilotRemoteControl")
  })
})
