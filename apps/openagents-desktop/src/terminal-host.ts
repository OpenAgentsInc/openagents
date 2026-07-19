/**
 * Workspace-bounded PTY terminal host (CUT-20, #8700 — MAIN PROCESS ONLY).
 *
 * Owns the typed PTY lifecycle for the interactive-terminal / stdin-steering
 * capability (audit D3). Every session is bound at create time to the currently
 * authorized workspace root + a bounded environment; the renderer can steer
 * stdin but never chooses the shell, the argv, the cwd, or the env.
 *
 * BACKEND SEAM. The spawn mechanism is a pluggable `TerminalBackend`. The
 * default `childProcessTerminalBackend` spawns the login shell in its OWN
 * process group (`detached: true`), which gives us a real child-process TREE we
 * can kill as a unit (`kill(-pgid)`), real stdin steering, real stdout/stderr,
 * and a real exit code — with ZERO native dependencies, so it runs identically
 * under `pnpm exec vp test` (the verify runtime) and in Electron main.
 *
 * NODE-PTY (documented, deferred). `node-pty` is the standard pseudo-TTY and is
 * the intended enhanced backend (line editing / colors / `isatty`). It is NOT
 * wired as the default here for two receipted reasons: (1) under Bun — the
 * runtime `pnpm exec vp test` uses — `node-pty` loads its native addon but its
 * fork-helper spawn fails (`posix_spawnp failed`), so it cannot back the
 * adversarial suite or the built-host receipt; (2) its shipped prebuilds do not
 * match this environment's Node/Electron ABI, so an Electron backend needs an
 * `electron-rebuild` step wired into the #8574 packaging lane. Dropping it in is
 * a one-file `TerminalBackend` swap once that lane exists. See #8700.
 */
import { randomUUID } from "node:crypto"
import { spawn as spawnChildProcess } from "node:child_process"
import { mkdirSync, readFileSync, renameSync, writeFileSync } from "node:fs"
import path from "node:path"

import type { TerminalEvent, TerminalSessionView, TerminalSnapshot } from "./terminal-contract.ts"
import { terminalSessionRefPattern } from "./terminal-contract.ts"

// ---------------------------------------------------------------------------
// Backend seam.
// ---------------------------------------------------------------------------

export type TerminalBackendProcess = Readonly<{
  pid: number | null
  write: (data: string) => void
  resize: (cols: number, rows: number) => void
  /** Ctrl-C: SIGINT to the owned process group. */
  interrupt: () => void
  /** Terminate the owned process TREE. Idempotent at the backend layer. */
  kill: () => void
  onData: (listener: (chunk: string) => void) => void
  onExit: (listener: (exitCode: number | null, signal: string | null) => void) => void
}>

export type TerminalBackendSpawnInput = Readonly<{
  shell: string
  args: ReadonlyArray<string>
  cwd: string
  env: Readonly<Record<string, string>>
  cols: number
  rows: number
}>

export type TerminalBackend = Readonly<{
  spawn: (input: TerminalBackendSpawnInput) => TerminalBackendProcess
}>

/**
 * Real child-process backend: a login shell in its own process group. `kill`
 * escalates SIGTERM -> SIGKILL against the WHOLE group so no orphan children
 * survive; a group kill after exit (ESRCH) is a harmless no-op.
 */
