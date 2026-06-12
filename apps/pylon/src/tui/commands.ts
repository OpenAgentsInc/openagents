// Command registry for the Pylon TUI (issue #4738). Every app-level action
// is registered exactly once on the @opentui/keymap catalog with title/
// category metadata; the footer, help dialog, and command palette all derive
// from this registry. User keybind overrides from keybinds.json replace the
// default key of the matching command.

import type { CliRenderer, KeyEvent, Renderable } from "@opentui/core"
import { createDefaultOpenTuiKeymap } from "@opentui/keymap/opentui"
import type { PylonContextProjection } from "../context-projection"
import type { PylonRoute } from "./store"
import { filterSelectItems, openAlert, openConfirm, openPrompt, openSelect, showToast } from "./dialogs"

export type PylonKeymap = ReturnType<typeof createDefaultOpenTuiKeymap>

// Actions the node side injects so view code never imports wallet/business
// modules directly (index.ts implements these over src/wallet.ts).
export interface WalletActions {
  send: (destinationRef: string, amountSats?: number) => Promise<unknown>
  receive: (amountSats: number) => Promise<unknown>
  admitPayoutTarget: (kind: string, ref: string) => Promise<unknown>
}

export interface AssignmentActions {
  poll: () => Promise<Array<{ assignmentRef: string; leaseRef: string; goal: string; paymentMode: string; expiresAt: string }>>
  accept: (leaseRef: string) => Promise<unknown>
}

export interface DevActions {
  check: () => Promise<unknown>
  apply: () => Promise<unknown>
  reload: () => Promise<unknown>
}

export interface ContextActions {
  refresh: () => Promise<PylonContextProjection>
}

export interface CommandContext {
  walletActions: WalletActions
  // null when no OpenAgents base URL is configured for this node.
  assignmentActions: AssignmentActions | null
  // null when attached to a remote node that does not expose local dev actions.
  devActions: DevActions | null
  // null when attached to a remote node or before local context probes are available.
  refreshContext: (() => Promise<PylonContextProjection>) | null
  setRoute: (route: PylonRoute) => void
  refreshAssignments: () => Promise<void>
  currentAssignments: () => Array<{ leaseRef: string; goal: string }>
  cycleComposerHistory: (direction: -1 | 1) => void
  focusLogs: () => void
  focusComposer: () => void
  focusedPane: () => "logs" | "composer"
  scrollLogs: (delta: number, unit?: "viewport" | "content" | "step") => void
  submitComposer: () => void
  toggleVerbose: () => boolean
  requestShutdown: () => void
  log: (message: string) => void
}

export interface CommandSpec {
  name: string
  title: string
  category: string
  key?: string
  // Shown in the one-line footer hint bar.
  footer?: boolean
  // Listed in the command palette (scroll keys etc. opt out).
  palette?: boolean
  run: () => void | Promise<void>
}

export const PAYOUT_TARGET_KINDS = ["bolt12_offer", "bolt11_invoice", "bip353_name", "lnurl_pay"] as const

function summarizeDevResult(result: unknown): string {
  const value = result as {
    schema?: unknown
    state?: unknown
    action?: unknown
    changeSummary?: { dirty?: { changedCount?: unknown; untrackedCount?: unknown } }
    commandResults?: Array<{ status?: unknown; exitCode?: unknown }>
    blockerRefs?: unknown[]
  }
  const summary = {
    schema: typeof value.schema === "string" ? value.schema : "unknown",
    action: typeof value.action === "string" ? value.action : "unknown",
    state: typeof value.state === "string" ? value.state : "unknown",
    changedCount: value.changeSummary?.dirty?.changedCount ?? null,
    untrackedCount: value.changeSummary?.dirty?.untrackedCount ?? null,
    checks: Array.isArray(value.commandResults)
      ? value.commandResults.map((command) => ({ exitCode: command.exitCode ?? null, status: command.status ?? "unknown" }))
      : [],
    blockerRefs: Array.isArray(value.blockerRefs) ? value.blockerRefs.slice(0, 5) : [],
  }
  return JSON.stringify(summary)
}

function summarizeContextResult(result: PylonContextProjection): string {
  return JSON.stringify({
    schema: result.schema,
    repo: result.repo.fullName ?? result.repo.state,
    branch: result.repo.branch,
    commitRef: result.repo.commitRef,
    primaryAdapter: result.adapters.primaryAdapter,
    reviewerAdapter: result.adapters.reviewerAdapter,
    mode: result.adapters.mode,
    blockers: result.blockerRefs.slice(0, 5),
  })
}

