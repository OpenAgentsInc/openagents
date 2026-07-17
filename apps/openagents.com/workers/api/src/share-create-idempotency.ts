import type {
  ShareAudience,
  ShareSource,
} from '@openagentsinc/sync-schema'

import type { ShareProjectionRecord } from './share-projections'

const IDEMPOTENCY_KEY_LIMIT = 128
const visibleAsciiPattern = /^[\x21-\x7e]+$/u
const namespace = Uint8Array.from([
  0x76, 0x3b, 0x82, 0x8d, 0xc4, 0x74, 0x4b, 0x53, 0xa7, 0x2c, 0x65, 0x11, 0xa6,
  0x02, 0xde, 0x31,
])

export type ShareCreateIdempotencyKey =
  | Readonly<{ _tag: 'Absent' }>
  | Readonly<{ _tag: 'Invalid' }>
  | Readonly<{ _tag: 'Valid'; value: string }>

export const decodeShareCreateIdempotencyKey = (
  value: string | null,
): ShareCreateIdempotencyKey =>
  value === null
    ? { _tag: 'Absent' }
    : value.length === 0 ||
        value.length > IDEMPOTENCY_KEY_LIMIT ||
        !visibleAsciiPattern.test(value)
      ? { _tag: 'Invalid' }
      : { _tag: 'Valid', value }

const hex = (value: number): string => value.toString(16).padStart(2, '0')

/** RFC 9562 UUIDv5 over a product-owned namespace and authenticated owner. */
export const deriveShareCreateId = async (
  ownerUserId: string,
  idempotencyKey: string,
): Promise<string> => {
  const name = new TextEncoder().encode(`${ownerUserId}\u0000${idempotencyKey}`)
  const input = new Uint8Array(namespace.length + name.length)
  input.set(namespace)
  input.set(name, namespace.length)
  const digest = new Uint8Array(await crypto.subtle.digest('SHA-1', input))
  const uuid = digest.slice(0, 16)
  uuid[6] = (uuid[6]! & 0x0f) | 0x50
  uuid[8] = (uuid[8]! & 0x3f) | 0x80
  const encoded = Array.from(uuid, hex).join('')

  return `${encoded.slice(0, 8)}-${encoded.slice(8, 12)}-${encoded.slice(12, 16)}-${encoded.slice(16, 20)}-${encoded.slice(20)}`
}

export type ShareCreateReplayExpectation = Readonly<{
  audience: ShareAudience
  canonicalUrl: string
  expiresAt: string | null
  ownerUserId: string
  redactionPolicyId: string
  source: ShareSource
  title?: string
}>

const sameJson = (left: unknown, right: unknown): boolean =>
  JSON.stringify(left) === JSON.stringify(right)

export const matchesShareCreateReplay = (
  record: ShareProjectionRecord,
  expected: ShareCreateReplayExpectation,
  nowEpochMillis: number,
): boolean =>
  record.status === 'active' &&
  record.revokedAt === null &&
  (record.expiresAt === null ||
    Date.parse(record.expiresAt) > nowEpochMillis) &&
  record.ownerUserId === expected.ownerUserId &&
  record.canonicalUrl === expected.canonicalUrl &&
  record.redactionPolicyId === expected.redactionPolicyId &&
  record.expiresAt === expected.expiresAt &&
  sameJson(record.source, expected.source) &&
  sameJson(record.audience, expected.audience) &&
  (expected.title === undefined || record.title === expected.title)
