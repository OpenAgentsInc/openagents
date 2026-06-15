#!/usr/bin/env bun
// OpenAgents release/provenance signer (ed25519). Produces a detached signature
// over an artifact or manifest so clients (Pylon, Autopilot, Psionic) can verify
// it came from OpenAgents infra and FAIL CLOSED otherwise (docs/ota plan §6b).
//
// The private key is loaded, in priority order, from:
//   1) env OPENAGENTS_RELEASE_SIGNING_PRIVATE_JWK_D  (how it arrives in our GCP
//      infra: Cloud Run / CI mounts the GCP Secret Manager secret as this env)
//   2) the local secrets file .secrets/openagents-release-signing.env
//   3) GCP Secret Manager: `gcloud secrets versions access latest
//      --secret=openagents-release-signing-key` (so a signer with gcloud auth
//      needs no local file)
// The key is NEVER printed. Output is a detached signature JSON.
//
// Usage:  bun sign-release.ts <file-to-sign>  > <file>.sig.json
import { createPrivateKey, createPublicKey, sign as edSign } from "node:crypto"
import { readFileSync, existsSync } from "node:fs"
import { execSync } from "node:child_process"

const LOCAL_SECRETS = process.env.OPENAGENTS_RELEASE_SECRETS_PATH
  ?? `${process.env.HOME}/work/.secrets/openagents-release-signing.env`
const GCP_PROJECT = process.env.OPENAGENTS_GCP_PROJECT ?? "openagentsgemini"
const GCP_SECRET = "openagents-release-signing-key"

const parseEnv = (text: string): Record<string, string> =>
  Object.fromEntries(
    text.split("\n").filter(l => l && !l.startsWith("#") && l.includes("="))
      .map(l => { const i = l.indexOf("="); return [l.slice(0, i).trim(), l.slice(i + 1).trim()] }),
  )

function loadKey(): { d: string; kid: string } {
  if (process.env.OPENAGENTS_RELEASE_SIGNING_PRIVATE_JWK_D) {
    return {
      d: process.env.OPENAGENTS_RELEASE_SIGNING_PRIVATE_JWK_D,
      kid: process.env.OPENAGENTS_RELEASE_SIGNING_KID ?? "env",
    }
  }
  if (existsSync(LOCAL_SECRETS)) {
    const e = parseEnv(readFileSync(LOCAL_SECRETS, "utf8"))
    if (e.OPENAGENTS_RELEASE_SIGNING_PRIVATE_JWK_D)
      return { d: e.OPENAGENTS_RELEASE_SIGNING_PRIVATE_JWK_D, kid: e.OPENAGENTS_RELEASE_SIGNING_KID ?? "local" }
  }
  // GCP Secret Manager fallback (our cloud) — never echoes the value.
  const raw = execSync(
    `gcloud secrets versions access latest --secret=${GCP_SECRET} --project=${GCP_PROJECT}`,
    { encoding: "utf8", stdio: ["ignore", "pipe", "ignore"] },
  )
  const e = parseEnv(raw)
  if (!e.OPENAGENTS_RELEASE_SIGNING_PRIVATE_JWK_D) throw new Error("release signing key not found (env / .secrets / GCP Secret Manager)")
  return { d: e.OPENAGENTS_RELEASE_SIGNING_PRIVATE_JWK_D, kid: e.OPENAGENTS_RELEASE_SIGNING_KID ?? "gcp" }
}

const target = process.argv[2]
if (!target || !existsSync(target)) { console.error("usage: sign-release.ts <file-to-sign>"); process.exit(2) }

const { d, kid } = loadKey()
// Reconstruct the ed25519 private key from the JWK seed (d). x is derivable but
// node requires it in the JWK; recompute the public x from the private key.
const privTmp = createPrivateKey({ key: { kty: "OKP", crv: "Ed25519", d, x: "" } as any, format: "jwk" })
  // node validates x; if it rejects empty x, derive via a throwaway with the matching pub.
const pubJwk = createPublicKey(privTmp).export({ format: "jwk" }) as { x: string }
const priv = createPrivateKey({ key: { kty: "OKP", crv: "Ed25519", d, x: pubJwk.x }, format: "jwk" })

const payload = readFileSync(target)
const sig = edSign(null, payload, priv).toString("base64url")
const out = {
  schema: "openagents.release_signature.v1",
  alg: "ed25519",
  kid,
  sha256: execSync(`shasum -a256 "${target}"`).toString().split(" ")[0],
  signedAt: process.env.OPENAGENTS_RELEASE_SIGNED_AT ?? "unset",
  signature: sig,
}
process.stdout.write(JSON.stringify(out, null, 2) + "\n")