export function buildCommandSpecs(ctx: CommandContext): CommandSpec[] {
  const specs: CommandSpec[] = [
    {
      name: "palette.open",
      title: "Open command palette",
      category: "App",
      key: "ctrl+k",
      footer: true,
      palette: false,
      run: async () => {
        await openCommandPalette()
      },
    },
    {
      name: "help.open",
      title: "Show keybindings",
      category: "App",
      key: "f1",
      footer: true,
      palette: true,
      run: async () => {
        await openHelpDialog()
      },
    },
    {
      name: "app.quit",
      title: "Quit Pylon",
      category: "App",
      key: "ctrl+c",
      footer: true,
      palette: true,
      run: () => ctx.requestShutdown(),
    },
    {
      name: "logs.verbose-toggle",
      title: "Toggle verbose logs",
      category: "Logs",
      key: "f2",
      palette: true,
      run: () => {
        const verbose = ctx.toggleVerbose()
        showToast(verbose ? "verbose logs on (new entries)" : "verbose logs off (new entries)")
      },
    },
    {
      name: "focus.toggle",
      title: "Switch focus (logs/composer)",
      category: "App",
      key: "tab",
      footer: true,
      palette: true,
      run: () => {
        if (ctx.focusedPane() === "composer") ctx.focusLogs()
        else ctx.focusComposer()
      },
    },
    {
      name: "composer.submit",
      title: "Submit composer prompt",
      category: "Composer",
      palette: true,
      run: () => ctx.submitComposer(),
    },
    {
      name: "dev.check",
      title: "Dev: run focused checks",
      category: "Dev",
      palette: true,
      run: async () => {
        if (!ctx.devActions) {
          await openAlert({ title: "Dev check", body: "Local dev actions are unavailable in this session." })
          return
        }
        try {
          const result = await ctx.devActions.check()
          showToast("dev check complete")
          ctx.log(`[Dev] Check ${summarizeDevResult(result)}`)
        } catch (error) {
          const message = error instanceof Error ? error.message : String(error)
          showToast(`dev check failed: ${message}`, "error")
          ctx.log(`[Dev] Check failed: ${message}`)
        }
      },
    },
    {
      name: "dev.apply",
      title: "Dev: capture apply summary",
      category: "Dev",
      palette: true,
      run: async () => {
        if (!ctx.devActions) {
          await openAlert({ title: "Dev apply", body: "Local dev actions are unavailable in this session." })
          return
        }
        try {
          const result = await ctx.devActions.apply()
          showToast("dev apply summary captured")
          ctx.log(`[Dev] Apply ${summarizeDevResult(result)}`)
        } catch (error) {
          const message = error instanceof Error ? error.message : String(error)
          showToast(`dev apply failed: ${message}`, "error")
          ctx.log(`[Dev] Apply failed: ${message}`)
        }
      },
    },
    {
      name: "dev.reload",
      title: "Dev: reload Pylon session",
      category: "Dev",
      palette: true,
      run: async () => {
        if (!ctx.devActions) {
          await openAlert({ title: "Dev reload", body: "Local dev actions are unavailable in this session." })
          return
        }
        const confirmed = await openConfirm({
          title: "Reload Pylon",
          body: "Run the explicit dev reload action? This does not commit, push, clean, or switch branches.",
          confirmLabel: "Reload",
        })
        if (!confirmed) {
          showToast("dev reload cancelled")
          return
        }
        try {
          const result = await ctx.devActions.reload()
          showToast("dev reload action complete")
          ctx.log(`[Dev] Reload ${summarizeDevResult(result)}`)
        } catch (error) {
          const message = error instanceof Error ? error.message : String(error)
          showToast(`dev reload failed: ${message}`, "error")
          ctx.log(`[Dev] Reload failed: ${message}`)
        }
      },
    },
    {
      name: "context.refresh",
      title: "Context: refresh repo & AI",
      category: "Context",
      palette: true,
      run: async () => {
        if (!ctx.refreshContext) {
          await openAlert({ title: "Context refresh", body: "Local repo and AI context refresh is unavailable in this session." })
          return
        }
        try {
          const result = await ctx.refreshContext()
          showToast("context refreshed")
          ctx.log(`[Context] Refresh ${summarizeContextResult(result)}`)
        } catch (error) {
          const message = error instanceof Error ? error.message : String(error)
          showToast(`context refresh failed: ${message}`, "error")
          ctx.log(`[Context] Refresh failed: ${message}`)
        }
      },
    },
    {
      name: "wallet.send",
      title: "Wallet: send sats",
      category: "Wallet",
      palette: true,
      run: async () => {
        const destination = await openPrompt({
          title: "Wallet send - destination ref",
          placeholder: "bolt12 offer / bolt11 invoice / admitted target ref",
        })
        if (!destination) return
        const amountRaw = await openPrompt({
          title: "Wallet send - amount (sats, empty = invoice amount)",
          placeholder: "e.g. 1000",
        })
        const amount = amountRaw ? Number(amountRaw) : undefined
        if (amountRaw && (!Number.isInteger(amount) || (amount as number) <= 0)) {
          await openAlert({ title: "Wallet send", body: `Invalid amount: ${amountRaw}` })
          return
        }
        const confirmed = await openConfirm({
          title: "Confirm wallet send",
          body: `Send ${amount === undefined ? "the invoice amount" : `${amount} sats`} to ${destination.slice(0, 60)}${destination.length > 60 ? "..." : ""}?`,
          confirmLabel: "Send",
        })
        if (!confirmed) {
          showToast("wallet send cancelled")
          return
        }
        try {
          await ctx.walletActions.send(destination, amount)
          showToast("wallet send dispatched")
          ctx.log(`[Wallet] Send dispatched to ${destination.slice(0, 60)}`)
        } catch (error) {
          const message = error instanceof Error ? error.message : String(error)
          showToast(`wallet send failed: ${message}`, "error")
          ctx.log(`[Wallet] Send failed: ${message}`)
        }
      },
    },
    {
      name: "wallet.receive",
      title: "Wallet: receive (create invoice)",
      category: "Wallet",
      palette: true,
      run: async () => {
        const amountRaw = await openPrompt({ title: "Wallet receive - amount (sats)", placeholder: "e.g. 1000" })
        if (!amountRaw) return
        const amount = Number(amountRaw)
        if (!Number.isFinite(amount) || amount <= 0) {
          await openAlert({ title: "Wallet receive", body: `Invalid amount: ${amountRaw}` })
          return
        }
        const confirmed = await openConfirm({
          title: "Confirm wallet receive",
          body: `Create an invoice for ${amount} sats?`,
          confirmLabel: "Create",
        })
        if (!confirmed) return
        try {
          const result = await ctx.walletActions.receive(amount)
          const invoice = (result as { invoice?: string } | null)?.invoice
          await openAlert({
            title: "Wallet receive",
            body: invoice ? `Invoice: ${invoice}` : JSON.stringify(result).slice(0, 400),
          })
          ctx.log(`[Wallet] Receive invoice created for ${amount} sats`)
        } catch (error) {
          const message = error instanceof Error ? error.message : String(error)
          showToast(`wallet receive failed: ${message}`, "error")
          ctx.log(`[Wallet] Receive failed: ${message}`)
        }
      },
    },
    {
      name: "wallet.admit-payout-target",
      title: "Wallet: admit payout target",
      category: "Wallet",
      palette: true,
      run: async () => {
        const kind = await openSelect({
          title: "Payout target kind",
          items: PAYOUT_TARGET_KINDS.map((value) => ({ id: value, label: value })),
        })
        if (!kind) return
        const ref = await openPrompt({ title: `Payout target ref (${kind})`, placeholder: "target ref" })
        if (!ref) return
        const confirmed = await openConfirm({
          title: "Confirm payout target admission",
          body: `Admit ${kind} target ${ref.slice(0, 60)}${ref.length > 60 ? "..." : ""}? Future payouts may settle to it.`,
          confirmLabel: "Admit",
        })
        if (!confirmed) return
        try {
          await ctx.walletActions.admitPayoutTarget(kind, ref)
          showToast("payout target admitted")
          ctx.log(`[Wallet] Payout target admitted (${kind})`)
        } catch (error) {
          const message = error instanceof Error ? error.message : String(error)
          showToast(`admission failed: ${message}`, "error")
          ctx.log(`[Wallet] Payout target admission failed: ${message}`)
        }
      },
    },
    {
      name: "view.dashboard",
      title: "View: dashboard",
      category: "View",
      key: "f3",
      palette: true,
      run: () => ctx.setRoute("dashboard"),
    },
    {
      name: "view.assignments",
      title: "View: assignments",
      category: "View",
      key: "f4",
      palette: true,
      run: async () => {
        ctx.setRoute("assignments")
        await ctx.refreshAssignments()
      },
    },
    {
      name: "view.wallet",
      title: "View: wallet",
      category: "View",
      key: "f5",
      palette: true,
      run: () => ctx.setRoute("wallet"),
    },
    {
      name: "view.context",
      title: "View: repo & AI context",
      category: "View",
      key: "f6",
      palette: true,
      run: () => ctx.setRoute("context"),
    },
    {
      name: "assignments.refresh",
      title: "Assignments: refresh leases",
      category: "Assignments",
      palette: true,
      run: async () => {
        await ctx.refreshAssignments()
      },
    },
    {
      name: "assignments.accept",
      title: "Assignments: accept a lease",
      category: "Assignments",
      palette: true,
      run: async () => {
        if (!ctx.assignmentActions) {
          await openAlert({ title: "Assignments", body: "PYLON_OPENAGENTS_BASE_URL is not configured." })
          return
        }
        const leases = ctx.currentAssignments()
        if (leases.length === 0) {
          await openAlert({ title: "Assignments", body: "No leases available. Refresh first (f4)." })
          return
        }
        const chosen = await openSelect({
          title: "Accept assignment lease",
          items: leases.map((lease) => ({ id: lease.leaseRef, label: lease.goal.slice(0, 70), detail: lease.leaseRef.slice(0, 24) })),
        })
        if (!chosen) return
        const confirmed = await openConfirm({
          title: "Confirm lease acceptance",
          body: `Accept lease ${chosen.slice(0, 50)}? The node commits to working this assignment.`,
          confirmLabel: "Accept",
        })
        if (!confirmed) return
        try {
          await ctx.assignmentActions.accept(chosen)
          showToast("lease accepted")
          ctx.log(`[Assignments] Lease accepted: ${chosen.slice(0, 40)}`)
          await ctx.refreshAssignments()
        } catch (error) {
          const message = error instanceof Error ? error.message : String(error)
          showToast(`accept failed: ${message}`, "error")
          ctx.log(`[Assignments] Accept failed: ${message}`)
        }
      },
    },
    {
      name: "composer.history-prev",
      title: "Composer: previous history entry",
      category: "Composer",
      palette: false,
      run: () => ctx.cycleComposerHistory(-1),
    },
    {
      name: "composer.history-next",
      title: "Composer: next history entry",
      category: "Composer",
      palette: false,
      run: () => ctx.cycleComposerHistory(1),
    },
    // Scroll commands: bound on the logs pane, hidden from the palette.
    { name: "logs.scroll.up", title: "Scroll logs up", category: "Logs", run: () => ctx.scrollLogs(-1, "step") },
    { name: "logs.scroll.down", title: "Scroll logs down", category: "Logs", run: () => ctx.scrollLogs(1, "step") },
    { name: "logs.scroll.page-up", title: "Scroll logs page up", category: "Logs", run: () => ctx.scrollLogs(-0.8, "viewport") },
    { name: "logs.scroll.page-down", title: "Scroll logs page down", category: "Logs", run: () => ctx.scrollLogs(0.8, "viewport") },
    { name: "logs.scroll.top", title: "Scroll logs to top", category: "Logs", run: () => ctx.scrollLogs(-1, "content") },
    { name: "logs.scroll.bottom", title: "Scroll logs to bottom", category: "Logs", run: () => ctx.scrollLogs(1, "content") },
  ]
  return specs
}

