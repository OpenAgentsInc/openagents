import { createHash } from "node:crypto"
import { existsSync, statSync } from "node:fs"
import path from "node:path"

import { migrationCategories, type MigrationCategory, type MigrationLedger } from "./update-rollback.ts"

export const UPDATE_MIGRATION_EVIDENCE_SCHEMA = "openagents.desktop.update_migration_evidence.v1" as const

export type UpdateMigrationEvidence = Readonly<{
  schema: typeof UPDATE_MIGRATION_EVIDENCE_SCHEMA
  strategy: "external_state_roots_unchanged"
  categories: Readonly<Record<MigrationCategory,
    | Readonly<{ disposition: "present"; rootRef: string; kind: "file" | "directory" }>
    | Readonly<{ disposition: "absent"; reason: "signed_out" | "no_sessions" }>
  >>
}>

const inside = (parent: string, candidate: string): boolean => {
  const relative = path.relative(path.resolve(parent), path.resolve(candidate))
  return relative === "" || (!relative.startsWith("..") && !path.isAbsolute(relative))
}

const rootRef = (value: string): string =>
  `sha256:${createHash("sha256").update(path.resolve(value)).digest("hex")}`

/**
 * Full-artifact updates do not migrate owner data. This evidence proves each
 * real durable store remains outside the replaceable application bundle;
 * an in-bundle, relative, or missing root fails closed instead of ceremonial
 * "preserved" assertions.
 */
export const evaluateNoMigrationInvariant = (input: Readonly<{
  installedApplicationRoot: string
  categoryRoots: Readonly<Record<MigrationCategory, string>>
  categoryKinds: Readonly<Record<MigrationCategory, "file" | "directory">>
  absentDispositions?: Readonly<Partial<Record<MigrationCategory, "signed_out" | "no_sessions">>>
}>): UpdateMigrationEvidence | null => {
  if (!path.isAbsolute(input.installedApplicationRoot)) return null
  const categories = {} as Record<MigrationCategory,
    | Readonly<{ disposition: "present"; rootRef: string; kind: "file" | "directory" }>
    | Readonly<{ disposition: "absent"; reason: "signed_out" | "no_sessions" }>>
  for (const category of migrationCategories) {
    const root = input.categoryRoots[category]
    if (!path.isAbsolute(root) || inside(input.installedApplicationRoot, root)) return null
    if (!existsSync(root)) {
      const reason = input.absentDispositions?.[category]
      if ((category === "vaultRefs" && reason === "signed_out") || (category === "sessions" && reason === "no_sessions")) {
        categories[category] = { disposition: "absent", reason }
        continue
      }
      return null
    }
    try {
      const status = statSync(root)
      if (input.categoryKinds[category] === "file" ? !status.isFile() : !status.isDirectory()) return null
    } catch {
      return null
    }
    categories[category] = { disposition: "present", rootRef: rootRef(root), kind: input.categoryKinds[category] }
  }
  return {
    schema: UPDATE_MIGRATION_EVIDENCE_SCHEMA,
    strategy: "external_state_roots_unchanged",
    categories,
  }
}

export const decodeUpdateMigrationEvidence = (value: unknown): UpdateMigrationEvidence | null => {
  if (typeof value !== "object" || value === null) return null
  const row = value as Record<string, unknown>
  if (row.schema !== UPDATE_MIGRATION_EVIDENCE_SCHEMA || row.strategy !== "external_state_roots_unchanged" ||
    typeof row.categories !== "object" || row.categories === null) return null
  const categories = row.categories as Record<string, unknown>
  if (!migrationCategories.every(category => {
    const entry = categories[category]
    if (typeof entry !== "object" || entry === null) return false
    const evidence = entry as Record<string, unknown>
    if (evidence.disposition === "present") return /^sha256:[0-9a-f]{64}$/.test(String(evidence.rootRef)) && (evidence.kind === "file" || evidence.kind === "directory")
    return evidence.disposition === "absent" && ((category === "vaultRefs" && evidence.reason === "signed_out") || (category === "sessions" && evidence.reason === "no_sessions"))
  })) return null
  return value as UpdateMigrationEvidence
}

export const migrationLedgerFromEvidence = (evidence: UpdateMigrationEvidence): MigrationLedger =>
  Object.fromEntries(migrationCategories.map(category => [category, evidence.categories[category].disposition === "present"
    ? { status: "preserved" }
    : { status: "loss_accounted", reasonRef: evidence.categories[category].reason }])) as MigrationLedger
