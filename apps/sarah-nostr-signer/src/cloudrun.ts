import { Runtime } from "@openagentsinc/runtime-platform"
import {
  SARAH_NOSTR_IDENTITY_SECRET_ENV,
} from "@openagentsinc/sarah/nostr-identity"
import { Config, Effect, Redacted } from "effect"

import {
  createStableSarahSigner,
  makeSarahNostrSignerHandler,
  parseCommunityAllowlist,
} from "./service.ts"

const config = Effect.runSync(
  Config.all({
    communitiesRaw: Config.string("SARAH_NOSTR_SIGNER_COMMUNITIES_JSON"),
    expectedPubkey: Config.string("SARAH_NOSTR_EXPECTED_PUBKEY"),
    relayUrl: Config.string("SARAH_NOSTR_RELAY_URL"),
    secretMaterial: Config.redacted(SARAH_NOSTR_IDENTITY_SECRET_ENV),
  }),
)
const communitiesRaw = config.communitiesRaw
const allowlist = parseCommunityAllowlist(communitiesRaw)

let secretMaterial: string | undefined = Redacted.value(config.secretMaterial)
delete process.env[SARAH_NOSTR_IDENTITY_SECRET_ENV]
if (secretMaterial === undefined || secretMaterial.trim() === "") {
  throw new Error("missing Sarah identity secret")
}
const signer = createStableSarahSigner({
  secretMaterial,
  expectedPubkey: config.expectedPubkey,
})
secretMaterial = undefined

const handle = makeSarahNostrSignerHandler({
  signer,
  allowlist,
  relayUrl: config.relayUrl,
})
const server = Runtime.serve({
  port: Number(process.env["PORT"] ?? 8080),
  async fetch(request) {
    try {
      return await handle(request)
    } catch {
      return Response.json(
        { error: "signing_unavailable" },
        { status: 503, headers: { "cache-control": "no-store" } },
      )
    }
  },
})

console.log(
  JSON.stringify({ event: "sarah_nostr_signer_started", port: server.port }),
)
