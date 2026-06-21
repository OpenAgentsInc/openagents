import {
  chatWorldRegionRefForRun,
  type ChatWorldMultiplayerProjection,
} from "../../src/shared/chat-world-multiplayer"
import type { SpacetimeWorldConnection } from "../../src/ui/chat-world-subscriptions"

export type TableCallback = (...args: ReadonlyArray<unknown>) => void

export class FakeSpacetimeTable {
  rows: unknown[]
  inserts: TableCallback[] = []
  updates: TableCallback[] = []
  deletes: TableCallback[] = []

  constructor(rows: ReadonlyArray<unknown> = []) {
    this.rows = [...rows]
  }

  iter(): Iterable<unknown> {
    return this.rows
  }

  onInsert(cb: TableCallback): void {
    this.inserts.push(cb)
  }

  onUpdate(cb: TableCallback): void {
    this.updates.push(cb)
  }

  onDelete(cb: TableCallback): void {
    this.deletes.push(cb)
  }

  removeOnInsert(cb: TableCallback): void {
    this.inserts = this.inserts.filter((item) => item !== cb)
  }

  removeOnUpdate(cb: TableCallback): void {
    this.updates = this.updates.filter((item) => item !== cb)
  }

  removeOnDelete(cb: TableCallback): void {
    this.deletes = this.deletes.filter((item) => item !== cb)
  }

  insert(row: unknown): void {
    this.rows.push(row)
    for (const cb of this.inserts) cb(row)
  }

  upsertBy(key: string, row: Record<string, unknown>): void {
    const index = this.rows.findIndex((candidate) =>
      candidate !== null &&
      typeof candidate === "object" &&
      (candidate as Record<string, unknown>)[key] === row[key],
    )
    if (index === -1) {
      this.insert(row)
      return
    }

    const previous = this.rows[index]
    this.rows[index] = row
    for (const cb of this.updates) cb(previous, row)
  }

  deleteBy(key: string, value: unknown): void {
    const index = this.rows.findIndex((candidate) =>
      candidate !== null &&
      typeof candidate === "object" &&
      (candidate as Record<string, unknown>)[key] === value,
    )
    if (index === -1) return
    const [removed] = this.rows.splice(index, 1)
    for (const cb of this.deletes) cb(removed)
  }
}

export type FakeChatWorldRows = {
  worldRegion: FakeSpacetimeTable
  pylonStation: FakeSpacetimeTable
  agentAvatar: FakeSpacetimeTable
  avatarPosition: FakeSpacetimeTable
  avatarPositionNear: FakeSpacetimeTable
  avatarPositionFar: FakeSpacetimeTable
  pylonAttention: FakeSpacetimeTable
  localChatMessage: FakeSpacetimeTable
  chatBubble: FakeSpacetimeTable
  localEmote: FakeSpacetimeTable
  agentIntent: FakeSpacetimeTable
}

export const createFakeChatWorldRows = (input: {
  readonly runRef: string
  readonly nowMs?: number
  readonly regionOverrides?: Record<string, unknown>
  readonly stationOverrides?: Record<string, unknown>
  readonly avatarOverrides?: Record<string, unknown>
  readonly positionOverrides?: Record<string, unknown>
}): FakeChatWorldRows => {
  const regionRef = chatWorldRegionRefForRun(input.runRef)
  const nowMs = input.nowMs ?? 1_000

  return {
    worldRegion: new FakeSpacetimeTable([{
      regionRef,
      runRef: input.runRef,
      label: "Tassadar main",
      minX: -40,
      minY: 0,
      minZ: -40,
      maxX: 40,
      maxY: 12,
      maxZ: 40,
      proximityRadiusMeters: 12,
      avatarPositionMinIntervalMs: 100,
      staleAvatarPositionMs: 20_000,
      ...input.regionOverrides,
    }]),
    pylonStation: new FakeSpacetimeTable([{
      pylonRef: "pylon.public.alpha",
      runRef: input.runRef,
      regionRef,
      label: "Alpha Pylon",
      positionX: 6,
      positionY: 0,
      positionZ: -2,
      ...input.stationOverrides,
    }]),
    agentAvatar: new FakeSpacetimeTable([{
      avatarRef: "avatar.forum.author",
      actorRef: "agent:forum_author",
      actorKind: "pylon_agent",
      displayName: "Forum Author",
      colorHex: "#f5b73a",
      ...input.avatarOverrides,
    }]),
    avatarPosition: new FakeSpacetimeTable([{
      avatarRef: "avatar.forum.author",
      regionRef,
      positionX: -4,
      positionY: 0,
      positionZ: 3,
      yaw: 0,
      movementMode: "idle",
      lastSeenEpochMs: nowMs,
      ...input.positionOverrides,
    }]),
    avatarPositionNear: new FakeSpacetimeTable([]),
    avatarPositionFar: new FakeSpacetimeTable([]),
    pylonAttention: new FakeSpacetimeTable([]),
    localChatMessage: new FakeSpacetimeTable([]),
    chatBubble: new FakeSpacetimeTable([]),
    localEmote: new FakeSpacetimeTable([]),
    agentIntent: new FakeSpacetimeTable([]),
  }
}

