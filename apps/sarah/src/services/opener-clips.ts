/**
 * Sarah pre-rendered clip library — the Hallo2 quality tier served over the
 * live avatar lane (epic #8610; KHS-6 canned-answer seam #8605).
 *
 * The clips are the openers-v2 Hallo2 still-animation renders
 * (gs://openagentsgemini-oa-artifacts/sarah-avatar/openers-v2/, manifest.json
 * is the QA source of truth; docs/sarah/2026-07-09-oav-quality-strategy.md
 * Round 4). They are baked into the serving container at deploy time
 * (deploy-cloudrun.sh → SARAH_CLIPS_DIR) and served from
 * GET /sarah/api/clips/:name with immutable cache headers.
 *
 * LICENSE LAW (non-negotiable): only the raw Hallo2 512² renders ship — they
 * are MIT. The `*-sr.mp4` texture-recovery variants are derived from
 * CodeFormer's video_sr.py (S-Lab License 1.0, NON-commercial) and MUST NEVER
 * be listed, resolved, or served from this module. The catalog below is a
 * closed allowlist: lookups resolve ONLY against these fixed records, so a
 * request path can never reach an unlisted or SR file.
 */

import { join } from "node:path"

export type SarahClipRole = "opener" | "canned"

export type SarahClip = Readonly<{
  /** Stable clip name — the /sarah/api/clips/:name key. */
  name: string
  /** Exact filename inside SARAH_CLIPS_DIR (never derived from input). */
  file: string
  role: SarahClipRole
  /** The owner-approved script the clip performs (verbatim). */
  script: string
  /** Per-segment whisper STT transcript from the openers-v2 QA gate. */
  sttTranscript: string
  mediaSeconds: number
  tier: "hallo2_512_mit"
  library: "sarah-avatar/openers-v2"
}>

const LIBRARY = "sarah-avatar/openers-v2" as const
const TIER = "hallo2_512_mit" as const

/**
 * The closed shippable catalog — openers-v2 manifest.json records for the
 * five MIT Hallo2 renders. Scripts/transcripts are verbatim from the GCS
 * manifest (2026-07-10 judge-winner-audio renders).
 */
export const SARAH_CLIP_CATALOG: ReadonlyArray<SarahClip> = [
  {
    name: "opener-01-hello",
    file: "opener-01-hello-hallo2.mp4",
    role: "opener",
    script: "Hello! I'm Sarah. What's on your mind today?",
    sttTranscript: "Hello, I'm Sarah. What's on your mind today?",
    mediaSeconds: 2.6,
    tier: TIER,
    library: LIBRARY,
  },
  {
    name: "opener-02-welcome-back",
    file: "opener-02-welcome-back-hallo2.mp4",
    role: "opener",
    script: "Hey, welcome back! Where did we leave off?",
    sttTranscript: "Hey, welcome back. Where do we leave off?",
    mediaSeconds: 2.44,
    tier: TIER,
    library: LIBRARY,
  },
  {
    name: "opener-03-good-question",
    file: "opener-03-good-question-hallo2.mp4",
    role: "canned",
    script: "Good question, tell me more about how that works today.",
    sttTranscript: "Good question. Tell me more about how that works today.",
    mediaSeconds: 2.8,
    tier: TIER,
    library: LIBRARY,
  },
  {
    name: "opener-04-got-it",
    file: "opener-04-got-it-hallo2.mp4",
    role: "canned",
    script: "Got it. And how much time does that eat up each week?",
    sttTranscript: "Got it. And how much time does that eat up each week?",
    mediaSeconds: 3.36,
    tier: TIER,
    library: LIBRARY,
  },
  {
    name: "opener-05-show-you",
    file: "opener-05-show-you-hallo2.mp4",
    role: "canned",
    script: "Let me show you what that would look like with an agent doing it.",
    sttTranscript:
      "Let me show you what that would look like with an agent doing it.",
    mediaSeconds: 4.96,
    tier: TIER,
    library: LIBRARY,
  },
]

/** Where the baked clips live (Dockerfile sets /app/sarah-clips). */
export function sarahClipsDir(): string {
  return (
    process.env.SARAH_CLIPS_DIR?.trim() ||
    join(process.cwd(), ".sarah", "clips")
  )
}

/** Public serving URL for a clip (path-mounted under /sarah). */
export function sarahClipUrl(name: string): string {
  return `/sarah/api/clips/${encodeURIComponent(name)}`
}

/** Closed-allowlist lookup — the ONLY way a name reaches the filesystem. */
export function getSarahClip(name: string): SarahClip | null {
  return SARAH_CLIP_CATALOG.find((clip) => clip.name === name) ?? null
}

async function clipFileIfAvailable(clip: SarahClip) {
  const file = Bun.file(join(sarahClipsDir(), clip.file))
  return (await file.exists()) ? file : null
}

export async function isSarahClipAvailable(clip: SarahClip): Promise<boolean> {
  return (await clipFileIfAvailable(clip)) !== null
}

