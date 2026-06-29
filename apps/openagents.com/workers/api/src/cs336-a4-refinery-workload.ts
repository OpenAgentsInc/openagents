/**
 * Bounded CS336 A4 data-refinery shard workload (issue #4680).
 *
 * Each shard runs one real deterministic A4 refinery stage over a
 * bounded, public-safe synthetic corpus and commits a SHA-256 digest
 * over the exact stage output plus public-safe counts. The digest is
 * the commitment the `deterministic_recompute` sampled re-runs check:
 * re-running the same stage on the same shard reproduces the digest;
 * any input perturbation changes it.
 *
 * The four stages mirror the Psionic
 * `psion_cs336_a4_data_refinery_reference_v1` adapters landed in
 * `psionic#1102` (PII masking, Gopher quality rules, exact-line dedup,
 * deterministic MinHash document dedup). This is the eval-delta-free
 * deterministic core of A4: the stages pay per *verified shard*, while
 * the eval-delta quality bonus stays a design-only policy until a
 * fixed-trainer eval loop and operator funding exist (see
 * `cs336-a4-data-refinery.ts`).
 *
 * Provenance binds to the A1 trainer (plan step 3): the synthetic
 * corpus seed derives from the A1 tokenizer shard digest in
 * `cs336-a1-homework-workload.ts`, so every A4 shard is downstream of
 * the same committed corpus pipeline as the #4675 run. The corpus is
 * synthetic by construction — the injected "PII" are template tokens
 * over a synthetic namespace, never real contributor data, so no real
 * Common Crawl payload or contributor-sourced sensitive material is
 * ever materialized or published. Outputs are public-safe: counts,
 * digests, and refs only.
 */

import { computeCs336A1TokenizerShard } from './cs336-a1-homework-workload'
import type { Cs336A4HomeworkStage } from './cs336-a4-data-refinery'

export class Cs336A4RefineryWorkloadError extends Error {
  readonly _tag = 'Cs336A4RefineryWorkloadError'
}

export const Cs336A4RefineryWorkloadRef =
  'workload.cs336_a4.seeded_synthetic_refinery.v1'
export const Cs336A4RefineryCorpusRef =
  'dataset.cs336_a4.seeded_synthetic_corpus.v1'

const Cs336A4DefaultDocumentCount = 64
const Cs336A4DefaultLinesPerDocument = 6
const Cs336A4MinHashBands = 8
const Cs336A4MinHashRowsPerBand = 2
const Cs336A4MinHashPermutations = Cs336A4MinHashBands * Cs336A4MinHashRowsPerBand
const Cs336A4ShingleSize = 3
const Cs336A4NearDuplicateJaccard = 0.7

const sha256Hex = async (value: string): Promise<string> => {
  const digest = await crypto.subtle.digest(
    'SHA-256',
    new TextEncoder().encode(value),
  )

  return [...new Uint8Array(digest)]
    .map(byte => byte.toString(16).padStart(2, '0'))
    .join('')
}

const fnv1a32 = (value: string): number => {
  let hash = 0x81_1c_9d_c5

  for (let index = 0; index < value.length; index += 1) {
    hash ^= value.charCodeAt(index)
    hash = Math.imul(hash, 0x01_00_01_93)
  }

  return hash >>> 0
}

/** Deterministic mulberry32 PRNG over a string-derived seed. */
const seededRandom = (seed: string): (() => number) => {
  let state = fnv1a32(seed)

  return () => {
    state = (state + 0x6d_2b_79_f5) >>> 0
    let mixed = state
    mixed = Math.imul(mixed ^ (mixed >>> 15), mixed | 1)
    mixed ^= mixed + Math.imul(mixed ^ (mixed >>> 7), mixed | 61)

    return ((mixed ^ (mixed >>> 14)) >>> 0) / 4_294_967_296
  }
}

const Cs336A4Topics = [
  'the harbor ledger reconciled every settled invoice',
  'a seeded model trained one bounded pass over synthetic tokens',
  'the validator recomputed the digest and the verdicts matched',
  'quality rules filtered the document before it reached the corpus',
  'duplicate lines were removed before the shard was committed',
  'the contributor device returned counts digests and refs only',
] as const

export type Cs336A4Document = Readonly<{
  documentRef: string
  lines: ReadonlyArray<string>
}>

/**
 * Builds the bounded synthetic corpus deterministically from the A1
 * tokenizer shard digest. The generator injects three controlled
 * conditions the refinery stages act on: repeated boilerplate lines
 * (exact-line dedup), near-duplicate documents (MinHash dedup), and
 * synthetic template "PII" plus low-quality documents (PII masking and
 * Gopher rules). Nothing here is real data: the emails, phones, and
 * IPv4 strings are template tokens over a synthetic `example.invalid`
 * namespace.
 */