export type FakeChatWorldConnection = SpacetimeWorldConnection & {
  readonly capturedQueries: ReadonlyArray<ReadonlyArray<string>>
  readonly joins: ReadonlyArray<unknown>
  readonly leaves: ReadonlyArray<unknown>
  readonly writes: ReadonlyArray<unknown>
  readonly triggerApplied: () => void
}

export const createFakeChatWorldConnection = (input: {
  readonly rows: FakeChatWorldRows
  readonly avatarRef: string
  readonly fallbackDisplayName: string
  readonly nowMs: () => number
}): FakeChatWorldConnection => {
  const capturedQueries: string[][] = []
  const joins: unknown[] = []
  const leaves: unknown[] = []
  const writes: unknown[] = []
  let applied: (() => void) | null = null

  const upsertAvatar = (displayName: string): void => {
    input.rows.agentAvatar.upsertBy("avatarRef", {
      avatarRef: input.avatarRef,
      actorRef: `identity.${input.avatarRef}`,
      actorKind: "guest",
      displayName,
      colorHex: "#cdd3e0",
    })
  }

  const upsertPosition = (args: {
    readonly regionRef: string
    readonly movementMode: string
    readonly positionX: number
    readonly positionY: number
    readonly positionZ: number
    readonly yaw: number
  }): void => {
    writes.push(args)
    input.rows.avatarPosition.upsertBy("avatarRef", {
      avatarRef: input.avatarRef,
      regionRef: args.regionRef,
      positionX: args.positionX,
      positionY: args.positionY,
      positionZ: args.positionZ,
      yaw: args.yaw,
      movementMode: args.movementMode,
      lastSeenEpochMs: input.nowMs(),
    })
  }

  const builder = {
    onApplied: (cb: () => void) => {
      applied = cb
      return builder
    },
    onError: () => builder,
    subscribe: (queries: ReadonlyArray<string>) => {
      capturedQueries.push([...queries])
      applied?.()
      return { unsubscribe: () => {} }
    },
  }

  return {
    db: input.rows,
    capturedQueries,
    joins,
    leaves,
    writes,
    triggerApplied: () => applied?.(),
    reducers: {
      joinRegion: (args: { readonly displayName: string; readonly regionRef: string }) => {
        const displayName = args.displayName || input.fallbackDisplayName
        joins.push({ avatarRef: input.avatarRef, displayName, regionRef: args.regionRef })
        upsertAvatar(displayName)
        input.rows.avatarPosition.upsertBy("avatarRef", {
          avatarRef: input.avatarRef,
          regionRef: args.regionRef,
          positionX: 0,
          positionY: 0,
          positionZ: 0,
          yaw: 0,
          movementMode: "idle",
          lastSeenEpochMs: input.nowMs(),
        })
      },
      leaveRegion: (args: { readonly regionRef: string }) => {
        leaves.push(args)
        input.rows.avatarPosition.deleteBy("avatarRef", input.avatarRef)
      },
      setAvatarPosition: upsertPosition,
    },
    subscriptionBuilder: () => builder,
  }
}

export class FakeActivityEventSource {
  static instances: FakeActivityEventSource[] = []

  readonly url: string
  private listeners = new Map<string, Set<EventListener>>()
  closed = false

  constructor(url: string) {
    this.url = url
    FakeActivityEventSource.instances.push(this)
  }

  addEventListener(type: string, listener: EventListener): void {
    const listeners = this.listeners.get(type) ?? new Set<EventListener>()
    listeners.add(listener)
    this.listeners.set(type, listeners)
  }

  removeEventListener(type: string, listener: EventListener): void {
    this.listeners.get(type)?.delete(listener)
  }

  emit(type: string, data: unknown): void {
    const event = { data: JSON.stringify(data) } as MessageEvent
    for (const listener of this.listeners.get(type) ?? []) listener(event)
  }

  close(): void {
    this.closed = true
  }
}

export const jsonResponse = (body: unknown, ok = true): Response =>
  ({
    ok,
    status: ok ? 200 : 500,
    json: async () => body,
  }) as unknown as Response

export const latestWorld = (
  worlds: ReadonlyArray<ChatWorldMultiplayerProjection>,
): ChatWorldMultiplayerProjection => {
  const world = worlds.at(-1)
  if (world === undefined) throw new Error("no multiplayer world was dispatched")
  return world
}
