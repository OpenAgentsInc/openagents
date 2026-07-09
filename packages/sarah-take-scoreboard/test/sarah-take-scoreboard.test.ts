import { describe, expect, test } from "bun:test"
import { readdirSync } from "node:fs"
import { join, resolve } from "node:path"

import {
  SCOREBOARD_SCHEMA_VERSION,
  renderScoreboardMarkdown,
  toCanonicalJson,
  toNdjsonLine,
  validateTakeScoreboard,
} from "../src/index.ts"

const repoRoot = resolve(import.meta.dir, "../../..")
const scoreboardsDir = join(repoRoot, "docs/sarah/scoreboards")

const validRecord = () => ({
  schemaVersion: SCOREBOARD_SCHEMA_VERSION,
  takeId: "fixture-take",
  title: "Fixture take",
  date: "2026-07-09",
  issueRefs: ["#8618"],
  inputRefs: {
    sourceClip: "clip 8 trimmed",
    script: "fixture script",
    ttsReference: "gs://bucket/voice-ref.wav",
    modelVersions: { musetalk: "1.5" },
    recipe: "raw MuseTalk",
    commits: [],
    artifactUris: ["gs://bucket/take.mp4"],
  },
  audioGates: {
    sttRoundTrip: { status: "pass", perSegment: true },
    loudnessLufs: -16,
    truePeakDbtp: -3,
    pauseTiming: { status: "pass" },
    prosody: { humanVerdict: "pass", llmJudgeScore: 8, llmJudgeModel: "gemini" },
  },
  videoGates: {
    ownerPlaybackVerdict: { status: "pass", notes: "owner watched playback" },
    avSync: { start: "pass", middle: "pass", end: "pass" },
    cropSharpness: { status: "pass" },
    temporalBoil: { status: "pass" },
    chunkBoundaryJerk: {
      status: "pass",
      metrics: { frames: 500, fps: 24, jerkMean: 0.15, jerkP95: 0.44 },
    },
    identityDrift: { status: "pass" },
    pasteBackSeam: { status: "pass" },
    badFrameExclusions: [],
  },
  operationalGates: {
    renderWallSeconds: 120,
    gpuType: "L4",
    costEstimateUsd: 0.03,
    artifactExistenceCheck: { status: "pass", method: "object_exists" },
    hostDisposition: { status: "stopped" },
    gcsIndexUpdated: true,
    noSecretsInArtifacts: true,
  },
  advance: true,
  verdictSummary: "Fixture advancing take.",
})

describe("sarah-take-scoreboard.v1 schema", () => {
  test("accepts a complete valid record", () => {
    const result = validateTakeScoreboard(validRecord())
    expect(result.valid).toBe(true)
    expect(result.errors).toHaveLength(0)
  })

  test("rejects a wrong schemaVersion", () => {
    const record = { ...validRecord(), schemaVersion: "sarah-take-scoreboard.v2" }
    const result = validateTakeScoreboard(record)
    expect(result.valid).toBe(false)
    expect(result.errors[0]?.code).toBe("invalid_scoreboard")
  })

  test("rejects an invalid takeId and date", () => {
    const record = { ...validRecord(), takeId: "Bad Take!", date: "07/09/2026" }
    const result = validateTakeScoreboard(record)
    expect(result.valid).toBe(false)
    const codes = result.errors.map((error) => error.code)
    expect(codes).toContain("invalid_take_id")
    expect(codes).toContain("invalid_date")
  })

  test("rejects an out-of-range LLM prosody judge score", () => {
    const record = validRecord()
    record.audioGates.prosody.llmJudgeScore = 11
    const result = validateTakeScoreboard(record)
    expect(result.valid).toBe(false)
    expect(result.errors.map((error) => error.code)).toContain("invalid_llm_judge_score")
  })

  test("rejects artifact checks that are not object_exists (log-marker ban)", () => {
    const record = validRecord()
    ;(record.operationalGates.artifactExistenceCheck as { method: string }).method = "log_marker"
    const result = validateTakeScoreboard(record)
    expect(result.valid).toBe(false)
    expect(result.errors[0]?.code).toBe("invalid_scoreboard")
  })

  test("rejects left_running without a reason (SQ-8 law)", () => {
    const record = validRecord()
    record.operationalGates.hostDisposition = { status: "left_running" } as never
    const result = validateTakeScoreboard(record)
    expect(result.valid).toBe(false)
    expect(result.errors.map((error) => error.code)).toContain("left_running_without_reason")
  })

  test("rejects secret patterns in committed scoreboards", () => {
    const record = validRecord()
    record.verdictSummary = "shipped with api_key=abc123 in the run env"
    const result = validateTakeScoreboard(record)
    expect(result.valid).toBe(false)
    expect(result.errors.map((error) => error.code)).toContain("secret_pattern")
  })
})

