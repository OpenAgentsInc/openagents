import { Runtime } from "@openagentsinc/runtime-platform"
/**
 * Gatekeeper release-oracle tests (DMG-1, #8786).
 *
 * Every oracle is a pure interpreter over recorded command output, so the
 * full set is provable WITHOUT the owner's Developer ID identity or notary
 * credentials. The recorded fixtures below reproduce the exact observed
 * shapes from the 2026-07-13 T3 Code incident (unsigned DMG around a
 * notarized app — docs/teardowns/2026-07-13-t3-code-teardown.md, night
 * addendum) on the red side, and real signed/notarized tool output shapes
 * on the green side. A live sweep additionally runs the real commands
 * against an unsigned fixture "dmg"/"app" on macOS and must come back all
 * red — the fail-closed half needs no ceremony to prove.
 */
import { describe, expect, test } from "vite-plus/test"
import { mkdtempSync, writeFileSync } from "node:fs"
import { tmpdir } from "node:os"
import path from "node:path"
import {
  UNSIGNED_DEV_MARKER,
  checkArtifactNotUnsignedDev,
  checkSigningCredentialsPresent,
  gatekeeperAppChecks,
  gatekeeperImageChecks,
  interpretCodesignDeepStrict,
  interpretSpctlAssessment,
  interpretStaplerValidate,
  isUnsignedDevArtifactName,
  missingCredentialNames,
  unsignedDevArtifactName,
} from "../scripts/macos-gatekeeper.ts"
import { runPreflight } from "../scripts/release-preflight.ts"

const APP = "/tmp/OpenAgents.app"
const DMG = "/tmp/OpenAgents-0.1.0-rc.9-arm64.dmg"

const FULL_CREDENTIALS = {
  developerIdApplication: "Developer ID Application: OpenAgents, Inc. (HQWSG26L43)",
  ascApiPrivateKeyPath: "/tmp/key.p8",
  ascApiKeyId: "KEYID",
  ascApiIssuerId: "issuer-uuid",
}

describe("codesign --verify --deep --strict interpreter", () => {
  test("passes only on the exact green shape", () => {
    const check = interpretCodesignDeepStrict(
      {
        exitCode: 0,
        stdout: "",
        stderr: `${APP}: valid on disk\n${APP}: satisfies its Designated Requirement\n`,
      },
      APP,
    )
    expect(check.ok).toBe(true)
    expect(check.id).toBe("gatekeeper_codesign_deep_strict")
  })

  test("fails on the T3 unsigned-container signature ('code object is not signed at all')", () => {
    const check = interpretCodesignDeepStrict(
      { exitCode: 1, stdout: "", stderr: `${DMG}: code object is not signed at all\n` },
      DMG,
    )
    expect(check.ok).toBe(false)
    expect(check.detail).toContain("not signed at all")
  })

  test("fails closed on exit 0 without the success lines, and on a missing binary", () => {
    expect(interpretCodesignDeepStrict({ exitCode: 0, stdout: "", stderr: "" }, APP).ok).toBe(false)
    expect(
      interpretCodesignDeepStrict({ exitCode: null, stdout: "", stderr: "spawn ENOENT" }, APP).ok,
    ).toBe(false)
  })
})

