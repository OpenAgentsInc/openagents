export type RemoteEnvironmentProjection = Readonly<{
  environmentRef: string
  state: "connected" | "disconnected" | "failed"
  shell: string | null
  cwdRef: string | null
}>

export type RemoteClientProjection = Readonly<{
  clientRef: string
  displayName: string | null
  platform: string | null
  state: "granted" | "revoked"
}>

export type RemoteConnectProjection = Readonly<{
  phase: "idle" | "loading" | "ready" | "unavailable" | "mutating"
  revision: number
  manifestReady: boolean
  environments: ReadonlyArray<RemoteEnvironmentProjection>
  remote: Readonly<{
    state: "disabled" | "connecting" | "connected" | "errored"
    environmentRef: string | null
    pairing: Readonly<{
      pairingRef: string
      state: "pending" | "claimed" | "expired" | "revoked"
      expiresAt: number
    }> | null
    clients: ReadonlyArray<RemoteClientProjection>
  }>
  notice: string | null
}>

export const emptyRemoteConnectProjection = (): RemoteConnectProjection => ({
  phase: "idle",
  revision: 0,
  manifestReady: false,
  environments: [],
  remote: { state: "disabled", environmentRef: null, pairing: null, clients: [] },
  notice: null,
})

const row = (value: unknown): Readonly<Record<string, unknown>> | null =>
  typeof value === "object" && value !== null && !Array.isArray(value)
    ? value as Readonly<Record<string, unknown>>
    : null
const string = (value: unknown): string | null => typeof value === "string" ? value : null
const number = (value: unknown): number | null => typeof value === "number" && Number.isFinite(value) ? value : null

export const decodeRemoteConnectSnapshot = (value: unknown): RemoteConnectProjection | null => {
  const source = row(value)
  const revision = number(source?.revision)
  const manifest = row(source?.manifest)
  const remote = row(source?.remoteControl)
  const remoteState = string(remote?.state)
  if (revision === null || manifest === null || typeof manifest.enabled !== "boolean" || remote === null ||
    !["disabled", "connecting", "connected", "errored"].includes(remoteState ?? "")) return null
  const environments = (Array.isArray(source?.environments) ? source.environments : []).flatMap(item => {
    const entry = row(item)
    const environmentRef = string(entry?.environmentRef)
    const state = string(entry?.state)
    if (environmentRef === null || !["connected", "disconnected", "failed"].includes(state ?? "")) return []
    return [{
      environmentRef,
      state: state as RemoteEnvironmentProjection["state"],
      shell: string(entry?.shell),
      cwdRef: string(entry?.cwdRef),
    }]
  })
  const pairingRow = row(remote.pairing)
  const pairingRef = string(pairingRow?.pairingRef)
  const pairingState = string(pairingRow?.state)
  const expiresAt = number(pairingRow?.expiresAt)
  const pairing = pairingRow === null || pairingRef === null || expiresAt === null ||
    !["pending", "claimed", "expired", "revoked"].includes(pairingState ?? "")
    ? null
    : { pairingRef, state: pairingState as "pending" | "claimed" | "expired" | "revoked", expiresAt }
  const clients = (Array.isArray(remote.clients) ? remote.clients : []).flatMap(item => {
    const entry = row(item)
    const clientRef = string(entry?.clientRef)
    const state = string(entry?.state)
    if (clientRef === null || !["granted", "revoked"].includes(state ?? "")) return []
    return [{ clientRef, displayName: string(entry?.displayName), platform: string(entry?.platform), state: state as "granted" | "revoked" }]
  })
  return {
    phase: "ready",
    revision,
    manifestReady: manifest.enabled,
    environments,
    remote: {
      state: remoteState as RemoteConnectProjection["remote"]["state"],
      environmentRef: string(remote.environmentRef),
      pairing,
      clients,
    },
    notice: null,
  }
}

export const decodeRemoteConnectResponse = (value: unknown): Readonly<{
  ok: boolean
  reason: string | null
  snapshot: RemoteConnectProjection | null
}> => {
  const source = row(value)
  return {
    ok: source?.ok === true,
    reason: string(source?.reason),
    snapshot: decodeRemoteConnectSnapshot(source?.snapshot),
  }
}

export type RemoteConnectBridge = Readonly<{
  snapshot: () => Promise<unknown>
  request: (value: unknown) => Promise<unknown>
}>

export const unavailableRemoteConnectBridge: RemoteConnectBridge = {
  snapshot: async () => null,
  request: async () => ({ ok: false, reason: "unavailable" }),
}
