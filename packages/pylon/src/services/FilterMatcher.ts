/**
 * Filter matching service for Nostr events
 * @module
 */

import { Context, Effect, Layer } from "effect"
import type { Filter, NostrEvent } from "../types.js"

export interface FilterMatcher {
  readonly matches: (event: NostrEvent, filter: Filter) => Effect.Effect<boolean>
}

export const FilterMatcher = Context.GenericTag<FilterMatcher>("pylon/FilterMatcher")

/**
 * Check if a string matches a prefix (for id/author filtering)
 */
const matchesPrefix = (value: string, prefix: string): boolean => {
  return value.toLowerCase().startsWith(prefix.toLowerCase())
}

/**
 * Check if event has a specific tag value
 */
const hasTagValue = (event: NostrEvent, tagName: string, tagValue: string): boolean => {
  return event.tags.some((tag: ReadonlyArray<string>) => tag.length >= 2 && tag[0] === tagName && tag[1] === tagValue)
}

/**
 * Live implementation of FilterMatcher
 */
export const FilterMatcherLive = Layer.succeed(
  FilterMatcher,
  {
    matches: (event: NostrEvent, filter: Filter): Effect.Effect<boolean> =>
      Effect.sync(() => {
        // Check IDs
        if (filter.ids && filter.ids.length > 0) {
          const matches = filter.ids.some((id: any) => matchesPrefix(event.id, id))
          if (!matches) return false
        }

        // Check authors
        if (filter.authors && filter.authors.length > 0) {
          const matches = filter.authors.some((author: any) => matchesPrefix(event.pubkey, author))
          if (!matches) return false
        }

        // Check kinds
        if (filter.kinds && filter.kinds.length > 0) {
          const matches = filter.kinds.includes(event.kind)
          if (!matches) return false
        }

        // Check time constraints
        if (filter.since && event.created_at < filter.since) {
          return false
        }

        if (filter.until && event.created_at > filter.until) {
          return false
        }

        // Check event tags (#e)
        if (filter["#e"] && filter["#e"].length > 0) {
          const matches = filter["#e"].some((value: any) => hasTagValue(event, "e", value))
          if (!matches) return false
        }

        // Check pubkey tags (#p)
        if (filter["#p"] && filter["#p"].length > 0) {
          const matches = filter["#p"].some((value: any) => hasTagValue(event, "p", value))
          if (!matches) return false
        }

        // Check any other single-letter tags
        const filterKeys = Object.keys(filter)
        for (const key of filterKeys) {
          if (key.startsWith("#") && key.length === 2) {
            const tagName = key[1]
            const values = filter[key as keyof Filter] as Array<string> | undefined
            if (values && values.length > 0) {
              const matches = values.some((value) => hasTagValue(event, tagName, value))
              if (!matches) return false
            }
          }
        }

        // All checks passed
        return true
      })
  }
)
