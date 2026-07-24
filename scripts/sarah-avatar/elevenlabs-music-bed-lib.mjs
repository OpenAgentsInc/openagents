// Pure helpers for ElevenLabs Sarah RC music beds (#9235).
// Keep HTTP and ffmpeg orchestration in the CLI; tests cover this module
// without live network or secrets.

import { existsSync, readFileSync } from 'node:fs'
import { dirname, join, resolve } from 'node:path'

export const DEFAULT_MODEL_ID = 'music_v2'
export const DEFAULT_OUTPUT_FORMAT = 'mp3_48000_192'
export const MUSIC_API_PATH = '/v1/music'
export const DEFAULT_PROMPT =
  'Instrumental cinematic sci-fi underscore. Sparse midrange. No vocals. Calm pulse under spoken narration.'
export const DEFAULT_FADE_IN_SEC = 1.5
export const DEFAULT_FADE_OUT_SEC = 3
export const DEFAULT_BED_VOLUME_DB = -3
export const MIN_MUSIC_LENGTH_MS = 3000
export const MAX_MUSIC_LENGTH_MS = 600000

export function defaultElevenLabsEnvPath(home = process.env.HOME || '') {
  return resolve(home, 'work/.secrets/elevenlabs.env')
}

export function musicApiUrl(outputFormat = DEFAULT_OUTPUT_FORMAT) {
  return `https://api.elevenlabs.io${MUSIC_API_PATH}?output_format=${encodeURIComponent(outputFormat)}`
}

/**
 * Resolve ELEVENLABS_API_KEY from process env or a KEY=VALUE secrets file.
 * Never logs or returns partial key material for diagnostics.
 */
export function resolveElevenLabsApiKey(options = {}) {
  const env = options.env ?? process.env
  if (env.ELEVENLABS_API_KEY && String(env.ELEVENLABS_API_KEY).trim()) {
    return String(env.ELEVENLABS_API_KEY).trim()
  }
  const envFile =
    options.envFile ||
    env.ELEVENLABS_ENV_FILE ||
    defaultElevenLabsEnvPath(env.HOME || '')
  const readFile = options.readFileSync ?? readFileSync
  try {
    const text = readFile(envFile, 'utf8')
    const match = text.match(/^ELEVENLABS_API_KEY=(\S+)/m)
    if (match) return match[1]
  } catch {
    // fall through
  }
  return undefined
}

export function buildMusicRequestBody({
  musicLengthMs,
  prompt = DEFAULT_PROMPT,
  modelId = DEFAULT_MODEL_ID,
  forceInstrumental = true,
} = {}) {
  const length = Number(musicLengthMs)
  if (!Number.isFinite(length) || !Number.isInteger(length)) {
    throw new Error('music_length_ms must be an integer millisecond duration')
  }
  if (length < MIN_MUSIC_LENGTH_MS || length > MAX_MUSIC_LENGTH_MS) {
    throw new Error(
      `music_length_ms must be between ${MIN_MUSIC_LENGTH_MS} and ${MAX_MUSIC_LENGTH_MS}`,
    )
  }
  const trimmedPrompt = String(prompt || '').trim()
  if (!trimmedPrompt) {
    throw new Error('prompt must be a non-empty string')
  }
  return {
    model_id: modelId,
    force_instrumental: Boolean(forceInstrumental),
    music_length_ms: length,
    prompt: trimmedPrompt,
  }
}

export function durationSecToMusicLengthMs(durationSec) {
  const sec = Number(durationSec)
  if (!Number.isFinite(sec) || sec <= 0) {
    throw new Error('durationSec must be a positive number')
  }
  return Math.max(MIN_MUSIC_LENGTH_MS, Math.round(sec * 1000))
}

/**
 * Derive *-rc-with-music.mp4 from *-rc-no-music.mp4 (or sibling naming).
 */
export function deriveWithMusicPath(noMusicPath) {
  const resolved = resolve(noMusicPath)
  if (/-rc-no-music\.mp4$/i.test(resolved)) {
    return resolved.replace(/-rc-no-music\.mp4$/i, '-rc-with-music.mp4')
  }
  if (/\.mp4$/i.test(resolved)) {
    return resolved.replace(/\.mp4$/i, '-with-music.mp4')
  }
  return `${resolved}-with-music.mp4`
}

/**
 * Derive <episode>-music-bed.mp3 beside an RC plate or episode dir.
 */
export function deriveBedPath({ rcPath, outDir, episode } = {}) {
  if (rcPath) {
    const resolved = resolve(rcPath)
    if (/-rc-no-music\.mp4$/i.test(resolved)) {
      return resolved.replace(/-rc-no-music\.mp4$/i, '-music-bed.mp3')
    }
    return join(dirname(resolved), 'music-bed.mp3')
  }
  if (outDir && episode != null) {
    return join(resolve(outDir), `${episode}-music-bed.mp3`)
  }
  if (outDir) {
    return join(resolve(outDir), 'music-bed.mp3')
  }
  throw new Error('deriveBedPath requires --rc or --out-dir')
}

