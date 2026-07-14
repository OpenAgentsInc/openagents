import { mkdirSync, readFileSync, statSync, writeFileSync } from "node:fs"
import path from "node:path"

import type { BrowserWindow } from "electron"

export const MvpProofEnvironment = "OPENAGENTS_DESKTOP_MVP_PROOF"
export const MvpProofArg = "--openagents-mvp-proof"

const proofArgValue = (argv: ReadonlyArray<string>, name: string): string | null => {
  const prefix = `${name}=`
  const value = argv.find(arg => arg.startsWith(prefix))?.slice(prefix.length).trim() ?? ""
  return value === "" ? null : value
}

/**
 * Signed macOS app launches cannot rely on a caller's custom environment
 * surviving LaunchServices/process handoff. Carry the already-isolated proof
 * coordinates as explicit argv, then reconstruct only this closed env set
 * before Electron chooses its user-data path or driver mode.
 */
export const mvpProofEnvironmentFromArgv = (
  argv: ReadonlyArray<string>,
): Readonly<Record<string, string>> | null => {
  if (!argv.includes(MvpProofArg)) return null
  const userData = proofArgValue(argv, "--openagents-mvp-proof-user-data")
  const workspace = proofArgValue(argv, "--openagents-mvp-proof-workspace")
  const receipts = proofArgValue(argv, "--openagents-mvp-proof-receipts")
  const phase = proofArgValue(argv, "--openagents-mvp-proof-phase")
  if (userData === null || workspace === null || receipts === null ||
      (phase !== "initial" && phase !== "restart")) return null
  return {
    OPENAGENTS_DESKTOP_MVP_PROOF: "1",
    OPENAGENTS_DESKTOP_MVP_PROOF_DIR: receipts,
    OPENAGENTS_DESKTOP_MVP_PROOF_PHASE: phase,
    OPENAGENTS_DESKTOP_ISOLATED_APP_PROOF: "1",
    OPENAGENTS_DESKTOP_ISOLATED_WORKSPACE_ROOT: workspace,
    OPENAGENTS_DESKTOP_USER_DATA: userData,
  }
}

export type MvpProofStepName =
  | "shell"
  | "codex-ready"
  | "root-coding-turn"
  | "root-artifact-verified"
  | "child-coding-turn"
  | "child-transcript"
  | "child-artifact-verified"
  | "renderer-reload-restored"
  | "app-restart-restored"

export const mvpProofRequiredSteps: ReadonlyArray<MvpProofStepName> = [
  "shell",
  "codex-ready",
  "root-coding-turn",
  "root-artifact-verified",
  "child-coding-turn",
  "child-transcript",
  "child-artifact-verified",
  "renderer-reload-restored",
  "app-restart-restored",
]

export type MvpProofJournalEntry = Readonly<{
  step: MvpProofStepName | "summary"
  ok: boolean
  detail: string
}>

export type MvpProofConfig = Readonly<{
  enabled: boolean
  conflict: boolean
  outDir: string
}>

export const resolveMvpProofCommand = (
  installedExecutable: string | undefined,
  packageRoot: string,
): string[] => {
  const installed = installedExecutable?.trim() ?? ""
  return installed === ""
    ? [path.join(path.resolve(packageRoot), "node_modules", ".bin", "electron"), "."]
    : [path.resolve(installed)]
}

export const resolveMvpProofConfig = (
  env: Readonly<Record<string, string | undefined>>,
  userDataDir: string,
): MvpProofConfig => {
  const enabled = env[MvpProofEnvironment] === "1"
  const requestedDir = env["OPENAGENTS_DESKTOP_MVP_PROOF_DIR"]?.trim() ?? ""
  return {
    enabled,
    conflict: enabled && (
      env["OPENAGENTS_DESKTOP_SMOKE"] === "1" ||
      env["OPENAGENTS_DESKTOP_LIVE_PROOF"] === "1" ||
      env["OPENAGENTS_DESKTOP_ISOLATED_APP_PROOF"] !== "1"
    ),
    outDir: requestedDir === "" ? path.join(userDataDir, "mvp-proof") : path.resolve(requestedDir),
  }
}

type Rec = Record<string, unknown>
const asRec = (value: unknown): Rec => typeof value === "object" && value !== null ? value as Rec : {}
const sleep = (ms: number): Promise<void> => new Promise(resolve => setTimeout(resolve, ms))
const bounded = (value: unknown): string =>
  (typeof value === "string" ? value : JSON.stringify(value) ?? "").slice(0, 700)

