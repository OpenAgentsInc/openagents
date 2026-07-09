/**
 * @openagentsinc/sarah-take-scoreboard — the Sarah Quality Scoreboard
 * (`sarah-take-scoreboard.v1`, SQ-1 #8618, epic #8610).
 *
 * One canonical, playback-first artifact per OAV/media take: input refs,
 * audio gates (per-segment STT round-trip + LLM prosody judge), video gates
 * (owner playback verdict + chunk-boundary jerk metrics), and operational
 * gates that embed the SQ-8 GPU media-run closeout checklist
 * (`docs/sarah/GPU_MEDIA_RUN_CLOSEOUT.md`, #8625).
 *
 * Cultural law encoded in the validator: **no take advances on stills**.
 * `advance: true` requires an explicit owner playback PASS plus passing
 * per-segment STT and human prosody verdicts, and no failed gate anywhere.
 * Context: the enhanced take passed stills QA and failed owner playback, and
 * opener library v1 passed stills/jerk/word-STT and failed owner playback
 * outright (#8610).
 */
import { Schema as S } from "effect"

export const SCOREBOARD_SCHEMA_VERSION = "sarah-take-scoreboard.v1" as const

const NonEmptyString = S.String.check(S.isMinLength(1))

// ---------------------------------------------------------------------------
// Gate primitives
// ---------------------------------------------------------------------------

export const GateStatus = S.Literals(["pass", "fail", "watch", "pending", "not_run"])
export type GateStatus = typeof GateStatus.Type

export const GateSchema = S.Struct({
  status: GateStatus,
  notes: S.optionalKey(S.String),
})
export type Gate = typeof GateSchema.Type

/** Owner playback is the final gate: it is never `not_run` for a shipped take. */
export const OwnerPlaybackStatus = S.Literals(["pass", "fail", "pending"])
export type OwnerPlaybackStatus = typeof OwnerPlaybackStatus.Type

// ---------------------------------------------------------------------------
// Input refs
// ---------------------------------------------------------------------------

export const InputRefsSchema = S.Struct({
  /** Source footage ref, e.g. "clip 8 (v2_traced-df7d3b47…), trimmed to 185 frames". */
  sourceClip: NonEmptyString,
  /** Script ref or short verbatim public-safe script description. */
  script: NonEmptyString,
  /** TTS voice reference, e.g. the GCS URI of sarah_voice_ref_v1.wav. */
  ttsReference: NonEmptyString,
  /** Model name → version/config, e.g. { musetalk: "1.5 (256^2 inpaint)" }. */
  modelVersions: S.Record(S.String, S.String),
  /** Render command or run-script ref (public-safe). */
  renderCommand: S.optionalKey(S.String),
  /** Enhancement/render recipe in one line. */
  recipe: NonEmptyString,
  /** Relevant commits, e.g. "hydralisk 40b2783 (spoken-form TTS normalizer)". */
  commits: S.Array(S.String),
  /** Artifact URIs (gs:// or public paths) for the take and its QA stills. */
  artifactUris: S.Array(NonEmptyString),
})
export type InputRefs = typeof InputRefsSchema.Type

// ---------------------------------------------------------------------------
// Audio gates
// ---------------------------------------------------------------------------

export const SttRoundTripSchema = S.Struct({
  status: GateStatus,
  /**
   * Whether STT verification ran per-segment. Whole-clip transcription
   * produced false defects on v3 ("a AI" transcriber noise); per-segment is
   * the required mode for a passing gate on new takes.
   */
  perSegment: S.Boolean,
  transcriber: S.optionalKey(S.String),
  notes: S.optionalKey(S.String),
})
export type SttRoundTrip = typeof SttRoundTripSchema.Type

export const ProsodySchema = S.Struct({
  /** Human (owner or agent) prosody verdict by ear — the hard gate. */
  humanVerdict: GateStatus,
  /** LLM audio-judge score 1–10 (e.g. Gemini naturalness/warmth/confidence). */
  llmJudgeScore: S.optionalKey(S.Number),
  llmJudgeModel: S.optionalKey(S.String),
  notes: S.optionalKey(S.String),
})
export type Prosody = typeof ProsodySchema.Type

