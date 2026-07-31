import {
  assertSarahNostrPublicSafe,
  createSealedSarahNostrSigner,
  parseSecretMaterial,
  publicKeyFromSecret,
  type SarahNostrSigner,
} from "@openagentsinc/sarah/nostr-identity"
import {
  SARAH_GROUP_TEXT_KIND,
  SARAH_PRESENCE_KIND,
  SARAH_SIGNING_REQUEST_SCHEMA,
  SARAH_SIGNING_RESPONSE_SCHEMA,
  SarahSignerTemplateSchema,
  SarahSigningRequestSchema,
  buildSarahSigningTemplate,
  type SarahSignerTemplate,
} from "@openagentsinc/sarah/nostr-signing-boundary"
import { Effect, Schema as S } from "effect"

export {
  SARAH_GROUP_TEXT_KIND,
  SARAH_PRESENCE_KIND,
  SARAH_SIGNING_REQUEST_SCHEMA,
  buildSarahSigningTemplate as buildSigningTemplate,
  type SarahSignerTemplate,
}

export const createStableSarahSigner = (input: Readonly<{
  secretMaterial: string
  expectedPubkey: string
}>): SarahNostrSigner => {
  const secretKey = parseSecretMaterial(input.secretMaterial)
  try {
    if (
      !/^[0-9a-f]{64}$/u.test(input.expectedPubkey) ||
      publicKeyFromSecret(secretKey) !== input.expectedPubkey
    ) {
      throw new Error("Sarah identity does not match configured public key")
    }
    return createSealedSarahNostrSigner({ secretKey })
  } finally {
    secretKey.fill(0)
  }
}

const SafeSlug = S.String.check(
  S.isPattern(/^[a-z0-9][a-z0-9._-]{0,127}$/u),
)

const CommunityAllowlistEntry = S.Struct({
  groupRef: SafeSlug,
  channelRefs: S.Array(SafeSlug).check(S.isMinLength(1)),
})
type CommunityAllowlistEntry = S.Schema.Type<
  typeof CommunityAllowlistEntry
>
const CommunityAllowlist = S.Array(CommunityAllowlistEntry).check(
  S.isMinLength(1),
)

const exactKeys = (
  value: Record<string, unknown>,
  expected: ReadonlyArray<string>,
): boolean => {
  const observed = Object.keys(value).sort()
  return (
    observed.length === expected.length &&
    observed.every((key, index) => key === [...expected].sort()[index])
  )
}

const isRecord = (value: unknown): value is Record<string, unknown> =>
  typeof value === "object" && value !== null && !Array.isArray(value)

const containsForbiddenPublicMaterial = (value: string): boolean =>
  /(?:\bnsec1|sk[-_][A-Za-z0-9_-]{12,}|(?:sess|pat|ghp|gho|ghu|ghs|ghr)_[A-Za-z0-9_-]{12,}|-----BEGIN [A-Z ]*PRIVATE KEY-----|authorization\s*:\s*bearer)/iu.test(
    value,
  )

export const parseCommunityAllowlist = (
  raw: string,
): ReadonlyArray<CommunityAllowlistEntry> => {
  let value: unknown
  try {
    value = JSON.parse(raw)
  } catch {
    throw new Error("invalid_community_allowlist")
  }
  if (!Array.isArray(value) || value.length === 0) {
    throw new Error("invalid_community_allowlist")
  }
  for (const entry of value) {
    if (!isRecord(entry) || !exactKeys(entry, ["groupRef", "channelRefs"])) {
      throw new Error("invalid_community_allowlist")
    }
  }
  let decoded: ReadonlyArray<CommunityAllowlistEntry>
  try {
    decoded = S.decodeUnknownSync(CommunityAllowlist)(value)
  } catch {
    throw new Error("invalid_community_allowlist")
  }
  const observed = new Set<string>()
  const result = decoded.map((entry): CommunityAllowlistEntry => {
    const channelRefs = [...new Set(entry["channelRefs"])]
    if (channelRefs.length !== entry["channelRefs"].length) {
      throw new Error("invalid_community_allowlist")
    }
    for (const channelRef of channelRefs) {
      const key = `${entry["groupRef"]}\u0000${channelRef}`
      if (observed.has(key)) throw new Error("invalid_community_allowlist")
      observed.add(key)
    }
    return { groupRef: entry["groupRef"], channelRefs }
  })
  return result
}

