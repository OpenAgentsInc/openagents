export const TASSADAR_GENERALIZATION_GUARD_SCHEMA_VERSION =
  'openagents.tassadar.generalization_guard.v0'

export const TASSADAR_GENERALIZATION_GUARD_FORBIDDEN_CONSUMERS = [
  'training',
  'memory_context',
  'trace_homework',
  'rag',
  'optimization',
  'homework_loop',
] as const

export const TASSADAR_GENERALIZATION_GUARD_ALLOWED_CONSUMERS = [
  'gg_eval',
  'replay_verification',
  'public_manifest_audit',
] as const

export type TassadarGeneralizationGuardPartition = Readonly<{
  familyIds: ReadonlyArray<string>
  kind: 'train_allowed' | 'gg_held_out'
  recordCount: number
  shardRefs: ReadonlyArray<string>
  split: string
  tokenCount: number
}>

export type TassadarGeneralizationGuard = Readonly<{
  allowedConsumers: ReadonlyArray<string>
  checksumAlgorithm: 'sha256'
  forbiddenConsumers: ReadonlyArray<string>
  guardDigest: string
  integrityMode: 'checksum_only_no_payload'
  issueRef: '#6419'
  metric: 'generalization_gain'
  partitions: ReadonlyArray<TassadarGeneralizationGuardPartition>
  schemaVersion: typeof TASSADAR_GENERALIZATION_GUARD_SCHEMA_VERSION
}>

export type TassadarGeneralizationGuardManifest = Readonly<{
  corpusId: string
  families?: ReadonlyArray<{
    familyId: string
    recordCount: number
    shard: string
    shardSha256: string
    tokenCount: number
  }>
  generalizationGuard?: TassadarGeneralizationGuard
  manifestVersion: string
  records?:
    | Readonly<{ sha256?: string }>
    | ReadonlyArray<{
        familyId: string
        shard: string
        split: string
        tokenCount: number
      }>
  shards?: ReadonlyArray<{
    familyId: string
    recordCount: number
    shard: string
    shardSha256: string
    tag?: string
    tokenCount: number
  }>
  snapshotDigest?: string
  splitPolicyVersion: string
}>

export type TassadarGeneralizationGuardViolation = Readonly<{
  detail: string
  kind:
    | 'missing_guard'
    | 'schema_version_mismatch'
    | 'integrity_mode_mismatch'
    | 'checksum_algorithm_mismatch'
    | 'missing_forbidden_consumer'
    | 'missing_allowed_consumer'
    | 'partition_overlap'
    | 'partition_missing'
    | 'guard_digest_mismatch'
    | 'held_out_shard_ref_malformed'
    | 'held_out_shard_ref_unknown'
    | 'held_out_shard_ref_checksum_mismatch'
    | 'held_out_partition_family_mismatch'
    | 'held_out_partition_count_mismatch'
    | 'held_out_partition_token_mismatch'
}>

type ManifestShard = Readonly<{
  familyId: string
  recordCount: number
  shard: string
  shardSha256: string
  tokenCount: number
}>

const manifestShards = (
  manifest: TassadarGeneralizationGuardManifest,
): ReadonlyArray<ManifestShard> => [
  ...(manifest.shards ?? []),
  ...(manifest.families ?? []),
]

const manifestRecordsDigest = (
  records: TassadarGeneralizationGuardManifest['records'],
): string | null =>
  records !== undefined && !Array.isArray(records) && 'sha256' in records
    ? (records.sha256 ?? null)
    : null

const recordsForSplit = (
  manifest: TassadarGeneralizationGuardManifest,
  split: string,
): ReadonlyArray<{
  familyId: string
  shard: string
  split: string
  tokenCount: number
}> => (Array.isArray(manifest.records)
  ? manifest.records.filter(record => record.split === split)
  : [])

