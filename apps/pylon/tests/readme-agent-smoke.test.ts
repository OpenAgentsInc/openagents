import { describe, expect, test } from "bun:test"
import { readFileSync } from "node:fs"
import { join } from "node:path"

import { PYLON_VERSION } from "../src/version"

const readme = readFileSync(join(import.meta.dir, "../README.md"), "utf8")

describe("Pylon README agent smoke path", () => {
  const marker = "### Agent smoke path"
  const section = readme.slice(readme.indexOf(marker))

  test("keeps the stable package copy on the current version", () => {
    expect(readme).toContain(`are \`${PYLON_VERSION}\``)
    expect(readme).toContain(`@openagentsinc/pylon@${PYLON_VERSION}`)
  })

  test("documents the fast Pylon and Tassadar smoke commands", () => {
    expect(section).toContain("pylon --version")
    expect(section).toContain("pylon help --json")
    expect(section).toContain("pylon bootstrap --json")
    expect(section).toContain("pylon status --json")
    expect(section).toContain('POST "$PYLON_OPENAGENTS_BASE_URL/api/agents/register"')
    expect(section).toContain('"$PYLON_OPENAGENTS_BASE_URL/api/agents/me"')
    expect(section).toContain("pylon presence register --base-url")
    expect(section).toContain("pylon presence heartbeat --base-url")
    expect(section).toContain("pylon training status --base-url")
    expect(section).toContain("pylon training preflight --base-url")
    expect(section).toContain("pylon wallet register-payout-target --kind spark-address")
    expect(section).toContain("pylon training claim --base-url")
    expect(section).toContain("--lease-seconds 300")
    expect(section).toContain("pylon training submit-trace --base-url")
    expect(section).toContain("pylon training validate --base-url")
    expect(section).toContain("--auto --max-iterations 1")
  })

  test("keeps smoke-path reporting bounded to public-safe outcomes", () => {
    expect(section).toContain("do not post agent tokens")
    expect(section).toContain("Report public-safe refs")
    expect(section).toMatch(/not\s+an earning or settlement claim/)
    expect(section).toContain("dereferenceable settlement receipt")
  })
})
