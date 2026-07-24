/**
 * HANDS-5 (#9176) — read-only FOLDER `RlmCorpusSource` over a bounded directory
 * of Markdown files.
 *
 * The existing production corpus adapters
 * ({@link ./desktop-history-corpus-source.ts} and
 * `./managed-rlm-corpus-policy.ts`) scope Thread / Run / ThreadSet over the
 * durable event log. Neither reads a folder of Markdown files, so RLM could not
 * point at `docs/transcripts/` (or repository docs) through a production adapter
 * — only the SDK inline builder in tests. This adapter closes that gap.
 *
 * Boundaries (kept identical to the other adapters):
 * - READ-ONLY. It lists and reads files under one host-configured root. It
 *   never writes, deletes, or follows a model-supplied path. The `Source`
 *   corpus ref carries ONLY a scope label that must match the host config; a
 *   model cannot redirect the traversal or widen it to another directory.
 * - BOUNDED. File count, per-file bytes, entry count, per-entry chars, and
 *   total encoded bytes are all capped below the SDK's 4 MiB inline ceiling.
 *   Truncations and skips are recorded as coverage exclusions, never silent.
 * - DETERMINISTIC. Files are sorted by relative path; paragraphs keep their
 *   in-file order. The same directory content always yields the same corpus
 *   content digest, so citations round-trip.
 * - The host owns visibility/redaction. Defaults are `public` / `none` because
 *   the transcript and repository-docs corpora are public; a host may narrow.
 *
 * The SDK owns digests, traversal, and citation validation. This module only
 * turns a directory into the generic immutable `RlmCorpusHandle`.
 */

import { Context, Effect, Layer } from "effect"
import { readFile, readdir, stat } from "node:fs/promises"
import path from "node:path"
import {
  buildInlineCorpusInput,
  makeInlineCorpusHandle,
  RlmCorpusError,
  RlmCorpusSource,
  type RlmCorpusHandle,
  type RlmCorpusInput,
  type RlmCorpusSourceShape,
  type RlmRedactionClass,
  type RlmSourcePlane,
  type RlmVisibility,
} from "@openagentsinc/rlm"

/** Address scheme for folder citations round-tripping to `<relPath>#p<index>`. */
export const FOLDER_CORPUS_ADDRESS_SCHEMA_ID = "openagents.desktop.folder_corpus.v1" as const

/** Strategy profile pin for a folder-corpus RLM train (no artifact sink). */
export const FOLDER_CORPUS_STRATEGY_REF = "openagents.desktop.rlm.folder.v1" as const

/** The `repository` source plane fits both transcript and repo-docs folders. */
const FOLDER_SOURCE_PLANE: RlmSourcePlane = "repository"
const FOLDER_SOURCE_KIND = "markdown.paragraph" as const

/**
 * Host-owned configuration for one folder corpus. The caller supplies an
 * absolute root and bounded caps. Nothing here is model-supplied.
 */
export interface FolderRlmCorpusConfig {
  /** Absolute directory to traverse read-only. */
  readonly rootDir: string
  /**
   * Stable scope label used in the scopeRef and the `Source` ref. Defaults to
   * the root basename. The `Source` ref must name this exact label, so a model
   * cannot point the source at another directory.
   */
  readonly scopeLabel?: string
  /**
   * Optional explicit relative-path allowlist (in traversal order after
   * sorting). When absent, every file with a matching extension directly under
   * `rootDir` is used, sorted by name.
   */
  readonly include?: ReadonlyArray<string>
  /** File extensions to include. Default `[".md"]`. */
  readonly fileExtensions?: ReadonlyArray<string>
  /** Max files traversed. Default 64. Extra files recorded as an exclusion. */
  readonly maxFiles?: number
  /** Max bytes read per file. Default 262144. Larger files are truncated. */
  readonly maxBytesPerFile?: number
  /** Max paragraph entries. Default 4000. Extra entries recorded as excluded. */
  readonly maxEntries?: number
  /** Paragraphs shorter than this many chars are skipped. Default 24. */
  readonly minEntryChars?: number
  /** Per-entry text truncation. Default 2048. */
  readonly maxCharsPerEntry?: number
  /**
   * Total encoded-byte ceiling for the built corpus. Default 3.5 MiB, kept
   * below the SDK's 4 MiB inline ceiling so a bounded folder never trips it.
   */
  readonly maxTotalEncodedBytes?: number
  readonly visibility?: RlmVisibility
  readonly redactionClass?: RlmRedactionClass
  /** Stable build timestamp override (tests). */
  readonly builtAt?: () => string
}

