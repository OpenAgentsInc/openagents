#!/usr/bin/env node
// Sarah RC ElevenLabs instrumental music bed (#9235).
//
// Reads ELEVENLABS_API_KEY from the environment or
// ~/work/.secrets/elevenlabs.env (never prints the key). Generates an
// instrumental bed via POST /v1/music (music_v2, force_instrumental).
// Optional --mix applies the Episode 261 louder recipe onto a
// *-rc-no-music.mp4 plate and writes *-rc-with-music.mp4 without touching
// the clean plate.
//
// Usage:
//   node scripts/sarah-avatar/elevenlabs-music-bed.mjs \
//     --rc ~/Desktop/Sarah/262/262-rc-no-music.mp4 \
//     --mix
//
//   node scripts/sarah-avatar/elevenlabs-music-bed.mjs \
//     --music-length-ms 44640 \
//     --bed-out ~/Desktop/Sarah/262/262-music-bed.mp3

import { execFileSync } from 'node:child_process'
import {
  existsSync,
  mkdirSync,
  writeFileSync,
} from 'node:fs'
import { dirname, resolve } from 'node:path'
import {
  assertNoMusicPlateUntouched,
  buildMixFfmpegArgs,
  DEFAULT_BED_VOLUME_DB,
  DEFAULT_FADE_IN_SEC,
  DEFAULT_FADE_OUT_SEC,
  DEFAULT_PROMPT,
  defaultElevenLabsEnvPath,
  deriveBedPath,
  deriveWithMusicPath,
  durationSecToMusicLengthMs,
  generateMusicBedBytes,
  resolveElevenLabsApiKey,
} from './elevenlabs-music-bed-lib.mjs'

const args = process.argv.slice(2)
const flag = (name, fallback = undefined) => {
  const i = args.indexOf(`--${name}`)
  return i >= 0 && i + 1 < args.length ? args[i + 1] : fallback
}
const hasFlag = (name) => args.includes(`--${name}`)

function printHelp() {
  console.log(`Sarah RC ElevenLabs music bed (#9235)

Usage:
  node scripts/sarah-avatar/elevenlabs-music-bed.mjs \\
    --rc <*-rc-no-music.mp4> [--mix] \\
    [--bed-out <mp3>] [--with-music-out <mp4>] \\
    [--music-length-ms <n>] [--prompt "..."] \\
    [--fade-in 1.5] [--fade-out 3] [--bed-volume-db -3] \\
    [--env-file ~/work/.secrets/elevenlabs.env]

Behavior:
  - Loads ELEVENLABS_API_KEY from env or secrets file (never prints it)
  - POSTs music_v2 with force_instrumental=true
  - Writes <episode>-music-bed.mp3 beside the RC when --bed-out omitted
  - --mix keeps *-rc-no-music.mp4 untouched; writes *-rc-with-music.mp4
  - Mix recipe: loudnorm → volume=-3dB → fades → amix → alimiter (Ep261 louder)
`)
}

if (hasFlag('help') || hasFlag('h')) {
  printHelp()
  process.exit(0)
}

function resolveFfmpegBin() {
  if (process.env.FFMPEG_BIN && existsSync(process.env.FFMPEG_BIN)) {
    return process.env.FFMPEG_BIN
  }
  for (const candidate of ['ffmpeg', '/opt/homebrew/bin/ffmpeg', '/usr/local/bin/ffmpeg']) {
    try {
      execFileSync(candidate, ['-version'], { stdio: 'ignore' })
      return candidate
    } catch {
      // try next
    }
  }
  throw new Error('ffmpeg not found on PATH (set FFMPEG_BIN)')
}

function resolveFfprobeBin(ffmpegBin) {
  if (process.env.FFPROBE_BIN && existsSync(process.env.FFPROBE_BIN)) {
    return process.env.FFPROBE_BIN
  }
  const sibling = ffmpegBin.replace(/ffmpeg$/, 'ffprobe')
  if (sibling !== ffmpegBin && existsSync(sibling)) return sibling
  for (const candidate of ['ffprobe', '/opt/homebrew/bin/ffprobe', '/usr/local/bin/ffprobe']) {
    try {
      execFileSync(candidate, ['-version'], { stdio: 'ignore' })
      return candidate
    } catch {
      // try next
    }
  }
  throw new Error('ffprobe not found on PATH (set FFPROBE_BIN)')
}

function probeDurationSec(ffprobeBin, mediaPath) {
  const out = execFileSync(
    ffprobeBin,
    [
      '-v',
      'error',
      '-show_entries',
      'format=duration',
      '-of',
      'default=noprint_wrappers=1:nokey=1',
      resolve(mediaPath),
    ],
    { encoding: 'utf8' },
  ).trim()
  const sec = Number(out)
  if (!Number.isFinite(sec) || sec <= 0) {
    throw new Error(`could not probe duration for ${mediaPath}`)
  }
  return sec
}

