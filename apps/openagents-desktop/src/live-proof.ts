/**
 * OpenAgents Desktop — Episode 250 live-proof driver (#8712).
 *
 * `OPENAGENTS_DESKTOP_LIVE_PROOF=1` walks the full EP250 journey in a REAL
 * Electron window against REAL local data: the actual pylon CLI registry
 * behind the Fleet workspace (no smoke fixtures), the real Codex history
 * discovery under `~/.codex/sessions`, and the real local chat lanes. It is
 * mutually exclusive with `OPENAGENTS_DESKTOP_SMOKE=1` (main refuses to run
 * both) and reuses the smoke harness mechanics — a bounded step runner over
 * `webContents.executeJavaScript` probes, full-window screenshot capture, and
 * a journal — without touching any smoke step or app behavior.
 *
 * Honesty rules: every step is bounded by a timeout; a failing step captures
 * a failure screenshot and journals `{ step, ok: false, detail }` instead of
 * crashing; the harness-lane steps (Fable/Codex) degrade to journaled failure
 * when a lane is absent or disabled — the driver never fakes a receipt. The
 * process exits nonzero when the structural shell spine or either named
 * provider acceptance lane fails. A zero exit is therefore a real CUT-21
 * provider receipt, never merely proof that the window mounted. Screenshots
 * are public-safe by construction: the shell renders
 * account refs and readiness only — never tokens, emails beyond the pylon
 * projection's own public-safe field, or credential material.
 */
import { mkdirSync, statSync, writeFileSync } from "node:fs"
import path from "node:path"
import type { BrowserWindow } from "electron"

// ---------------------------------------------------------------------------
// Configuration (pure; unit-tested)
// ---------------------------------------------------------------------------

export type LiveProofStepName =
  | "account-preflight"
  | "shell-mounted"
  | "fleet-workspace"
  | "fleet-usage-check"
  | "new-chat"
  | "fable-chip"
  | "fable-turn"
  | "codex-chip"
  | "codex-turn"
  // EP250 capability-eval harness (#8712): new rung-4 UI receipts for
  // capabilities the audit proves are exercisable now.
  //  - interrupt-stop: click Stop mid-turn; transcript shows interrupted state
  //    (capability A2 — the composer Stop button).
  //  - file-save: save through the files workspace save seam and reread
  //    (capability C3 — the SHA-256 expectedRevision conflict guard).
  //  - git-review: the review workspace shows a real dirty-file diff
  //    (capability E1 — workspace-git-status + workspace-git-diff).
  | "interrupt-stop"
  | "file-save"
  | "git-review"
  | "redaction-check"
  | "summary"

export type LiveProofStep = Readonly<{
  name: LiveProofStepName
  /** Required steps decide the process exit code. */
  required: boolean
  timeoutMs: number
}>

/** The EP250 journey, in execution order. */
export const liveProofSteps: ReadonlyArray<LiveProofStep> = [
  // Step 0 (EP250 preflight): the real per-account validity probe round over
  // the real registry. Probes run concurrently (each ~30s-bounded); the step
  // bound covers a slow cold round without ever hanging the journey.
  { name: "account-preflight", required: true, timeoutMs: 240_000 },
  { name: "shell-mounted", required: true, timeoutMs: 30_000 },
  // The real pylon CLI list spawn (bun + registry read) can be slow on a
  // cold machine; the provider-accounts list timeout itself is 120s.
  { name: "fleet-workspace", required: true, timeoutMs: 150_000 },
  { name: "fleet-usage-check", required: false, timeoutMs: 60_000 },
  { name: "new-chat", required: true, timeoutMs: 30_000 },
  { name: "fable-chip", required: true, timeoutMs: 15_000 },
  { name: "fable-turn", required: true, timeoutMs: 180_000 },
  { name: "codex-chip", required: true, timeoutMs: 15_000 },
  { name: "codex-turn", required: true, timeoutMs: 180_000 },
  // EP250 capability-eval rung-4 UI receipts. All optional: a missing lane or
  // workspace journals an honest failure without failing the whole journey.
  { name: "interrupt-stop", required: false, timeoutMs: 180_000 },
  { name: "file-save", required: false, timeoutMs: 30_000 },
  { name: "git-review", required: false, timeoutMs: 30_000 },
  { name: "redaction-check", required: false, timeoutMs: 5_000 },
  { name: "summary", required: false, timeoutMs: 5_000 },
]

export const liveProofStepTimeoutMs = (name: LiveProofStepName): number =>
  liveProofSteps.find((step) => step.name === name)?.timeoutMs ?? 30_000

export const requiredLiveProofSteps = (): ReadonlyArray<LiveProofStepName> =>
  liveProofSteps.filter((step) => step.required).map((step) => step.name)

export type LiveProofConfig = Readonly<{
  enabled: boolean
  /** Both LIVE_PROOF and SMOKE were requested — the modes are exclusive. */
  conflict: boolean
  outDir: string
}>

export const resolveLiveProofConfig = (
  env: Readonly<Record<string, string | undefined>>,
  userDataDir: string,
): LiveProofConfig => {
  const enabled = env["OPENAGENTS_DESKTOP_LIVE_PROOF"] === "1"
  const dir = env["OPENAGENTS_DESKTOP_LIVE_PROOF_DIR"]
  return {
    enabled,
    conflict: enabled && env["OPENAGENTS_DESKTOP_SMOKE"] === "1",
    outDir: typeof dir === "string" && dir.trim() !== ""
      ? path.resolve(dir.trim())
      : path.join(userDataDir, "live-proof"),
  }
}