interface ResolvedCaps {
  readonly rootDir: string
  readonly scopeLabel: string
  readonly scopeRef: string
  readonly fileExtensions: ReadonlyArray<string>
  readonly maxFiles: number
  readonly maxBytesPerFile: number
  readonly maxEntries: number
  readonly minEntryChars: number
  readonly maxCharsPerEntry: number
  readonly maxTotalEncodedBytes: number
  readonly visibility: RlmVisibility
  readonly redactionClass: RlmRedactionClass
}

const SDK_INLINE_BYTE_CEILING = 4 * 1024 * 1024

const resolveCaps = (config: FolderRlmCorpusConfig): ResolvedCaps => {
  const scopeLabel =
    config.scopeLabel ?? (path.basename(config.rootDir.replace(/[/\\]+$/, "")) || "folder")
  return {
    rootDir: config.rootDir,
    scopeLabel,
    scopeRef: `folder:${scopeLabel}`,
    fileExtensions: config.fileExtensions ?? [".md"],
    maxFiles: Math.max(1, Math.floor(config.maxFiles ?? 64)),
    maxBytesPerFile: Math.max(1, Math.floor(config.maxBytesPerFile ?? 262_144)),
    maxEntries: Math.max(1, Math.floor(config.maxEntries ?? 4000)),
    minEntryChars: Math.max(1, Math.floor(config.minEntryChars ?? 24)),
    maxCharsPerEntry: Math.max(1, Math.floor(config.maxCharsPerEntry ?? 2048)),
    maxTotalEncodedBytes: Math.min(
      SDK_INLINE_BYTE_CEILING - 1,
      Math.max(1, Math.floor(config.maxTotalEncodedBytes ?? 3_670_016)),
    ),
    visibility: config.visibility ?? "public",
    redactionClass: config.redactionClass ?? "none",
  }
}

/**
 * Encode a folder citation address as a JSON envelope of the file relative path
 * and the in-file paragraph index. JSON avoids any separator-byte ambiguity for
 * paths that themselves contain unusual characters.
 */
export const encodeFolderCorpusAddress = (input: {
  readonly relPath: string
  readonly paragraphIndex: number
}): { readonly addressSchemaId: string; readonly encodedAddress: string } => ({
  addressSchemaId: FOLDER_CORPUS_ADDRESS_SCHEMA_ID,
  encodedAddress: JSON.stringify({ f: input.relPath, p: input.paragraphIndex }),
})

/** Split a folder citation encoded address into `<relPath>` and `<index>`. */
export const splitFolderCorpusAddress = (
  encodedAddress: string,
): { readonly relPath: string; readonly paragraphIndex: number } | null => {
  try {
    const parsed = JSON.parse(encodedAddress) as unknown
    if (typeof parsed !== "object" || parsed === null) return null
    const relPath = (parsed as { f?: unknown }).f
    const paragraphIndex = (parsed as { p?: unknown }).p
    if (typeof relPath !== "string" || relPath.length === 0) return null
    if (typeof paragraphIndex !== "number" || !Number.isInteger(paragraphIndex)) return null
    if (paragraphIndex < 0) return null
    return { relPath, paragraphIndex }
  } catch {
    return null
  }
}

/** Decode a folder citation address; null when the schema does not match. */
export const decodeFolderCorpusAddress = (address: {
  readonly addressSchemaId: string
  readonly encodedAddress: string
}): { readonly relPath: string; readonly paragraphIndex: number } | null => {
  if (address.addressSchemaId !== FOLDER_CORPUS_ADDRESS_SCHEMA_ID) return null
  return splitFolderCorpusAddress(address.encodedAddress)
}

/** Split Markdown into ordered paragraph strings (blank-line separated). */
export const splitMarkdownParagraphs = (text: string): ReadonlyArray<string> =>
  text
    .split(/\r?\n\s*\r?\n/)
    .map((paragraph) => paragraph.replace(/\s+/g, " ").trim())
    .filter((paragraph) => paragraph.length > 0)

interface FolderCorpusBuild {
  readonly corpusInput: Extract<RlmCorpusInput, { readonly _tag: "Inline" }>
  readonly entryCount: number
  readonly fileCount: number
  readonly exclusions: ReadonlyArray<{ readonly reason: string; readonly count: number }>
}

const corpusUnavailable = (detailSafe: string): RlmCorpusError =>
  new RlmCorpusError({ reason: "unavailable", detailSafe })

