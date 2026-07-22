/**
 * HANDS-5 (#9176) — folder-corpus roadmap recall proof.
 *
 * Two kinds of proof, both spend-free (Tier D deterministic and a scripted
 * Tier S model with fixed tokens):
 *
 * 1. Hermetic: a temp folder of synthetic Markdown proves the folder corpus
 *    adapter + {@link runFullAutoRoadmapRecall} produce bounded CITED candidates
 *    deterministically, that a scoped `Source` ref cannot point at another
 *    directory, and that an admitted scripted Tier S synthesis commits with
 *    preserved citations (else `invalid_citations`).
 * 2. Real bounded run: the actual transcript archive under `docs/transcripts/`
 *    (episodes 248–251) is mined into a cited candidate feature list with zero
 *    provider spend, matching the audit's demonstration.
 */

import { existsSync, mkdtempSync, writeFileSync } from "node:fs"
import { tmpdir } from "node:os"
import path from "node:path"

import { Effect } from "effect"
import { describe, expect, test } from "vite-plus/test"

import {
  buildFolderRlmCorpus,
  makeFolderRlmCorpusSource,
  resolveFolderRlmCorpus,
  splitMarkdownParagraphs,
  type FolderRlmCorpusConfig,
} from "./folder-corpus-source.ts"
import type {
  DesktopRlmCompleteFn,
  DesktopSemanticRecallAdmission,
} from "./history-recall-semantic.ts"
import {
  FULL_AUTO_ROADMAP_SIGNAL_PATTERN,
  fullAutoRoadmapRecallIsFrameable,
  fullAutoRoadmapRecallText,
  runFullAutoRoadmapRecall,
} from "./full-auto-recall.ts"

const REPO_ROOT = path.resolve(import.meta.dirname, "../../..")
const TRANSCRIPTS_DIR = path.join(REPO_ROOT, "docs/transcripts")

const ADMISSION: DesktopSemanticRecallAdmission = {
  admitted: true,
  basis: "user_explicit",
  grantRef: "grant.test.hands5.1",
}

// ---------------------------------------------------------------------------
// Synthetic folder fixture (hermetic, deterministic).
// ---------------------------------------------------------------------------

const makeSyntheticFolder = (): string => {
  const root = mkdtempSync(path.join(tmpdir(), "hands5-folder-"))
  writeFileSync(
    path.join(root, "001.md"),
    [
      "# Episode one",
      "We need a conversation-first desktop with durable unattended work.",
      "A short line.",
      "The product should schedule overnight work across several coding agents.",
    ].join("\n\n"),
    "utf8",
  )
  writeFileSync(
    path.join(root, "002.md"),
    [
      "# Episode two",
      "Nothing decided here, just a routine status update on the build.",
      "It would be great to inspect every subagent from one clickable flow.",
    ].join("\n\n"),
    "utf8",
  )
  return root
}

const syntheticConfig = (root: string): FolderRlmCorpusConfig => ({
  rootDir: root,
  scopeLabel: "synthetic",
  minEntryChars: 12,
})

// ---------------------------------------------------------------------------
// Scripted Tier S model (no spend): Grep → Commit(cite hits).
// ---------------------------------------------------------------------------

const grepCommitRoot =
  (pattern: string): DesktopRlmCompleteFn =>
  () =>
    Effect.succeed({
      text: JSON.stringify({
        schemaId: "openagents.ai.rlm_program.v1",
        programRef: "program.roadmap-grep-commit",
        nodes: [
          {
            _tag: "CorpusOp",
            nodeRef: "n1",
            operator: "Grep",
            params: { pattern, caseSensitive: false },
            inputValueRefs: [],
            outputValueRef: "v1",
          },
          { _tag: "Commit", nodeRef: "n2", valueRef: "v1", citationValueRefs: ["v1"] },
        ],
      }),
      inputTokens: 120,
      outputTokens: 24,
    })

describe("HANDS-5 folder-corpus splitter and adapter", () => {
  test("splitMarkdownParagraphs yields ordered non-empty paragraphs", () => {
    const paragraphs = splitMarkdownParagraphs("A one.\n\nTwo two.\n\n\n  \n\nThree.")
    expect(paragraphs).toEqual(["A one.", "Two two.", "Three."])
  })

  test("buildFolderRlmCorpus reads a bounded folder into deterministic entries", async () => {
    const root = makeSyntheticFolder()
    const build = await Effect.runPromise(buildFolderRlmCorpus(syntheticConfig(root)))
    expect(build.fileCount).toBe(2)
    expect(build.entryCount).toBeGreaterThan(0)
    // Entries are ordered by file then in-file paragraph index with stable refs.
    const refs = build.corpusInput.entries.map((entry) => entry.entryRef)
    expect(refs[0]).toBe("001.md#p0")
    expect(refs.every((ref, index) => index === 0 || ref >= refs[index - 1]!)).toBe(true)
  })

  test("a Source ref for a different scope label is refused before any read", async () => {
    const root = makeSyntheticFolder()
    const source = makeFolderRlmCorpusSource(syntheticConfig(root))
    const error = await Effect.runPromise(
      source
        .resolve({
          _tag: "Source",
          sourceRef: {
            addressSchemaId: "openagents.desktop.folder_corpus.v1",
            encodedAddress: JSON.stringify({ scopeLabel: "some-other-folder" }),
          },
        })
        .pipe(Effect.flip),
    )
    expect(error.reason).toBe("invalid_inline")
  })

  test("resolveFolderRlmCorpus builds a self-consistent handle", async () => {
    const root = makeSyntheticFolder()
    const handle = await Effect.runPromise(resolveFolderRlmCorpus(syntheticConfig(root)))
    expect(handle.manifest.coverage.entryCount).toBeGreaterThan(0)
    expect(handle.identity.contentDigest.length).toBeGreaterThan(0)
  })
})