// Module-level registry handle so the palette/help/footer can enumerate the
// active command set without threading the keymap through every component.
let activeSpecs: CommandSpec[] = []
let activeKeymap: PylonKeymap | null = null
let activeOverrides: Record<string, string> = {}

export function commandKey(spec: CommandSpec): string | undefined {
  return activeOverrides[spec.name] ?? spec.key
}

export function getActiveCommandSpecs(): CommandSpec[] {
  return activeSpecs
}

export function runCommandByName(name: string): void {
  const spec = activeSpecs.find((candidate) => candidate.name === name)
  if (spec) void spec.run()
}

export async function openCommandPalette(): Promise<void> {
  const items = activeSpecs
    .filter((spec) => spec.palette !== false)
    .map((spec) => ({
      id: spec.name,
      label: spec.title,
      detail: [spec.category, commandKey(spec)].filter(Boolean).join(" · "),
    }))
  const chosen = await openSelect({ title: "Command palette", items })
  if (chosen) runCommandByName(chosen)
}

export async function openHelpDialog(): Promise<void> {
  const lines = activeSpecs
    .filter((spec) => commandKey(spec) !== undefined || spec.palette !== false)
    .map((spec) => `${(commandKey(spec) ?? "(palette)").padEnd(12)} ${spec.title}`)
  await openAlert({
    title: "Keybindings",
    body: [...lines, "", "meta+return  Submit composer (while composing)"].join("\n"),
  })
}

