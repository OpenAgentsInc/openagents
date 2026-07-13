import { mkdirSync, statSync, writeFileSync } from "node:fs"
import path from "node:path"

import type { BrowserWindow } from "electron"

export const MvpProofEnvironment = "OPENAGENTS_DESKTOP_MVP_PROOF"

export type MvpProofStepName =
  | "shell"
  | "codex-ready"
  | "product-spec-open"
  | "plan-accepted"
  | "root-packet-turn"
  | "root-packet-verified"
  | "child-packet-turn"
  | "child-transcript"
  | "child-packet-verified"
  | "owner-gate-pending"

export const mvpProofRequiredSteps: ReadonlyArray<MvpProofStepName> = [
  "shell",
  "codex-ready",
  "product-spec-open",
  "plan-accepted",
  "root-packet-turn",
  "root-packet-verified",
  "child-packet-turn",
  "child-transcript",
  "child-packet-verified",
  "owner-gate-pending",
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
  specPath: string
}>

const validRelativeSpecPath = (value: string): boolean =>
  value.endsWith(".product-spec.md") && !path.isAbsolute(value) &&
  value.split(/[\\/]/).every(part => part !== "" && part !== "." && part !== "..")

export const resolveMvpProofConfig = (
  env: Readonly<Record<string, string | undefined>>,
  userDataDir: string,
): MvpProofConfig => {
  const enabled = env[MvpProofEnvironment] === "1"
  const requestedPath = env["OPENAGENTS_DESKTOP_MVP_PROOF_SPEC_PATH"]?.trim() ?? ""
  const requestedDir = env["OPENAGENTS_DESKTOP_MVP_PROOF_DIR"]?.trim() ?? ""
  return {
    enabled,
    conflict: enabled && (
      env["OPENAGENTS_DESKTOP_SMOKE"] === "1" ||
      env["OPENAGENTS_DESKTOP_LIVE_PROOF"] === "1" ||
      env["OPENAGENTS_DESKTOP_ISOLATED_APP_PROOF"] !== "1" ||
      !validRelativeSpecPath(requestedPath)
    ),
    outDir: requestedDir === "" ? path.join(userDataDir, "mvp-proof") : path.resolve(requestedDir),
    specPath: validRelativeSpecPath(requestedPath) ? requestedPath : "",
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

const productSpecProbe = `(() => ({
  mounted: document.querySelector('[data-en-key="product-spec-workspace"]') !== null,
  executable: document.querySelector('[data-en-key="product-spec-ready-badge"]') !== null,
  invalid: document.querySelector('[data-en-key="product-spec-invalid-badge"]') !== null,
  planAccepted: document.querySelector('[data-en-key="product-spec-plan-state"]')?.textContent?.trim() === 'accepted',
  packetStates: Array.from(document.querySelectorAll('[data-en-key^="product-spec-packet-state-"]')).map(node => ({
    key: node.getAttribute('data-en-key'), value: node.textContent?.trim() ?? ''
  })),
  ownerPending: document.querySelectorAll('[data-en-key^="product-spec-packet-owner-pending-"]').length,
}))()`

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

export type MvpProofRunOptions = Readonly<{
  outDir: string
  specPath: string
  verifyArtifact: (packet: "root" | "child") => Readonly<{ ok: boolean; receiptRef: string }>
  exit: (code: number) => void
}>

export const runMvpProof = (window: BrowserWindow, options: MvpProofRunOptions): void => {
  mkdirSync(options.outDir, { recursive: true })
  const journal: MvpProofJournalEntry[] = []
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
    const result = asRec(await evaluate(click(key)))
    if (result["clicked"] !== true) throw new Error(`control unavailable: ${key}`)
  }
  const requireField = async (key: string, value: string): Promise<void> => {
    const result = asRec(await evaluate(setField(key, value)))
    if (result["changed"] !== true) throw new Error(`field unavailable: ${key}`)
  }
  const verifyPacket = async (packet: "root" | "child", packetRef: string): Promise<void> => {
    const artifact = options.verifyArtifact(packet)
    if (!artifact.ok) throw new Error(`${packet} artifact verification failed`)
    await requireClick("workspace-product-spec")
    await poll(productSpecProbe, value => value["mounted"] === true, 30_000)
    await requireField("product-spec-evidence-ref", artifact.receiptRef)
    await requireClick(`product-spec-evidence-${packetRef}`)
    await poll(productSpecProbe, value => JSON.stringify(value["packetStates"]).includes("evidence present"), 30_000)
    await requireField("product-spec-verifier-ref", "verifier.mvp-proof.host")
    await requireField("product-spec-verification-output-ref", `${artifact.receiptRef}.verified`)
    await requireClick(`product-spec-verify-${packetRef}`)
    const verified = await poll(productSpecProbe, value =>
      (value["packetStates"] as Array<Rec> | undefined)?.some(row =>
        row["key"] === `product-spec-packet-state-${packetRef}` && row["value"] === "verified") === true,
    30_000)
    if (!verified.ok) throw new Error(`${packet} packet did not reach verified`)
    await capture(`${packet}-packet-verified`)
    record(`${packet}-packet-verified`, true, { packetRef, receiptRef: artifact.receiptRef })
  }

  void (async () => {
    const overall = setTimeout(() => {
      record("summary", false, "bounded MVP proof timeout")
      options.exit(1)
    }, 20 * 60_000)
    try {
      const shell = await poll(`(() => ({ ready: document.querySelector('[data-en-key="shell-root"]') !== null }))()`, value => value["ready"] === true, 30_000)
      if (!shell.ok) throw new Error("shell did not mount")
      record("shell", true, "Effect Native shell mounted")

      await requireClick("workspace-new-chat")
      const codexReady = await poll(`(() => {
        const select = document.querySelector('[data-en-key="shell-harness-select"]')
        const option = select instanceof HTMLSelectElement
          ? Array.from(select.options).find(value => value.value === 'codex')
          : null
        return {
          present: select !== null && option !== null,
          selected: select instanceof HTMLSelectElement && select.value === 'codex',
          ready: option !== null && option.disabled !== true,
          reason: document.querySelector('[data-en-key="shell-note"]')?.getAttribute('aria-label')?.slice(0, 160) ?? null,
        }
      })()`, value => value["ready"] === true && value["selected"] === true, 240_000)
      if (!codexReady.ok) throw new Error(`named Codex capacity unavailable: ${String(codexReady.value["reason"] ?? "unknown")}`)
      record("codex-ready", true, "named isolated Codex capacity selected after host preflight")

      await requireClick("workspace-product-spec")
      if (!(await poll(productSpecProbe, value => value["mounted"] === true, 30_000)).ok) throw new Error("ProductSpec workspace did not mount")
      await requireField("product-spec-path", options.specPath)
      await requireClick("product-spec-open")
      const opened = await poll(productSpecProbe, value => value["executable"] === true || value["invalid"] === true, 30_000)
      if (!opened.ok || opened.value["executable"] !== true) throw new Error("ProductSpec did not validate as executable")
      await capture("product-spec-open")
      record("product-spec-open", true, "validator-clean ProductSpec opened through the workroom")

      await requireClick("product-spec-propose-plan")
      await requireClick("product-spec-accept-plan")
      const accepted = await poll(productSpecProbe, value => value["planAccepted"] === true, 30_000)
      if (!accepted.ok) throw new Error("plan was not accepted")
      record("plan-accepted", true, { packetCount: (accepted.value["packetStates"] as unknown[] | undefined)?.length ?? 0 })

      await requireClick("product-spec-admit-packet.fx-ac-01")
      const rootTurn = await poll(turnProbe, value => value["pending"] === false && Number(value["assistantCount"]) > 0 && Number(value["toolCount"]) > 0, 300_000)
      if (!rootTurn.ok) throw new Error("root packet real Codex turn did not terminalize with a non-text item")
      await capture("root-packet-turn")
      record("root-packet-turn", true, rootTurn.value)
      await verifyPacket("root", "packet.fx-ac-01")

      await requireClick("product-spec-admit-packet.fx-ac-02")
      const childTurn = await poll(turnProbe, value => value["pending"] === false && Number(value["assistantCount"]) > 0 && Number(value["childCount"]) > 0, 300_000)
      if (!childTurn.ok) throw new Error("child-allocated Codex turn did not retain a child card")
      await capture("child-packet-turn")
      record("child-packet-turn", true, childTurn.value)
      const childOpen = asRec(await evaluate(`(() => {
        const node = document.querySelector('[data-en-key^="child-open-"]')
        if (node === null) return { clicked: false }
        node.click()
        return { clicked: true }
      })()`))
      if (childOpen["clicked"] !== true) throw new Error("child transcript control was unavailable")
      await capture("child-transcript")
      record("child-transcript", true, "causal child card opened its independent transcript")
      await verifyPacket("child", "packet.fx-ac-02")

      const pending = await poll(productSpecProbe, value => Number(value["ownerPending"]) === 2, 30_000)
      if (!pending.ok) throw new Error("owner dispositions were not retained as separate pending gates")
      await capture("owner-gates-pending")
      record("owner-gate-pending", true, "two verified packets await explicit owner disposition")
      record("summary", true, { requiredSteps: mvpProofRequiredSteps.length, ownerAcceptanceFabricated: false })
      clearTimeout(overall)
      options.exit(0)
    } catch (error) {
      await capture("failure").catch(() => {})
      record("summary", false, error instanceof Error ? error.message : "MVP proof failed")
      clearTimeout(overall)
      options.exit(1)
    }
  })()
}