describe("HANDS-5 runFullAutoRoadmapRecall (Tier D, zero spend)", () => {
  test("mines a synthetic folder into cited candidates", async () => {
    const root = makeSyntheticFolder()
    const result = await Effect.runPromise(
      runFullAutoRoadmapRecall({
        runRef: "run.hands5.d",
        recallRef: "recall.1",
        corpus: syntheticConfig(root),
      }),
    )
    expect(result.status).toBe("completed")
    expect(result.tier).toBe("deterministic")
    expect(result.label).toBe("cited-candidate")
    expect(result.verified).toBe(false)
    expect(result.usage.modelCalls).toBe(0)
    expect(result.candidates.length).toBeGreaterThan(0)
    // Feature-signal paragraphs are cited; the short line is not.
    const excerpts = result.candidates.map((candidate) => candidate.excerpt).join(" | ")
    expect(excerpts).toContain("conversation-first desktop")
    expect(excerpts).not.toContain("A short line")
    // Every candidate carries a source file and a stable entry ref.
    for (const candidate of result.candidates) {
      expect(candidate.entryRef).toMatch(/\.md#p\d+$/)
      expect(candidate.sourceFile).not.toBeNull()
    }
    // Frameable, and the rendered text is a labeled cited candidate.
    expect(fullAutoRoadmapRecallIsFrameable(result)).toBe(true)
    const text = fullAutoRoadmapRecallText(result)
    expect(text).not.toBeNull()
    expect(text).toContain("cited candidate — NOT verified")
    expect(text).toContain("deterministic")
  })
})

describe("HANDS-5 runFullAutoRoadmapRecall (Tier S, scripted model, zero spend)", () => {
  test("admitted semantic synthesis commits with preserved citations", async () => {
    const root = makeSyntheticFolder()
    const result = await Effect.runPromise(
      runFullAutoRoadmapRecall({
        runRef: "run.hands5.s",
        recallRef: "recall.1",
        corpus: syntheticConfig(root),
        semantic: {
          question: "What features do the transcripts ask for?",
          admission: ADMISSION,
          // NOTE: a LITERAL, case-matching pattern, because the published
          // @openagentsinc/rlm (0.2.1-rc.2) that this package depends on still
          // escapes the Tier S CorpusOp grep pattern AND hardcodes
          // case-sensitive matching — the exact bug this HANDS-5 change fixes in
          // the SDK. Once a build with the fix is published, a case-insensitive
          // regex pattern (e.g. "we need|product should") works here too.
          completeRoot: grepCommitRoot("conversation-first"),
        },
      }),
    )
    expect(result.tier).toBe("semantic")
    expect(result.status).toBe("completed")
    expect(result.verified).toBe(false)
    // One root model call, exact-usage recorded.
    expect(result.usage.modelCalls).toBe(1)
    expect(result.usage.totalTokens).toBe(144)
    expect(result.usageRows.length).toBe(1)
    expect(result.usageRows[0]?.usageTruth).toBe("exact")
    expect(result.candidates.length).toBeGreaterThan(0)
  })

  test("a missing admission falls back to Tier D (no spend)", async () => {
    const root = makeSyntheticFolder()
    const result = await Effect.runPromise(
      runFullAutoRoadmapRecall({
        runRef: "run.hands5.s.noadmit",
        recallRef: "recall.1",
        corpus: syntheticConfig(root),
        semantic: {
          question: "unused",
          admission: null,
          completeRoot: grepCommitRoot("we need"),
        },
      }),
    )
    expect(result.tier).toBe("deterministic")
    expect(result.usage.modelCalls).toBe(0)
  })
})

// ---------------------------------------------------------------------------
// Real bounded run over the actual transcript archive (audit §5.1), zero spend.
// ---------------------------------------------------------------------------

describe("HANDS-5 real transcript corpus (docs/transcripts 248–251)", () => {
  const available = existsSync(path.join(TRANSCRIPTS_DIR, "248.md"))

  test.skipIf(!available)(
    "mines a few real episodes into a cited candidate feature list",
    async () => {
      const result = await Effect.runPromise(
        runFullAutoRoadmapRecall({
          runRef: "run.hands5.transcripts",
          recallRef: "recall.1",
          corpus: {
            rootDir: TRANSCRIPTS_DIR,
            scopeLabel: "transcripts-248-251",
            include: ["248.md", "249.md", "250.md", "251.md"],
            maxEntries: 800,
          },
          deterministicPattern: FULL_AUTO_ROADMAP_SIGNAL_PATTERN,
          maxCandidates: 12,
        }),
      )
      expect(result.status === "completed" || result.status === "partial").toBe(true)
      expect(result.tier).toBe("deterministic")
      expect(result.label).toBe("cited-candidate")
      expect(result.verified).toBe(false)
      expect(result.usage.modelCalls).toBe(0)
      expect(result.candidates.length).toBeGreaterThan(0)
      for (const candidate of result.candidates) {
        expect(candidate.sourceFile).toMatch(/^25[01]\.md$|^24[89]\.md$/)
        expect(candidate.excerpt.length).toBeGreaterThan(0)
      }
    },
  )
})