export const buildCs336A4SyntheticCorpus = (
  input: Readonly<{
    documentCount?: number
    linesPerDocument?: number
    shardDigestHex: string
  }>,
): ReadonlyArray<Cs336A4Document> => {
  const documentCount = input.documentCount ?? Cs336A4DefaultDocumentCount
  const linesPerDocument = input.linesPerDocument ?? Cs336A4DefaultLinesPerDocument

  if (!Number.isInteger(documentCount) || documentCount < 4) {
    throw new Cs336A4RefineryWorkloadError(
      'CS336 A4 corpus needs at least 4 documents.',
    )
  }

  if (!Number.isInteger(linesPerDocument) || linesPerDocument < 2) {
    throw new Cs336A4RefineryWorkloadError(
      'CS336 A4 corpus needs at least 2 lines per document.',
    )
  }

  const random = seededRandom(`cs336_a4.corpus.${input.shardDigestHex}`)
  const boilerplate = 'shared boilerplate footer line for the synthetic corpus'
  const documents: Cs336A4Document[] = []

  for (let documentIndex = 0; documentIndex < documentCount; documentIndex += 1) {
    const lines: string[] = []
    // Near-duplicate clusters: every fourth doc is a light edit of the
    // previous one, so MinHash dedup has real near-duplicates to find.
    const nearDuplicateOf =
      documentIndex % 4 === 3 && documents.length > 0
        ? documents[documents.length - 1]!
        : undefined

    for (let lineIndex = 0; lineIndex < linesPerDocument; lineIndex += 1) {
      if (nearDuplicateOf !== undefined && lineIndex < linesPerDocument - 1) {
        lines.push(nearDuplicateOf.lines[lineIndex]!)
        continue
      }

      // Repeated boilerplate (exact-line dedup target).
      if (lineIndex === linesPerDocument - 1) {
        lines.push(boilerplate)
        continue
      }

      const topic = Cs336A4Topics[Math.floor(random() * Cs336A4Topics.length)]!
      const draw = random()

      if (draw < 0.18) {
        // Synthetic PII line (masking target). example.invalid is a
        // reserved non-routable namespace; the IPv4 is documentation-range.
        lines.push(
          `contact user${documentIndex}@example.invalid or call 555-0${(100 + documentIndex).toString().padStart(3, '0')} from 192.0.2.${documentIndex % 254}`,
        )
      } else if (draw < 0.3) {
        // Low-quality line (Gopher rule target): symbol-heavy, low alpha.
        lines.push('### $$$ %% @@ ^^ && ** (( )) -- ++ == ~~ ## !!')
      } else {
        lines.push(`${topic} number ${documentIndex}.${lineIndex}`)
      }
    }

    documents.push({
      documentRef: `doc.cs336_a4.${documentIndex}`,
      lines,
    })
  }

  return documents
}

export type Cs336A4StageResult = Readonly<{
  elapsedMs: number
  inputDocumentCount: number
  outputDigestHex: string
  stage: Cs336A4HomeworkStage
  stats: Readonly<Record<string, number>>
  workloadRef: typeof Cs336A4RefineryWorkloadRef
}>

const emailPattern = /[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}/g
const phonePattern = /\b\d{3}-\d{4}\b/g
const ipv4Pattern = /\b(?:\d{1,3}\.){3}\d{1,3}\b/g

const runPiiMasking = (
  documents: ReadonlyArray<Cs336A4Document>,
): { maskedLines: ReadonlyArray<string>; stats: Record<string, number> } => {
  let maskedEmails = 0
  let maskedPhones = 0
  let maskedIpv4 = 0
  const maskedLines: string[] = []

  for (const document of documents) {
    for (const line of document.lines) {
      const afterEmail = line.replace(emailPattern, () => {
        maskedEmails += 1
        return '[EMAIL]'
      })
      const afterIpv4 = afterEmail.replace(ipv4Pattern, () => {
        maskedIpv4 += 1
        return '[IPV4]'
      })
      const afterPhone = afterIpv4.replace(phonePattern, () => {
        maskedPhones += 1
        return '[PHONE]'
      })

      maskedLines.push(afterPhone)
    }
  }

  return {
    maskedLines,
    stats: {
      maskedEmails,
      maskedIpv4,
      maskedPhones,
      maskedTotal: maskedEmails + maskedPhones + maskedIpv4,
    },
  }
}

