#!/usr/bin/env bun
// OpenAgents release/provenance verifier (ed25519) — the reference fail-closed
// check clients (Pylon, Autopilot, Psionic) embed. Verifies that a file was
// signed by the PINNED OpenAgents public key. Exit 0 = valid; exit 1 = missing/
// invalid/mismatched signature (FAIL CLOSED — never trust on TLS/host alone).
//
// Usage: bun verify-release.ts <file> <file.sig.json>
import { createPublicKey, verify as edVerify } from "node:crypto"
import { readFileSync, existsSync } from "node:fs"
import { createHash } from "node:crypto"

// Pinned public key (committed; clients embed this at build time).
const PUBKEY_PATH = process.env.OPENAGENTS_RELEASE_PUBKEY_PATH
  ?? `${import.meta.dir}/../keys/release-pubkey.json`

const file = process.argv[2]
const sigPath = process.argv[3]
if (!file || !sigPath || !existsSync(file) || !existsSync(sigPath)) {
  console.error("usage: verify-release.ts <file> <file.sig.json>")
  process.exit(1)
}

const pin = JSON.parse(readFileSync(PUBKEY_PATH, "utf8")) as { kid: string; x: string }
const sig = JSON.parse(readFileSync(sigPath, "utf8")) as {
  alg: string; kid: string; sha256?: string; signature: string
}

const fail = (m: string) => { console.error(`REJECTED: ${m}`); process.exit(1) }

if (sig.alg !== "ed25519") fail(`unexpected alg ${sig.alg}`)
if (sig.kid !== pin.kid) fail(`kid ${sig.kid} is not the pinned key ${pin.kid}`)

const payload = readFileSync(file)
if (sig.sha256) {
  const got = createHash("sha256").update(payload).digest("hex")
  if (got !== sig.sha256) fail(`sha256 mismatch (got ${got})`)
}

const pub = createPublicKey({ key: { kty: "OKP", crv: "Ed25519", x: pin.x }, format: "jwk" })
const ok = edVerify(null, payload, pub, Buffer.from(sig.signature, "base64url"))
if (!ok) fail("ed25519 signature does not verify against the pinned OpenAgents key")
console.log(`OK: ${file} is signed by OpenAgents (kid ${pin.kid})`)
process.exit(0)