export function footerHints(): string {
  const hints = activeSpecs
    .filter((spec) => spec.footer && commandKey(spec))
    .map((spec) => `${commandKey(spec)} ${spec.title.toLowerCase().replace(/ \(.*\)$/, "")}`)
  return ` ${hints.join("  ·  ")}`
}

export function filterPaletteItems(query: string) {
  return filterSelectItems(
    activeSpecs.filter((spec) => spec.palette !== false).map((spec) => ({ id: spec.name, label: spec.title })),
    query,
  )
}

// Installs the registry on a fresh keymap: command catalog + global layer
// bindings + a logs-pane focus layer for scroll keys.
export function installPylonKeymap(
  renderer: CliRenderer,
  ctx: CommandContext,
  options: {
    overrides?: Record<string, string>
  } = {},
): PylonKeymap {
  const keymap = createDefaultOpenTuiKeymap(renderer)
  const specs = buildCommandSpecs(ctx)
  activeSpecs = specs
  activeKeymap = keymap
  activeOverrides = options.overrides ?? {}

  keymap.registerLayer({
    commands: specs.map((spec) => ({
      name: spec.name,
      title: spec.title,
      desc: spec.title,
      category: spec.category,
      run: () => {
        void spec.run()
      },
    })),
  })

  const globalBindings = specs
    .filter((spec) => commandKey(spec) && !spec.name.startsWith("logs.scroll."))
    .map((spec) => ({ key: commandKey(spec) as string, cmd: spec.name, desc: spec.title }))
  keymap.registerLayer({ bindings: globalBindings })

  return keymap
}