const runExactLineDedup = (
  documents: ReadonlyArray<Cs336A4Document>,
): { keptLines: ReadonlyArray<string>; stats: Record<string, number> } => {
  const seen = new Set<string>()
  const keptLines: string[] = []
  let inputLines = 0

  for (const document of documents) {
    for (const line of document.lines) {
      inputLines += 1

      if (seen.has(line)) {
        continue
      }

      seen.add(line)
      keptLines.push(line)
    }
  }

  return {
    keptLines,
    stats: {
      inputLines,
      removedLines: inputLines - keptLines.length,
      uniqueLines: keptLines.length,
    },
  }
}

const gopherVerdict = (
  text: string,
): { rejectionRule: string | null } => {
  const characters = [...text]
  const alphaCount = characters.filter(character =>
    /[A-Za-z]/.test(character),
  ).length
  const alphaRatio = characters.length === 0 ? 0 : alphaCount / characters.length
  const words = text.split(/\s+/).filter(word => word.length > 0)

  if (words.length < 3) {
    return { rejectionRule: 'too_few_words' }
  }

  if (alphaRatio < 0.6) {
    return { rejectionRule: 'low_alpha_ratio' }
  }

  const symbolCount = characters.filter(character =>
    /[#$%^&*()\-+=~!]/.test(character),
  ).length

  if (characters.length > 0 && symbolCount / characters.length > 0.1) {
    return { rejectionRule: 'symbol_heavy' }
  }

  return { rejectionRule: null }
}

const runGopherRules = (
  documents: ReadonlyArray<Cs336A4Document>,
): {
  keptDocumentRefs: ReadonlyArray<string>
  stats: Record<string, number>
} => {
  const keptDocumentRefs: string[] = []
  let rejectedTooFewWords = 0
  let rejectedLowAlpha = 0
  let rejectedSymbolHeavy = 0

  for (const document of documents) {
    const text = document.lines.join(' ')
    const { rejectionRule } = gopherVerdict(text)

    if (rejectionRule === null) {
      keptDocumentRefs.push(document.documentRef)
      continue
    }

    if (rejectionRule === 'too_few_words') {
      rejectedTooFewWords += 1
    } else if (rejectionRule === 'low_alpha_ratio') {
      rejectedLowAlpha += 1
    } else {
      rejectedSymbolHeavy += 1
    }
  }

  return {
    keptDocumentRefs,
    stats: {
      inputDocuments: documents.length,
      keptDocuments: keptDocumentRefs.length,
      rejectedLowAlphaRatio: rejectedLowAlpha,
      rejectedSymbolHeavy,
      rejectedTooFewWords,
    },
  }
}

const documentShingles = (document: Cs336A4Document): ReadonlySet<number> => {
  const tokens = document.lines
    .join(' ')
    .toLowerCase()
    .split(/[^a-z0-9]+/)
    .filter(token => token.length > 0)
  const shingles = new Set<number>()

  for (
    let index = 0;
    index + Cs336A4ShingleSize <= tokens.length;
    index += 1
  ) {
    shingles.add(
      fnv1a32(tokens.slice(index, index + Cs336A4ShingleSize).join(' ')),
    )
  }

  // Degenerate short documents still get at least one shingle.
  if (shingles.size === 0 && tokens.length > 0) {
    shingles.add(fnv1a32(tokens.join(' ')))
  }

  return shingles
}

const exactJaccard = (
  left: ReadonlySet<number>,
  right: ReadonlySet<number>,
): number => {
  if (left.size === 0 && right.size === 0) {
    return 1
  }

  let intersection = 0

  for (const value of left) {
    if (right.has(value)) {
      intersection += 1
    }
  }

  return intersection / (left.size + right.size - intersection)
}

/**
 * Deterministic MinHash + LSH banding document dedup. Hash family is
 * seeded from the shard digest, candidate pairs come from shared LSH
 * bands, and every candidate is confirmed by exact Jaccard before being
 * unioned — so the kept-document set is a deterministic function of the
 * corpus, never a probabilistic approximation that drifts between runs.
 */
const runMinHashDedup = (
  documents: ReadonlyArray<Cs336A4Document>,
  shardDigestHex: string,
): {
  keptDocumentRefs: ReadonlyArray<string>
  stats: Record<string, number>
} => {
  const hashSeeds: number[] = []
  const seedRandom = seededRandom(`cs336_a4.minhash.${shardDigestHex}`)

  for (let index = 0; index < Cs336A4MinHashPermutations; index += 1) {
    hashSeeds.push((Math.floor(seedRandom() * 0xff_ff_ff_ff) >>> 0) | 1)
  }

  const shingleSets = documents.map(documentShingles)
  const signatures = shingleSets.map(shingles => {
    const signature = new Array<number>(Cs336A4MinHashPermutations).fill(
      0xff_ff_ff_ff,
    )

    for (const shingle of shingles) {
      for (let index = 0; index < Cs336A4MinHashPermutations; index += 1) {
        const hashed = (Math.imul(shingle ^ hashSeeds[index]!, 0x01_00_01_93) >>> 0)

        if (hashed < signature[index]!) {
          signature[index] = hashed
        }
      }
    }

    return signature
  })

  const parent = documents.map((_, index) => index)
  const find = (node: number): number => {
    let root = node

    while (parent[root] !== root) {
      root = parent[root]!
    }

    let current = node

    while (parent[current] !== root) {
      const next = parent[current]!
      parent[current] = root
      current = next
    }

    return root
  }
  const union = (left: number, right: number): void => {
    const leftRoot = find(left)
    const rightRoot = find(right)

    if (leftRoot !== rightRoot) {
      parent[Math.max(leftRoot, rightRoot)] = Math.min(leftRoot, rightRoot)
    }
  }

  let confirmedPairs = 0

  for (let band = 0; band < Cs336A4MinHashBands; band += 1) {
    const buckets = new Map<string, number[]>()

    for (let document = 0; document < signatures.length; document += 1) {
      const start = band * Cs336A4MinHashRowsPerBand
      const key = signatures[document]!
        .slice(start, start + Cs336A4MinHashRowsPerBand)
        .join(':')
      const bucket = buckets.get(key) ?? []

      bucket.push(document)
      buckets.set(key, bucket)
    }

    for (const bucket of buckets.values()) {
      for (let left = 0; left < bucket.length; left += 1) {
        for (let right = left + 1; right < bucket.length; right += 1) {
          const a = bucket[left]!
          const b = bucket[right]!

          if (
            find(a) !== find(b) &&
            exactJaccard(shingleSets[a]!, shingleSets[b]!) >=
              Cs336A4NearDuplicateJaccard
          ) {
            union(a, b)
            confirmedPairs += 1
          }
        }
      }
    }
  }

  const keptDocumentRefs: string[] = []
  const clusterRoots = new Set<number>()

  for (let index = 0; index < documents.length; index += 1) {
    const root = find(index)

    clusterRoots.add(root)

    if (root === index) {
      keptDocumentRefs.push(documents[index]!.documentRef)
    }
  }

  return {
    keptDocumentRefs,
    stats: {
      confirmedNearDuplicatePairs: confirmedPairs,
      inputDocuments: documents.length,
      keptDocuments: keptDocumentRefs.length,
      nearDuplicateClusters: clusterRoots.size,
      removedDocuments: documents.length - keptDocumentRefs.length,
    },
  }
}

/**
 * Runs one A4 refinery stage over the bounded synthetic corpus and
 * returns the public-safe counts plus the SHA-256 output commitment.
 * Fully deterministic for a given shard digest, which is what the
 * sampled-shard `deterministic_recompute` re-runs rely on.
 */
export const runCs336A4RefineryStage = async (
  input: Readonly<{
    documentCount?: number
    linesPerDocument?: number
    stage: Cs336A4HomeworkStage
  }>,
): Promise<Cs336A4StageResult> => {
  const startedAt = performance.now()
  const shard = await computeCs336A1TokenizerShard()
  const documents = buildCs336A4SyntheticCorpus({
    ...(input.documentCount === undefined
      ? {}
      : { documentCount: input.documentCount }),
    ...(input.linesPerDocument === undefined
      ? {}
      : { linesPerDocument: input.linesPerDocument }),
    shardDigestHex: shard.digestHex,
  })

  const { stats, outputBody } = ((): {
    outputBody: unknown
    stats: Record<string, number>
  } => {
    if (input.stage === 'pii_masking') {
      const result = runPiiMasking(documents)
      return { outputBody: result.maskedLines, stats: result.stats }
    }

    if (input.stage === 'exact_line_dedup') {
      const result = runExactLineDedup(documents)
      return { outputBody: result.keptLines, stats: result.stats }
    }

    if (input.stage === 'gopher_rules') {
      const result = runGopherRules(documents)
      return { outputBody: result.keptDocumentRefs, stats: result.stats }
    }

    const result = runMinHashDedup(documents, shard.digestHex)
    return { outputBody: result.keptDocumentRefs, stats: result.stats }
  })()

  const outputDigestHex = await sha256Hex(
    JSON.stringify({
      output: outputBody,
      shardDigestHex: shard.digestHex,
      stage: input.stage,
      stats,
      workloadRef: Cs336A4RefineryWorkloadRef,
    }),
  )

  return {
    elapsedMs: performance.now() - startedAt,
    inputDocumentCount: documents.length,
    outputDigestHex,
    stage: input.stage,
    stats,
    workloadRef: Cs336A4RefineryWorkloadRef,
  }
}