export const AudioGatesSchema = S.Struct({
  sttRoundTrip: SttRoundTripSchema,
  /** Integrated loudness; target ≈ -16 LUFS. */
  loudnessLufs: S.optionalKey(S.Number),
  /** True peak; target ≤ -3 dBTP. */
  truePeakDbtp: S.optionalKey(S.Number),
  pauseTiming: GateSchema,
  prosody: ProsodySchema,
  /** Initialism-risk notes (AI/API/URL letter-speech handling). */
  initialismRisk: S.optionalKey(S.String),
})
export type AudioGates = typeof AudioGatesSchema.Type

// ---------------------------------------------------------------------------
// Video gates
// ---------------------------------------------------------------------------

export const MotionMetricsSchema = S.Struct({
  frames: S.optionalKey(S.Number),
  fps: S.optionalKey(S.Number),
  /** Mouth-crop frame-to-frame motion (d1) mean / p95. */
  motionMean: S.optionalKey(S.Number),
  motionP95: S.optionalKey(S.Number),
  /** Mouth-crop jerk (d2 = |Δd1|) mean / p95 — lower is smoother. */
  jerkMean: S.optionalKey(S.Number),
  jerkP95: S.optionalKey(S.Number),
  /** Periodic-hitch analysis, e.g. LatentSync 16-frame chunk-boundary phases. */
  periodicHitch: S.optionalKey(S.String),
})
export type MotionMetrics = typeof MotionMetricsSchema.Type

export const OwnerPlaybackVerdictSchema = S.Struct({
  status: OwnerPlaybackStatus,
  notes: S.optionalKey(S.String),
})
export type OwnerPlaybackVerdict = typeof OwnerPlaybackVerdictSchema.Type

export const AvSyncSchema = S.Struct({
  start: GateStatus,
  middle: GateStatus,
  end: GateStatus,
  notes: S.optionalKey(S.String),
})
export type AvSync = typeof AvSyncSchema.Type

export const ChunkBoundaryJerkSchema = S.Struct({
  status: GateStatus,
  metrics: S.optionalKey(MotionMetricsSchema),
  notes: S.optionalKey(S.String),
})
export type ChunkBoundaryJerk = typeof ChunkBoundaryJerkSchema.Type

export const VideoGatesSchema = S.Struct({
  /** THE gate. In-motion human playback verdict; stills never substitute. */
  ownerPlaybackVerdict: OwnerPlaybackVerdictSchema,
  avSync: AvSyncSchema,
  cropSharpness: GateSchema,
  temporalBoil: GateSchema,
  chunkBoundaryJerk: ChunkBoundaryJerkSchema,
  identityDrift: GateSchema,
  pasteBackSeam: GateSchema,
  /** Frames/poses excluded and why (no-face trims, eyes-closed pins, …). */
  badFrameExclusions: S.Array(S.String),
  notes: S.optionalKey(S.String),
})
export type VideoGates = typeof VideoGatesSchema.Type

// ---------------------------------------------------------------------------
// Operational gates — embeds the SQ-8 GPU media-run closeout checklist
// (docs/sarah/GPU_MEDIA_RUN_CLOSEOUT.md, receipt schema
// openagents.sarah.gpu_media_run_closeout.v1)
// ---------------------------------------------------------------------------

export const HostDispositionStatus = S.Literals(["stopped", "deleted", "left_running"])
export type HostDispositionStatus = typeof HostDispositionStatus.Type

export const HostDispositionSchema = S.Struct({
  status: HostDispositionStatus,
  /** Required when status is left_running (e.g. "prod_render_node"). */
  reason: S.optionalKey(S.String),
})
export type HostDisposition = typeof HostDispositionSchema.Type

export const ArtifactExistenceCheckSchema = S.Struct({
  status: GateStatus,
  /** SQ-8 law: artifact-existence monitors only — log markers burned us twice. */
  method: S.Literal("object_exists"),
  notes: S.optionalKey(S.String),
})
export type ArtifactExistenceCheck = typeof ArtifactExistenceCheckSchema.Type

export const OperationalGatesSchema = S.Struct({
  renderWallSeconds: S.optionalKey(S.Number),
  gpuType: S.optionalKey(S.String),
  costEstimateUsd: S.optionalKey(S.Number),
  costNotes: S.optionalKey(S.String),
  artifactExistenceCheck: ArtifactExistenceCheckSchema,
  hostDisposition: HostDispositionSchema,
  gcsIndexUpdated: S.Boolean,
  noSecretsInArtifacts: S.Boolean,
  /** Path/URI of the matching gpu_media_run_closeout.v1 receipt, when written. */
  closeoutReceipt: S.optionalKey(S.String),
})
export type OperationalGates = typeof OperationalGatesSchema.Type

