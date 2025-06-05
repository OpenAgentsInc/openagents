/**
 * Event storage service for Nostr relay
 * @module
 */

import { Context, Effect, HashMap, Layer, Option, Ref } from "effect"
import type { EventId, Filter, NostrEvent } from "../types.js"

export interface EventStorage {
  readonly store: (event: NostrEvent) => Effect.Effect<void>
  readonly get: (id: EventId) => Effect.Effect<Option.Option<NostrEvent>>
  readonly query: (filters: ReadonlyArray<Filter>) => Effect.Effect<ReadonlyArray<NostrEvent>>
  readonly delete: (id: EventId) => Effect.Effect<void>
  readonly count: () => Effect.Effect<number>
}

export const EventStorage = Context.GenericTag<EventStorage>("pylon/EventStorage")

/**
 * In-memory event storage implementation
 */
export const EventStorageMemoryLive = Layer.effect(
  EventStorage,
  Effect.gen(function*() {
    // Simple in-memory storage
    const events = yield* Ref.make<HashMap.HashMap<EventId, NostrEvent>>(HashMap.empty())

    // Indexes for efficient querying
    const authorIndex = yield* Ref.make<HashMap.HashMap<string, ReadonlyArray<EventId>>>(HashMap.empty())
    const kindIndex = yield* Ref.make<HashMap.HashMap<number, ReadonlyArray<EventId>>>(HashMap.empty())
    const tagIndex = yield* Ref.make<HashMap.HashMap<string, ReadonlyArray<EventId>>>(HashMap.empty())

    const store = (event: NostrEvent): Effect.Effect<void> =>
      Effect.gen(function*() {
        // Check if event already exists
        const existing = yield* Ref.get(events).pipe(
          Effect.map(HashMap.get(event.id))
        )

        if (Option.isSome(existing)) {
          return // Event already stored
        }

        // Store event
        yield* Ref.update(events, HashMap.set(event.id, event))

        // Update author index
        yield* Ref.update(authorIndex, (index) => {
          const current = HashMap.get(index, event.pubkey)
          if (Option.isSome(current)) {
            return HashMap.set(index, event.pubkey, [...current.value, event.id])
          }
          return HashMap.set(index, event.pubkey, [event.id])
        })

        // Update kind index
        yield* Ref.update(kindIndex, (index) => {
          const current = HashMap.get(index, event.kind)
          if (Option.isSome(current)) {
            return HashMap.set(index, event.kind, [...current.value, event.id])
          }
          return HashMap.set(index, event.kind, [event.id])
        })

        // Update tag indexes
        yield* Effect.forEach(event.tags, (tag) => {
          if (tag.length >= 2) {
            const [tagName, tagValue] = tag
            const tagKey = `${tagName}:${tagValue}`
            return Ref.update(tagIndex, (index) => {
              const current = HashMap.get(index, tagKey)
              if (Option.isSome(current)) {
                return HashMap.set(index, tagKey, [...current.value, event.id])
              }
              return HashMap.set(index, tagKey, [event.id])
            })
          }
          return Effect.succeed(undefined)
        })
      })

    const get = (id: EventId): Effect.Effect<Option.Option<NostrEvent>> =>
      Ref.get(events).pipe(Effect.map(HashMap.get(id)))

    const query = (filters: ReadonlyArray<Filter>): Effect.Effect<ReadonlyArray<NostrEvent>> =>
      Effect.gen(function*() {
        if (filters.length === 0) {
          const allEvents = yield* Ref.get(events)
          return Array.from(HashMap.values(allEvents))
        }

        // Collect matching event IDs for each filter
        const matchingSets = yield* Effect.forEach(filters, (filter) =>
          Effect.gen(function*() {
            const matchingIds = new Set<EventId>()

            // Filter by IDs
            if (filter.ids && filter.ids.length > 0) {
              const currentEvents = yield* Ref.get(events)
              for (const id of filter.ids) {
                // Handle prefix matching
                const matching = Array.from(HashMap.keys(currentEvents)).filter(
                  (eventId) => eventId.startsWith(id)
                )
                matching.forEach((id) => matchingIds.add(id as EventId))
              }
            }

            // Filter by authors
            if (filter.authors && filter.authors.length > 0) {
              const index = yield* Ref.get(authorIndex)
              for (const author of filter.authors) {
                // Handle prefix matching
                const matchingAuthors = Array.from(HashMap.keys(index)).filter(
                  (pubkey) => pubkey.startsWith(author)
                )
                for (const pubkey of matchingAuthors) {
                  const ids = HashMap.get(index, pubkey)
                  if (Option.isSome(ids)) {
                    ids.value.forEach((id) => matchingIds.add(id))
                  }
                }
              }
            }

            // Filter by kinds
            if (filter.kinds && filter.kinds.length > 0) {
              const index = yield* Ref.get(kindIndex)
              for (const kind of filter.kinds) {
                const ids = HashMap.get(index, kind)
                if (Option.isSome(ids)) {
                  ids.value.forEach((id) => matchingIds.add(id))
                }
              }
            }

            // Filter by tags
            if (filter["#e"] && filter["#e"].length > 0) {
              const index = yield* Ref.get(tagIndex)
              for (const value of filter["#e"]) {
                const ids = HashMap.get(index, `e:${value}`)
                if (Option.isSome(ids)) {
                  ids.value.forEach((id) => matchingIds.add(id))
                }
              }
            }

            if (filter["#p"] && filter["#p"].length > 0) {
              const index = yield* Ref.get(tagIndex)
              for (const value of filter["#p"]) {
                const ids = HashMap.get(index, `p:${value}`)
                if (Option.isSome(ids)) {
                  ids.value.forEach((id) => matchingIds.add(id))
                }
              }
            }

            // If no specific filters, get all events
            if (!filter.ids && !filter.authors && !filter.kinds && !filter["#e"] && !filter["#p"]) {
              const allEvents = yield* Ref.get(events)
              Array.from(HashMap.keys(allEvents)).forEach((id) => matchingIds.add(id))
            }

            return matchingIds
          }))

        // Union all matching sets (OR between filters)
        const allMatchingIds = new Set<EventId>()
        matchingSets.forEach((set) => {
          set.forEach((id) => allMatchingIds.add(id))
        })

        // Fetch events and apply remaining filters
        const currentEvents = yield* Ref.get(events)
        const results: Array<NostrEvent> = []

        for (const id of allMatchingIds) {
          const event = HashMap.get(currentEvents, id)
          if (Option.isSome(event)) {
            // Apply time filters
            let matches = true

            for (const filter of filters) {
              if (filter.since && event.value.created_at < filter.since) {
                matches = false
                break
              }
              if (filter.until && event.value.created_at > filter.until) {
                matches = false
                break
              }
            }

            if (matches) {
              results.push(event.value)
            }
          }
        }

        // Sort by created_at descending and apply limit
        results.sort((a, b) => b.created_at - a.created_at)

        // Apply limit from first filter that has one
        for (const filter of filters) {
          if (filter.limit) {
            return results.slice(0, filter.limit)
          }
        }

        return results
      })

    const deleteEvent = (id: EventId): Effect.Effect<void> =>
      Effect.gen(function*() {
        const event = yield* get(id)
        if (Option.isNone(event)) return

        const e = event.value

        // Remove from main storage
        yield* Ref.update(events, HashMap.remove(id))

        // Remove from author index
        yield* Ref.update(authorIndex, (index) => {
          const current = HashMap.get(index, e.pubkey)
          if (Option.isSome(current)) {
            const filtered = current.value.filter((eventId) => eventId !== id)
            if (filtered.length === 0) {
              return HashMap.remove(index, e.pubkey)
            }
            return HashMap.set(index, e.pubkey, filtered)
          }
          return index
        })

        // Remove from kind index
        yield* Ref.update(kindIndex, (index) => {
          const current = HashMap.get(index, e.kind)
          if (Option.isSome(current)) {
            const filtered = current.value.filter((eventId) => eventId !== id)
            if (filtered.length === 0) {
              return HashMap.remove(index, e.kind)
            }
            return HashMap.set(index, e.kind, filtered)
          }
          return index
        })

        // Remove from tag indexes
        yield* Effect.forEach(e.tags, (tag) => {
          if (tag.length >= 2) {
            const [tagName, tagValue] = tag
            const tagKey = `${tagName}:${tagValue}`
            return Ref.update(tagIndex, (index) => {
              const current = HashMap.get(index, tagKey)
              if (Option.isSome(current)) {
                const filtered = current.value.filter((eventId) => eventId !== id)
                if (filtered.length === 0) {
                  return HashMap.remove(index, tagKey)
                }
                return HashMap.set(index, tagKey, filtered)
              }
              return index
            })
          }
          return Effect.succeed(undefined)
        })
      })

    const count = (): Effect.Effect<number> => Ref.get(events).pipe(Effect.map(HashMap.size))

    return {
      store,
      get,
      query,
      delete: deleteEvent,
      count
    }
  })
)
