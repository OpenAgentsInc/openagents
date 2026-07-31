import {
  SARAH_NOSTR_PRINCIPAL,
  assertSarahNostrPublicSafe,
  createSealedSarahNostrSigner,
  parseSecretMaterial,
  publicKeyFromSecret,
  type SarahNostrEventTemplate,
  type SarahNostrSigner,
} from "@openagentsinc/sarah/nostr-identity"
import { Effect, Schema as S } from "effect"

export const SARAH_SIGNING_REQUEST_SCHEMA =
  "openagents.sarah.nostr_signing_request.v1" as const
export const SARAH_SIGNING_RESPONSE_SCHEMA =
  "openagents.sarah.nostr_signing_response.v1" as const
export const SARAH_PRESENCE_KIND = 30315
export const SARAH_GROUP_TEXT_KIND = 9

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
const RoomEpochRef = S.String.check(
  S.isPattern(/^room-epoch:[0-9a-f]{32,64}$/u),
)
const ParticipantRef = S.String.check(
  S.isPattern(/^participant:[0-9a-f]{64}$/u),
)
const Timestamp = S.Number.check(S.isInt(), S.isGreaterThan(0))

const PresenceTemplate = S.Struct({
  type: S.Literal("presence"),
  createdAt: Timestamp,
  expiresAt: Timestamp,
  groupRef: SafeSlug,
  channelRef: SafeSlug,
  roomEpochRef: RoomEpochRef,
  participantRef: ParticipantRef,
})

const Kind9ProjectionTemplate = S.Struct({
  type: S.Literal("kind9_projection"),
  createdAt: Timestamp,
  groupRef: SafeSlug,
  channelRef: SafeSlug,
  content: S.String.check(S.isMinLength(1), S.isMaxLength(2_000)),
})

export const SarahSignerTemplateSchema = S.Union([
  PresenceTemplate,
  Kind9ProjectionTemplate,
])
export type SarahSignerTemplate = S.Schema.Type<
  typeof SarahSignerTemplateSchema
>

export const SarahSigningRequest = S.Struct({
  schemaVersion: S.Literal(SARAH_SIGNING_REQUEST_SCHEMA),
  template: SarahSignerTemplateSchema,
})
export interface SarahSigningRequest
  extends S.Schema.Type<typeof SarahSigningRequest> {}

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

export const buildSigningTemplate = (
  template: SarahSignerTemplate,
): SarahNostrEventTemplate => {
  if (template.type === "presence") {
    return {
      kind: SARAH_PRESENCE_KIND,
      created_at: template.createdAt,
      tags: [
        ["d", `${template.roomEpochRef}:${template.participantRef}`],
        ["h", template.groupRef],
        ["channel", template.channelRef],
        ["principal", SARAH_NOSTR_PRINCIPAL],
        ["room_epoch", template.roomEpochRef],
        ["participant", template.participantRef],
        ["expiration", String(template.expiresAt)],
        ["status", "active"],
        ["authority", "presence-only"],
        ["alt", "OpenAgents Sarah room presence binding"],
      ],
      content: "",
    }
  }
  return {
    kind: SARAH_GROUP_TEXT_KIND,
    created_at: template.createdAt,
    tags: [
      ["h", template.groupRef],
      ["channel", template.channelRef],
      ["principal", SARAH_NOSTR_PRINCIPAL],
      ["openagents-projection", "sarah-text-v1"],
      ["authority", "projection-only"],
      ["alt", "Sarah group text projection"],
    ],
    content: template.content,
  }
}

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
              "roomEpochRef",
              "participantRef",
            ]
          : ["type", "createdAt", "groupRef", "channelRef", "content"],
      )
    ) {
      return noStoreJson({ error: "invalid_request" }, 400)
    }
    const decoded = yield* S.decodeUnknownEffect(SarahSigningRequest)(body).pipe(
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
      options.signer.signEvent(buildSigningTemplate(template)),
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