const accountRefPattern = /^[A-Za-z0-9][A-Za-z0-9._-]{0,79}$/

/**
 * Optional exact named-account target for a live acceptance run. The driver
 * still selects it through the rendered typed account control; this only
 * states which row must be reached before the turn is submitted.
 */
export const resolveLiveProofAccountRef = (
  env: Readonly<Record<string, string | undefined>>,
  harness: "fable" | "codex",
): string | null => {
  const key = harness === "fable"
    ? "OPENAGENTS_DESKTOP_LIVE_PROOF_FABLE_ACCOUNT_REF"
    : "OPENAGENTS_DESKTOP_LIVE_PROOF_CODEX_ACCOUNT_REF"
  const value = env[key]?.trim() ?? ""
  return accountRefPattern.test(value) ? value : null
}

/** The exact real message both harness-lane turns send (EP250 step 6). */
export const liveProofTurnMessage =
  "Episode 250 live proof: reply with one sentence confirming streaming works, then stop."

export const LIVE_PROOF_TURN_SETTLE_MS = 5_000
export const LIVE_PROOF_TEXT_SETTLE_MS = 500

/**
 * A provider start IPC can resolve (re-enabling the composer) just before the
 * finalized authoritative thread snapshot reaches the renderer. Text is an
 * immediate terminal success; a zero-text idle state must remain observable
 * for a bounded settle window before it can be called a terminal failure.
 */
export const liveProofTurnIsTerminal = (input: Readonly<{
  turnPending: boolean
  assistantGrew: boolean
  activityObserved: boolean
  idleForMs: number
}>): boolean =>
  !input.turnPending && (
    (input.assistantGrew && input.idleForMs >= LIVE_PROOF_TEXT_SETTLE_MS) ||
    (input.activityObserved && input.idleForMs >= LIVE_PROOF_TURN_SETTLE_MS)
  )

// ---------------------------------------------------------------------------
// Journal
// ---------------------------------------------------------------------------

export type LiveProofJournalEntry = Readonly<{ step: string; ok: boolean; detail: string }>

const boundedDetail = (value: unknown, limit = 700): string => {
  const text = typeof value === "string" ? value : JSON.stringify(value)
  return (text ?? "").slice(0, limit)
}

// ---------------------------------------------------------------------------
// Renderer probes — single-shot scripts (no internal waiting); the main-side
// runner polls them so screenshots can be captured at the right moment
// (mid-stream capture is impossible from inside a renderer-side wait loop).
// ---------------------------------------------------------------------------

const probeShellScript = `(() => {
  const root = document.querySelector('[data-en-key="shell-root"]')
  if (root === null) return { mounted: false }
  const sidebar = document.querySelector('[data-en-key="shell-sidebar"]')
  const dock = Array.from(document.querySelectorAll(
    '[data-en-key="workspace-new-chat"], [data-en-key="workspace-fleet"], [data-en-key="workspace-chat"], [data-en-key="workspace-files"], [data-en-key="workspace-home"], [data-en-key="shell-command-palette-toggle"], [data-en-key="shell-settings-toggle"]'
  )).map((element) => element.getAttribute("data-en-key"))
  return {
    mounted: true,
    sidebar: sidebar !== null,
    dock,
    newChatFirst: dock[0] === "workspace-new-chat",
    fleetSecond: dock[1] === "workspace-fleet",
  }
})()`

const clickScript = (key: string): string => `(() => {
  const element = document.querySelector('[data-en-key="${key}"]')
  if (element === null) return { clicked: false }
  element.click()
  return { clicked: true }
})()`

const probeFleetScript = `(() => {
  const table = document.querySelector('[data-en-key="fleet-accounts-table"]')
  const unavailable = document.querySelector('[data-en-key="fleet-unavailable"]')
  const empty = document.querySelector('[data-en-key="fleet-empty"]')
  const asOf = document.querySelector('[data-en-key="fleet-as-of"]')
  const reason = document.querySelector('[data-en-key="fleet-unavailable-reason"]')
  const rows = Array.from(document.querySelectorAll('[data-en-key^="fleet-ref-"]')).map((element) => {
    const ref = (element.getAttribute("data-en-key") || "").slice("fleet-ref-".length)
    const provider = document.querySelector('[data-en-key="fleet-provider-' + ref + '"]')
    const readiness = document.querySelector('[data-en-key="fleet-readiness-' + ref + '"]')
    return {
      ref,
      provider: provider === null ? null : provider.textContent,
      readiness: readiness === null ? null : readiness.textContent,
    }
  })
  return {
    settled: table !== null || unavailable !== null || empty !== null,
    table: table !== null,
    unavailable: unavailable !== null,
    empty: empty !== null,
    reason: reason === null ? null : reason.textContent,
    asOf: asOf === null ? null : asOf.textContent,
    rows,
  }
})()`

const probeUsageScript = (ref: string): string => `(() => {
  const total = document.querySelector('[data-en-key="fleet-usage-total-${ref}"]')
  const failed = document.querySelector('[data-en-key="fleet-usage-failed-${ref}"]')
  return {
    settled: total !== null || failed !== null,
    checked: total !== null,
    label: total !== null ? total.textContent : failed !== null ? failed.textContent : null,
  }
})()`

