#!/usr/bin/env node
// Drive Omega's Welcome / onboarding UI for Sarah episode screenshares.
//
// Prefer a live screenshare of this controlled UI over a static screenshot.
// The spoken script stays short; this tool only moves the picture.
//
// Requirements (macOS):
// - An Omega binary or Omega.app
// - Accessibility permission for the terminal / agent host (System Settings →
//   Privacy & Security → Accessibility)
// - Optional: `cliclick` on PATH for coordinate fallback
//
// Usage:
//   node scripts/omega-screen-control/omega-screen-control.mjs launch
//   node scripts/omega-screen-control/omega-screen-control.mjs click --title "Aiur"
//   node scripts/omega-screen-control/omega-screen-control.mjs key --combo "cmd+enter"
//   node scripts/omega-screen-control/omega-screen-control.mjs dump-ax
//   node scripts/omega-screen-control/omega-screen-control.mjs shot welcome-setup
//   node scripts/omega-screen-control/omega-screen-control.mjs record --shot welcome-hold --seconds 20 --out ~/Desktop/Sarah/262/welcome.mp4
//   node scripts/omega-screen-control/omega-screen-control.mjs record-motion --shot welcome-tour --seconds 28 --out ~/Desktop/Sarah/262/262-screenshare-omega-welcome.mp4 --safe-tail-seconds 1
//   node scripts/omega-screen-control/omega-screen-control.mjs quit
//     (SIGTERM/SIGKILL on tracked Omega pid — never Cmd+Q)
//
// Recording lifecycle (#9233):
//   Hard-stop ffmpeg BEFORE quit/teardown. Optional --safe-tail-seconds freezes
//   the last clean frame over a dirty quit tail. Finder/Desktop-like tails are
//   trimmed when the heuristic fires.
//
// Env:
//   OMEGA_BIN                 Absolute path to the omega binary
//   OMEGA_APP                 Absolute path to Omega.app (fallback)
//   OMEGA_USER_DATA_DIR       Reuse a data dir (default: fresh temp dir)
//   OMEGA_KEEP_USER_DATA=1    Do not delete the temp data dir on quit
//   OMEGA_PROCESS_NAME        AX process name (default: omega)

