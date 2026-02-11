/**
 * Action registry for hypermedia actions.
 */

import { Context } from "effect"
import type { EzAction } from "./types.js"

export type EzRegistry = Map<string, EzAction>

export class EzRegistryTag extends Context.Tag("@openagentsinc/effuse/EzRegistry")<
  EzRegistryTag,
  EzRegistry
>() {}

export const makeEzRegistry = (
  entries?: Iterable<readonly [string, EzAction]>
): EzRegistry => new Map(entries ?? [])
