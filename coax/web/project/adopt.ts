// Gives the site a hand-made project's key, so a member's drop on its page can grow it
// (rc-later-drops; coax/web/api/migrations/0041_project_grow.sql, docs/projects/new-project.md Part 2):
//
//   bun coax/web/project/adopt.ts SLUG [--api https://staging.openagents.com]
//
// It reads apps/openagents-web/projects/SLUG/project.json (name, owner_pubkey, maintainers) and the key
// at ~/work/.secrets/openagents-project-SLUG.key. The call is NIP-98 signed with that same key, so only
// whoever holds a project's key can hand it over; the key crosses TLS once, and the API seals it
// (AES-256-GCM under OA_PROJECT_KEK) and keeps only the sealed bytes with the members who may drop on
// the project. Nothing here prints the key.
import { getPublicKey } from "nostr-effect/pure"
import { hexToBytes } from "@noble/hashes/utils"
import { createHash } from "node:crypto"
import { join } from "node:path"
import { Signer } from "./relay"

const args = process.argv.slice(2)
const slug = args.find((a, k) => !a.startsWith("--") && !args[k - 1]?.startsWith("--")) ?? ""
const flag = (name: string, fallback: string) => { const k = args.indexOf(`--${name}`); return k >= 0 ? args[k + 1] : fallback }
if (!/^[a-z0-9-]{1,80}$/.test(slug)) { console.error("usage: bun adopt.ts SLUG [--api URL] [--key FILE]"); process.exit(2) }
const api = flag("api", "https://openagents.com")
const project = await Bun.file(join(import.meta.dir, "../../../apps/openagents-web/projects", slug, "project.json")).json()
const hex = (await Bun.file(flag("key", `${process.env.HOME}/work/.secrets/openagents-project-${slug}.key`)).text()).trim()
const key = hexToBytes(hex)
if (getPublicKey(key) !== project.pubkey) { console.error(`the key file is not ${slug}'s key`); process.exit(1) }
const members = [project.owner_pubkey, ...(project.maintainers ?? [])].filter((k: string) => /^[0-9a-f]{64}$/.test(k ?? ""))
// written as the API reads it (drops.c's str_of: a '": "' after each name)
const body = `{"slug": ${JSON.stringify(slug)}, "name": ${JSON.stringify(project.name)}, "key": ${JSON.stringify(hex)}, "members": ${JSON.stringify(members)}}`
const url = `${api}/api/drops/adopt`
const auth = new Signer(key).sign({ kind: 27235, content: "", tags: [["u", url], ["method", "POST"],
  ["payload", createHash("sha256").update(body).digest("hex")]] })
const r = await fetch(url, { method: "POST", headers: { "Content-Type": "application/json", Authorization: `Nostr ${btoa(JSON.stringify(auth))}` }, body })
console.log(r.status, await r.text())
process.exit(r.ok ? 0 : 1)