const allowedCommunity = (
  allowlist: ReadonlyArray<CommunityAllowlistEntry>,
  template: SarahSignerTemplate,
): boolean =>
  allowlist.some(
    (entry) =>
      entry.groupRef === template.groupRef &&
      entry.channelRefs.includes(template.channelRef),
  )

const noStoreJson = (value: unknown, status: number): Response =>
  Response.json(value, {
    status,
    headers: {
      "cache-control": "no-store",
      "content-type": "application/json; charset=utf-8",
    },
  })

export const makeSarahNostrSignerHandler = (options: Readonly<{
  signer: SarahNostrSigner
  allowlist: ReadonlyArray<CommunityAllowlistEntry>
  nowSeconds?: () => number
}>): ((request: Request) => Promise<Response>) => {
  const nowSeconds = options.nowSeconds ?? (() => Math.floor(Date.now() / 1_000))
  const handle = Effect.fn("SarahNostrSigner.handle")(function* (
    request: Request,
  ) {
    const url = new URL(request.url)
    if (url.pathname === "/health" && request.method === "GET" && url.search === "") {
      return noStoreJson({ ok: true, service: "sarah-nostr-signer" }, 200)
    }
    if (
      url.pathname !== "/v1/sign" ||
      request.method !== "POST" ||
      url.search !== ""
    ) {
      return noStoreJson({ error: "not_found" }, 404)
    }
    const declaredLength = Number(request.headers.get("content-length") ?? "0")
    if (!Number.isFinite(declaredLength) || declaredLength > 16_384) {
      return noStoreJson({ error: "invalid_request" }, 400)
    }

    let body: unknown
    try {
      const rawBody = yield* Effect.tryPromise(() => request.text())
      if (new TextEncoder().encode(rawBody).byteLength > 16_384) {
        return noStoreJson({ error: "invalid_request" }, 400)
      }
      body = JSON.parse(rawBody)
    } catch {
      return noStoreJson({ error: "invalid_request" }, 400)
    }
    if (
      !isRecord(body) ||
      !exactKeys(body, ["schemaVersion", "template"]) ||
      body["schemaVersion"] !== SARAH_SIGNING_REQUEST_SCHEMA
    ) {
      return noStoreJson({ error: "invalid_request" }, 400)
    }

    if (
      !isRecord(body["template"]) ||
      !exactKeys(
        body["template"],
        body["template"]["type"] === "presence"
          ? [
              "type",
              "createdAt",
              "expiresAt",
              "groupRef",
              "channelRef",
              "presenceLeaseRef",
              "roomEpochDigest",
              "sessionDigest",
              "generation",
              "membershipRevision",
              "e2eeKeyRevision",
              "admissionDigest",
              "authorityDigest",
            ]
          : [
              "type",
              "createdAt",
              "groupRef",
              "channelRef",
              "messageRef",
              "presenceLeaseRef",
              "generation",
              "content",
            ],
      )
    ) {
      return noStoreJson({ error: "invalid_request" }, 400)
    }
    const decoded = yield* S.decodeUnknownEffect(SarahSigningRequestSchema)(body).pipe(
      Effect.option,
    )
    if (decoded._tag === "None") {
      return noStoreJson({ error: "invalid_request" }, 400)
    }
    const template = decoded.value.template
    if (
      (template.type === "kind9_projection" &&
        containsForbiddenPublicMaterial(template.content)) ||
      (() => {
        try {
          assertSarahNostrPublicSafe(template)
          return false
        } catch {
          return true
        }
      })()
    ) {
      return noStoreJson({ error: "invalid_request" }, 400)
    }
    if (!allowedCommunity(options.allowlist, template)) {
      return noStoreJson({ error: "template_not_allowed" }, 403)
    }
    const now = nowSeconds()
    if (template.createdAt < now - 60 || template.createdAt > now + 5) {
      return noStoreJson({ error: "stale_template" }, 409)
    }
    if (
      template.type === "presence" &&
      (template.expiresAt <= template.createdAt ||
        template.expiresAt > template.createdAt + 300)
    ) {
      return noStoreJson({ error: "invalid_expiry" }, 400)
    }

    const signed = yield* Effect.sync(() =>
      options.signer.signEvent(buildSarahSigningTemplate(template)),
    )
    return noStoreJson(
      {
        schemaVersion: SARAH_SIGNING_RESPONSE_SCHEMA,
        eventId: signed.id,
        pubkey: signed.pubkey,
        signature: signed.sig,
      },
      200,
    )
  })
  return request => Effect.runPromise(handle(request))
}