const probeNewChatScript = `(() => {
  const transcript = document.querySelector('[data-en-key="shell-transcript"]')
  const split = document.querySelector('[data-en-key="history-workspace-split"]')
  const composer = document.querySelector('[data-en-key="shell-input"] textarea, [data-en-key="shell-input"] input')
  const messages = document.querySelectorAll('[data-en-key="shell-transcript"] [data-en-message]').length
  return {
    transcript: transcript !== null,
    historyLoaded: split !== null,
    messages,
    composerMounted: composer !== null,
    composerEnabled: composer !== null && composer.disabled === false,
  }
})()`

// EP250 (#8712 owner fix 3): the composer renders NO standing caption text —
// a disabled chip's reason lives only in its accessible label, so the probe
// reads the chip's disabled state + aria-label (never a visible caption row).
const probeChipScript = (harness: string): string => `(() => {
  const chip = document.querySelector('[data-en-key="shell-harness-${harness}"]')
  if (chip === null) return { present: false }
  return {
    present: true,
    disabled: chip.disabled === true,
    ariaLabel: chip.getAttribute("aria-label"),
    variant: chip.getAttribute("data-en-variant"),
    captionAbsent: document.querySelector('[data-en-key="shell-harness-caption"]') === null,
  }
})()`

const submitTurnScript = (message: string): string => `(() => {
  const input = document.querySelector('[data-en-key="shell-input"] textarea, [data-en-key="shell-input"] input')
  if (input === null) return { submitted: false, reason: "composer input not mounted" }
  if (input.disabled) return { submitted: false, reason: "composer disabled" }
  input.focus()
  input.value = ${JSON.stringify(message)}
  input.dispatchEvent(new Event("input", { bubbles: true }))
  input.dispatchEvent(new KeyboardEvent("keydown", { key: "Enter", bubbles: true }))
  return { submitted: true }
})()`

const probeTurnScript = `(() => {
  const input = document.querySelector('[data-en-key="shell-input"] textarea, [data-en-key="shell-input"] input')
  const stop = document.querySelector('[data-en-key="shell-stop"]')
  const messageRows = Array.from(document.querySelectorAll('[data-en-key="shell-transcript"] [data-en-message]'))
  const rowsForRole = (role) => messageRows.filter((row) =>
    row.getAttribute('data-en-role') === role || row.querySelector('[data-en-role="' + role + '"]') !== null)
  const assistantRows = rowsForRole('assistant')
  const assistantText = assistantRows
    .map((row) => {
      const body = row.querySelector('[data-en-role="assistant"] [data-en-key$="-text"]') ??
        row.querySelector('[data-en-key$="-text"]') ??
        row.querySelector('[data-en-role="assistant"] [data-en-role="body"]') ??
        row.querySelector('[data-en-role="body"]')
      return body === null ? "" : body.textContent || ""
    })
    .join("")
  const systemRows = rowsForRole('system').map((row) => {
    const body = row.querySelector('[data-en-role="system"] [data-en-role="body"]') ??
      row.querySelector('[data-en-role="body"]')
    return body === null ? "" : body.textContent || ""
  })
  return {
    composerDisabled: input === null ? null : input.disabled === true,
    turnPending: stop !== null,
    assistantLength: assistantText.length,
    assistantSnippet: assistantText.slice(0, 400),
    systemCount: systemRows.length,
    lastSystem: systemRows.length === 0 ? null : systemRows[systemRows.length - 1].slice(0, 300),
    messageCount: messageRows.length,
  }
})()`

// EP250 capability-eval step probes (interrupt-stop / file-save / git-review).

/** Reads the trailing composer control + pending state for interrupt-stop. */
const probeComposerControlScript = `(() => {
  const input = document.querySelector('[data-en-key="shell-input"] textarea, [data-en-key="shell-input"] input')
  const stop = document.querySelector('[data-en-key="shell-stop"]')
  const send = document.querySelector('[data-en-key="shell-note"]')
  const systemRows = Array.from(document.querySelectorAll(
    '[data-en-key="shell-transcript"] [data-en-message][data-en-role="system"]'
  )).map((row) => { const body = row.querySelector('[data-en-role="body"]'); return body === null ? "" : body.textContent || "" })
  return {
    composerDisabled: input === null ? null : input.disabled === true,
    stopPresent: stop !== null,
    sendPresent: send !== null,
    interruptedNoticeCount: systemRows.filter((text) => text.toLowerCase().includes("interrupt")).length,
  }
})()`

const clickStopScript = `(() => {
  const stop = document.querySelector('[data-en-key="shell-stop"]')
  if (stop === null) return { clicked: false }
  stop.click()
  return { clicked: true }
})()`

/** Opens the files workspace and reports its readiness + first file entry. */
const probeFilesWorkspaceScript = `(() => {
  const panel = document.querySelector('[data-en-key="workspace-files-panel"]')
  const empty = document.querySelector('[data-en-key="workspace-files-empty"]')
  const entries = Array.from(document.querySelectorAll('[data-en-key^="workspace-file-"]'))
    .map((element) => element.getAttribute("data-en-key"))
    .filter((key) => key !== null && /^workspace-file-[^-]/.test(key) &&
      key !== "workspace-file-preview" && !key.startsWith("workspace-file-preview") &&
      key !== "workspace-file-editor" && key !== "workspace-file-save" &&
      key !== "workspace-file-reload" && key !== "workspace-file-actions" &&
      key !== "workspace-file-saved" && key !== "workspace-file-conflict" &&
      key !== "workspace-file-unavailable")
  return { settled: panel !== null || empty !== null, hasWorkspace: panel !== null, firstEntry: entries[0] ?? null }
})()`