// ---------------------------------------------------------------------------
// The scoreboard record
// ---------------------------------------------------------------------------

export const TakeScoreboardSchema = S.Struct({
  schemaVersion: S.Literal(SCOREBOARD_SCHEMA_VERSION),
  /** Kebab/snake id, e.g. "sarah-reply-v3". */
  takeId: NonEmptyString,
  title: NonEmptyString,
  /** YYYY-MM-DD. */
  date: NonEmptyString,
  /** Issue refs, e.g. ["#8610", "#8618"]. */
  issueRefs: S.Array(S.String),
  inputRefs: InputRefsSchema,
  audioGates: AudioGatesSchema,
  videoGates: VideoGatesSchema,
  operationalGates: OperationalGatesSchema,
  /** May this take advance (ship/serve/win its lane)? Playback-first law. */
  advance: S.Boolean,
  verdictSummary: NonEmptyString,
})
export type TakeScoreboard = typeof TakeScoreboardSchema.Type

// ---------------------------------------------------------------------------
// Validation
// ---------------------------------------------------------------------------

export type ValidationIssue = {
  code: string
  message: string
  path?: string
}

export type ValidationResult =
  | { valid: true; scoreboard: TakeScoreboard; errors: []; warnings: ValidationIssue[] }
  | { valid: false; scoreboard?: undefined; errors: ValidationIssue[]; warnings: ValidationIssue[] }

const TAKE_ID = /^[a-z0-9]+(?:[-_][a-z0-9]+)*$/
const DATE = /^\d{4}-\d{2}-\d{2}$/

/**
 * Public-safety tripwire, mirroring the SQ-8 closeout validator's forbidden
 * patterns: scoreboards are committed artifacts and must never carry secret
 * material.
 */
const SECRET_PATTERNS: ReadonlyArray<[RegExp, string]> = [
  [/api[_-]?key/i, "api key reference"],
  [/\bBearer\s+[A-Za-z0-9._-]{8,}/, "bearer token"],
  [/\bsk-[A-Za-z0-9]{16,}/, "provider secret key"],
  [/mnemonic/i, "wallet mnemonic reference"],
  [/BEGIN [A-Z ]*PRIVATE KEY/, "private key block"],
]

const collectGateStatuses = (scoreboard: TakeScoreboard): Array<[string, string]> => [
  ["audioGates.sttRoundTrip", scoreboard.audioGates.sttRoundTrip.status],
  ["audioGates.pauseTiming", scoreboard.audioGates.pauseTiming.status],
  ["audioGates.prosody.humanVerdict", scoreboard.audioGates.prosody.humanVerdict],
  ["videoGates.ownerPlaybackVerdict", scoreboard.videoGates.ownerPlaybackVerdict.status],
  ["videoGates.avSync.start", scoreboard.videoGates.avSync.start],
  ["videoGates.avSync.middle", scoreboard.videoGates.avSync.middle],
  ["videoGates.avSync.end", scoreboard.videoGates.avSync.end],
  ["videoGates.cropSharpness", scoreboard.videoGates.cropSharpness.status],
  ["videoGates.temporalBoil", scoreboard.videoGates.temporalBoil.status],
  ["videoGates.chunkBoundaryJerk", scoreboard.videoGates.chunkBoundaryJerk.status],
  ["videoGates.identityDrift", scoreboard.videoGates.identityDrift.status],
  ["videoGates.pasteBackSeam", scoreboard.videoGates.pasteBackSeam.status],
  ["operationalGates.artifactExistenceCheck", scoreboard.operationalGates.artifactExistenceCheck.status],
]