describe("the playback-first law: no take advances on stills", () => {
  test("advance requires an owner playback PASS", () => {
    const record = validRecord()
    record.videoGates.ownerPlaybackVerdict = {
      status: "pending",
      notes: "stills look great",
    }
    const result = validateTakeScoreboard(record)
    expect(result.valid).toBe(false)
    expect(result.errors.map((error) => error.code)).toContain("stills_cannot_advance")
  })

  test("advance is refused on an owner playback FAIL even with perfect metrics", () => {
    const record = validRecord()
    record.videoGates.ownerPlaybackVerdict = {
      status: "fail",
      notes: "choppy, plastic, less humanlike in motion",
    }
    const result = validateTakeScoreboard(record)
    expect(result.valid).toBe(false)
    const codes = result.errors.map((error) => error.code)
    expect(codes).toContain("stills_cannot_advance")
    expect(codes).toContain("failed_gate_cannot_advance")
  })

  test("advance requires a passing human prosody verdict (opener library v1 lesson)", () => {
    const record = validRecord()
    record.audioGates.prosody = { humanVerdict: "pending" } as never
    const result = validateTakeScoreboard(record)
    expect(result.valid).toBe(false)
    expect(result.errors.map((error) => error.code)).toContain("prosody_gate_required")
  })

  test("advance requires per-segment STT, not whole-clip", () => {
    const record = validRecord()
    record.audioGates.sttRoundTrip = { status: "pass", perSegment: false }
    const result = validateTakeScoreboard(record)
    expect(result.valid).toBe(false)
    expect(result.errors.map((error) => error.code)).toContain("stt_gate_required")
  })

  test("advance requires the no-secrets attestation", () => {
    const record = validRecord()
    record.operationalGates.noSecretsInArtifacts = false
    const result = validateTakeScoreboard(record)
    expect(result.valid).toBe(false)
    expect(result.errors.map((error) => error.code)).toContain("privacy_gate_required")
  })

  test("a non-advancing take may carry failed gates without validator errors", () => {
    const record = validRecord()
    record.advance = false
    record.videoGates.ownerPlaybackVerdict = { status: "fail", notes: "owner fail" }
    record.audioGates.prosody = { humanVerdict: "fail" } as never
    const result = validateTakeScoreboard(record)
    expect(result.valid).toBe(true)
  })

  test("a pending owner playback verdict is surfaced as a warning", () => {
    const record = validRecord()
    record.advance = false
    record.videoGates.ownerPlaybackVerdict = { status: "pending", notes: "awaiting owner" }
    const result = validateTakeScoreboard(record)
    expect(result.valid).toBe(true)
    expect(result.warnings.map((warning) => warning.code)).toContain("owner_playback_pending")
  })
})

describe("markdown renderer", () => {
  test("renders the load-bearing sections without trailing whitespace", () => {
    const result = validateTakeScoreboard(validRecord())
    expect(result.valid).toBe(true)
    if (!result.valid) return
    const markdown = renderScoreboardMarkdown(result.scoreboard)
    expect(markdown).toContain("# Take scoreboard — Fixture take")
    expect(markdown).toContain("**Advance: YES**")
    expect(markdown).toContain("**Owner playback verdict**")
    expect(markdown).toContain("Operational gates (SQ-8 closeout)")
    expect(markdown).toContain("no take advances on stills")
    expect(markdown.endsWith("\n")).toBe(true)
    for (const line of markdown.split("\n")) {
      expect(line).toBe(line.trimEnd())
    }
  })
})

describe("checked-in scoreboards under docs/sarah/scoreboards", () => {
  const files = readdirSync(scoreboardsDir)
    .filter((name) => name.endsWith(".json"))
    .sort()

  test("the retrofit corpus is present", () => {
    expect(files).toEqual([
      "sarah-openers-v1.json",
      "sarah-reply-enhanced.json",
      "sarah-reply-hq.json",
      "sarah-reply-latentsync.json",
      "sarah-reply-v1.json",
      "sarah-reply-v3.json",
    ])
  })

  for (const name of files) {
    test(`${name} validates and matches its rendered markdown`, async () => {
      const raw = await Bun.file(join(scoreboardsDir, name)).text()
      const result = validateTakeScoreboard(JSON.parse(raw))
      expect(result.valid).toBe(true)
      if (!result.valid) return
      expect(raw).toBe(toCanonicalJson(result.scoreboard))
      const markdown = await Bun.file(
        join(scoreboardsDir, name.replace(/\.json$/, ".md")),
      ).text()
      expect(markdown).toBe(renderScoreboardMarkdown(result.scoreboard))
      expect(result.scoreboard.takeId).toBe(name.replace(/\.json$/, ""))
    })
  }

  test("no retrofit take advances (owner playback never passed)", async () => {
    for (const name of files) {
      const result = validateTakeScoreboard(
        JSON.parse(await Bun.file(join(scoreboardsDir, name)).text()),
      )
      expect(result.valid).toBe(true)
      if (result.valid) expect(result.scoreboard.advance).toBe(false)
    }
  })

  test("owner-failed takes carry explicit FAIL playback verdicts", async () => {
    for (const name of ["sarah-reply-enhanced.json", "sarah-openers-v1.json"]) {
      const result = validateTakeScoreboard(
        JSON.parse(await Bun.file(join(scoreboardsDir, name)).text()),
      )
      expect(result.valid).toBe(true)
      if (result.valid) {
        expect(result.scoreboard.videoGates.ownerPlaybackVerdict.status).toBe("fail")
      }
    }
  })

  test("index.ndjson mirrors the JSON corpus", async () => {
    const lines = (await Bun.file(join(scoreboardsDir, "index.ndjson")).text())
      .trim()
      .split("\n")
    expect(lines).toHaveLength(files.length)
    const byTakeId = new Map(
      lines.map((line) => {
        const parsed = JSON.parse(line) as { takeId: string }
        return [parsed.takeId, line] as const
      }),
    )
    for (const name of files) {
      const result = validateTakeScoreboard(
        JSON.parse(await Bun.file(join(scoreboardsDir, name)).text()),
      )
      expect(result.valid).toBe(true)
      if (result.valid) {
        expect(byTakeId.get(result.scoreboard.takeId)).toBe(toNdjsonLine(result.scoreboard))
      }
    }
  })
})
