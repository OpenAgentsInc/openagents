export type Migration = {
  readonly version: number
}

export type MigrationState = {
  readonly currentVersion: number
  readonly applied: readonly number[]
}

export class MigrationGapError extends Error {
  readonly missingVersion: number

  constructor(missingVersion: number) {
    super(`Missing migration version ${missingVersion}`)
    this.name = "MigrationGapError"
    this.missingVersion = missingVersion
  }
}

export function planMigrations<T extends Migration>(
  currentVersion: number,
  migrations: readonly T[],
): T[] {
  return [...migrations]
    .filter((migration) => migration.version > currentVersion)
    .sort((left, right) => left.version - right.version)
}

export function applyPlan<T extends Migration>(
  currentVersion: number,
  applied: readonly number[],
  migrations: readonly T[],
): MigrationState {
  const appliedSet = new Set(applied)
  const migrationsByVersion = new Map<number, T>()

  for (const migration of migrations) {
    migrationsByVersion.set(migration.version, migration)
  }

  const targetVersion = Math.max(
    currentVersion,
    ...migrations.map((migration) => migration.version),
    ...applied,
  )
  const nextApplied = [...applied]

  for (
    let version = currentVersion + 1;
    version <= targetVersion;
    version += 1
  ) {
    if (appliedSet.has(version)) {
      continue
    }

    if (!migrationsByVersion.has(version)) {
      throw new MigrationGapError(version)
    }

    appliedSet.add(version)
    nextApplied.push(version)
  }

  return {
    currentVersion: targetVersion,
    applied: nextApplied,
  }
}