const scrollKeyDefaults: Record<string, string> = {
  "logs.scroll.up": "up",
  "logs.scroll.down": "down",
  "logs.scroll.page-up": "pageup",
  "logs.scroll.page-down": "pagedown",
  "logs.scroll.top": "home",
  "logs.scroll.bottom": "end",
}

// Focus-scoped scroll bindings for the logs pane; installed once the
// scrollbox renderable exists (called from the ref callback in app.tsx).
export function registerLogsScrollLayer(keymap: PylonKeymap, target: Renderable): () => void {
  return keymap.registerLayer({
    target,
    targetMode: "focus",
    bindings: Object.entries(scrollKeyDefaults).map(([cmd, key]) => ({
      key: activeOverrides[cmd] ?? key,
      cmd,
      desc: cmd,
    })),
  })
}

export function resetCommandRegistry(): void {
  activeSpecs = []
  activeKeymap = null
  activeOverrides = {}
}

export function getActiveKeymap(): PylonKeymap | null {
  return activeKeymap
}

export type { KeyEvent }

// Composer focus layer: Tab must switch focus even while the textarea's own
// editing bindings are active, so it is bound on the composer target with
// focus scope (focus layers outrank globals).
export function registerComposerFocusLayer(keymap: PylonKeymap, target: Renderable): () => void {
  return keymap.registerLayer({
    target,
    targetMode: "focus",
    bindings: [
      { key: activeOverrides["focus.toggle"] ?? "tab", cmd: "focus.toggle", desc: "Switch focus" },
      { key: activeOverrides["composer.history-prev"] ?? "ctrl+p", cmd: "composer.history-prev", desc: "History prev" },
      { key: activeOverrides["composer.history-next"] ?? "ctrl+n", cmd: "composer.history-next", desc: "History next" },
    ],
  })
}