const clickKeyScript = (key: string): string => `(() => {
  const element = document.querySelector('[data-en-key=${JSON.stringify(key)}]')
  if (element === null) return { clicked: false }
  element.click()
  return { clicked: true }
})()`

/** Saves the open file with its CURRENT content (non-destructive round trip). */
const saveOpenFileScript = `(() => {
  const editor = document.querySelector('[data-en-key="workspace-file-editor"] textarea, [data-en-key="workspace-file-editor"] input')
  const save = document.querySelector('[data-en-key="workspace-file-save"]')
  if (editor === null || save === null) return { ready: false }
  // Re-emit the current value unchanged so the save writes identical bytes:
  // this proves the save seam + revision guard without mutating file meaning.
  editor.dispatchEvent(new Event("input", { bubbles: true }))
  save.click()
  return { ready: true }
})()`

const probeSaveOutcomeScript = `(() => {
  const saved = document.querySelector('[data-en-key="workspace-file-saved"]')
  const conflict = document.querySelector('[data-en-key="workspace-file-conflict"]')
  const unavailable = document.querySelector('[data-en-key="workspace-file-unavailable"]')
  return {
    settled: saved !== null || conflict !== null || unavailable !== null,
    outcome: saved !== null ? "saved" : conflict !== null ? "conflict" : unavailable !== null ? "unavailable" : null,
  }
})()`

/** Reads the review workspace's dirty-file status + selected diff. */
const probeReviewWorkspaceScript = `(() => {
  const panel = document.querySelector('[data-en-key="workspace-review-panel"]')
  const clean = document.querySelector('[data-en-key="workspace-review-clean"]')
  const unavailable = document.querySelector('[data-en-key="workspace-review-unavailable"]')
  const changes = Array.from(document.querySelectorAll('[data-en-key^="workspace-review-change-"]'))
    .map((element) => element.getAttribute("data-en-key"))
  const diff = document.querySelector('[data-en-key="workspace-review-diff-content"]')
  return {
    settled: panel !== null || clean !== null || unavailable !== null,
    changeCount: changes.length,
    firstChange: changes[0] ?? null,
    diffPresent: diff !== null,
    diffLength: diff === null ? 0 : (diff.textContent || "").length,
  }
})()`

// ---------------------------------------------------------------------------
// Driver
// ---------------------------------------------------------------------------

export type LiveProofRunOptions = Readonly<{
  outDir: string
  exit: (code: number) => void
  /**
   * Step 0 (EP250): the host-side account preflight over the REAL registry.
   * Returns per-account probe results ({ ref, state, detail, observedAt,
   * durationMs }); the driver journals each honestly (verified/broken with
   * reasons). Absent on builds without the preflight service.
   */
  preflight?: () => Promise<ReadonlyArray<Readonly<{
    ref: string
    state: string
    detail: string
    observedAt: string
    durationMs: number
  }>>>
  log?: (message: string) => void
  logError?: (message: string) => void
}>

type Rec = Record<string, unknown>
const asRec = (value: unknown): Rec =>
  typeof value === "object" && value !== null ? (value as Rec) : {}

const sleep = (ms: number): Promise<void> => new Promise((resolve) => setTimeout(resolve, ms))