export const childProcessTerminalBackend = (
  killGraceMs = 2_000,
): TerminalBackend => ({
  spawn: (input) => {
    const child = spawnChildProcess(input.shell, [...input.args], {
      cwd: input.cwd,
      env: { ...input.env, COLUMNS: String(input.cols), LINES: String(input.rows) },
      detached: true, // new process group => kill(-pgid) reaps the whole tree
      stdio: ["pipe", "pipe", "pipe"],
    })
    const pgid = typeof child.pid === "number" ? child.pid : null
    let killTimer: ReturnType<typeof setTimeout> | null = null
    const signalGroup = (signal: NodeJS.Signals): void => {
      if (pgid === null) return
      try {
        process.kill(-pgid, signal)
      } catch {
        // ESRCH: the group already exited — nothing to reap.
      }
    }
    const dataListeners = new Set<(chunk: string) => void>()
    const emitData = (chunk: string): void => {
      for (const listener of dataListeners) listener(chunk)
    }
    child.stdout?.on("data", (buffer: Buffer) => emitData(buffer.toString("utf8")))
    child.stderr?.on("data", (buffer: Buffer) => emitData(buffer.toString("utf8")))
    return {
      pid: pgid,
      write: (data) => {
        try {
          // xterm emits Enter as CR because a real PTY's line discipline
          // translates it. This fallback is pipe-backed, so perform that one
          // compatibility translation here; otherwise ordinary Enter leaves
          // the shell waiting forever. A future node-pty backend receives the
          // original data through its own implementation of `write`.
          child.stdin?.write(data.replace(/\r\n?/g, "\n"))
        } catch {
          // A closed stdin (already-exited shell) drops the frame; the exit
          // path is what the renderer observes.
        }
      },
      resize: (cols, rows) => {
        // No PTY => no TIOCSWINSZ. Best-effort SIGWINCH after publishing the
        // new geometry via env is meaningless post-spawn; a real winsize needs
        // the node-pty backend. Signal the group so a SIGWINCH-aware child can
        // re-query if it chooses.
        void cols
        void rows
        signalGroup("SIGWINCH")
      },
      interrupt: () => signalGroup("SIGINT"),
      kill: () => {
        signalGroup("SIGTERM")
        if (killTimer === null) {
          killTimer = setTimeout(() => signalGroup("SIGKILL"), killGraceMs)
          killTimer.unref?.()
        }
      },
      onData: (listener) => { dataListeners.add(listener) },
      onExit: (listener) => {
        child.on("exit", (code, signal) => {
          if (killTimer !== null) {
            clearTimeout(killTimer)
            killTimer = null
          }
          listener(code, signal)
        })
        child.on("error", () => listener(null, null))
      },
    }
  },
})

// ---------------------------------------------------------------------------
// Redaction — secret env VALUES and token-shaped strings never reach renderer.
// ---------------------------------------------------------------------------

const secretNamePattern =
  /(?:^|_)(?:token|secret|password|passwd|api[_-]?key|access[_-]?key|private[_-]?key|credential|auth|session|cookie|mnemonic|seed)(?:$|_)/i

const secretValuePattern =
  /-----BEGIN(?: [A-Z]+)? PRIVATE KEY-----|(?:github_pat|gh[pousr]_|sk-|AKIA|xox[baprs]-)[A-Za-z0-9_\-]{8,}/

/** Build the value->label redaction map from the bound environment. */
export const buildRedactionMap = (
  env: Readonly<Record<string, string>>,
): ReadonlyMap<string, string> => {
  const map = new Map<string, string>()
  for (const [name, value] of Object.entries(env)) {
    if (typeof value !== "string" || value.length < 4) continue
    if (secretNamePattern.test(name) || secretValuePattern.test(value)) {
      map.set(value, name)
    }
  }
  return map
}

const escapeRegExp = (value: string): string => value.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")

/**
 * Redact a chunk: every bound secret VALUE becomes `«redacted:NAME»`, then any
 * remaining token-shaped literal becomes `«redacted»`. Longest values first so
 * a secret that contains a shorter secret is fully masked.
 */
export const redactChunk = (
  chunk: string,
  redactions: ReadonlyMap<string, string>,
): string => {
  let out = chunk
  const values = [...redactions.keys()].sort((left, right) => right.length - left.length)
  for (const value of values) {
    if (out.includes(value)) {
      out = out.split(value).join(`«redacted:${redactions.get(value)}»`)
    }
  }
  out = out.replace(
    /(?:github_pat|gh[pousr]_|sk-|AKIA|xox[baprs]-)[A-Za-z0-9_\-]{8,}/g,
    "«redacted»",
  )
  out = out.replace(/-----BEGIN(?: [A-Z]+)? PRIVATE KEY-----[\s\S]*?-----END(?: [A-Z]+)? PRIVATE KEY-----/g, "«redacted»")
  return out
}

// ---------------------------------------------------------------------------
// Bounded ring buffer.
// ---------------------------------------------------------------------------

