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
 * process exits nonzero only when a REQUIRED step (shell, fleet, new chat)
 * failed. Screenshots are public-safe by construction: the shell renders
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
  | "shell-mounted"
  | "fleet-workspace"
  | "fleet-usage-check"
  | "new-chat"
  | "fable-chip"
  | "fable-turn"
  | "codex-chip"
  | "codex-turn"
  | "redaction-check"
  | "summary"

export type LiveProofStep = Readonly<{
  name: LiveProofStepName
  /** Required steps decide the process exit code (EP250 steps 1, 2, 4). */
  required: boolean
  timeoutMs: number
}>

/** The EP250 journey, in execution order. */
export const liveProofSteps: ReadonlyArray<LiveProofStep> = [
  { name: "shell-mounted", required: true, timeoutMs: 30_000 },
  // The real pylon CLI list spawn (bun + registry read) can be slow on a
  // cold machine; the provider-accounts list timeout itself is 120s.
  { name: "fleet-workspace", required: true, timeoutMs: 150_000 },
  { name: "fleet-usage-check", required: false, timeoutMs: 60_000 },
  { name: "new-chat", required: true, timeoutMs: 30_000 },
  { name: "fable-chip", required: false, timeoutMs: 15_000 },
  { name: "fable-turn", required: false, timeoutMs: 180_000 },
  { name: "codex-chip", required: false, timeoutMs: 15_000 },
  { name: "codex-turn", required: false, timeoutMs: 180_000 },
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

/** The exact real message both harness-lane turns send (EP250 step 6). */
export const liveProofTurnMessage =
  "Episode 250 live proof: reply with one sentence confirming streaming works, then stop."

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
  const composer = document.querySelector('[data-en-key="shell-input"] input')
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
  const input = document.querySelector('[data-en-key="shell-input"] input')
  if (input === null) return { submitted: false, reason: "composer input not mounted" }
  if (input.disabled) return { submitted: false, reason: "composer disabled" }
  input.focus()
  input.value = ${JSON.stringify(message)}
  input.dispatchEvent(new Event("input", { bubbles: true }))
  input.dispatchEvent(new KeyboardEvent("keydown", { key: "Enter", bubbles: true }))
  return { submitted: true }
})()`

const probeTurnScript = `(() => {
  const input = document.querySelector('[data-en-key="shell-input"] input')
  const assistantRows = Array.from(document.querySelectorAll(
    '[data-en-key="shell-transcript"] [data-en-message][data-en-role="assistant"]'
  ))
  const assistantText = assistantRows
    .map((row) => { const body = row.querySelector('[data-en-role="body"]'); return body === null ? "" : body.textContent || "" })
    .join("")
  const systemRows = Array.from(document.querySelectorAll(
    '[data-en-key="shell-transcript"] [data-en-message][data-en-role="system"]'
  )).map((row) => { const body = row.querySelector('[data-en-role="body"]'); return body === null ? "" : body.textContent || "" })
  return {
    composerDisabled: input === null ? null : input.disabled === true,
    assistantLength: assistantText.length,
    assistantSnippet: assistantText.slice(0, 400),
    systemCount: systemRows.length,
    lastSystem: systemRows.length === 0 ? null : systemRows[systemRows.length - 1].slice(0, 300),
    messageCount: document.querySelectorAll('[data-en-key="shell-transcript"] [data-en-message]').length,
  }
})()`

// ---------------------------------------------------------------------------
// Driver
// ---------------------------------------------------------------------------

export type LiveProofRunOptions = Readonly<{
  outDir: string
  exit: (code: number) => void
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
      await capture(`harness-${harness}`)
      record(stepName, true, { ariaLabel: selected.value["ariaLabel"], variant: selected.value["variant"] })
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
    let last: Rec = {}
    while (Date.now() < deadline) {
      last = asRec(await evalIn(probeTurnScript))
      const disabled = last["composerDisabled"] === true
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
      const terminal = last["composerDisabled"] === false && (
        maxAssistant > baselineAssistant || systemCount > baselineSystem || sawPending
      )
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

  const journey = async (): Promise<void> => {
    try {
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