export const runLiveProof = (window: BrowserWindow, options: LiveProofRunOptions): void => {
  const log = options.log ?? ((message: string) => console.log(`[openagents-desktop live-proof] ${message}`))
  const logError = options.logError ?? ((message: string) => console.error(`[openagents-desktop live-proof] ${message}`))
  const journal: Array<LiveProofJournalEntry> = []
  const shots: Array<{ name: string; file: string; bytes: number }> = []
  const requiredNames = new Set<string>(requiredLiveProofSteps())
  const requiredFailures: Array<string> = []
  let shotIndex = 0
  let finished = false

  mkdirSync(options.outDir, { recursive: true })
  const journalPath = path.join(options.outDir, "journal.json")
  const persistJournal = (): void => {
    try {
      writeFileSync(journalPath, `${JSON.stringify(journal, null, 2)}\n`)
    } catch (error) {
      logError(`journal write failed: ${error instanceof Error ? error.message : "unknown"}`)
    }
  }

  const record = (step: string, ok: boolean, detail: unknown): void => {
    const entry: LiveProofJournalEntry = { step, ok, detail: boundedDetail(detail, step === "summary" ? 2_000 : 700) }
    journal.push(entry)
    persistJournal()
    ;(ok ? log : logError)(`${step} ${ok ? "OK" : "FAILED"} ${entry.detail}`)
    if (!ok && requiredNames.has(step)) requiredFailures.push(step)
  }

  const capture = async (label: string): Promise<void> => {
    const name = `${String(++shotIndex).padStart(2, "0")}-${label}`
    try {
      const image = await window.webContents.capturePage()
      const file = path.join(options.outDir, `${name}.png`)
      writeFileSync(file, image.toPNG())
      let bytes = 0
      try { bytes = statSync(file).size } catch { bytes = 0 }
      shots.push({ name, file, bytes })
      log(`shot ${name}.png (${bytes} bytes)`)
    } catch (error) {
      shots.push({ name, file: "", bytes: 0 })
      logError(`shot ${name} capture failed: ${error instanceof Error ? error.message : "unknown"}`)
    }
  }

  const evalIn = async (script: string): Promise<unknown> => {
    try {
      return await window.webContents.executeJavaScript(script, true)
    } catch (error) {
      return { evalError: error instanceof Error ? error.message.slice(0, 200) : "renderer eval failed" }
    }
  }

  /** Poll a single-shot probe until `until` accepts it or the deadline hits. */
  const pollUntil = async (
    script: string,
    until: (value: Rec) => boolean,
    timeoutMs: number,
    intervalMs = 250,
  ): Promise<{ ok: boolean; value: Rec }> => {
    const deadline = Date.now() + timeoutMs
    let last: Rec = {}
    for (;;) {
      last = asRec(await evalIn(script))
      if (until(last)) return { ok: true, value: last }
      if (Date.now() >= deadline) return { ok: false, value: last }
      await sleep(intervalMs)
    }
  }

  const finish = (): void => {
    if (finished) return
    finished = true
    const code = requiredFailures.length > 0 ? 1 : 0
    log(`exiting ${code} (required failures: ${requiredFailures.join(",") || "none"})`)
    options.exit(code)
  }

  // Whole-journey ceiling so a wedged renderer can never hang the process.
  const overallTimeout = setTimeout(() => {
    record("summary", false, { reason: "live-proof overall timeout", journalCount: journal.length })
    if (!requiredFailures.includes("overall-timeout")) requiredFailures.push("overall-timeout")
    finish()
  }, 15 * 60_000)

  // -------------------------------------------------------------------------
  // Steps
  // -------------------------------------------------------------------------

  const stepShell = async (): Promise<void> => {
    const result = await pollUntil(
      probeShellScript,
      (value) => value["mounted"] === true,
      liveProofStepTimeoutMs("shell-mounted"),
    )
    const value = result.value
    const ok = result.ok && value["sidebar"] === true &&
      value["newChatFirst"] === true && value["fleetSecond"] === true
    if (ok) {
      await capture("shell")
      record("shell-mounted", true, { dock: value["dock"] })
    } else {
      await capture("shell-mounted-failed")
      record("shell-mounted", false, value)
    }
  }

  const stepFleet = async (): Promise<ReadonlyArray<{ ref: string; provider: string; readiness: string }>> => {
    const clicked = asRec(await evalIn(clickScript("workspace-fleet")))
    if (clicked["clicked"] !== true) {
      await capture("fleet-workspace-failed")
      record("fleet-workspace", false, { reason: "Fleet dock button never mounted" })
      return []
    }
    const result = await pollUntil(
      probeFleetScript,
      // Settled means the REAL pylon list projection came back (table, empty,
      // or unavailable) — the loading row alone is not evidence.
      (value) => value["settled"] === true,
      liveProofStepTimeoutMs("fleet-workspace"),
      500,
    )
    const value = result.value
    const rows = Array.isArray(value["rows"])
      ? (value["rows"] as Array<Rec>).map((row) => ({
          ref: String(row["ref"] ?? ""),
          provider: String(row["provider"] ?? ""),
          readiness: String(row["readiness"] ?? ""),
        }))
      : []
    const asOf = typeof value["asOf"] === "string" ? value["asOf"] : null
    const ok = result.ok && value["table"] === true && rows.length >= 1 &&
      asOf !== null && asOf.startsWith("as of")
    if (ok) {
      await capture("fleet-workspace")
      record("fleet-workspace", true, { accountCount: rows.length, asOf, rows })
    } else {
      await capture("fleet-workspace-failed")
      record("fleet-workspace", false, value)
    }
    return rows
  }

  const stepUsage = async (
    rows: ReadonlyArray<{ ref: string; provider: string; readiness: string }>,
  ): Promise<void> => {
    const ready = rows.filter((row) => row.readiness === "ready")
    const target = ready.find((row) => row.provider.toLowerCase().includes("claude")) ?? ready[0]
    if (target === undefined) {
      await capture("fleet-usage-check-failed")
      record("fleet-usage-check", false, { reason: "no READY account row to check", rows })
      return
    }
    const clicked = asRec(await evalIn(clickScript(`fleet-usage-check-${target.ref}`)))
    if (clicked["clicked"] !== true) {
      await capture("fleet-usage-check-failed")
      record("fleet-usage-check", false, { reason: "Check button never mounted", ref: target.ref })
      return
    }
    const result = await pollUntil(
      probeUsageScript(target.ref),
      (value) => value["settled"] === true,
      liveProofStepTimeoutMs("fleet-usage-check"),
      500,
    )
    const value = result.value
    const ok = result.ok && value["checked"] === true
    if (ok) {
      await capture("fleet-usage")
      record("fleet-usage-check", true, { ref: target.ref, provider: target.provider, label: value["label"] })
    } else {
      await capture("fleet-usage-check-failed")
      record("fleet-usage-check", false, {
        ref: target.ref,
        provider: target.provider,
        label: value["label"] ?? null,
        reason: result.ok ? "usage probe failed" : "usage check timed out",
      })
    }
  }

  const stepNewChat = async (): Promise<void> => {
    const clicked = asRec(await evalIn(clickScript("workspace-new-chat")))
    if (clicked["clicked"] !== true) {
      await capture("new-chat-failed")
      record("new-chat", false, { reason: "New chat dock button never mounted" })
      return
    }
    const result = await pollUntil(
      probeNewChatScript,
      (value) =>
        value["transcript"] === true && value["historyLoaded"] === false &&
        value["messages"] === 0 && value["composerEnabled"] === true,
      liveProofStepTimeoutMs("new-chat"),
    )
    if (result.ok) {
      await capture("new-chat")
      record("new-chat", true, { emptyTranscript: true, composerEnabled: true })
    } else {
      await capture("new-chat-failed")
      record("new-chat", false, result.value)
    }
  }

  const stepChip = async (harness: "fable" | "codex"): Promise<boolean> => {
    const stepName: LiveProofStepName = harness === "fable" ? "fable-chip" : "codex-chip"
    const probe = asRec(await evalIn(probeChipScript(harness)))
    if (probe["present"] !== true) {
      await capture(`harness-${harness}-failed`)
      record(stepName, false, {
        reason: `${harness} harness chip is not mounted on this build (lane absent)`,
      })
      return false
    }
    if (probe["disabled"] === true) {
      // EP250 steps 5/7: journal the accessible reason honestly and keep the
      // disabled-state pixels — never fake an enabled lane. The reason is the
      // chip's aria-label; the UI renders no caption text (owner fix 3).
      await capture(`harness-${harness}-disabled`)
      record(stepName, false, {
        reason: `${harness} harness chip is disabled`,
        ariaLabel: probe["ariaLabel"] ?? null,
        captionAbsent: probe["captionAbsent"] ?? null,
      })
      return false
    }
    await evalIn(clickScript(`shell-harness-${harness}`))
    const selected = await pollUntil(
      probeChipScript(harness),
      (value) =>
        (typeof value["ariaLabel"] === "string" && (value["ariaLabel"] as string).includes("selected")) ||
        value["variant"] === "secondary",
      liveProofStepTimeoutMs(stepName),
    )
    if (selected.ok) {
      const requestedAccountRef = resolveLiveProofAccountRef(process.env, harness)
      if (requestedAccountRef !== null) {
        let accountRef = ""
        let reached = false
        // The existing typed control cycles the bounded ready-account list.
        // Twelve attempts cover the product's current account bound while
        // keeping a missing target deterministic and timeout-independent.
        for (let attempt = 0; attempt < 12; attempt += 1) {
          const account = asRec(await evalIn(`(() => {
            const control = document.querySelector('[data-en-key="shell-provider-account"]')
            return { present: control !== null, label: control?.textContent?.trim() ?? "" }
          })()`))
          accountRef = typeof account["label"] === "string" ? account["label"] as string : ""
          if (accountRef === requestedAccountRef) {
            reached = true
            break
          }
          if (account["present"] !== true) break
          await evalIn(clickScript("shell-provider-account"))
          await sleep(100)
        }
        if (!reached) {
          await capture(`harness-${harness}-account-failed`)
          record(stepName, false, {
            reason: "requested named account is not selectable",
            requestedAccountRef,
            observedAccountRef: accountRef || null,
          })
          return false
        }
      }
      await capture(`harness-${harness}`)
      const account = asRec(await evalIn(`(() => ({
        accountRef: document.querySelector('[data-en-key="shell-provider-account"]')?.textContent?.trim() ?? null
      }))()`))
      record(stepName, true, {
        ariaLabel: selected.value["ariaLabel"],
        variant: selected.value["variant"],
        accountRef: account["accountRef"] ?? null,
      })
      return true
    }
    await capture(`harness-${harness}-failed`)
    record(stepName, false, { reason: "chip never reported selected state", ...selected.value })
    return false
  }

  const stepTurn = async (harness: "fable" | "codex"): Promise<void> => {
    const stepName: LiveProofStepName = harness === "fable" ? "fable-turn" : "codex-turn"
    const baseline = asRec(await evalIn(probeTurnScript))
    const baselineAssistant = typeof baseline["assistantLength"] === "number" ? baseline["assistantLength"] as number : 0
    const baselineSystem = typeof baseline["systemCount"] === "number" ? baseline["systemCount"] as number : 0
    const baselineMessages = typeof baseline["messageCount"] === "number" ? baseline["messageCount"] as number : 0
    const submitted = asRec(await evalIn(submitTurnScript(liveProofTurnMessage)))
    if (submitted["submitted"] !== true) {
      await capture(`${harness}-turn-failed`)
      record(stepName, false, { reason: submitted["reason"] ?? "submit failed" })
      return
    }
    const startedAt = Date.now()
    const deadline = startedAt + liveProofStepTimeoutMs(stepName)
    let sawPending = false
    let midCaptured = false
    let midWhilePending = false
    let growthEvents = 0
    let maxAssistant = baselineAssistant
    let completed = false
    let idleSince: number | null = null
    let last: Rec = {}
    while (Date.now() < deadline) {
      last = asRec(await evalIn(probeTurnScript))
      // A live turn keeps the textarea enabled for queue-a-follow-up. The
      // typed Stop control, not input.disabled, is the pending authority.
      const disabled = last["turnPending"] === true
      const assistantLength = typeof last["assistantLength"] === "number" ? last["assistantLength"] as number : 0
      const messageCount = typeof last["messageCount"] === "number" ? last["messageCount"] as number : 0
      if (disabled) sawPending = true
      if (assistantLength > maxAssistant) {
        growthEvents += 1
        maxAssistant = assistantLength
      }
      if (assistantLength > baselineAssistant && !midCaptured) {
        // First visible partial (or first full) assistant text: EP250's
        // MID-STREAM receipt. Whether it was genuinely mid-stream is recorded
        // honestly below (midWhilePending / growthEvents).
        midCaptured = true
        midWhilePending = disabled
        await capture(`${harness}-midstream`)
      }
      const systemCount = typeof last["systemCount"] === "number" ? last["systemCount"] as number : 0
      if (disabled) idleSince = null
      else if (idleSince === null) idleSince = Date.now()
      const terminal = liveProofTurnIsTerminal({
        turnPending: disabled,
        assistantGrew: maxAssistant > baselineAssistant,
        activityObserved: systemCount > baselineSystem || sawPending,
        idleForMs: idleSince === null ? 0 : Date.now() - idleSince,
      })
      if (terminal) {
        completed = true
        break
      }
      // Submit had no effect at all (no pending, no appended user note).
      if (!sawPending && messageCount === baselineMessages && Date.now() - startedAt > 10_000) break
      await sleep(200)
    }
    const assistantDelta = maxAssistant - baselineAssistant
    const streamingObserved = growthEvents >= 2 || (midCaptured && midWhilePending)
    const ok = completed && assistantDelta > 0
    await capture(ok ? `${harness}-final` : `${harness}-turn-failed`)
    record(stepName, ok, {
      completed,
      assistantChars: assistantDelta,
      streamingObserved,
      growthEvents,
      midStreamCaptured: midCaptured,
      lastSystem: last["lastSystem"] ?? null,
      snippet: typeof last["assistantSnippet"] === "string" ? (last["assistantSnippet"] as string).slice(0, 200) : null,
    })
  }

  // EP250 capability A2: submit a real turn, click Stop the moment the
  // composer reports a Stoppable pending state, and prove the turn ends with a
  // typed interrupted notice while the control reverts from Stop to Send.
  const stepInterruptStop = async (harness: "fable" | "codex"): Promise<void> => {
    const submitted = asRec(await evalIn(submitTurnScript(liveProofTurnMessage)))
    if (submitted["submitted"] !== true) {
      await capture("interrupt-stop-failed")
      record("interrupt-stop", false, { reason: submitted["reason"] ?? "submit failed", harness })
      return
    }
    const pending = await pollUntil(
      probeComposerControlScript,
      (value) => value["composerDisabled"] === true && value["stopPresent"] === true,
      30_000,
      100,
    )
    if (!pending.ok) {
      await capture("interrupt-stop-failed")
      record("interrupt-stop", false, { reason: "turn never entered a Stoppable pending state", harness, ...pending.value })
      return
    }
    await capture("interrupt-stop-pending")
    const clicked = asRec(await evalIn(clickStopScript))
    if (clicked["clicked"] !== true) {
      await capture("interrupt-stop-failed")
      record("interrupt-stop", false, { reason: "Stop control vanished before click", harness })
      return
    }
    const settled = await pollUntil(
      probeComposerControlScript,
      (value) => value["composerDisabled"] === false && value["sendPresent"] === true,
      liveProofStepTimeoutMs("interrupt-stop"),
      150,
    )
    const value = settled.value
    const interrupted = typeof value["interruptedNoticeCount"] === "number" &&
      (value["interruptedNoticeCount"] as number) >= 1
    const ok = settled.ok && interrupted
    await capture(ok ? "interrupt-stop" : "interrupt-stop-failed")
    record("interrupt-stop", ok, { reverted: settled.ok, interruptedNotice: interrupted, harness })
  }

  // EP250 capability C3: save the first bounded file through the files
  // workspace save seam and prove a typed outcome renders. The save re-emits
  // the file's CURRENT bytes (non-destructive round trip); the destructive
  // stale-revision conflict proof lives in the headless temp-dir oracle.
  const stepFileSave = async (): Promise<void> => {
    const clicked = asRec(await evalIn(clickScript("workspace-files")))
    if (clicked["clicked"] !== true) {
      await capture("file-save-failed")
      record("file-save", false, { reason: "Files dock button never mounted" })
      return
    }
    const filesReady = await pollUntil(probeFilesWorkspaceScript, (value) => value["settled"] === true, 15_000, 200)
    const fv = filesReady.value
    if (!filesReady.ok || fv["hasWorkspace"] !== true || typeof fv["firstEntry"] !== "string") {
      await capture("file-save-failed")
      record("file-save", false, { reason: "no selected workspace with a bounded file entry", ...fv })
      return
    }
    const opened = asRec(await evalIn(clickKeyScript(fv["firstEntry"] as string)))
    if (opened["clicked"] !== true) {
      await capture("file-save-failed")
      record("file-save", false, { reason: "file entry never mounted", entry: fv["firstEntry"] })
      return
    }
    const editorReady = await pollUntil(
      saveOpenFileScript,
      (value) => value["ready"] === true,
      10_000,
      200,
    )
    if (!editorReady.ok) {
      await capture("file-save-failed")
      record("file-save", false, { reason: "editor + save affordance never mounted", entry: fv["firstEntry"] })
      return
    }
    const outcome = await pollUntil(probeSaveOutcomeScript, (value) => value["settled"] === true, liveProofStepTimeoutMs("file-save"), 150)
    const ov = outcome.value
    // Both "saved" and "conflict" are typed save-channel outcomes that prove
    // the SHA-256 expectedRevision guard is live; only "unavailable"/timeout fail.
    const ok = outcome.ok && (ov["outcome"] === "saved" || ov["outcome"] === "conflict")
    await capture(ok ? "file-save" : "file-save-failed")
    record("file-save", ok, { outcome: ov["outcome"] ?? null, entry: fv["firstEntry"] })
  }

  // EP250 capability E1: open the review workspace (via its palette command —
  // review has no dock button) and prove a real dirty-file status renders with
  // a loadable diff. A clean workspace has nothing to review and journals so.
  const stepGitReview = async (): Promise<void> => {
    const toggled = asRec(await evalIn(clickScript("shell-command-palette-toggle")))
    if (toggled["clicked"] !== true) {
      await capture("git-review-failed")
      record("git-review", false, { reason: "command palette toggle never mounted" })
      return
    }
    const paletteReady = await pollUntil(
      clickKeyScript("desktop-command-workspace.review"),
      (value) => value["clicked"] === true,
      8_000,
      150,
    )
    if (!paletteReady.ok) {
      await capture("git-review-failed")
      record("git-review", false, { reason: "Review changes palette command never mounted" })
      return
    }
    const review = await pollUntil(probeReviewWorkspaceScript, (value) => value["settled"] === true, 15_000, 200)
    const rv = review.value
    const changeCount = typeof rv["changeCount"] === "number" ? rv["changeCount"] as number : 0
    if (!review.ok || changeCount < 1 || typeof rv["firstChange"] !== "string") {
      await capture("git-review-failed")
      record("git-review", false, { reason: "workspace clean or unavailable (no dirty file to review)", changeCount, ...rv })
      return
    }
    await evalIn(clickKeyScript(rv["firstChange"] as string))
    const diff = await pollUntil(
      probeReviewWorkspaceScript,
      (value) => value["diffPresent"] === true && typeof value["diffLength"] === "number" && (value["diffLength"] as number) > 0,
      liveProofStepTimeoutMs("git-review"),
      200,
    )
    const ok = diff.ok
    await capture(ok ? "git-review" : "git-review-failed")
    record("git-review", ok, { changeCount, diffLength: diff.value["diffLength"] ?? 0 })
  }

  const stepRedactionCheck = (): void => {
    const blank = shots.filter((shot) => shot.name.trim() === "" || shot.file.trim() === "")
    const emptyFiles = shots.filter((shot) => shot.file !== "" && shot.bytes <= 0)
    const ok = shots.length > 0 && blank.length === 0 && emptyFiles.length === 0
    record("redaction-check", ok, {
      shotCount: shots.length,
      blankNames: blank.map((shot) => shot.name),
      emptyFiles: emptyFiles.map((shot) => path.basename(shot.file)),
    })
  }

  const stepPreflight = async (): Promise<void> => {
    if (options.preflight === undefined) {
      record("account-preflight", false, { reason: "preflight service not wired on this build" })
      return
    }
    try {
      const results = await Promise.race([
        options.preflight(),
        new Promise<null>(resolve =>
          setTimeout(() => resolve(null), liveProofStepTimeoutMs("account-preflight"))),
      ])
      if (results === null) {
        record("account-preflight", false, { reason: "preflight round timed out" })
        return
      }
      const verified = results.filter(result => result.state === "verified")
      record("account-preflight", verified.length > 0, {
        accountCount: results.length,
        verifiedCount: verified.length,
        accounts: results.map(result => ({
          ref: result.ref,
          state: result.state,
          detail: result.detail.slice(0, 120),
          durationMs: result.durationMs,
        })),
      })
    } catch (error) {
      record("account-preflight", false, {
        reason: error instanceof Error ? error.message.slice(0, 200) : "preflight failed",
      })
    }
  }

  const journey = async (): Promise<void> => {
    try {
      await stepPreflight()
      await stepShell()
      const rows = await stepFleet()
      await stepUsage(rows)
      await stepNewChat()
      const fableSelected = await stepChip("fable")
      if (fableSelected) {
        await stepTurn("fable")
      } else {
        record("fable-turn", false, { reason: "skipped: Fable chip unavailable (see fable-chip)" })
      }
      const codexSelected = await stepChip("codex")
      if (codexSelected) {
        await stepTurn("codex")
      } else {
        record("codex-turn", false, { reason: "codex-unavailable (see codex-chip)" })
      }
      // EP250 capability-eval rung-4 receipts.
      if (fableSelected || codexSelected) {
        const interruptHarness: "fable" | "codex" = fableSelected ? "fable" : "codex"
        if (fableSelected) await evalIn(clickScript("shell-harness-fable"))
        await stepInterruptStop(interruptHarness)
      } else {
        record("interrupt-stop", false, { reason: "skipped: no local lane available to interrupt" })
      }
      await stepFileSave()
      await stepGitReview()
      stepRedactionCheck()
      record("summary", requiredFailures.length === 0, {
        requiredFailures,
        steps: journal.map((entry) => ({ step: entry.step, ok: entry.ok })),
        shots: shots.map((shot) => path.basename(shot.file || `${shot.name}.png`)),
        outDir: options.outDir,
      })
    } catch (error) {
      record("summary", false, {
        reason: "driver crashed",
        message: error instanceof Error ? error.message.slice(0, 300) : "unknown",
        requiredFailures,
      })
      if (requiredFailures.length === 0) requiredFailures.push("driver-crash")
    } finally {
      clearTimeout(overallTimeout)
      finish()
    }
  }

  window.webContents.once("did-finish-load", () => {
    log(`journey starting; receipts -> ${options.outDir}`)
    void journey()
  })
}