/** List the target files under the root in deterministic traversal order. */
const listFolderFiles = (
  caps: ResolvedCaps,
  include?: ReadonlyArray<string>,
): Effect.Effect<ReadonlyArray<string>, RlmCorpusError> =>
  Effect.gen(function* () {
    if (include !== undefined) {
      return include.toSorted((left, right) => left.localeCompare(right))
    }
    const names = yield* Effect.tryPromise({
      try: () => readdir(caps.rootDir),
      catch: (cause) => corpusUnavailable(`folder read failed: ${String(cause).slice(0, 120)}`),
    })
    return names
      .filter((name) => caps.fileExtensions.some((ext) => name.endsWith(ext)))
      .toSorted((left, right) => left.localeCompare(right))
  })

/**
 * Read the bounded folder into ordered paragraph entries and build the generic
 * inline corpus. Deterministic ordering; every skip/truncation recorded.
 */
export const buildFolderRlmCorpus = (
  config: FolderRlmCorpusConfig,
): Effect.Effect<FolderCorpusBuild, RlmCorpusError> =>
  Effect.gen(function* () {
    const caps = resolveCaps(config)
    const allFiles = yield* listFolderFiles(caps, config.include)
    const files = allFiles.slice(0, caps.maxFiles)

    const entries: Array<{
      readonly scopeRef: string
      readonly sourceKind: string
      readonly sourcePlane: RlmSourcePlane
      readonly sourceAddress: { readonly addressSchemaId: string; readonly encodedAddress: string }
      readonly entryRef: string
      readonly text: string
      readonly visibility: RlmVisibility
      readonly redactionClass: RlmRedactionClass
    }> = []

    const excludedFiles = allFiles.length - files.length
    let excludedSmallParagraphs = 0
    let truncatedFiles = 0
    let truncatedEntries = 0
    let cappedByEntryLimit = 0
    let cappedByByteLimit = 0
    let approxEncodedBytes = 0
    let entriesCappedStop = false

    for (const relPath of files) {
      if (entriesCappedStop) break
      const absPath = path.join(caps.rootDir, relPath)
      // Read-only guard: only regular files directly resolvable under root.
      const info = yield* Effect.tryPromise({
        try: () => stat(absPath),
        catch: () => null,
      }).pipe(Effect.orElseSucceed(() => null))
      if (info === null || !info.isFile()) continue

      const raw = yield* Effect.tryPromise({
        try: () => readFile(absPath, "utf8"),
        catch: () => null,
      }).pipe(Effect.orElseSucceed(() => null))
      if (raw === null) continue

      const bounded = raw.length > caps.maxBytesPerFile ? raw.slice(0, caps.maxBytesPerFile) : raw
      if (bounded.length < raw.length) truncatedFiles += 1

      const paragraphs = splitMarkdownParagraphs(bounded)
      for (let index = 0; index < paragraphs.length; index++) {
        if (entries.length >= caps.maxEntries) {
          cappedByEntryLimit += paragraphs.length - index
          entriesCappedStop = true
          break
        }
        const paragraph = paragraphs[index]!
        if (paragraph.length < caps.minEntryChars) {
          excludedSmallParagraphs += 1
          continue
        }
        const text =
          paragraph.length > caps.maxCharsPerEntry
            ? paragraph.slice(0, caps.maxCharsPerEntry)
            : paragraph
        if (text.length < paragraph.length) truncatedEntries += 1

        // Deterministic per-entry byte estimate mirrors the SDK JSON encoding
        // closely enough to stop before the inline ceiling.
        const entryBytes = text.length + relPath.length + 80
        if (approxEncodedBytes + entryBytes > caps.maxTotalEncodedBytes) {
          cappedByByteLimit += 1
          entriesCappedStop = true
          break
        }
        approxEncodedBytes += entryBytes

        entries.push({
          scopeRef: caps.scopeRef,
          sourceKind: FOLDER_SOURCE_KIND,
          sourcePlane: FOLDER_SOURCE_PLANE,
          sourceAddress: encodeFolderCorpusAddress({ relPath, paragraphIndex: index }),
          entryRef: `${relPath}#p${index}`,
          text,
          visibility: caps.visibility,
          redactionClass: caps.redactionClass,
        })
      }
    }

    if (entries.length === 0) {
      return yield* corpusUnavailable("folder corpus resolved to zero entries")
    }

    const exclusions: Array<{ readonly reason: string; readonly count: number }> = []
    if (excludedFiles > 0)
      exclusions.push({ reason: "excluded_by_file_cap", count: excludedFiles })
    if (truncatedFiles > 0)
      exclusions.push({ reason: "file_truncated_by_byte_cap", count: truncatedFiles })
    if (excludedSmallParagraphs > 0)
      exclusions.push({ reason: "excluded_short_paragraph", count: excludedSmallParagraphs })
    if (truncatedEntries > 0)
      exclusions.push({ reason: "entry_truncated_by_char_cap", count: truncatedEntries })
    if (cappedByEntryLimit > 0)
      exclusions.push({ reason: "excluded_by_entry_cap", count: cappedByEntryLimit })
    if (cappedByByteLimit > 0)
      exclusions.push({ reason: "excluded_by_total_byte_cap", count: cappedByByteLimit })

    const corpusInput = buildInlineCorpusInput({
      corpusRef: `desktop.folder:${caps.scopeRef}`,
      scopeRef: caps.scopeRef,
      entries: entries.map((entry) => ({
        scopeRef: entry.scopeRef,
        sourceKind: entry.sourceKind,
        sourcePlane: entry.sourcePlane,
        sourceAddress: entry.sourceAddress,
        entryRef: entry.entryRef,
        text: entry.text,
        visibility: entry.visibility,
        redactionClass: entry.redactionClass,
      })),
      policy: {
        includeVisibilities: [caps.visibility],
        includeRedactionClasses: [caps.redactionClass],
      },
      orderingRule: "source_declared",
      orderingNote: "Folder files sorted by relative path; paragraphs keep in-file order.",
      coverageNote: `Bounded read-only folder corpus for ${caps.scopeRef}.`,
      exclusions,
    })

    return {
      corpusInput,
      entryCount: entries.length,
      fileCount: files.length,
      exclusions,
    } satisfies FolderCorpusBuild
  })

