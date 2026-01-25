/**
 * Action registry for hypermedia actions.
 */

import { Context } from "effect"
import type { EzAction } from "./types.js"

export type EzRegistry = Map<string, EzAction>

export const EzRegistryTag = Context.GenericTag<EzRegistry>("EzRegistry")

export const makeEzRegistry = (
  entries?: Iterable<readonly [string, EzAction]>
): EzRegistry => new Map(entries ?? [])