const click = (key: string): string => `(() => {
  const node = document.querySelector('[data-en-key=${JSON.stringify(key)}]')
  if (node === null || node.disabled === true) return { clicked: false }
  node.click()
  return { clicked: true }
})()`

const setField = (key: string, value: string): string => `(() => {
  const host = document.querySelector('[data-en-key=${JSON.stringify(key)}]')
  const field = host?.matches?.('input,textarea') ? host : host?.querySelector?.('input,textarea')
  if (field === null || field === undefined || field.disabled === true) return { changed: false }
  const setter = Object.getOwnPropertyDescriptor(Object.getPrototypeOf(field), 'value')?.set
  if (setter === undefined) return { changed: false }
  setter.call(field, ${JSON.stringify(value)})
  field.dispatchEvent(new Event('input', { bubbles: true }))
  field.dispatchEvent(new Event('change', { bubbles: true }))
  return { changed: field.value === ${JSON.stringify(value)} }
})()`

const turnProbe = `(() => {
  const transcript = document.querySelector('[data-en-key="shell-transcript"]')
  return {
    mounted: transcript !== null,
    pending: document.querySelector('[data-en-key="shell-stop"]') !== null,
    userCount: transcript?.querySelectorAll('[data-en-message][data-en-role="user"]').length ?? 0,
    assistantCount: transcript?.querySelectorAll('[data-en-message][data-en-role="assistant"]').length ?? 0,
    toolCount: transcript?.querySelectorAll('[data-en-message][data-en-role="tool"]').length ?? 0,
    childCount: transcript?.querySelectorAll('[data-en-key^="child-card-"]').length ?? 0,
  }
})()`

/**
 * The MVP has one engine: the user's ordinary logged-in Codex session. Read
 * readiness from that fixed engine and the evidence-gated Send control; never
 * wait for the retired provider/account selector.
 */
export const mvpCodexReadyProbe = `(() => {
  const engine = document.querySelector('[data-en-key="shell-codex-engine"]')
  const send = document.querySelector('button[data-en-key="shell-note"], [data-en-key="shell-note"] button')
  return {
    present: engine?.textContent?.trim() === 'Codex' && send !== null,
    ready: send instanceof HTMLButtonElement && send.disabled !== true,
    reason: send?.getAttribute('aria-label')?.slice(0, 160) ?? null,
  }
})()`

export type MvpProofRunOptions = Readonly<{
  outDir: string
  phase: "initial" | "restart"
  verifyArtifact: (packet: "root" | "child") => Readonly<{ ok: boolean; receiptRef: string }>
  exit: (code: number) => void
}>