/** Byte-capped output ring. Tracks whether it ever dropped (loss-accounted). */
export const makeRing = (capBytes: number) => {
  let text = ""
  let dropped = false
  return {
    append: (chunk: string): void => {
      text += chunk
      if (text.length > capBytes) {
        text = text.slice(text.length - capBytes)
        dropped = true
      }
    },
    tail: (maxBytes: number): string => text.slice(Math.max(0, text.length - maxBytes)),
    dropped: (): boolean => dropped,
    load: (value: string): void => { text = value.slice(Math.max(0, value.length - capBytes)) },
  }
}

// ---------------------------------------------------------------------------
// Announced-port detection (parse the dev-server's OWN output; never scan).
// ---------------------------------------------------------------------------

const portPatterns: ReadonlyArray<RegExp> = [
  /https?:\/\/(?:localhost|127\.0\.0\.1|0\.0\.0\.0|\[::1\]):(\d{2,5})\b/gi,
  /(?:listening|running|ready|started|available).{0,40}?\bport[:\s]+(\d{2,5})\b/gi,
  /\bLocal:\s+https?:\/\/[^\s:]+:(\d{2,5})\b/gi,
]

export const detectAnnouncedPorts = (text: string): ReadonlyArray<number> => {
  const found = new Set<number>()
  for (const pattern of portPatterns) {
    pattern.lastIndex = 0
    let match: RegExpExecArray | null
    while ((match = pattern.exec(text)) !== null) {
      const port = Number(match[1])
      if (Number.isInteger(port) && port >= 1 && port <= 65_535) found.add(port)
    }
  }
  return [...found]
}

// ---------------------------------------------------------------------------
// Host.
// ---------------------------------------------------------------------------

export type TerminalWorkspaceBinding = Readonly<{ root: string; grantRef: string }>

export type TerminalHostOptions = Readonly<{
  backend?: TerminalBackend
  /** The authorized workspace at create time (root + grant). null => no seam. */
  workspace: () => TerminalWorkspaceBinding | null
  /** Bounded environment bound into every session. Defaults to a safe subset. */
  env?: () => Readonly<Record<string, string>>
  /** Fixed shell + argv. NEVER renderer-provided. */
  shell?: Readonly<{ command: string; args: ReadonlyArray<string> }>
  emit: (event: TerminalEvent) => void
  /** Restart-recovery persistence file (JSON). Omit to disable persistence. */
  persistencePath?: string
  now?: () => number
  maxSessions?: number
  ringCapBytes?: number
  tailBytes?: number
  /** Preview open: confirm + external-open. Returns true iff opened. */
  openPreview?: (url: string) => Promise<boolean> | boolean
}>

type Session = {
  sessionRef: string
  grantRef: string
  cwdLabel: string
  shellLabel: string
  cols: number
  rows: number
  status: "running" | "exited"
  exitCode: number | null
  process: TerminalBackendProcess
  ring: ReturnType<typeof makeRing>
  redactions: ReadonlyMap<string, string>
  previews: Map<number, { url: string; ready: boolean }>
  disposing: boolean
  killed: boolean
}

export type TerminalHost = Readonly<{
  create: (input: Readonly<{ sessionRef?: string; cols?: number; rows?: number }>) =>
    | { ok: true; sessionRef: string; cwdLabel: string; shellLabel: string; cols: number; rows: number }
    | { ok: false; reason: "no_workspace" | "duplicate" | "at_capacity" | "spawn_failed"; message: string }
  input: (sessionRef: string, data: string) => { ok: true } | { ok: false; reason: "not_found" | "grant_revoked" | "exited" }
  resize: (sessionRef: string, cols: number, rows: number) => { ok: true } | { ok: false; reason: "not_found" | "grant_revoked" | "exited" }
  interrupt: (sessionRef: string) => { ok: true } | { ok: false; reason: "not_found" | "grant_revoked" | "exited" }
  restart: (sessionRef: string) => { ok: true } | { ok: false; reason: "not_found" | "grant_revoked" }
  close: (sessionRef: string) => { ok: true }
  openPreview: (sessionRef: string, port: number) => Promise<
    | { ok: true; url: string }
    | { ok: false; reason: "not_found" | "unknown_port" | "declined" | "unavailable" }
  >
  /** Kill + forget every session bound to a now-revoked workspace grant. */
  revokeWorkspace: (grantRef: string) => void
  snapshot: () => TerminalSnapshot
  liveSessionCount: () => number
  dispose: () => void
}>

