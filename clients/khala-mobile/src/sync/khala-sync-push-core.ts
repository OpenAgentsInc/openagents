/** Pure push-request wiring, mirroring khala-sync-entities-core's bootstrap/
 * connect builders. No native/RN imports. */

export const buildPushUrl = (baseUrl: string): string => `${baseUrl.replace(/\/$/, "")}/api/sync/push`

export type MutationEnvelopeInput = Readonly<{
  mutationId: number
  name: string
  argsJson: string
}>

export type PushRequestBody = Readonly<{
  protocolVersion: number
  schemaVersion: number
  clientGroupId: string
  clientId: string
  mutations: ReadonlyArray<MutationEnvelopeInput>
}>

export const buildPushRequestBody = (input: {
  clientGroupId: string
  clientId: string
  mutations: ReadonlyArray<MutationEnvelopeInput>
}): PushRequestBody => ({
  clientGroupId: input.clientGroupId,
  clientId: input.clientId,
  mutations: input.mutations,
  protocolVersion: 1,
  schemaVersion: 1
})

/** A minimal, dependency-free stable stringify — key order doesn't need to
 * match the server's canonicalJson exactly (argsJson is just decoded as
 * regular JSON on the server), but sorting keys keeps push payloads
 * deterministic for tests/logging. */
export const stableArgsJson = (value: Record<string, unknown>): string => {
  const sortedEntries = Object.entries(value)
    .filter(([, v]) => v !== undefined)
    .sort(([a], [b]) => a.localeCompare(b))
  return JSON.stringify(Object.fromEntries(sortedEntries))
}

const randomHex = (length: number): string =>
  Array.from({ length }, () => Math.floor(Math.random() * 16).toString(16)).join("")

/** Generates a Khala Sync safe-ref-shaped id (`^[A-Za-z0-9][A-Za-z0-9._:-]*$`)
 * unique enough for client-issued messageId/turnId/nonce values. */
export const makeSafeRef = (prefix: string): string => `${prefix}.${Date.now().toString(36)}${randomHex(10)}`