/**
 * Episode 261 louder mix recipe as one ffmpeg filter_complex string.
 * Input 0 = RC (voice), input 1 = music bed.
 */
export function buildMixFilterComplex({
  durationSec,
  fadeInSec = DEFAULT_FADE_IN_SEC,
  fadeOutSec = DEFAULT_FADE_OUT_SEC,
  bedVolumeDb = DEFAULT_BED_VOLUME_DB,
} = {}) {
  const D = Number(durationSec)
  if (!Number.isFinite(D) || D <= 0) {
    throw new Error('durationSec must be a positive number for mix filters')
  }
  const fadeIn = Number(fadeInSec)
  const fadeOut = Number(fadeOutSec)
  if (!Number.isFinite(fadeIn) || fadeIn < 0) {
    throw new Error('fadeInSec must be a non-negative number')
  }
  if (!Number.isFinite(fadeOut) || fadeOut < 0) {
    throw new Error('fadeOutSec must be a non-negative number')
  }
  const fadeOutStart = Math.max(0, D - fadeOut)
  const vol = Number(bedVolumeDb)
  if (!Number.isFinite(vol)) {
    throw new Error('bedVolumeDb must be a number')
  }
  // loudnorm → volume → fades on the bed; amix with voice; alimiter on sum.
  return (
    `[1:a]loudnorm,volume=${vol}dB,` +
    `afade=t=in:st=0:d=${fadeIn},` +
    `afade=t=out:st=${fadeOutStart}:d=${fadeOut}[bed];` +
    `[0:a][bed]amix=inputs=2:duration=first:dropout_transition=2,alimiter[aout]`
  )
}

export function buildMixFfmpegArgs({
  rcPath,
  bedPath,
  outPath,
  durationSec,
  fadeInSec,
  fadeOutSec,
  bedVolumeDb,
} = {}) {
  const filter = buildMixFilterComplex({
    durationSec,
    fadeInSec,
    fadeOutSec,
    bedVolumeDb,
  })
  return [
    '-y',
    '-i',
    resolve(rcPath),
    '-i',
    resolve(bedPath),
    '-filter_complex',
    filter,
    '-map',
    '0:v:0',
    '-map',
    '[aout]',
    '-c:v',
    'copy',
    '-c:a',
    'aac',
    '-b:a',
    '192k',
    '-shortest',
    resolve(outPath),
  ]
}

/**
 * POST ElevenLabs music compose. Inject fetchImpl in tests (never live).
 * Returns { bytes: Buffer, contentType }.
 */
export async function generateMusicBedBytes({
  apiKey,
  musicLengthMs,
  prompt,
  modelId,
  forceInstrumental = true,
  outputFormat = DEFAULT_OUTPUT_FORMAT,
  fetchImpl = globalThis.fetch,
} = {}) {
  if (!apiKey) {
    throw new Error(
      'missing ELEVENLABS_API_KEY (set env or ~/work/.secrets/elevenlabs.env)',
    )
  }
  if (typeof fetchImpl !== 'function') {
    throw new Error('fetchImpl is required')
  }
  const body = buildMusicRequestBody({
    musicLengthMs,
    prompt,
    modelId,
    forceInstrumental,
  })
  const url = musicApiUrl(outputFormat)
  const response = await fetchImpl(url, {
    method: 'POST',
    headers: {
      'xi-api-key': apiKey,
      'Content-Type': 'application/json',
      Accept: 'audio/mpeg',
    },
    body: JSON.stringify(body),
  })
  if (!response.ok) {
    let detail = ''
    try {
      detail = (await response.text()).slice(0, 200)
    } catch {
      detail = ''
    }
    // Never echo response bodies that might contain request echoes of keys.
    throw new Error(
      `ElevenLabs music HTTP ${response.status}` +
        (detail ? ` (${detail.replace(/xi-api-key[^\s"']*/gi, '[redacted]')})` : ''),
    )
  }
  const arrayBuffer = await response.arrayBuffer()
  return {
    bytes: Buffer.from(arrayBuffer),
    contentType: response.headers?.get?.('content-type') || 'audio/mpeg',
    requestBody: body,
    url,
  }
}

export function assertNoMusicPlateUntouched(noMusicPath, withMusicPath) {
  const a = resolve(noMusicPath)
  const b = resolve(withMusicPath)
  if (a === b) {
    throw new Error(
      'refuse to overwrite the no-music plate; with-music path must differ',
    )
  }
  if (!existsSync(a)) {
    throw new Error(`no-music RC plate missing: ${a}`)
  }
}