function parseNumberFlag(raw, name) {
  if (raw == null) return undefined
  const n = Number(raw)
  if (!Number.isFinite(n)) {
    throw new Error(`${name} must be a number`)
  }
  return n
}

async function main() {
  const envFile = flag('env-file') || process.env.ELEVENLABS_ENV_FILE
  const apiKey = resolveElevenLabsApiKey({ envFile })
  if (!apiKey) {
    console.error(
      'error: no ElevenLabs API key. Set ELEVENLABS_API_KEY or place ' +
        `ELEVENLABS_API_KEY=… in ${envFile || defaultElevenLabsEnvPath()}. ` +
        'Never invent a key. Do not print the key.',
    )
    process.exit(1)
  }

  const rcPath = flag('rc') ? resolve(flag('rc')) : undefined
  const wantMix = hasFlag('mix')
  if (wantMix && !rcPath) {
    console.error('error: --mix requires --rc <*-rc-no-music.mp4>')
    process.exit(1)
  }
  if (rcPath && !existsSync(rcPath)) {
    console.error(`error: RC path not found: ${rcPath}`)
    process.exit(1)
  }

  const outDir = flag('out-dir') ? resolve(flag('out-dir')) : undefined
  const episode = flag('episode')
  const bedOut =
    flag('bed-out') ||
    (rcPath || outDir
      ? deriveBedPath({ rcPath, outDir, episode })
      : undefined)
  if (!bedOut) {
    console.error(
      'error: pass --rc, --bed-out, or --out-dir (and optional --episode) for the bed path',
    )
    process.exit(1)
  }

  let durationSec
  let musicLengthMs = parseNumberFlag(flag('music-length-ms'), '--music-length-ms')
  if (musicLengthMs != null && !Number.isInteger(musicLengthMs)) {
    musicLengthMs = Math.round(musicLengthMs)
  }

  let ffmpegBin
  let ffprobeBin
  if (rcPath || wantMix) {
    ffmpegBin = resolveFfmpegBin()
    ffprobeBin = resolveFfprobeBin(ffmpegBin)
  }

  if (musicLengthMs == null) {
    if (!rcPath) {
      console.error(
        'error: pass --music-length-ms when --rc is omitted (needed for music_length_ms)',
      )
      process.exit(1)
    }
    durationSec = probeDurationSec(ffprobeBin, rcPath)
    musicLengthMs = durationSecToMusicLengthMs(durationSec)
    console.log(
      `duration: RC ${durationSec.toFixed(3)}s → music_length_ms=${musicLengthMs}`,
    )
  } else if (rcPath && wantMix) {
    durationSec = probeDurationSec(ffprobeBin, rcPath)
    console.log(`duration: RC ${durationSec.toFixed(3)}s (music_length_ms override ${musicLengthMs})`)
  }

  const prompt = flag('prompt', DEFAULT_PROMPT)
  console.log(`generate: ElevenLabs music_v2 instrumental → ${bedOut}`)
  const generated = await generateMusicBedBytes({
    apiKey,
    musicLengthMs,
    prompt,
  })
  mkdirSync(dirname(resolve(bedOut)), { recursive: true })
  writeFileSync(bedOut, generated.bytes)
  console.log(`wrote: bed ${bedOut} (${generated.bytes.length} bytes)`)

  if (!wantMix) {
    console.log('done: bed only (pass --mix to write *-rc-with-music.mp4)')
    return
  }

  const withMusicOut = resolve(
    flag('with-music-out') || deriveWithMusicPath(rcPath),
  )
  assertNoMusicPlateUntouched(rcPath, withMusicOut)
  if (durationSec == null) {
    durationSec = probeDurationSec(ffprobeBin, rcPath)
  }

  const fadeIn = parseNumberFlag(flag('fade-in'), '--fade-in') ?? DEFAULT_FADE_IN_SEC
  const fadeOut = parseNumberFlag(flag('fade-out'), '--fade-out') ?? DEFAULT_FADE_OUT_SEC
  const bedVolumeDb =
    parseNumberFlag(flag('bed-volume-db'), '--bed-volume-db') ?? DEFAULT_BED_VOLUME_DB

  const ffmpegArgs = buildMixFfmpegArgs({
    rcPath,
    bedPath: bedOut,
    outPath: withMusicOut,
    durationSec,
    fadeInSec: fadeIn,
    fadeOutSec: fadeOut,
    bedVolumeDb,
  })
  console.log(
    `mix: Ep261 louder (loudnorm, volume=${bedVolumeDb}dB, fade-in ${fadeIn}s, fade-out ${fadeOut}s) → ${withMusicOut}`,
  )
  mkdirSync(dirname(withMusicOut), { recursive: true })
  execFileSync(ffmpegBin, ffmpegArgs, { stdio: 'inherit' })
  console.log(`wrote: ${withMusicOut}`)
  console.log(`kept:  ${rcPath} (clean no-music plate)`)
}

main().catch((err) => {
  console.error(`error: ${err?.message || err}`)
  process.exit(1)
})