export const validateTakeScoreboard = (input: unknown): ValidationResult => {
  const errors: ValidationIssue[] = []
  const warnings: ValidationIssue[] = []

  let scoreboard: TakeScoreboard
  try {
    scoreboard = S.decodeUnknownSync(TakeScoreboardSchema)(input)
  } catch (error) {
    return {
      valid: false,
      errors: [
        {
          code: "invalid_scoreboard",
          message: `Scoreboard failed schema validation: ${error instanceof Error ? error.message : String(error)}`,
        },
      ],
      warnings: [],
    }
  }

  if (!TAKE_ID.test(scoreboard.takeId)) {
    errors.push({
      code: "invalid_take_id",
      message: `takeId must be lower-case kebab/snake: ${scoreboard.takeId}`,
      path: "takeId",
    })
  }
  if (!DATE.test(scoreboard.date)) {
    errors.push({
      code: "invalid_date",
      message: `date must be YYYY-MM-DD: ${scoreboard.date}`,
      path: "date",
    })
  }

  const llmScore = scoreboard.audioGates.prosody.llmJudgeScore
  if (llmScore !== undefined && (llmScore < 1 || llmScore > 10)) {
    errors.push({
      code: "invalid_llm_judge_score",
      message: `LLM prosody judge score must be within 1–10: ${llmScore}`,
      path: "audioGates.prosody.llmJudgeScore",
    })
  }

  const disposition = scoreboard.operationalGates.hostDisposition
  if (disposition.status === "left_running" && !disposition.reason?.trim()) {
    errors.push({
      code: "left_running_without_reason",
      message: "hostDisposition.status left_running requires a non-empty reason (SQ-8 law).",
      path: "operationalGates.hostDisposition.reason",
    })
  }

  // The playback-first law: no take advances on stills.
  if (scoreboard.advance) {
    const playback = scoreboard.videoGates.ownerPlaybackVerdict.status
    if (playback !== "pass") {
      errors.push({
        code: "stills_cannot_advance",
        message:
          "advance requires an explicit owner playback PASS — stills, motion statistics, and word-level STT never advance a take.",
        path: "videoGates.ownerPlaybackVerdict",
      })
    }
    if (scoreboard.audioGates.prosody.humanVerdict !== "pass") {
      errors.push({
        code: "prosody_gate_required",
        message: "advance requires a passing human prosody verdict (opener-library-v1 lesson).",
        path: "audioGates.prosody.humanVerdict",
      })
    }
    if (
      scoreboard.audioGates.sttRoundTrip.status !== "pass" ||
      !scoreboard.audioGates.sttRoundTrip.perSegment
    ) {
      errors.push({
        code: "stt_gate_required",
        message: "advance requires a passing per-segment STT round-trip.",
        path: "audioGates.sttRoundTrip",
      })
    }
    for (const [path, status] of collectGateStatuses(scoreboard)) {
      if (status === "fail") {
        errors.push({
          code: "failed_gate_cannot_advance",
          message: `advance is not allowed while a gate is failing: ${path}`,
          path,
        })
      }
    }
    if (!scoreboard.operationalGates.noSecretsInArtifacts) {
      errors.push({
        code: "privacy_gate_required",
        message: "advance requires the no-secrets-in-artifacts attestation.",
        path: "operationalGates.noSecretsInArtifacts",
      })
    }
  }

  const serialized = JSON.stringify(input)
  for (const [pattern, label] of SECRET_PATTERNS) {
    if (pattern.test(serialized)) {
      errors.push({
        code: "secret_pattern",
        message: `Scoreboard content matches a forbidden secret pattern (${label}). Scoreboards are public-safe committed artifacts.`,
      })
    }
  }

  if (scoreboard.videoGates.ownerPlaybackVerdict.status === "pending") {
    warnings.push({
      code: "owner_playback_pending",
      message: "Owner playback verdict is pending — this take cannot advance until it lands.",
      path: "videoGates.ownerPlaybackVerdict",
    })
  }
  if (scoreboard.audioGates.prosody.llmJudgeScore === undefined) {
    warnings.push({
      code: "llm_prosody_judge_missing",
      message: "No LLM prosody judge score recorded — required for new takes after opener library v1.",
      path: "audioGates.prosody.llmJudgeScore",
    })
  }
  if (scoreboard.inputRefs.artifactUris.length === 0) {
    warnings.push({
      code: "no_artifact_uris",
      message: "No artifact URIs recorded — the take should be findable from its scoreboard.",
      path: "inputRefs.artifactUris",
    })
  }

  if (errors.length > 0) return { valid: false, errors, warnings }
  return { valid: true, scoreboard, errors: [], warnings }
}

// ---------------------------------------------------------------------------
// Serialization: canonical JSON + NDJSON
// ---------------------------------------------------------------------------

/** Canonical pretty JSON body for `<takeId>.json` (trailing newline included). */
export const toCanonicalJson = (scoreboard: TakeScoreboard): string =>
  `${JSON.stringify(scoreboard, null, 2)}\n`

/** One canonical single-line record for `index.ndjson`. */
export const toNdjsonLine = (scoreboard: TakeScoreboard): string => JSON.stringify(scoreboard)