const partitionRowsForHeldOut = (
  manifest: TassadarGeneralizationGuardManifest,
  partition: TassadarGeneralizationGuardPartition,
): ReadonlyArray<Pick<ManifestShard, 'familyId' | 'recordCount' | 'tokenCount'>> => {
  const records = recordsForSplit(manifest, partition.split)
  if (records.length > 0) {
    const byFamily = new Map<string, { recordCount: number; tokenCount: number }>()
    for (const record of records) {
      const current = byFamily.get(record.familyId) ?? {
        recordCount: 0,
        tokenCount: 0,
      }
      current.recordCount += 1
      current.tokenCount += record.tokenCount
      byFamily.set(record.familyId, current)
    }

    return [...byFamily].map(([familyId, totals]) => ({
      familyId,
      recordCount: totals.recordCount,
      tokenCount: totals.tokenCount,
    }))
  }

  const shardsByRef = new Map(
    manifestShards(manifest).map(shard => [
      `${shard.shard}#${shard.shardSha256}`,
      shard,
    ]),
  )

  return partition.shardRefs
    .map(ref => shardsByRef.get(ref))
    .filter((shard): shard is ManifestShard => shard !== undefined)
}

const validateHeldOutPartitionReferences = (
  manifest: TassadarGeneralizationGuardManifest,
  partition: TassadarGeneralizationGuardPartition,
): ReadonlyArray<TassadarGeneralizationGuardViolation> => {
  const violations: Array<TassadarGeneralizationGuardViolation> = []
  const shardsByPath = new Map(
    manifestShards(manifest).map(shard => [shard.shard, shard]),
  )

  for (const ref of partition.shardRefs) {
    const separatorIndex = ref.indexOf('#')
    if (
      separatorIndex <= 0 ||
      separatorIndex !== ref.lastIndexOf('#') ||
      separatorIndex === ref.length - 1
    ) {
      violations.push({
        detail: `Held-out shard ref ${ref} must be path#sha256.`,
        kind: 'held_out_shard_ref_malformed',
      })
      continue
    }

    const path = ref.slice(0, separatorIndex)
    const checksum = ref.slice(separatorIndex + 1)
    const shard = shardsByPath.get(path)
    if (shard === undefined) {
      violations.push({
        detail: `Held-out shard ref ${ref} does not match a manifest shard.`,
        kind: 'held_out_shard_ref_unknown',
      })
      continue
    }
    if (shard.shardSha256 !== checksum) {
      violations.push({
        detail: `Held-out shard ref ${ref} checksum does not match ${shard.shardSha256}.`,
        kind: 'held_out_shard_ref_checksum_mismatch',
      })
    }
  }

  const rows = partitionRowsForHeldOut(manifest, partition)
  const rowFamilies = [...new Set(rows.map(row => row.familyId))].sort()
  const partitionFamilies = [...partition.familyIds].sort()
  if (JSON.stringify(rowFamilies) !== JSON.stringify(partitionFamilies)) {
    violations.push({
      detail: `Held-out partition families ${partitionFamilies.join(',')} do not match manifest rows ${rowFamilies.join(',')}.`,
      kind: 'held_out_partition_family_mismatch',
    })
  }

  const recordCount = rows.reduce((sum, row) => sum + row.recordCount, 0)
  if (recordCount !== partition.recordCount) {
    violations.push({
      detail: `Held-out partition declares ${partition.recordCount} records but manifest rows sum to ${recordCount}.`,
      kind: 'held_out_partition_count_mismatch',
    })
  }

  const tokenCount = rows.reduce((sum, row) => sum + row.tokenCount, 0)
  if (tokenCount !== partition.tokenCount) {
    violations.push({
      detail: `Held-out partition declares ${partition.tokenCount} tokens but manifest rows sum to ${tokenCount}.`,
      kind: 'held_out_partition_token_mismatch',
    })
  }

  return violations
}

const digestInputForGuard = (
  manifest: TassadarGeneralizationGuardManifest,
  guard: TassadarGeneralizationGuard,
): string =>
  JSON.stringify({
    corpusId: manifest.corpusId,
    manifestVersion: manifest.manifestVersion,
    partitions: [...guard.partitions]
      .map(partition => ({
        familyIds: [...partition.familyIds].sort(),
        kind: partition.kind,
        recordCount: partition.recordCount,
        shardRefs: [...partition.shardRefs].sort(),
        split: partition.split,
        tokenCount: partition.tokenCount,
      }))
      .sort((a, b) => `${a.kind}:${a.split}`.localeCompare(`${b.kind}:${b.split}`)),
    recordsSha256: manifestRecordsDigest(manifest.records),
    schemaVersion: guard.schemaVersion,
    snapshotDigest: manifest.snapshotDigest ?? null,
    splitPolicyVersion: manifest.splitPolicyVersion,
  })