describe("spctl assessment interpreter", () => {
  test("image assessment passes only when accepted AS Notarized Developer ID", () => {
    const green = interpretSpctlAssessment(
      {
        exitCode: 0,
        stdout: "",
        stderr: `${DMG}: accepted\nsource=Notarized Developer ID\norigin=Developer ID Application: OpenAgents, Inc. (HQWSG26L43)\n`,
      },
      "open_image",
      DMG,
    )
    expect(green.ok).toBe(true)
    expect(green.id).toBe("gatekeeper_spctl_image_notarized")
  })

  test("fails on the T3 'no usable signature' rejection", () => {
    const check = interpretSpctlAssessment(
      { exitCode: 3, stdout: "", stderr: `${DMG}: rejected\nsource=no usable signature\n` },
      "open_image",
      DMG,
    )
    expect(check.ok).toBe(false)
  })

  test("fails closed when accepted but NOT notarized (signed-only DMGs still draw a Gatekeeper block)", () => {
    const check = interpretSpctlAssessment(
      { exitCode: 0, stdout: "", stderr: `${DMG}: accepted\nsource=Developer ID\n` },
      "open_image",
      DMG,
    )
    expect(check.ok).toBe(false)
    expect(check.detail).toContain("not as Notarized Developer ID")
  })

  test("exec assessment uses its own oracle id and the same fail-closed rules", () => {
    const green = interpretSpctlAssessment(
      { exitCode: 0, stdout: "", stderr: `${APP}: accepted\nsource=Notarized Developer ID\n` },
      "exec_app",
      APP,
    )
    expect(green.ok).toBe(true)
    expect(green.id).toBe("gatekeeper_spctl_exec_notarized")
    expect(
      interpretSpctlAssessment(
        { exitCode: 3, stdout: "", stderr: `${APP}: rejected\n` },
        "exec_app",
        APP,
      ).ok,
    ).toBe(false)
  })
})

describe("stapler validate interpreter", () => {
  test("passes only on the explicit success line", () => {
    const green = interpretStaplerValidate(
      { exitCode: 0, stdout: `Processing: ${DMG}\nThe validate action worked!\n`, stderr: "" },
      "dmg",
      DMG,
    )
    expect(green.ok).toBe(true)
    expect(green.id).toBe("gatekeeper_stapler_dmg")
  })

  test("fails on an unstapled artifact (exit 65, no ticket)", () => {
    const check = interpretStaplerValidate(
      {
        exitCode: 65,
        stdout: `Processing: ${APP}\n`,
        stderr: `${APP} does not have a ticket stapled to it.\n`,
      },
      "app",
      APP,
    )
    expect(check.ok).toBe(false)
    expect(check.id).toBe("gatekeeper_stapler_app")
  })
})

describe("credentials refusal (no unsigned release fallback)", () => {
  test("complete credentials pass", () => {
    expect(missingCredentialNames(FULL_CREDENTIALS)).toEqual([])
    expect(checkSigningCredentialsPresent(FULL_CREDENTIALS, false).ok).toBe(true)
  })

  test("missing credentials REFUSE by default and name exactly what is missing", () => {
    const none = {
      developerIdApplication: undefined,
      ascApiPrivateKeyPath: undefined,
      ascApiKeyId: undefined,
      ascApiIssuerId: undefined,
    }
    const check = checkSigningCredentialsPresent(none, false)
    expect(check.ok).toBe(false)
    expect(check.detail).toContain("REFUSED")
    expect(check.detail).toContain("no unsigned release fallback")
    for (const name of [
      "OA_DEVELOPER_ID_APPLICATION",
      "ASC_API_PRIVATE_KEY_PATH",
      "ASC_API_KEY_ID",
      "ASC_API_ISSUER_ID",
    ]) expect(check.detail).toContain(name)
    // Partial credentials refuse too — identity without notary access (or
    // vice versa) cannot produce a shippable artifact.
    expect(
      checkSigningCredentialsPresent({ ...FULL_CREDENTIALS, ascApiKeyId: undefined }, false).ok,
    ).toBe(false)
  })

  test("the --allow-unsigned-dev escape valve is explicit and honest about non-releasability", () => {
    const none = {
      developerIdApplication: undefined,
      ascApiPrivateKeyPath: undefined,
      ascApiKeyId: undefined,
      ascApiIssuerId: undefined,
    }
    const check = checkSigningCredentialsPresent(none, true)
    expect(check.ok).toBe(true)
    expect(check.detail).toContain("UNSIGNED-DEV")
    expect(check.detail).toContain("NEVER be published")
  })
})