/** Build one authorized RLM corpus handle from a host-configured folder. */
export const resolveFolderRlmCorpus = (
  config: FolderRlmCorpusConfig,
): Effect.Effect<RlmCorpusHandle, RlmCorpusError> =>
  buildFolderRlmCorpus(config).pipe(
    Effect.flatMap((build) => makeInlineCorpusHandle(build.corpusInput)),
  )

/**
 * Parse a folder `Source` ref. It carries ONLY the scope label, which must
 * match the host config exactly. A model-supplied ref that names another label
 * is refused before any filesystem read — the traversal root and caps are
 * always the host's.
 */
export const parseFolderCorpusScopeLabel = (encodedAddress: string): string | null => {
  try {
    const parsed = JSON.parse(encodedAddress) as unknown
    if (typeof parsed !== "object" || parsed === null) return null
    const label = (parsed as { scopeLabel?: unknown }).scopeLabel
    return typeof label === "string" && label.length > 0 ? label : null
  } catch {
    return null
  }
}

/** Build a `Source` corpus input naming the host-configured folder scope. */
export const folderCorpusSourceInput = (
  config: FolderRlmCorpusConfig,
): Extract<RlmCorpusInput, { readonly _tag: "Source" }> => {
  const scopeLabel = resolveCaps(config).scopeLabel
  return {
    _tag: "Source",
    sourceRef: {
      addressSchemaId: FOLDER_CORPUS_ADDRESS_SCHEMA_ID,
      encodedAddress: JSON.stringify({ scopeLabel }),
    },
  }
}

/**
 * A folder-backed `RlmCorpusSource` bound to one host config. `Inline` is
 * accepted only for hermetic tests; a `Source` ref must name the configured
 * scope label. Anything else is refused before a read.
 */
export const makeFolderRlmCorpusSource = (config: FolderRlmCorpusConfig): RlmCorpusSourceShape => {
  const scopeLabel = resolveCaps(config).scopeLabel
  return {
    resolve: (input) =>
      Effect.gen(function* () {
        if (input._tag === "Inline") {
          return yield* makeInlineCorpusHandle(input)
        }
        if (input.sourceRef.addressSchemaId !== FOLDER_CORPUS_ADDRESS_SCHEMA_ID) {
          return yield* corpusUnavailable("unsupported corpus source address schema")
        }
        const requested = parseFolderCorpusScopeLabel(input.sourceRef.encodedAddress)
        if (requested === null || requested !== scopeLabel) {
          return yield* new RlmCorpusError({
            reason: "invalid_inline",
            detailSafe: "corpus source scope label is not the host-authorized folder",
          })
        }
        return yield* resolveFolderRlmCorpus(config)
      }),
  }
}

export class FolderRlmCorpusSource extends Context.Service<
  FolderRlmCorpusSource,
  RlmCorpusSourceShape
>()("@openagentsinc/desktop/FolderRlmCorpusSource") {}

export const folderRlmCorpusSourceLayer = (
  config: FolderRlmCorpusConfig,
): Layer.Layer<RlmCorpusSource | FolderRlmCorpusSource> => {
  const shape = makeFolderRlmCorpusSource(config)
  return Layer.mergeAll(
    Layer.succeed(RlmCorpusSource, RlmCorpusSource.of(shape)),
    Layer.succeed(FolderRlmCorpusSource, FolderRlmCorpusSource.of(shape)),
  )
}
