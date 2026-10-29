// Offline release checks for the actual project work seeds, not a generated snapshot.
import { expect, test } from "bun:test"
import { readFileSync, readdirSync } from "node:fs"
import { join } from "node:path"
import { checkWork, seedItems, topo } from "./model"
const root = join(import.meta.dir, "../../..")
const dir = join(root, "apps/openagents-web/projects/coax/work")
const seeds = readdirSync(dir).filter(f => f.endsWith(".json")).sort().map(f => JSON.parse(readFileSync(join(dir, f), "utf8")))
const seed = seeds.find(s => s.initiative.slug === "coax-nostr")!
const items = seeds.flatMap(s => seedItems(s))
const tasks = new Map(seed.tasks.map((t: any) => [t.slug, t])) as Map<string, any>
const reaches = (from: string, target: string, seen = new Set<string>()): boolean => {
  if (seen.has(from)) return false
  seen.add(from)
  return (tasks.get(from)?.blocked_by ?? []).some((s: string) => s === target || reaches(s, target, seen))
}
const stable = "coax-fix-compound-assign-view coax-intrinsic-value-barrier coax-intrinsic-wrapping-ops coax-intrinsic-mul-wide coax-debug-only-fields crypto-bytes crypto-vectors-pinned crypto-vector-runner crypto-sha256 crypto-hmac-hkdf crypto-pbkdf2 crypto-chacha20 nostr-hex-base64 secp-field-repr secp-field-mul secp-field-inv-sqrt secp-scalar secp-libsecp-harness secp-group secp-mult-verify schnorr-verify verify-one-real-event secp-mult-base secp-mult-const schnorr-sign crypto-ecdh-xonly crypto-ct-timing-harness nip44-v2 nip59-seal-wrap open-sealed-deck-native nostr-json nostr-event nostr-filter nostr-bech32 nostr-jcs nostr-envelope host-random-import host-socket-pipe host-storage-pipe nostr-relay-state-machine nostr-nip42 nostr-keys relay-in-page unlock-opener-coax unlock-gate-coax coax-nostr-cli invariant-coax-only publish-project-events project-page-live claim-from-page secp-glv ios-coax-nostr-native ios-key-custody nip46-signer nip46-client-page coax-qr-encoder ios-scan-to-sign-in server-nip98-session web-sign-in-page phone-sign-in-e2e project-page-verify".split(" ")
test("published task identities survive; all project seeds form one bounded DAG", () => {
  expect(() => checkWork(items)).not.toThrow()
  const ordered = topo(items), position = new Map(ordered.map((it, i) => [it.slug, i]))
  for (const slug of stable) expect(tasks.has(slug)).toBe(true)
  for (const task of seed.tasks) {
    expect(task.files.length).toBeGreaterThan(0)
    expect(task.acceptance.length).toBeGreaterThan(0)
    for (const blocker of task.blocked_by ?? []) expect(position.get(blocker)!).toBeLessThan(position.get(task.slug)!)
    const inverse = seed.tasks.filter((t: any) => (t.blocked_by ?? []).includes(task.slug)).map((t: any) => t.slug).sort()
    expect([...task.blocks].sort()).toEqual(inverse)
  }
  const capacity = Number(readFileSync(join(root, "apps/openagents-web/project.coax"), "utf8").match(/MAX_ITEMS :: (\d+);/)![1])
  expect(items.length).toBeLessThanOrEqual(capacity)
})
test("verification ships before signing; authoritative live rows require local verification", () => {
  expect(reaches("nostr-event", "schnorr-verify")).toBe(true)
  expect(reaches("nostr-event", "schnorr-sign")).toBe(false)
  expect(reaches("nostr-event-sign", "schnorr-sign")).toBe(true)
  expect(reaches("project-page-verify", "nostr-event")).toBe(true)
  expect(reaches("project-page-verify", "schnorr-sign")).toBe(false)
  expect(reaches("claim-from-page", "coord-fenced-claims")).toBe(true)
})
test("production secrets and retirement wait for independent evidence and recoverable cutover", () => {
  for (const consumer of ["secp-mult-base", "secp-mult-const", "crypto-secret-codegen"]) {
    expect(reaches(consumer, "crypto-secret-aggregate-closure")).toBe(true)
  }
  for (const gate of ["crypto-independent-review", "crypto-target-conformance", "crypto-secure-zero", "crypto-secret-codegen"]) {
    expect(reaches("nostr-production-cutover", gate)).toBe(true)
  }
  expect(reaches("invariant-coax-only", "nostr-production-cutover")).toBe(true)
  expect(reaches("unlock-gate-coax", "unlock-gate-format")).toBe(true)
  expect(reaches("phone-sign-in-e2e", "server-session-lifecycle")).toBe(true)
})
test("full integration includes profiles, grants, identity, channels, private data and optional backup", () => {
  for (const slug of ["nostr-support-profiles", "nostr-proto-adapters", "nostr-event-store", "nostr-multirelay", "host-grants", "reach-freshness", "host-codex-e2e", "block-owner-admission", "fi-jwks-policy", "fi-jws-verifier", "block-channel-policy", "block-artifact-cas", "agent-private-data", "nip49-backup"]) expect(tasks.has(slug)).toBe(true)
  for (const [slug, commit] of [["coax-fix-compound-assign-view", "e2f8432ea2"], ["host-socket-pipe", "df008f96a4"], ["coax-intrinsic-value-barrier", "b9a5f216f0"], ["nostr-json", "8f23a77f35"]]) {
    expect(tasks.get(slug).status).toBe("resolved")
    expect(tasks.get(slug).body).toContain(commit)
  }
})

test("decoder mutation coverage remains a separate gate from structural policy", () => {
  for (const slug of ["nostr-event", "nostr-jcs", "fi-jws-verifier"]) expect(reaches(slug, "nostr-json-parser-checks")).toBe(true)
  expect(tasks.get("nostr-json").status).toBe("resolved")
})

test("FI selects one asymmetric profile before crypto and actual transport integration", () => {
  const task = tasks.get("fi-jws-verifier")
  expect(task.body).toContain("RS256 only")
  expect(task.body).toContain("fi-rs256-policy-v1")
  expect(task.body).toContain("grants no identity authority")
  expect(task.acceptance.join(" ")).toContain("Full AssertionPolicyId")
  expect(task.acceptance.join(" ")).toContain("Real key-source adapters")
  expect(reaches("fi-jws-verifier", "fi-jwks-policy")).toBe(true)
})