import { spawn, execFileSync } from 'node:child_process'
import {
  existsSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from 'node:fs'
import { homedir, tmpdir } from 'node:os'
import { dirname, join, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'
import {
  assertNotCmdQQuit,
  buildSafeTailVideoFilter,
  computeSafeTailPlan,
  estimateFinderTailSeconds,
  parseSafeTailSeconds,
} from './recording-lifecycle.mjs'

const __dirname = dirname(fileURLToPath(import.meta.url))
const STATE_PATH = join(tmpdir(), 'openagents-omega-screen-control.json')
const SHOTS_DIR = join(__dirname, 'shots')

const args = process.argv.slice(2)
const command = args[0]
const flag = (name, fallback = undefined) => {
  const i = args.indexOf(`--${name}`)
  return i >= 0 && i + 1 < args.length ? args[i + 1] : fallback
}
const hasFlag = (name) => args.includes(`--${name}`)

function usage() {
  console.log(`Usage:
  omega-screen-control.mjs launch [--bin <path>] [--app <path>] [--user-data-dir <dir>]
  omega-screen-control.mjs menu --bar Help --item "Editor Onboarding"
  omega-screen-control.mjs click --title <AX title>
  omega-screen-control.mjs click-at --x 0.22 --y 0.42
  omega-screen-control.mjs key --combo cmd+enter|cmd+shift+p|...
  omega-screen-control.mjs wait --contains <text> [--timeout-ms 30000]
  omega-screen-control.mjs dump-ax [--out <path>]
  omega-screen-control.mjs shot <shot-id>
  omega-screen-control.mjs record --shot <shot-id> --out <mp4> [--seconds 20] [--screen-device 4] [--safe-tail-seconds 0]
  omega-screen-control.mjs record-motion --shot <shot-id> --out <mp4> [--seconds 28] [--safe-tail-seconds 1]
  omega-screen-control.mjs quit   # SIGTERM tracked pid; never Cmd+Q

Shots live in scripts/omega-screen-control/shots/*.json

Notes:
  Recording always hard-stops before Omega quit/teardown (#9233).
  Prefer --safe-tail-seconds 1 so a dirty post-UI tail is replaced with a
  freeze of the last clean Omega frame. Finder/Desktop-like tails are trimmed
  when the frame heuristic fires.
  Quit is SIGTERM-only. Never Cmd+Q (refused by key --combo and quit).
  First-run Welcome opens only when Nostr custody is not Ready.
  Reset Ready custody with Omega's omega-identity CLI:
    cargo run -p omega_identity --bin omega-identity -- --channel rc wipe --yes
  On a machine that already has a Ready identity, open Welcome with:
    Help → Editor Onboarding   (or Help → Show Welcome)
  Screenshare MP4s stay local under ~/Desktop/Sarah/<episode>/ (do not commit).
`)
}

function targetPid() {
  const state = loadState()
  if (state?.pid) return Number(state.pid)
  return null
}

function activateOmega() {
  const pid = targetPid()
  if (pid) {
    runOsascript(`
tell application "System Events"
  set p to first process whose unix id is ${pid}
  set frontmost of p to true
end tell
`)
    return
  }
  const name = processName()
  runOsascript(`
tell application "System Events"
  if not (exists process "${name}") then error "Omega process '${name}' is not running"
  set frontmost of process "${name}" to true
end tell
`)
}

function processRefAppleScript() {
  const pid = targetPid()
  if (pid) return `first process whose unix id is ${pid}`
  return `process "${processName()}"`
}

function menuClick(bar, item) {
  activateOmega()
  const ref = processRefAppleScript()
  const barEsc = String(bar).replace(/"/g, '\\"')
  const itemEsc = String(item).replace(/"/g, '\\"')
  runOsascript(`
tell application "System Events"
  set p to ${ref}
  set frontmost of p to true
  click menu item "${itemEsc}" of menu "${barEsc}" of menu bar item "${barEsc}" of menu bar 1 of p
end tell
`)
  console.log(`menu: ${bar} → ${item}`)
}

function windowBounds() {
  activateOmega()
  const ref = processRefAppleScript()
  const raw = runOsascript(`
tell application "System Events"
  set p to ${ref}
  set frontmost of p to true
  set w to window 1 of p
  set b to position of w
  set s to size of w
  return (item 1 of b as text) & "," & (item 2 of b as text) & "," & (item 1 of s as text) & "," & (item 2 of s as text)
end tell
`)
  const [x, y, w, h] = raw.split(',').map((n) => Number(n))
  if (![x, y, w, h].every((n) => Number.isFinite(n))) {
    throw new Error(`Could not read window bounds: ${raw}`)
  }
  return { x, y, w, h }
}

function clickAt(xFrac, yFrac) {
  const bounds = windowBounds()
  const x = Math.round(bounds.x + bounds.w * Number(xFrac))
  const y = Math.round(bounds.y + bounds.h * Number(yFrac))
  if (!commandExists('cliclick') && !existsSync('/opt/homebrew/bin/cliclick')) {
    // Fallback: AppleScript click at screen position via System Events is limited;
    // use cliclick when available.
    throw new Error(
      `click-at needs cliclick on PATH (wanted ${x},${y} for fractions ${xFrac},${yFrac})`,
    )
  }
  const bin = commandExists('cliclick') ? 'cliclick' : '/opt/homebrew/bin/cliclick'
  execFileSync(bin, [`c:${x},${y}`])
  console.log(`clicked-at: ${xFrac},${yFrac} → ${x},${y}`)
}

function loadState() {
  if (!existsSync(STATE_PATH)) return null
  try {
    return JSON.parse(readFileSync(STATE_PATH, 'utf8'))
  } catch {
    return null
  }
}

function saveState(state) {
  writeFileSync(STATE_PATH, JSON.stringify(state, null, 2) + '\n')
}

function clearState() {
  if (existsSync(STATE_PATH)) rmSync(STATE_PATH)
}

function resolveOmegaBin() {
  const fromFlag = flag('bin') || process.env.OMEGA_BIN
  if (fromFlag) return resolve(fromFlag)

  const app =
    flag('app') ||
    process.env.OMEGA_APP ||
    '/Users/christopherdavid/work/omega-worktrees/issue-8-identity-proof/target/omega-identity-rc/dmg-src/Omega.app'
  const nested = join(app, 'Contents/MacOS/omega')
  if (existsSync(nested)) return nested

  const candidates = [
    '/Applications/Omega.app/Contents/MacOS/omega',
    join(process.env.HOME || '', 'Applications/Omega.app/Contents/MacOS/omega'),
  ]
  for (const c of candidates) {
    if (existsSync(c)) return c
  }
  throw new Error(
    'Omega binary not found. Set OMEGA_BIN or OMEGA_APP, or pass --bin / --app.',
  )
}

function processName() {
  return process.env.OMEGA_PROCESS_NAME || 'omega'
}

function runOsascript(source) {
  try {
    return execFileSync('osascript', ['-e', source], {
      encoding: 'utf8',
      stdio: ['ignore', 'pipe', 'pipe'],
    }).trim()
  } catch (err) {
    const stderr = err.stderr?.toString?.() || err.message
    throw new Error(`osascript failed: ${stderr}`)
  }
}

function sleep(ms) {
  Atomics.wait(new Int32Array(new SharedArrayBuffer(4)), 0, 0, ms)
}

function dumpAx() {
  activateOmega()
  const ref = processRefAppleScript()
  return runOsascript(`
tell application "System Events"
  set p to ${ref}
  set frontmost of p to true
  set out to ""
  repeat with w in windows of p
    set out to out & "WINDOW: " & (name of w as text) & linefeed
    try
      repeat with ui in entire contents of w
        try
          set r to role of ui as text
          set n to ""
          try
            set n to name of ui as text
          end try
          if n is not "" and n is not "missing value" then
            set out to out & r & " | " & n & linefeed
          end if
        end try
      end repeat
    end try
  end repeat
  return out
end tell
`)
}

function axContains(text) {
  const tree = dumpAx()
  return tree.toLowerCase().includes(String(text).toLowerCase())
}

function waitContains(text, timeoutMs) {
  const deadline = Date.now() + timeoutMs
  while (Date.now() < deadline) {
    try {
      if (axContains(text)) return true
    } catch {
      // process may still be starting
    }
    sleep(400)
  }
  throw new Error(`Timed out waiting for AX text containing: ${text}`)
}

function clickTitle(title) {
  activateOmega()
  const ref = processRefAppleScript()
  const escaped = String(title).replace(/\\/g, '\\\\').replace(/"/g, '\\"')
  try {
    runOsascript(`
tell application "System Events"
  set p to ${ref}
  set frontmost of p to true
  set target to missing value
  repeat with w in windows of p
    try
      set matches to (every UI element of w whose name is "${escaped}")
      if (count of matches) > 0 then
        set target to item 1 of matches
        exit repeat
      end if
    end try
    try
      set deep to (every UI element of entire contents of w whose name is "${escaped}")
      if (count of deep) > 0 then
        set target to item 1 of deep
        exit repeat
      end if
    end try
  end repeat
  if target is missing value then error "No UI element named ${escaped}"
  click target
end tell
`)
    console.log(`clicked: ${title}`)
    return
  } catch (primaryErr) {
    if (!existsSync('/opt/homebrew/bin/cliclick') && !commandExists('cliclick')) {
      throw primaryErr
    }
    const pos = runOsascript(`
tell application "System Events"
  set p to ${ref}
  set frontmost of p to true
  set target to missing value
  repeat with w in windows of p
    try
      set deep to (every UI element of entire contents of w whose name is "${escaped}")
      if (count of deep) > 0 then
        set target to item 1 of deep
        exit repeat
      end if
    end try
  end repeat
  if target is missing value then error "No UI element named ${escaped}"
  set posn to position of target
  set sz to size of target
  return ((item 1 of posn) + (item 1 of sz) / 2) & "," & ((item 2 of posn) + (item 2 of sz) / 2)
end tell
`)
    const [x, y] = pos.split(',').map((n) => Math.round(Number(n)))
    const bin = commandExists('cliclick') ? 'cliclick' : '/opt/homebrew/bin/cliclick'
    execFileSync(bin, [`c:${x},${y}`])
    console.log(`clicked via cliclick: ${title} @ ${x},${y}`)
  }
}

function commandExists(bin) {
  try {
    execFileSync('which', [bin], { stdio: 'ignore' })
    return true
  } catch {
    return false
  }
}

function keyCombo(combo) {
  // Never allow Cmd+Q through this tool — it can quit the wrong frontmost app.
  assertNotCmdQQuit(combo)
  activateOmega()
  const parts = String(combo)
    .toLowerCase()
    .split('+')
    .map((p) => p.trim())
    .filter(Boolean)
  const key = parts[parts.length - 1]
  const mods = parts.slice(0, -1)
  const modMap = {
    cmd: 'command down',
    command: 'command down',
    ctrl: 'control down',
    control: 'control down',
    alt: 'option down',
    option: 'option down',
    shift: 'shift down',
  }
  const using = mods
    .map((m) => modMap[m])
    .filter(Boolean)
    .join(', ')
  // Named keys that need AppleScript key codes (not keystroke names).
  const keyCodeOnly = {
    up: 126,
    down: 125,
    left: 123,
    right: 124,
    pageup: 116,
    pagedown: 121,
    home: 115,
    end: 119,
  }
  const keyCodeMap = {
    enter: 'return',
    return: 'return',
    escape: 'escape',
    esc: 'escape',
    tab: 'tab',
    space: 'space',
  }
  const usingClause = using ? ` using {${using}}` : ''
  if (keyCodeOnly[key] != null) {
    runOsascript(
      `tell application "System Events" to key code ${keyCodeOnly[key]}${usingClause}`,
    )
  } else {
    const keyName = keyCodeMap[key] || key
    if (keyName.length === 1) {
      runOsascript(
        `tell application "System Events" to keystroke "${keyName}"${usingClause}`,
      )
    } else {
      runOsascript(
        `tell application "System Events" to keystroke ${keyName}${usingClause}`,
      )
    }
  }
  console.log(`keyed: ${combo}`)
}

function enlargeOmegaWindow() {
  activateOmega()
  const ref = processRefAppleScript()
  // Large centered window so Welcome UI is legible when cropped/zoomed.
  runOsascript(`
tell application "System Events"
  set p to ${ref}
  set frontmost of p to true
  set w to window 1 of p
  set size of w to {1480, 980}
  set position of w to {220, 60}
end tell
`)
  console.log('enlarged Omega window to ~1480x980')
}

function dragScroll(direction, amount = 1) {
  // Scroll the RIGHT Welcome pane (split onboarding: content is x>~0.5).
  const bounds = windowBounds()
  const bin = commandExists('cliclick') ? 'cliclick' : '/opt/homebrew/bin/cliclick'
  const x = Math.round(bounds.x + bounds.w * 0.72)
  const y = Math.round(bounds.y + bounds.h * 0.55)
  // Move pointer into the pane first.
  execFileSync(bin, [`m:${x},${y}`])
  sleep(120)
  const delta = direction === 'up' ? 8 : -8
  for (let i = 0; i < amount; i++) {
    try {
      execFileSync(
        'python3',
        [
          '-c',
          `
import Quartz, time
e = Quartz.CGEventCreateScrollWheelEvent(None, Quartz.kCGScrollEventUnitLine, 1, ${delta})
Quartz.CGEventPost(Quartz.kCGHIDEventTap, e)
time.sleep(0.05)
`,
        ],
        { stdio: 'ignore' },
      )
    } catch {
      const yStart =
        direction === 'up'
          ? Math.round(bounds.y + bounds.h * 0.38)
          : Math.round(bounds.y + bounds.h * 0.78)
      const yEnd =
        direction === 'up'
          ? Math.round(bounds.y + bounds.h * 0.78)
          : Math.round(bounds.y + bounds.h * 0.38)
      execFileSync(bin, [
        `m:${x},${yStart}`,
        `dd:${x},${yStart}`,
        `dm:${x},${yEnd}`,
        `du:${x},${yEnd}`,
      ])
    }
    sleep(350)
  }
  console.log(`scroll ${direction} x${amount} @ (${x},${y})`)
}

function displayScaleFactor() {
  // ffmpeg avfoundation captures physical pixels on Retina; AX bounds are points.
  try {
    const raw = runOsascript(`
tell application "Finder" to return 1
`)
    void raw
  } catch {
    // ignore
  }
  try {
    const out = execFileSync(
      'python3',
      [
        '-c',
        'import AppKit; print(AppKit.NSScreen.mainScreen().backingScaleFactor())',
      ],
      { encoding: 'utf8' },
    ).trim()
    const n = Number(out)
    if (Number.isFinite(n) && n >= 1) return n
  } catch {
    // fall through
  }
  return 2
}

function listOmegaPids() {
  try {
    const out = execFileSync('pgrep', ['-f', '/Contents/MacOS/omega'], {
      encoding: 'utf8',
    })
    return out
      .split('\n')
      .map((s) => s.trim())
      .filter(Boolean)
      .map(Number)
      .filter((n) => Number.isFinite(n))
  } catch {
    return []
  }
}

function launch() {
  const bin = resolveOmegaBin()
  const reuse = flag('user-data-dir') || process.env.OMEGA_USER_DATA_DIR
  const userDataDir = reuse
    ? resolve(reuse)
    : mkdtempSync(join(tmpdir(), 'omega-screen-control-'))
  // Fresh Welcome needs a disposable data dir AND a separate instance.
  // Without ZED_STATELESS, a second launch hands off to an existing Omega.
  const env = {
    ...process.env,
    ZED_EXPERIMENTAL_A11Y: '1',
    ZED_STATELESS: process.env.ZED_STATELESS || '1',
  }
  const before = new Set(listOmegaPids())
  console.log(`launching: ${bin}`)
  console.log(`user-data-dir: ${userDataDir}`)
  console.log(`a11y: ZED_EXPERIMENTAL_A11Y=1`)
  console.log(`stateless: ZED_STATELESS=${env.ZED_STATELESS}`)
  if (before.size > 0) {
    console.log(
      `note: ${before.size} Omega process(es) already running; using a separate ZED_STATELESS instance`,
    )
  }
  const child = spawn(bin, [`--user-data-dir=${userDataDir}`], {
    env,
    detached: true,
    stdio: 'ignore',
  })
  child.unref()
  // Resolve the real app pid (spawn pid can be a short-lived wrapper).
  let appPid = child.pid
  const deadline = Date.now() + 8000
  while (Date.now() < deadline) {
    const now = listOmegaPids().filter((pid) => !before.has(pid))
    if (now.length > 0) {
      appPid = now[now.length - 1]
      break
    }
    sleep(200)
  }
  saveState({
    pid: appPid,
    spawnPid: child.pid,
    bin,
    userDataDir,
    createdAt: new Date().toISOString(),
    ephemeral: !reuse,
  })
  console.log(`pid: ${appPid}`)
  console.log(`state: ${STATE_PATH}`)
}

/** Global guard so quit() cannot run while ffmpeg is still capturing. */
let activeRecording = null

function quit() {
  // Hard rule (#9233): SIGTERM/SIGKILL on tracked Omega pid only.
  // Never Cmd+Q — that can quit Cursor (or any other frontmost app).
  if (activeRecording && !activeRecording.stopped) {
    throw new Error(
      'Refusing quit while screen recording is still active. Hard-stop capture first.',
    )
  }
  const state = loadState()
  const pids = []
  for (const key of ['pid', 'spawnPid']) {
    const n = Number(state?.[key])
    if (Number.isFinite(n) && n > 0 && !pids.includes(n)) pids.push(n)
  }
  for (const pid of pids) {
    try {
      process.kill(pid, 'SIGTERM')
      console.log(`SIGTERM: ${pid}`)
    } catch {
      // already gone
    }
  }
  if (pids.length > 0) sleep(800)
  for (const pid of pids) {
    try {
      process.kill(pid, 0)
      process.kill(pid, 'SIGKILL')
      console.log(`SIGKILL: ${pid}`)
    } catch {
      // already gone
    }
  }
  if (pids.length === 0) {
    console.log('quit: no tracked Omega pid in state; nothing to signal')
  }
  if (
    state?.ephemeral &&
    state?.userDataDir &&
    process.env.OMEGA_KEEP_USER_DATA !== '1'
  ) {
    try {
      rmSync(state.userDataDir, { recursive: true, force: true })
      console.log(`removed user-data-dir: ${state.userDataDir}`)
    } catch (err) {
      console.warn(`could not remove user-data-dir: ${err.message}`)
    }
  }
  clearState()
  console.log('quit requested (SIGTERM-only; never Cmd+Q)')
}

function resolveFfmpegBin() {
  if (commandExists('ffmpeg')) return 'ffmpeg'
  if (existsSync('/opt/homebrew/bin/ffmpeg')) return '/opt/homebrew/bin/ffmpeg'
  throw new Error('ffmpeg not found on PATH')
}

function waitChildExit(child, timeoutMs) {
  return new Promise((resolvePromise) => {
    if (child.exitCode != null || child.signalCode != null) {
      resolvePromise(child.exitCode ?? 0)
      return
    }
    let settled = false
    const done = (code) => {
      if (settled) return
      settled = true
      clearTimeout(timer)
      resolvePromise(code)
    }
    const timer = setTimeout(() => done(null), timeoutMs)
    child.once('exit', (code) => done(code ?? 0))
  })
}

/**
 * Start an avfoundation screen capture. Caller MUST await stop() before quit().
 * Prefer hard-stop (stdin 'q' / SIGINT) over waiting for teardown-adjacent frames.
 */
function startScreenRecording({ outPath, screenDevice, maxSeconds }) {
  const ffmpegBin = resolveFfmpegBin()
  const absolute = resolve(expandHome(outPath))
  mkdirSync(dirname(absolute), { recursive: true })
  const ffArgs = [
    '-y',
    '-f',
    'avfoundation',
    '-framerate',
    '30',
    '-capture_cursor',
    '1',
    '-i',
    `${screenDevice}:none`,
  ]
  if (maxSeconds != null && Number.isFinite(Number(maxSeconds))) {
    ffArgs.push('-t', String(maxSeconds))
  }
  ffArgs.push('-c:v', 'libx264', '-pix_fmt', 'yuv420p', '-an', absolute)
  console.log(
    `recording screen device ${screenDevice}` +
      (maxSeconds != null ? ` (max ${maxSeconds}s)` : '') +
      ` → ${absolute}`,
  )
  const ff = spawn(ffmpegBin, ffArgs, { stdio: ['pipe', 'inherit', 'inherit'] })
  const handle = {
    path: absolute,
    stopped: false,
    process: ff,
    async stop(reason = 'hard-stop') {
      if (handle.stopped) return absolute
      handle.stopped = true
      if (activeRecording === handle) activeRecording = null
      if (ff.exitCode != null || ff.signalCode != null) {
        console.log(`recording already ended (${reason})`)
        return absolute
      }
      console.log(`hard-stopping recording (${reason}) before any app teardown`)
      try {
        ff.stdin.write('q')
        ff.stdin.end()
      } catch {
        // stdin may already be closed
      }
      let code = await waitChildExit(ff, 2500)
      if (code == null && ff.exitCode == null && ff.signalCode == null) {
        try {
          ff.kill('SIGINT')
        } catch {
          // ignore
        }
        code = await waitChildExit(ff, 2000)
      }
      if (ff.exitCode == null && ff.signalCode == null) {
        try {
          ff.kill('SIGKILL')
        } catch {
          // ignore
        }
        await waitChildExit(ff, 1000)
      }
      console.log(`recording stopped → ${absolute}`)
      return absolute
    },
    async waitNaturalEnd() {
      const code = await new Promise((resolvePromise) => {
        ff.on('exit', (exitCode) => resolvePromise(exitCode ?? 1))
      })
      handle.stopped = true
      if (activeRecording === handle) activeRecording = null
      if (code !== 0 && code !== null) {
        throw new Error(`ffmpeg exited ${code}`)
      }
      return absolute
    },
  }
  activeRecording = handle
  return handle
}

function probeMediaDurationSec(ffmpegBin, mediaPath) {
  try {
    const out = execFileSync(
      ffmpegBin,
      ['-i', mediaPath, '-f', 'null', '-'],
      { encoding: 'utf8', stdio: ['ignore', 'pipe', 'pipe'] },
    )
    void out
  } catch (err) {
    const text = `${err.stderr || ''}${err.stdout || ''}`
    const match = text.match(/Duration:\s*(\d+):(\d+):(\d+(?:\.\d+)?)/)
    if (!match) return null
    const h = Number(match[1])
    const m = Number(match[2])
    const s = Number(match[3])
    if (![h, m, s].every((n) => Number.isFinite(n))) return null
    return h * 3600 + m * 60 + s
  }
  return null
}

/**
 * Sample mean luma / variance / dark-ui ratio at time t via ffmpeg+python.
 * Fail-soft: returns null when tools are unavailable.
 */
function sampleFrameStats(ffmpegBin, mediaPath, timeSec) {
  const tmpDir = mkdtempSync(join(tmpdir(), 'omega-frame-stats-'))
  const png = join(tmpDir, 'frame.png')
  try {
    execFileSync(
      ffmpegBin,
      [
        '-y',
        '-ss',
        String(Math.max(0, timeSec)),
        '-i',
        mediaPath,
        '-frames:v',
        '1',
        png,
      ],
      { stdio: 'ignore' },
    )
    const raw = execFileSync(
      'python3',
      [
        '-c',
        `
from PIL import Image
import statistics
im = Image.open(${JSON.stringify(png)}).convert("RGB").resize((160, 90))
px = list(im.getdata())
lumas = [0.2126*r + 0.7152*g + 0.0722*b for r,g,b in px]
mean = statistics.fmean(lumas)
var = statistics.pvariance(lumas) if len(lumas) > 1 else 0.0
dark = sum(1 for y in lumas if y < 40) / len(lumas)
print(f"{mean:.4f},{var:.4f},{dark:.4f}")
`,
      ],
      { encoding: 'utf8' },
    ).trim()
    const [meanLuma, variance, darkUiRatio] = raw.split(',').map(Number)
    if (![meanLuma, variance, darkUiRatio].every((n) => Number.isFinite(n))) {
      return null
    }
    return { t: timeSec, meanLuma, variance, darkUiRatio }
  } catch {
    return null
  } finally {
    try {
      rmSync(tmpDir, { recursive: true, force: true })
    } catch {
      // ignore
    }
  }
}

function scanFinderTailSeconds(ffmpegBin, mediaPath, durationSec) {
  if (!Number.isFinite(durationSec) || durationSec < 1) return 0
  const midT = Math.max(0.2, durationSec * 0.45)
  const referenceStats = sampleFrameStats(ffmpegBin, mediaPath, midT)
  if (!referenceStats) {
    console.log('finder-heuristic: skipped (could not sample reference frame)')
    return 0
  }
  const step = 0.2
  const maxScan = Math.min(1.5, durationSec * 0.4)
  const sampleStats = []
  for (let t = Math.max(0, durationSec - maxScan); t < durationSec - 0.05; t += step) {
    const stats = sampleFrameStats(ffmpegBin, mediaPath, t)
    if (stats) sampleStats.push(stats)
  }
  const trim = estimateFinderTailSeconds({
    durationSec,
    sampleStepSec: step,
    sampleStats,
    referenceStats,
    maxScanSec: maxScan,
  })
  if (trim > 0) {
    console.log(
      `finder-heuristic: trimming ~${trim.toFixed(2)}s Desktop/Finder-like tail`,
    )
  } else {
    console.log('finder-heuristic: tail looks clean')
  }
  return trim
}

/**
 * Apply optional --safe-tail-seconds freeze and Finder/Desktop heuristic trim.
 * Runs on an already-stopped recording (Omega may still be alive).
 */
function applyTailSafety(mediaPath, { safeTailSeconds = 0 } = {}) {
  const ffmpegBin = resolveFfmpegBin()
  const durationSec = probeMediaDurationSec(ffmpegBin, mediaPath)
  if (!Number.isFinite(durationSec) || durationSec <= 0) {
    console.warn('tail-safety: could not probe duration; leaving file unchanged')
    return mediaPath
  }
  const finderTrimSeconds = scanFinderTailSeconds(ffmpegBin, mediaPath, durationSec)
  const plan = computeSafeTailPlan({
    durationSec,
    safeTailSeconds,
    finderTrimSeconds,
  })
  const vf = buildSafeTailVideoFilter(plan)
  if (!vf) {
    console.log('tail-safety: no trim/freeze needed')
    return mediaPath
  }
  const tmpOut = mediaPath.replace(/\.mp4$/i, '') + '-tail-safe.mp4'
  console.log(
    `tail-safety: action=${plan.action} keepUntil=${plan.keepUntilSec.toFixed(3)}s` +
      (plan.freezeDurationSec > 0
        ? ` freeze=${plan.freezeDurationSec.toFixed(3)}s`
        : ''),
  )
  execFileSync(
    ffmpegBin,
    [
      '-y',
      '-i',
      mediaPath,
      '-vf',
      vf,
      '-c:v',
      'libx264',
      '-pix_fmt',
      'yuv420p',
      '-an',
      tmpOut,
    ],
    { stdio: ['ignore', 'inherit', 'inherit'] },
  )
  // Replace original in place.
  rmSync(mediaPath, { force: true })
  execFileSync('mv', [tmpOut, mediaPath])
  console.log(`tail-safety: wrote ${mediaPath}`)
  return mediaPath
}

async function runShot(shotId) {
  const path = join(SHOTS_DIR, `${shotId}.json`)
  if (!existsSync(path)) {
    throw new Error(`Shot not found: ${path}`)
  }
  const shot = JSON.parse(readFileSync(path, 'utf8'))
  console.log(`shot: ${shot.id}`)
  if (shot.label) console.log(`label: ${shot.label}`)
  for (const [index, step] of (shot.steps || []).entries()) {
    const op = step.op
    console.log(`[${index + 1}/${shot.steps.length}] ${op}${step.title ? ` ${step.title}` : ''}${step.contains ? ` ${step.contains}` : ''}`)
    switch (op) {
      case 'launch':
        launch()
        break
      case 'wait':
        sleep(Number(step.ms || 0))
        break
      case 'wait_ax':
        waitContains(step.contains, Number(step.timeoutMs || 30000))
        break
      case 'menu':
        menuClick(step.bar, step.item)
        break
      case 'click':
        clickTitle(step.title)
        break
      case 'click_at':
        clickAt(step.x, step.y)
        break
      case 'key':
        keyCombo(step.combo)
        break
      case 'scroll_drag':
        dragScroll(step.direction || 'down', Number(step.amount || 1))
        break
      case 'enlarge':
        enlargeOmegaWindow()
        break
      case 'dump_ax':
        console.log(dumpAx())
        break
      case 'quit':
        quit()
        break
      default:
        throw new Error(`Unknown shot op: ${op}`)
    }
    if (step.pauseMs) sleep(Number(step.pauseMs))
  }
  console.log('shot complete')
}

function expandHome(path) {
  if (path.startsWith('~/')) return join(homedir(), path.slice(2))
  return path
}

function detectScreenDevice() {
  const fromFlag = flag('screen-device') || process.env.OMEGA_SCREEN_DEVICE
  if (fromFlag != null) return String(fromFlag)
  try {
    execFileSync('ffmpeg', ['-f', 'avfoundation', '-list_devices', 'true', '-i', ''], {
      encoding: 'utf8',
      stdio: ['ignore', 'pipe', 'pipe'],
    })
  } catch (err) {
    const text = `${err.stderr || ''}${err.stdout || ''}`
    const match = text.match(/\[(\d+)\] Capture screen 0/)
    if (match) return match[1]
  }
  return '4'
}

async function recordShot() {
  const shotId = flag('shot') || args[1]
  const out = flag('out')
  const seconds = Number(flag('seconds', '20'))
  const safeTailSeconds = parseSafeTailSeconds(flag('safe-tail-seconds'), 0)
  if (!shotId) throw new Error('--shot <shot-id> is required')
  if (!out) throw new Error('--out <mp4 path> is required')
  if (!Number.isFinite(seconds) || seconds < 1) {
    throw new Error('--seconds must be a positive number')
  }
  const screenDevice = detectScreenDevice()
  await runShot(shotId)
  activateOmega()
  sleep(500)
  const recording = startScreenRecording({
    outPath: out,
    screenDevice,
    maxSeconds: seconds,
  })
  try {
    await recording.waitNaturalEnd()
  } finally {
    // Hard-stop before any Omega teardown (#9233).
    await recording.stop('pre-quit')
  }
  applyTailSafety(recording.path, { safeTailSeconds })
  quit()
}

async function recordMotion() {
  // Setup Welcome, enlarge for legibility, then record WHILE running motion.
  // Hard-stop capture before quit. Post-process crops/zooms to the Omega window.
  const shotId = flag('shot') || 'welcome-tour'
  const out = flag('out')
  const seconds = Number(flag('seconds', '28'))
  // Default 1s safe-tail for motion captures: Ep262 leaked ~0.9s Finder.
  const safeTailSeconds = parseSafeTailSeconds(flag('safe-tail-seconds'), 1)
  if (!out) throw new Error('--out <mp4 path> is required')
  if (!Number.isFinite(seconds) || seconds < 5) {
    throw new Error('--seconds must be >= 5')
  }

  const path = join(SHOTS_DIR, `${shotId}.json`)
  if (!existsSync(path)) throw new Error(`Shot not found: ${path}`)
  const shot = JSON.parse(readFileSync(path, 'utf8'))
  const steps = shot.steps || []
  const setupOps = new Set(['launch', 'wait', 'wait_ax', 'menu', 'enlarge'])
  const setup = []
  const motion = []
  let seenMotionBoundary = false
  for (const step of steps) {
    if (!seenMotionBoundary && setupOps.has(step.op)) {
      setup.push(step)
      continue
    }
    seenMotionBoundary = true
    motion.push(step)
  }
  if (setup.length === 0) {
    throw new Error('record-motion shot needs launch/menu setup steps first')
  }

  console.log(`record-motion shot=${shotId} setup=${setup.length} motion=${motion.length}`)
  // Run setup as a temporary shot.
  for (const [index, step] of setup.entries()) {
    console.log(`[setup ${index + 1}/${setup.length}] ${step.op}`)
    switch (step.op) {
      case 'launch':
        launch()
        break
      case 'wait':
        sleep(Number(step.ms || 0))
        break
      case 'wait_ax':
        waitContains(step.contains, Number(step.timeoutMs || 30000))
        break
      case 'menu':
        menuClick(step.bar, step.item)
        break
      case 'enlarge':
        enlargeOmegaWindow()
        break
      default:
        throw new Error(`Unexpected setup op: ${step.op}`)
    }
    if (step.pauseMs) sleep(Number(step.pauseMs))
  }
  enlargeOmegaWindow()
  sleep(600)

  const absolute = resolve(expandHome(out))
  mkdirSync(dirname(absolute), { recursive: true })
  const rawPath = absolute.replace(/\.mp4$/i, '') + '-raw-full.mp4'
  const screenDevice = detectScreenDevice()
  const ffmpegBin = resolveFfmpegBin()

  activateOmega()
  const boundsBefore = windowBounds()
  console.log(
    `recording motion ${seconds}s device=${screenDevice} window=${JSON.stringify(boundsBefore)}`,
  )
  // Cap with -t as a safety bound, but hard-stop as soon as motion ends so we
  // never keep capturing through quit/teardown into Finder/Desktop.
  const recording = startScreenRecording({
    outPath: rawPath,
    screenDevice,
    maxSeconds: seconds,
  })

  try {
    // Give ffmpeg a moment to start, then drive the UI.
    sleep(900)
    for (const [index, step] of motion.entries()) {
      console.log(`[motion ${index + 1}/${motion.length}] ${step.op}`)
      switch (step.op) {
        case 'wait':
          sleep(Number(step.ms || 0))
          break
        case 'click_at':
          clickAt(step.x, step.y)
          break
        case 'click':
          clickTitle(step.title)
          break
        case 'key':
          keyCombo(step.combo)
          break
        case 'scroll_drag':
          dragScroll(step.direction || 'down', Number(step.amount || 1))
          break
        case 'enlarge':
          enlargeOmegaWindow()
          break
        case 'menu':
          menuClick(step.bar, step.item)
          break
        default:
          console.warn(`skipping unsupported motion op: ${step.op}`)
      }
      if (step.pauseMs) sleep(Number(step.pauseMs))
    }
    // Brief settle on the last clean Omega frame, then hard-stop.
    sleep(400)
  } finally {
    await recording.stop('motion-complete-pre-quit')
  }

  // Crop/zoom to the RIGHT onboarding content pane (legible Welcome copy).
  // AX bounds are points; Retina capture is in physical pixels.
  const scale = displayScaleFactor()
  const b = boundsBefore
  const pad = 16
  // Content lives on the right ~58% of the Welcome/Onboarding window.
  const contentX = b.x + b.w * 0.42
  const contentY = b.y + 36
  const contentW = b.w * 0.56
  const contentH = b.h - 56
  const cropX = Math.max(0, Math.round((contentX - pad) * scale))
  const cropY = Math.max(0, Math.round((contentY - pad) * scale))
  const cropW = Math.round((contentW + pad * 2) * scale)
  const cropH = Math.round((contentH + pad * 2) * scale)
  // Even dimensions for yuv420p.
  const even = (n) => (n % 2 === 0 ? n : n - 1)
  const vf = `crop=${even(cropW)}:${even(cropH)}:${even(cropX)}:${even(cropY)},scale=1920:1088:force_original_aspect_ratio=decrease,pad=1920:1088:(ow-iw)/2:(oh-ih)/2,setsar=1`
  console.log(`zoom crop scale=${scale}: ${vf}`)
  execFileSync(
    ffmpegBin,
    ['-y', '-i', rawPath, '-vf', vf, '-c:v', 'libx264', '-pix_fmt', 'yuv420p', '-an', absolute],
    { stdio: ['ignore', 'inherit', 'inherit'] },
  )
  console.log(`wrote zoomed motion capture ${absolute}`)
  applyTailSafety(absolute, { safeTailSeconds })
  // Teardown only after recording is fully stopped and tail-safe.
  quit()
}

try {
  switch (command) {
    case 'launch':
      launch()
      break
    case 'menu': {
      const bar = flag('bar')
      const item = flag('item')
      if (!bar || !item) throw new Error('--bar and --item are required')
      menuClick(bar, item)
      break
    }
    case 'click': {
      const title = flag('title')
      if (!title) throw new Error('--title is required')
      clickTitle(title)
      break
    }
    case 'click-at': {
      const x = flag('x')
      const y = flag('y')
      if (x == null || y == null) throw new Error('--x and --y are required (0..1 fractions)')
      clickAt(x, y)
      break
    }
    case 'key': {
      const combo = flag('combo')
      if (!combo) throw new Error('--combo is required')
      keyCombo(combo)
      break
    }
    case 'wait': {
      const contains = flag('contains')
      if (!contains) throw new Error('--contains is required')
      waitContains(contains, Number(flag('timeout-ms', '30000')))
      console.log(`found: ${contains}`)
      break
    }
    case 'dump-ax': {
      const tree = dumpAx()
      const out = flag('out')
      if (out) {
        writeFileSync(resolve(out), tree + '\n')
        console.log(`wrote ${out}`)
      } else {
        console.log(tree)
      }
      break
    }
    case 'shot': {
      const shotId = args[1]
      if (!shotId) throw new Error('shot id required')
      await runShot(shotId)
      break
    }
    case 'record':
      await recordShot()
      break
    case 'record-motion':
      await recordMotion()
      break
    case 'quit':
      quit()
      break
    case undefined:
    case 'help':
    case '-h':
    case '--help':
      usage()
      break
    default:
      usage()
      throw new Error(`Unknown command: ${command}`)
  }
} catch (err) {
  console.error(err.message || err)
  process.exit(1)
}