describe("-UNSIGNED-DEV artifact naming", () => {
  test("rename inserts the marker before the extension, idempotently", () => {
    expect(unsignedDevArtifactName("OpenAgents-0.1.0-rc.9-arm64.dmg")).toBe(
      "OpenAgents-0.1.0-rc.9-arm64-UNSIGNED-DEV.dmg",
    )
    expect(unsignedDevArtifactName("OpenAgents-0.1.0-rc.9-arm64-UNSIGNED-DEV.dmg")).toBe(
      "OpenAgents-0.1.0-rc.9-arm64-UNSIGNED-DEV.dmg",
    )
    expect(unsignedDevArtifactName("no-extension")).toBe(`no-extension${UNSIGNED_DEV_MARKER}`)
    expect(isUnsignedDevArtifactName("OpenAgents-0.1.0-rc.9-arm64-UNSIGNED-DEV.zip")).toBe(true)
    expect(isUnsignedDevArtifactName("OpenAgents-0.1.0-rc.9-arm64.zip")).toBe(false)
  })

  test("an -UNSIGNED-DEV artifact is refused for release UNCONDITIONALLY", () => {
    expect(checkArtifactNotUnsignedDev("/out/make/OpenAgents-0.1.0-rc.9-arm64.dmg").ok).toBe(true)
    const refused = checkArtifactNotUnsignedDev(
      "/out/make/OpenAgents-0.1.0-rc.9-arm64-UNSIGNED-DEV.dmg",
    )
    expect(refused.ok).toBe(false)
    expect(refused.detail).toContain("refused for release unconditionally")
  })

  test("publish-release refuses an -UNSIGNED-DEV artifact before touching any signing key", () => {
    const dir = mkdtempSync(path.join(tmpdir(), "oa-unsigned-dev-"))
    const artifact = path.join(dir, "OpenAgents-9.9.9-arm64-UNSIGNED-DEV.dmg")
    writeFileSync(artifact, "not a real dmg")
    const result = Runtime.spawnSync(
      [
        process.execPath,
        path.join(import.meta.dirname, "..", "scripts", "publish-release.ts"),
        "--channel",
        "rc",
        "--version",
        "9.9.9",
        "--artifact",
        artifact,
        "--dry-run",
      ],
      { stdout: "pipe", stderr: "pipe" },
    )
    expect(result.exitCode).toBe(1)
    expect(result.stderr.toString()).toContain("never releasable")
  })
})

describe("fixture-artifact sweep (fail-closed without owner credentials)", () => {
  test.if(process.platform === "darwin")(
    "an unsigned/unstapled fixture dmg and app fail EVERY Gatekeeper oracle",
    () => {
      const dir = mkdtempSync(path.join(tmpdir(), "oa-gatekeeper-fixture-"))
      const fixtureDmg = path.join(dir, "OpenAgents-0.0.0-arm64.dmg")
      writeFileSync(fixtureDmg, "fixture bytes — not a signed disk image")
      for (const check of gatekeeperImageChecks(fixtureDmg)) {
        expect({ id: check.id, ok: check.ok }).toEqual({ id: check.id, ok: false })
      }
      // An unsigned directory posing as an .app fails all three app oracles.
      const fixtureApp = path.join(dir, "OpenAgents.app")
      for (const check of gatekeeperAppChecks(fixtureApp)) {
        expect({ id: check.id, ok: check.ok }).toEqual({ id: check.id, ok: false })
      }
    },
    30_000,
  )

  test.if(process.platform === "darwin")(
    "runPreflight surfaces the Gatekeeper rows and fails closed on the fixture artifact",
    () => {
      const dir = mkdtempSync(path.join(tmpdir(), "oa-preflight-fixture-"))
      const fixtureDmg = path.join(dir, "OpenAgents-0.0.0-arm64.dmg")
      writeFileSync(fixtureDmg, "fixture bytes")
      const checks = runPreflight({
        channel: "rc",
        latestReleased: null,
        dmgPath: fixtureDmg,
        allowUnsignedDev: true, // isolates the artifact oracles from this env's credentials
      })
      const byId = new Map(checks.map((check) => [check.id, check]))
      expect(byId.get("signing_credentials_present")).toBeDefined()
      expect(byId.get("artifact_not_unsigned_dev")?.ok).toBe(true)
      expect(byId.get("gatekeeper_spctl_image_notarized")?.ok).toBe(false)
      expect(byId.get("gatekeeper_stapler_dmg")?.ok).toBe(false)
      expect(checks.every((check) => check.ok)).toBe(false)
    },
    30_000,
  )
})