export const sha256HexOfGuardInput = async (input: string): Promise<string> => {
  const bytes = new TextEncoder().encode(input)
  const digest = await globalThis.crypto.subtle.digest('SHA-256', bytes)

  return [...new Uint8Array(digest)]
    .map(byte => byte.toString(16).padStart(2, '0'))
    .join('')
}

export const tassadarGeneralizationGuardDigest = async (
  manifest: TassadarGeneralizationGuardManifest,
  guard: TassadarGeneralizationGuard,
): Promise<string> => sha256HexOfGuardInput(digestInputForGuard(manifest, guard))

export const validateTassadarGeneralizationGuard = async (
  manifest: TassadarGeneralizationGuardManifest,
): Promise<ReadonlyArray<TassadarGeneralizationGuardViolation>> => {
  const guard = manifest.generalizationGuard
  if (guard === undefined) {
    return [{ detail: 'Manifest has no generalizationGuard.', kind: 'missing_guard' }]
  }

  const violations: Array<TassadarGeneralizationGuardViolation> = []
  if (guard.schemaVersion !== TASSADAR_GENERALIZATION_GUARD_SCHEMA_VERSION) {
    violations.push({
      detail: `Expected ${TASSADAR_GENERALIZATION_GUARD_SCHEMA_VERSION}, got ${guard.schemaVersion}.`,
      kind: 'schema_version_mismatch',
    })
  }
  if (guard.integrityMode !== 'checksum_only_no_payload') {
    violations.push({
      detail: `Expected checksum_only_no_payload, got ${guard.integrityMode}.`,
      kind: 'integrity_mode_mismatch',
    })
  }
  if (guard.checksumAlgorithm !== 'sha256') {
    violations.push({
      detail: `Expected sha256, got ${guard.checksumAlgorithm}.`,
      kind: 'checksum_algorithm_mismatch',
    })
  }
  for (const consumer of TASSADAR_GENERALIZATION_GUARD_FORBIDDEN_CONSUMERS) {
    if (!guard.forbiddenConsumers.includes(consumer)) {
      violations.push({
        detail: `Missing forbidden consumer ${consumer}.`,
        kind: 'missing_forbidden_consumer',
      })
    }
  }
  for (const consumer of TASSADAR_GENERALIZATION_GUARD_ALLOWED_CONSUMERS) {
    if (!guard.allowedConsumers.includes(consumer)) {
      violations.push({
        detail: `Missing allowed consumer ${consumer}.`,
        kind: 'missing_allowed_consumer',
      })
    }
  }

  const train = guard.partitions.filter(
    partition => partition.kind === 'train_allowed',
  )
  const heldOut = guard.partitions.filter(
    partition => partition.kind === 'gg_held_out',
  )
  if (train.length === 0) {
    violations.push({
      detail: 'No train_allowed partition is declared.',
      kind: 'partition_missing',
    })
  }
  if (heldOut.length === 0) {
    violations.push({
      detail: 'No gg_held_out partition is declared.',
      kind: 'partition_missing',
    })
  }

  const trainFamilies = new Set(train.flatMap(partition => partition.familyIds))
  for (const familyId of heldOut.flatMap(partition => partition.familyIds)) {
    if (trainFamilies.has(familyId)) {
      violations.push({
        detail: `Family ${familyId} appears in both train and GG held-out partitions.`,
        kind: 'partition_overlap',
      })
    }
  }
  for (const partition of heldOut) {
    violations.push(...validateHeldOutPartitionReferences(manifest, partition))
  }

  const expectedDigest = await tassadarGeneralizationGuardDigest(manifest, guard)
  if (guard.guardDigest !== expectedDigest) {
    violations.push({
      detail: `Guard digest ${guard.guardDigest} does not match ${expectedDigest}.`,
      kind: 'guard_digest_mismatch',
    })
  }

  return violations
}