export type SarahClipManifestEntry = Readonly<{
  name: string
  role: SarahClipRole
  script: string
  transcript: string
  mediaSeconds: number
  tier: string
  available: boolean
  url: string
}>

/**
 * GET /sarah/api/clips — the typed public manifest. Lists only the shippable
 * catalog (SR variants are unrepresentable here) with per-clip availability
 * from the local clip dir.
 */
export async function listSarahClipsForApi(): Promise<{
  library: string
  licenseTier: string
  clips: SarahClipManifestEntry[]
}> {
  const clips = await Promise.all(
    SARAH_CLIP_CATALOG.map(async (clip) => ({
      name: clip.name,
      role: clip.role,
      script: clip.script,
      transcript: clip.sttTranscript,
      mediaSeconds: clip.mediaSeconds,
      tier: clip.tier,
      available: await isSarahClipAvailable(clip),
      url: sarahClipUrl(clip.name),
    })),
  )
  return {
    library: LIBRARY,
    licenseTier:
      "hallo2_512_mit_only (SR/CodeFormer-derived variants are non-commercial and never served)",
    clips,
  }
}

/**
 * GET /sarah/api/clips/:name — stream the MP4 with immutable caching and
 * single-range support (Safari requires 206 for <video> playback).
 */
export async function serveSarahClipRequest(
  name: string,
  request: Request,
): Promise<Response> {
  const clip = getSarahClip(name)
  if (!clip) {
    return Response.json(
      { error: { code: "clip_not_found", name } },
      { status: 404 },
    )
  }
  const file = await clipFileIfAvailable(clip)
  if (!file) {
    return Response.json(
      { error: { code: "clip_unavailable", name } },
      { status: 404 },
    )
  }
  const size = file.size
  const headers = new Headers({
    "content-type": "video/mp4",
    "accept-ranges": "bytes",
    // The clips are content-addressed by recipe/version in practice — a new
    // take is a new name/deploy — so immutable is honest.
    "cache-control": "public, max-age=31536000, immutable",
  })
  const range = request.headers.get("range")
  const match = range?.match(/^bytes=(\d*)-(\d*)$/)
  if (match && (match[1] || match[2])) {
    let start = match[1] ? Number(match[1]) : NaN
    let end = match[2] ? Number(match[2]) : size - 1
    if (Number.isNaN(start)) {
      // suffix range: bytes=-N (last N bytes)
      start = Math.max(0, size - Number(match[2]))
      end = size - 1
    }
    end = Math.min(end, size - 1)
    if (start > end || start >= size) {
      headers.set("content-range", `bytes */${size}`)
      return new Response(null, { status: 416, headers })
    }
    headers.set("content-range", `bytes ${start}-${end}/${size}`)
    headers.set("content-length", String(end - start + 1))
    return new Response(file.slice(start, end + 1), { status: 206, headers })
  }
  headers.set("content-length", String(size))
  return new Response(file, { status: 200, headers })
}

// --- opener selection (session-start greeting clip) --------------------------

let rotationCursor = 0

function openerRotationNames(): string[] {
  const raw = process.env.SARAH_OPENER_CLIP_ROTATION?.trim()
  if (!raw) return ["opener-01-hello"]
  const names = raw
    .split(",")
    .map((name) => name.trim())
    .filter((name) => getSarahClip(name) !== null)
  return names.length > 0 ? names : ["opener-01-hello"]
}

export type SarahOpenerClipPick = Readonly<{
  name: string
  url: string
  script: string
  transcript: string
  mediaSeconds: number
}>

/**
 * Pick the opener clip for a fresh avatar session (rotation over the
 * configured opener names; default opener-01). Returns null when no
 * configured clip is actually present on disk — callers must then fall back
 * to the live TTS greeting (never dead air).
 */
export async function pickSarahOpenerClip(): Promise<SarahOpenerClipPick | null> {
  const names = openerRotationNames()
  for (let i = 0; i < names.length; i++) {
    const clip = getSarahClip(names[(rotationCursor + i) % names.length]!)
    if (!clip) continue
    if (await isSarahClipAvailable(clip)) {
      rotationCursor = (rotationCursor + i + 1) % names.length
      return {
        name: clip.name,
        url: sarahClipUrl(clip.name),
        script: clip.script,
        transcript: clip.sttTranscript,
        mediaSeconds: clip.mediaSeconds,
      }
    }
  }
  return null
}

/**
 * KHS-6 canned-answer seam: resolve an answer-bank `clipRef`
 * (`clip:<name>` or bare `<name>`) to a servable clip. Null when the ref is
 * unknown or the file is not present — the caller degrades to the TTS path.
 */
export async function resolveServableSarahClip(
  clipRef: string,
): Promise<{ name: string; url: string } | null> {
  const name = clipRef.startsWith("clip:") ? clipRef.slice(5) : clipRef
  const clip = getSarahClip(name)
  if (!clip) return null
  if (!(await isSarahClipAvailable(clip))) return null
  return { name: clip.name, url: sarahClipUrl(clip.name) }
}

export function __resetSarahClipRotationForTest(): void {
  rotationCursor = 0
}
