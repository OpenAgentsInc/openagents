import { createHash } from "node:crypto"
import { execFileSync } from "node:child_process"
import { mkdtempSync, readFileSync, rmSync } from "node:fs"
import { tmpdir } from "node:os"
import { join } from "node:path"

import { bundledCodex01441ProtocolManifest } from "@openagentsinc/codex-app-server-protocol/parity"
import { bundledCodexExecutableSha256 } from "@openagentsinc/codex-app-server-protocol/compatibility"

const binary = process.env.CODEX_BIN
if (!binary) throw new Error("CODEX_BIN must name the exact packaged Codex executable")
const digest = createHash("sha256").update(readFileSync(binary)).digest("hex")
if (digest !== bundledCodexExecutableSha256) throw new Error(`binary hash mismatch: ${digest}`)

const root = mkdtempSync(join(tmpdir(), "oa-codex-schema-"))
try {
  execFileSync(binary, ["app-server", "generate-json-schema", "--experimental", "--out", root], { stdio: "pipe" })
  const files = { "client-request": "ClientRequest.json", "client-notification": "ClientNotification.json", "server-request": "ServerRequest.json", "server-notification": "ServerNotification.json" } as const
  for (const [direction, file] of Object.entries(files)) {
    const schema = JSON.parse(readFileSync(join(root, file), "utf8")) as { oneOf?: ReadonlyArray<{ properties?: { method?: { enum?: ReadonlyArray<string> } } }> }
    const actual = (schema.oneOf ?? []).flatMap(value => value.properties?.method?.enum?.[0] ?? []).sort()
    const expected = bundledCodex01441ProtocolManifest.members.filter(member => member.direction === direction && member.generation === "upstream-generated").map(member => member.method).sort()
    if (JSON.stringify(actual) !== JSON.stringify(expected)) throw new Error(`${direction} export differs: expected ${expected.length}, received ${actual.length}`)
  }
  const counts = bundledCodex01441ProtocolManifest.counts
  console.log(`Verified exact bundled binary schema manifest ${counts["client-request"]}/${counts["client-notification"]}/${counts["server-request"]}/${counts["server-notification"]}, including reviewed compatibility supplements.`)
} finally {
  rmSync(root, { recursive: true, force: true })
}