// ---------------------------------------------------------------------------
// Markdown renderer
// ---------------------------------------------------------------------------

const statusLabel = (status: string): string => {
  switch (status) {
    case "pass":
      return "PASS"
    case "fail":
      return "FAIL"
    case "watch":
      return "WATCH"
    case "pending":
      return "PENDING"
    case "not_run":
      return "not run"
    default:
      return status
  }
}

const cell = (value: string | undefined): string =>
  (value ?? "").replace(/\|/g, "\\|").replace(/\s+/g, " ").trim() || "—"

const gateRow = (name: string, status: string, notes?: string): string =>
  `| ${cell(name)} | ${statusLabel(status)} | ${cell(notes)} |`

const numberCell = (value: number | undefined, unit = ""): string =>
  value === undefined ? "—" : `${value}${unit}`

export const renderScoreboardMarkdown = (scoreboard: TakeScoreboard): string => {
  const lines: string[] = []
  const push = (line = "") => lines.push(line)

  push(`# Take scoreboard — ${scoreboard.title}`)
  push()
  push(`- Schema: \`${scoreboard.schemaVersion}\``)
  push(`- Take: \`${scoreboard.takeId}\` (${scoreboard.date})`)
  if (scoreboard.issueRefs.length > 0) push(`- Refs: ${scoreboard.issueRefs.join(", ")}`)
  push(
    `- **Advance: ${scoreboard.advance ? "YES" : "NO"}** — ${scoreboard.verdictSummary}`,
  )
  push()

  push("## Input refs")
  push()
  push("| Field | Value |")
  push("| --- | --- |")
  push(`| Source clip | ${cell(scoreboard.inputRefs.sourceClip)} |`)
  push(`| Script | ${cell(scoreboard.inputRefs.script)} |`)
  push(`| TTS reference | ${cell(scoreboard.inputRefs.ttsReference)} |`)
  for (const [model, version] of Object.entries(scoreboard.inputRefs.modelVersions)) {
    push(`| Model: ${cell(model)} | ${cell(version)} |`)
  }
  push(`| Recipe | ${cell(scoreboard.inputRefs.recipe)} |`)
  if (scoreboard.inputRefs.renderCommand !== undefined) {
    push(`| Render command | ${cell(scoreboard.inputRefs.renderCommand)} |`)
  }
  if (scoreboard.inputRefs.commits.length > 0) {
    push(`| Commits | ${cell(scoreboard.inputRefs.commits.join("; "))} |`)
  }
  for (const uri of scoreboard.inputRefs.artifactUris) {
    push(`| Artifact | ${cell(uri)} |`)
  }
  push()

  push("## Audio gates")
  push()
  push("| Gate | Status | Notes |")
  push("| --- | --- | --- |")
  const stt = scoreboard.audioGates.sttRoundTrip
  push(
    gateRow(
      `STT round-trip (${stt.perSegment ? "per-segment" : "whole-clip"}${stt.transcriber ? `, ${stt.transcriber}` : ""})`,
      stt.status,
      stt.notes,
    ),
  )
  push(
    `| Loudness / true peak | ${numberCell(scoreboard.audioGates.loudnessLufs, " LUFS")} / ${numberCell(scoreboard.audioGates.truePeakDbtp, " dBTP")} | target ≈ -16 LUFS / ≤ -3 dBTP |`,
  )
  push(
    gateRow(
      "Pause timing",
      scoreboard.audioGates.pauseTiming.status,
      scoreboard.audioGates.pauseTiming.notes,
    ),
  )
  const prosody = scoreboard.audioGates.prosody
  push(gateRow("Prosody (human verdict)", prosody.humanVerdict, prosody.notes))
  push(
    `| Prosody (LLM judge) | ${prosody.llmJudgeScore === undefined ? "not run" : `${prosody.llmJudgeScore}/10`} | ${cell(prosody.llmJudgeModel)} |`,
  )
  if (scoreboard.audioGates.initialismRisk !== undefined) {
    push(`| Initialism risk | — | ${cell(scoreboard.audioGates.initialismRisk)} |`)
  }
  push()

  push("## Video gates")
  push()
  push("| Gate | Status | Notes |")
  push("| --- | --- | --- |")
  const playback = scoreboard.videoGates.ownerPlaybackVerdict
  push(gateRow("**Owner playback verdict**", playback.status, playback.notes))
  const avSync = scoreboard.videoGates.avSync
  push(
    gateRow(
      "A/V sync (start / middle / end)",
      `${statusLabel(avSync.start)} / ${statusLabel(avSync.middle)} / ${statusLabel(avSync.end)}`,
      avSync.notes,
    ),
  )
  push(
    gateRow(
      "Crop sharpness",
      scoreboard.videoGates.cropSharpness.status,
      scoreboard.videoGates.cropSharpness.notes,
    ),
  )
  push(
    gateRow(
      "Temporal boil/flicker",
      scoreboard.videoGates.temporalBoil.status,
      scoreboard.videoGates.temporalBoil.notes,
    ),
  )
  const jerk = scoreboard.videoGates.chunkBoundaryJerk
  push(gateRow("Chunk-boundary jerk", jerk.status, jerk.notes))
  push(
    gateRow(
      "Identity drift",
      scoreboard.videoGates.identityDrift.status,
      scoreboard.videoGates.identityDrift.notes,
    ),
  )
  push(
    gateRow(
      "Paste-back seam",
      scoreboard.videoGates.pasteBackSeam.status,
      scoreboard.videoGates.pasteBackSeam.notes,
    ),
  )
  push()
  if (jerk.metrics) {
    const metrics = jerk.metrics
    push("Motion metrics (mouth crop, lower jerk = smoother):")
    push()
    push("| Frames | fps | Motion mean / p95 | Jerk mean / p95 |")
    push("| --- | --- | --- | --- |")
    push(
      `| ${numberCell(metrics.frames)} | ${numberCell(metrics.fps)} | ${numberCell(metrics.motionMean)} / ${numberCell(metrics.motionP95)} | ${numberCell(metrics.jerkMean)} / ${numberCell(metrics.jerkP95)} |`,
    )
    if (metrics.periodicHitch !== undefined) {
      push()
      push(`Periodic hitch: ${metrics.periodicHitch}`)
    }
    push()
  }
  if (scoreboard.videoGates.badFrameExclusions.length > 0) {
    push("Bad-frame exclusions:")
    push()
    for (const exclusion of scoreboard.videoGates.badFrameExclusions) {
      push(`- ${exclusion}`)
    }
    push()
  }
  if (scoreboard.videoGates.notes !== undefined) {
    push(scoreboard.videoGates.notes)
    push()
  }

  push("## Operational gates (SQ-8 closeout)")
  push()
  push("| Item | Value |")
  push("| --- | --- |")
  push(`| Render wall | ${numberCell(scoreboard.operationalGates.renderWallSeconds, " s")} |`)
  push(`| GPU | ${cell(scoreboard.operationalGates.gpuType)} |`)
  push(
    `| Cost estimate | ${scoreboard.operationalGates.costEstimateUsd === undefined ? "—" : `$${scoreboard.operationalGates.costEstimateUsd}`}${scoreboard.operationalGates.costNotes ? ` (${cell(scoreboard.operationalGates.costNotes)})` : ""} |`,
  )
  const existence = scoreboard.operationalGates.artifactExistenceCheck
  push(
    `| Artifact existence (${existence.method}) | ${statusLabel(existence.status)}${existence.notes ? ` — ${cell(existence.notes)}` : ""} |`,
  )
  push(
    `| Host disposition | ${scoreboard.operationalGates.hostDisposition.status}${scoreboard.operationalGates.hostDisposition.reason ? ` (${cell(scoreboard.operationalGates.hostDisposition.reason)})` : ""} |`,
  )
  push(`| GCS index updated | ${scoreboard.operationalGates.gcsIndexUpdated ? "yes" : "NO"} |`)
  push(
    `| No secrets in artifacts | ${scoreboard.operationalGates.noSecretsInArtifacts ? "attested" : "NOT ATTESTED"} |`,
  )
  if (scoreboard.operationalGates.closeoutReceipt !== undefined) {
    push(`| Closeout receipt | ${cell(scoreboard.operationalGates.closeoutReceipt)} |`)
  }
  push()
  push(
    "Closeout checklist: `docs/sarah/GPU_MEDIA_RUN_CLOSEOUT.md` (#8625) — artifact-existence monitors only, host disposition with reason, GCS index updated, cost recorded, no secrets.",
  )
  push()
  push("---")
  push()
  push(
    "Law: **no take advances on stills.** Owner playback verdict plus temporal evidence decide (SQ-1 #8618).",
  )

  return `${lines.join("\n")}\n`
}