const admittedHostEnvironmentKeys = [
  "HOME",
  "USER",
  "LOGNAME",
  "PATH",
  "SHELL",
  "LANG",
  "LC_ALL",
  "LC_CTYPE",
  "TERM",
  "COLORTERM",
  "TMPDIR",
] as const

/**
 * Construct the bounded environment used by the low-level terminal transport.
 * The IDE-10 Effect graph owns the manifest and policy. This adapter receives
 * only the admitted values; it never copies the complete host environment.
 */
export const defaultSafeTerminalEnvironment = (
  source: Readonly<Record<string, string | undefined>> = process.env,
): Readonly<Record<string, string>> => {
  const env: Record<string, string> = {}
  for (const name of admittedHostEnvironmentKeys) {
    const value = source[name]
    if (typeof value === "string" && value.length > 0) env[name] = value
  }
  env.TERM = env.TERM ?? "xterm-256color"
  env.COLORTERM = env.COLORTERM ?? "truecolor"
  env.OPENAGENTS_DESKTOP_TERMINAL = "1"
  return env
}

export const makeTerminalHost = (options: TerminalHostOptions): TerminalHost => {
  const backend = options.backend ?? childProcessTerminalBackend()
  const envFactory = options.env ?? defaultSafeTerminalEnvironment
  const shell = options.shell ?? {
    command: process.env.SHELL ?? "/bin/bash",
    args: [] as ReadonlyArray<string>,
  }
  const now = options.now ?? Date.now
  const maxSessions = options.maxSessions ?? 8
  const ringCap = options.ringCapBytes ?? 262_144
  const tailBytes = options.tailBytes ?? 65_536
  const sessions = new Map<string, Session>()
  const portOwners = new Map<number, string>()
  const recovered: TerminalSessionView[] = []
  // Final projections of sessions that have closed this run, retained so their
  // bounded tail survives into the persisted file (a closed session is removed
  // from the live map before persist runs).
  const retained: TerminalSessionView[] = []
  let disposed = false

  const persist = (): void => {
    if (options.persistencePath === undefined) return
    try {
      const sessionViews = [
        ...recovered,
        ...retained,
        ...[...sessions.values()].map((session) => projectSession(session)),
      ]
      mkdirSync(path.dirname(options.persistencePath), { recursive: true })
      const temporary = `${options.persistencePath}.tmp`
      writeFileSync(
        temporary,
        JSON.stringify({ version: 1, savedAt: now(), sessions: sessionViews }),
        { encoding: "utf8", mode: 0o600 },
      )
      renameSync(temporary, options.persistencePath)
    } catch {
      // Persistence is best-effort; a failed write never breaks a live session.
    }
  }

  const projectSession = (session: Session): TerminalSessionView => ({
    sessionRef: session.sessionRef,
    cwdLabel: session.cwdLabel,
    shellLabel: session.shellLabel,
    status: session.status,
    exitCode: session.exitCode,
    recovered: false,
    gap: session.ring.dropped(),
    tail: session.ring.tail(tailBytes),
    previews: [...session.previews.entries()].map(([port, view]) => ({
      port,
      url: view.url,
      ready: view.ready,
    })),
  })

  // Restart recovery: reload persisted tails as read-only recovered sessions.
  if (options.persistencePath !== undefined) {
    try {
      const raw = JSON.parse(readFileSync(options.persistencePath, "utf8")) as {
        sessions?: ReadonlyArray<Partial<TerminalSessionView>>
      }
      for (const row of raw.sessions ?? []) {
        if (typeof row.sessionRef !== "string" || !terminalSessionRefPattern.test(row.sessionRef)) continue
        recovered.push({
          sessionRef: row.sessionRef,
          cwdLabel: typeof row.cwdLabel === "string" ? row.cwdLabel : "recovered",
          shellLabel: typeof row.shellLabel === "string" ? row.shellLabel : "",
          status: "recovered",
          exitCode: typeof row.exitCode === "number" ? row.exitCode : null,
          recovered: true,
          // A recovered tail is loss-accounted: it is the persisted window,
          // whatever streamed after the last flush before the restart is a gap.
          gap: true,
          tail: typeof row.tail === "string" ? row.tail.slice(-tailBytes) : "",
          previews: [],
        })
      }
    } catch {
      // No prior state (or unreadable) => no recovered sessions.
    }
  }

  const detectPreviews = (session: Session, chunk: string): void => {
    for (const port of detectAnnouncedPorts(chunk)) {
      if (session.previews.has(port)) continue
      const owner = portOwners.get(port)
      if (owner !== undefined && owner !== session.sessionRef && sessions.has(owner)) {
        // Port collision: another LIVE session already owns this announced
        // port. Do not steal it; surface a typed error and drop the claim.
        options.emit({
          kind: "error",
          sessionRef: session.sessionRef,
          message: `preview port ${port} is already owned by another terminal session`,
        })
        continue
      }
      portOwners.set(port, session.sessionRef)
      const url = `http://localhost:${port}/`
      session.previews.set(port, { url, ready: true })
      options.emit({ kind: "preview", sessionRef: session.sessionRef, port, url, ready: true })
    }
  }

  const finishExit = (session: Session, exitCode: number | null, signal: string | null): void => {
    if (session.status === "exited") return
    session.status = "exited"
    session.exitCode = exitCode
    for (const [port, owner] of [...portOwners.entries()]) {
      if (owner === session.sessionRef) portOwners.delete(port)
    }
    options.emit({ kind: "exit", sessionRef: session.sessionRef, exitCode, signal })
    persist()
  }

  const startProcess = (session: Session): void => {
    const process = session.process
    process.onData((raw) => {
      const chunk = redactChunk(raw, session.redactions)
      session.ring.append(chunk)
      options.emit({ kind: "output", sessionRef: session.sessionRef, chunk })
      detectPreviews(session, chunk)
    })
    process.onExit((exitCode, signal) => finishExit(session, exitCode, signal))
  }

  const spawnSession = (
    sessionRef: string,
    binding: TerminalWorkspaceBinding,
    cols: number,
    rows: number,
  ): Session => {
    const env = envFactory()
    const backendProcess = backend.spawn({
      shell: shell.command,
      args: shell.args,
      cwd: binding.root,
      env,
      cols,
      rows,
    })
    const session: Session = {
      sessionRef,
      grantRef: binding.grantRef,
      cwdLabel: path.basename(binding.root) || binding.root,
      shellLabel: path.basename(shell.command),
      cols,
      rows,
      status: "running",
      exitCode: null,
      process: backendProcess,
      ring: makeRing(ringCap),
      redactions: buildRedactionMap(env),
      previews: new Map(),
      disposing: false,
      killed: false,
    }
    return session
  }

  const disposeSession = (
    session: Session,
    reason: "user" | "workspace_revoked" | "app_quit",
  ): void => {
    // Exactly-once process-tree disposal: a second close is a no-op.
    if (session.disposing) return
    session.disposing = true
    if (!session.killed) {
      session.killed = true
      session.process.kill()
    }
    retained.push(projectSession(session))
    if (retained.length > maxSessions * 2) retained.shift()
    sessions.delete(session.sessionRef)
    for (const [port, owner] of [...portOwners.entries()]) {
      if (owner === session.sessionRef) portOwners.delete(port)
    }
    options.emit({ kind: "closed", sessionRef: session.sessionRef, reason })
    persist()
  }

  const liveGrant = (session: Session): boolean => {
    const current = options.workspace()
    return current !== null && current.grantRef === session.grantRef
  }

  return {
    create: (input) => {
      if (disposed) return { ok: false, reason: "no_workspace", message: "The terminal host is disposed." }
      const binding = options.workspace()
      if (binding === null) {
        return { ok: false, reason: "no_workspace", message: "Choose a workspace to open a terminal." }
      }
      if (sessions.size >= maxSessions) {
        return { ok: false, reason: "at_capacity", message: `At most ${maxSessions} terminals at once.` }
      }
      const sessionRef = input.sessionRef ?? `terminal.${randomUUID()}`
      if (!terminalSessionRefPattern.test(sessionRef) || sessions.has(sessionRef)) {
        return { ok: false, reason: "duplicate", message: "That terminal is already open." }
      }
      const cols = input.cols ?? 80
      const rows = input.rows ?? 24
      let session: Session
      try {
        session = spawnSession(sessionRef, binding, cols, rows)
      } catch {
        return { ok: false, reason: "spawn_failed", message: "The terminal could not be started." }
      }
      sessions.set(sessionRef, session)
      startProcess(session)
      options.emit({
        kind: "ready",
        sessionRef,
        cwdLabel: session.cwdLabel,
        shellLabel: session.shellLabel,
        cols,
        rows,
      })
      persist()
      return { ok: true, sessionRef, cwdLabel: session.cwdLabel, shellLabel: session.shellLabel, cols, rows }
    },
    input: (sessionRef, data) => {
      const session = sessions.get(sessionRef)
      if (session === undefined) return { ok: false, reason: "not_found" }
      if (!liveGrant(session)) return { ok: false, reason: "grant_revoked" }
      if (session.status === "exited") return { ok: false, reason: "exited" }
      // The data is written to the backend's STDIN. It is never interpolated
      // into any argv — the spawn command/args are fixed and workspace-bound.
      session.process.write(data)
      return { ok: true }
    },
    resize: (sessionRef, cols, rows) => {
      const session = sessions.get(sessionRef)
      if (session === undefined) return { ok: false, reason: "not_found" }
      if (!liveGrant(session)) return { ok: false, reason: "grant_revoked" }
      if (session.status === "exited") return { ok: false, reason: "exited" }
      session.cols = cols
      session.rows = rows
      session.process.resize(cols, rows)
      return { ok: true }
    },
    interrupt: (sessionRef) => {
      const session = sessions.get(sessionRef)
      if (session === undefined) return { ok: false, reason: "not_found" }
      if (!liveGrant(session)) return { ok: false, reason: "grant_revoked" }
      if (session.status === "exited") return { ok: false, reason: "exited" }
      session.process.interrupt()
      return { ok: true }
    },
    restart: (sessionRef) => {
      const session = sessions.get(sessionRef)
      if (session === undefined) return { ok: false, reason: "not_found" }
      const binding = options.workspace()
      if (binding === null || binding.grantRef !== session.grantRef) {
        return { ok: false, reason: "grant_revoked" }
      }
      // Kill the old tree exactly once, then respawn a fresh process under the
      // SAME sessionRef and (still-authorized) workspace binding.
      if (!session.killed) {
        session.killed = true
        session.process.kill()
      }
      for (const [port, owner] of [...portOwners.entries()]) {
        if (owner === sessionRef) portOwners.delete(port)
      }
      const next = spawnSession(sessionRef, binding, session.cols, session.rows)
      sessions.set(sessionRef, next)
      startProcess(next)
      options.emit({
        kind: "ready",
        sessionRef,
        cwdLabel: next.cwdLabel,
        shellLabel: next.shellLabel,
        cols: next.cols,
        rows: next.rows,
      })
      persist()
      return { ok: true }
    },
    close: (sessionRef) => {
      const session = sessions.get(sessionRef)
      if (session !== undefined) disposeSession(session, "user")
      // Idempotent: closing an unknown/already-closed session is a no-op ok.
      return { ok: true }
    },
    openPreview: async (sessionRef, port) => {
      const session = sessions.get(sessionRef)
      if (session === undefined) return { ok: false, reason: "not_found" }
      const preview = session.previews.get(port)
      if (preview === undefined) return { ok: false, reason: "unknown_port" }
      if (options.openPreview === undefined) return { ok: false, reason: "unavailable" }
      const opened = await options.openPreview(preview.url)
      return opened ? { ok: true, url: preview.url } : { ok: false, reason: "declined" }
    },
    revokeWorkspace: (grantRef) => {
      for (const session of [...sessions.values()]) {
        if (session.grantRef === grantRef) disposeSession(session, "workspace_revoked")
      }
    },
    snapshot: () => ({
      sessions: [...recovered, ...[...sessions.values()].map((session) => projectSession(session))],
    }),
    liveSessionCount: () =>
      [...sessions.values()].filter((session) => session.status === "running").length,
    dispose: () => {
      if (disposed) return
      disposed = true
      for (const session of [...sessions.values()]) disposeSession(session, "app_quit")
    },
  }
}
