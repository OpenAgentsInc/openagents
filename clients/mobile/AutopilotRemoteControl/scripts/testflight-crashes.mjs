#!/usr/bin/env bun
// Pull TestFlight crash submissions + symbolicated crash logs from App Store
// Connect — no Expo/EAS. Auth: App Store Connect API key (ES256 JWT).
//
// Secrets (workspace `.secrets/appstoreconnect.env`, git-ignored):
//   ASC_API_KEY_ID, ASC_API_ISSUER_ID, ASC_API_PRIVATE_KEY_PATH (.p8)
//
// Usage:
//   ASC_ENV=../../../.secrets/appstoreconnect.env bun scripts/testflight-crashes.mjs [--app <ascAppId>] [--limit N] [--full]
// Default app id is the Autopilot Remote Control ASC app (6779949704).
import { readFileSync } from "node:fs"
import { createSign } from "node:crypto"

const args = process.argv.slice(2)
const opt = (k, d) => { const i = args.indexOf(k); return i >= 0 ? args[i + 1] : d }
const appId = opt("--app", "6779949704")
const limit = opt("--limit", "5")
const full = args.includes("--full")
const envPath = process.env.ASC_ENV || ".secrets/appstoreconnect.env"

const env = Object.fromEntries(
  readFileSync(envPath, "utf8").split("\n").filter((l) => l && !l.startsWith("#"))
    .map((l) => { const i = l.indexOf("="); return [l.slice(0, i), l.slice(i + 1)] }),
)
const b64u = (b) => Buffer.from(b).toString("base64").replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/, "")
const header = b64u(JSON.stringify({ alg: "ES256", kid: env.ASC_API_KEY_ID, typ: "JWT" }))
const now = Math.floor(Date.now() / 1000)
const payload = b64u(JSON.stringify({ iss: env.ASC_API_ISSUER_ID, iat: now, exp: now + 600, aud: "appstoreconnect-v1" }))
const signer = createSign("SHA256"); signer.update(`${header}.${payload}`); signer.end()
// ieee-p1363 => raw r||s (JOSE), NOT DER — required for ES256 JWT.
const jwt = `${header}.${payload}.${b64u(signer.sign({ key: readFileSync(env.ASC_API_PRIVATE_KEY_PATH, "utf8"), dsaEncoding: "ieee-p1363" }))}`
const api = async (path) => {
  const r = await fetch(`https://api.appstoreconnect.apple.com${path}`, { headers: { authorization: `Bearer ${jwt}` } })
  return { status: r.status, body: await r.json().catch(() => ({})) }
}

// Crash submissions are collection-queryable at the APP level (not the build level).
const subs = await api(`/v1/apps/${appId}/betaFeedbackCrashSubmissions?limit=${limit}&sort=-createdDate`)
if (subs.status !== 200) { console.error("query failed", subs.status, JSON.stringify(subs.body)); process.exit(1) }
const rows = subs.body.data ?? []
console.log(`${rows.length} crash submission(s) for app ${appId}:\n`)
for (const s of rows) {
  const a = s.attributes
  console.log(`• ${a.createdDate}  ${a.deviceModel} iOS ${a.osVersion}  "${a.comment ?? ""}"  (${s.id})`)
  const log = await api(`/v1/betaFeedbackCrashSubmissions/${s.id}/crashLog`)
  const text = log.body?.data?.attributes?.logText ?? "(no logText)"
  if (full) {
    console.log("\n" + text + "\n" + "=".repeat(60))
  } else {
    // concise: exception + the app's own frames + the reason/required-key line
    const lines = text.split("\n")
    const exc = lines.find((l) => l.includes("Exception Type"))
    const last = text.indexOf("Last Exception Backtrace")
    const appFrames = lines.filter((l) => /\bAutopilot\b\s+0x|requiredValue|reason:|Terminating|RCTFatal|Reanimated|worklet/.test(l)).slice(0, 12)
    console.log(`   ${exc ?? ""}`)
    appFrames.forEach((l) => console.log("   " + l.trim()))
    console.log()
  }
}