export const runMvpProof = (window: BrowserWindow, options: MvpProofRunOptions): void => {
  mkdirSync(options.outDir, { recursive: true })
  const journal: MvpProofJournalEntry[] = options.phase === "restart"
    ? (() => {
        try {
          const value = JSON.parse(readFileSync(path.join(options.outDir, "journal.json"), "utf8"))
          return Array.isArray(value) ? value as MvpProofJournalEntry[] : []
        } catch { return [] }
      })()
    : []
  let shot = 0
  const persist = (): void => writeFileSync(
    path.join(options.outDir, "journal.json"),
    `${JSON.stringify(journal, null, 2)}\n`,
    { mode: 0o600 },
  )
  const record = (step: MvpProofJournalEntry["step"], ok: boolean, detail: unknown): void => {
    const entry = { step, ok, detail: bounded(detail) }
    journal.push(entry)
    persist()
    ;(ok ? console.log : console.error)(`[openagents-desktop mvp-proof] ${step} ${ok ? "OK" : "FAILED"} ${entry.detail}`)
  }
  const capture = async (label: string): Promise<void> => {
    const image = await window.webContents.capturePage()
    const file = path.join(options.outDir, `${String(++shot).padStart(2, "0")}-${label}.png`)
    writeFileSync(file, image.toPNG(), { mode: 0o600 })
    console.log(`[openagents-desktop mvp-proof] screenshot ${path.basename(file)} ${statSync(file).size} bytes`)
  }
  const evaluate = async (script: string): Promise<unknown> => {
    try { return await window.webContents.executeJavaScript(script, true) }
    catch { return { evaluationFailed: true } }
  }
  const poll = async (
    script: string,
    ready: (value: Rec) => boolean,
    timeoutMs: number,
  ): Promise<{ ok: boolean; value: Rec }> => {
    const deadline = Date.now() + timeoutMs
    let value: Rec = {}
    do {
      value = asRec(await evaluate(script))
      if (ready(value)) return { ok: true, value }
      await sleep(250)
    } while (Date.now() < deadline)
    return { ok: false, value }
  }
  const requireClick = async (key: string): Promise<void> => {
    const result = await poll(click(key), value => value["clicked"] === true, 30_000)
    if (!result.ok) throw new Error(`control unavailable: ${key}`)
  }
  const requireField = async (key: string, value: string): Promise<void> => {
    const result = asRec(await evaluate(setField(key, value)))
    if (result["changed"] !== true) throw new Error(`field unavailable: ${key}`)
  }
  const verifyArtifact = async (packet: "root" | "child"): Promise<void> => {
    const artifact = options.verifyArtifact(packet)
    if (!artifact.ok) throw new Error(`${packet} artifact verification failed`)
    await capture(`${packet}-artifact-verified`)
    record(`${packet}-artifact-verified`, true, { receiptRef: artifact.receiptRef })
  }

  void (async () => {
    const overall = setTimeout(() => {
      record("summary", false, "bounded MVP proof timeout")
      options.exit(1)
    }, 20 * 60_000)
    try {
      const shell = await poll(`(() => ({ ready: document.querySelector('[data-en-key="shell-root"]') !== null }))()`, value => value["ready"] === true, 30_000)
      if (!shell.ok) throw new Error("shell did not mount")
      if (options.phase === "restart") {
        await verifyArtifact("root")
        await verifyArtifact("child")
        await capture("app-restart-restored")
        record("app-restart-restored", true, "both exact coding artifacts remained independently readable in a second app process")
        record("summary", true, { requiredSteps: mvpProofRequiredSteps.length, hiddenSpecToolingUsed: false, ownerAcceptanceFabricated: false })
        clearTimeout(overall)
        options.exit(0)
        return
      }
      record("shell", true, "Effect Native shell mounted")

      await requireClick("workspace-new-chat")
      const codexReady = await poll(mvpCodexReadyProbe, value =>
        value["present"] === true && value["ready"] === true, 240_000)
      if (!codexReady.ok) throw new Error(`logged-in Codex session unavailable: ${String(codexReady.value["reason"] ?? "unknown")}`)
      record("codex-ready", true, "ordinary logged-in Codex session selected after host preflight")

      const rootPrompt = "Installed OpenAgents MVP coding proof. Work only in the current workspace. Create mvp-proof/root-output.txt containing exactly root packet complete followed by one newline. Run a shell assertion over the exact bytes, then report completion."
      await requireField("shell-input", rootPrompt)
      await requireClick("shell-note")
      const rootTurn = await poll(turnProbe, value => value["pending"] === false && Number(value["assistantCount"]) > 0 && Number(value["toolCount"]) > 0, 300_000)
      if (!rootTurn.ok) throw new Error("root coding turn did not terminalize with tool evidence")
      await capture("root-coding-turn")
      record("root-coding-turn", true, rootTurn.value)
      await verifyArtifact("root")

      const childPrompt = "Installed OpenAgents MVP coding proof. Use the native Codex child-agent tool to delegate creation of mvp-proof/child-output.txt containing exactly child packet complete followed by one newline. Inspect the child result, run a shell assertion over the exact bytes, then report completion."
      await requireField("shell-input", childPrompt)
      await requireClick("shell-note")
      const childTurn = await poll(turnProbe, value => value["pending"] === false && Number(value["assistantCount"]) > 0 && Number(value["childCount"]) > 0, 300_000)
      if (!childTurn.ok) throw new Error("child-allocated Codex turn did not retain a child card")
      await capture("child-coding-turn")
      record("child-coding-turn", true, childTurn.value)
      const childOpen = asRec(await evaluate(`(() => {
        const node = document.querySelector('[data-en-key^="child-open-"]')
        if (node === null) return { clicked: false }
        node.click()
        return { clicked: true }
      })()`))
      if (childOpen["clicked"] !== true) throw new Error("child transcript control was unavailable")
      await capture("child-transcript")
      record("child-transcript", true, "causal child card opened its independent transcript")
      await verifyArtifact("child")

      window.webContents.reload()
      if (!(await poll(`(() => ({ ready: document.querySelector('[data-en-key="shell-root"]') !== null }))()`, value => value["ready"] === true, 30_000)).ok) {
        throw new Error("shell did not restore after renderer reload")
      }
      await verifyArtifact("root")
      await verifyArtifact("child")
      await capture("renderer-reload-restored")
      record("renderer-reload-restored", true, "both exact coding artifacts remained independently readable after renderer reload")
      clearTimeout(overall)
      options.exit(75)
    } catch (error) {
      await capture("failure").catch(() => {})
      record("summary", false, error instanceof Error ? error.message : "MVP proof failed")
      clearTimeout(overall)
      options.exit(1)
    }
  })()
}
