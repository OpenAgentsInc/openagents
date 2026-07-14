/**
 * Zero-Bun perimeter for the completed Node migration.
 *
 * These arrays deliberately stay empty. The scanner rejects new Bun imports
 * or globals, and any future exception requires an explicit policy change.
 */
export type BunApiCategory = "bun-import" | "bun-global"

export type BunApiPerimeterEntry = {
  readonly path: string
  readonly category: BunApiCategory
  readonly reason: string
}

export type BunApiGrandfatheredEntry = {
  readonly path: string
  readonly categories: readonly BunApiCategory[]
}

export const bunApiPerimeter = [] satisfies readonly BunApiPerimeterEntry[]
export const bunApiGrandfathered = [] satisfies readonly BunApiGrandfatheredEntry[]
