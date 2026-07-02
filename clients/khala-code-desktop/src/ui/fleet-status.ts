import { iconElement } from "@openagentsinc/ui/icon-dom"
import type { IconName } from "@openagentsinc/ui/icon"
import type {
  KhalaCodeDesktopConnectStart,
  KhalaCodeDesktopFleetAccount,
  KhalaCodeDesktopFleetDelegateRunRequest,
  KhalaCodeDesktopFleetDelegateRunResult,
  KhalaCodeDesktopFleetRunControlRequest,
  KhalaCodeDesktopFleetRunControlResult,
  KhalaCodeDesktopFleetRunListRequest,
  KhalaCodeDesktopFleetRunListResult,
  KhalaCodeDesktopFleetRunProjection,
  KhalaCodeDesktopFleetRunStartRequest,
  KhalaCodeDesktopFleetRunStartResult,
  KhalaCodeDesktopFleetStatus,
  KhalaCodeDesktopFleetWorkerControlRequest,
  KhalaCodeDesktopFleetWorkerControlResult,
} from "../shared/rpc"
import { buildKhalaFleetBoardProjection } from "./fleet-board-projection"
import { renderKhalaFleetBoardHtml } from "./fleet-board-renderer"
import {
  buildKhalaFleetWorkerCards,
  consumeKhalaFleetWorkerLifecycleNdjson,
  createKhalaFleetWorkerCardThrottler,
  type KhalaFleetWorkerLifecycleFrame,
  type KhalaFleetWorkerCard,
} from "./fleet-worker-cards"
import {
  defaultKhalaFleetDelegationActiveParameters,
  type KhalaGymDelegationOptimizationRun,
} from "./gym-proof-loader"

// Fleet status panel for Khala Code Desktop: current Codex fleet state — all
// linked accounts (with signed-in email + readiness), local Pylon health +
// capacity, active assignments, and running codex_exec processes. Accounts can
// be removed, reconnected, or freshly connected (device-auth) from here.

export type FleetPanelHandle = Readonly<{
  refresh: () => Promise<void>
  setVisible: (visible: boolean) => void
}>

export type FleetPanelOptions = Readonly<{
  delegateRun: (
    request: KhalaCodeDesktopFleetDelegateRunRequest,
  ) => Promise<KhalaCodeDesktopFleetDelegateRunResult>
  fleetRunStart: (
    request: KhalaCodeDesktopFleetRunStartRequest,
  ) => Promise<KhalaCodeDesktopFleetRunStartResult>
  fleetRunControl: (
    request: KhalaCodeDesktopFleetRunControlRequest,
  ) => Promise<KhalaCodeDesktopFleetRunControlResult>
  fleetWorkerControl: (
    request: KhalaCodeDesktopFleetWorkerControlRequest,
  ) => Promise<KhalaCodeDesktopFleetWorkerControlResult>
  fleetRunList: (
    request?: KhalaCodeDesktopFleetRunListRequest,
  ) => Promise<KhalaCodeDesktopFleetRunListResult>
  loadGymDemoProof: () =>
    | KhalaGymDelegationOptimizationRun
    | Promise<KhalaGymDelegationOptimizationRun>
  startDelegationOptimization: () => Promise<KhalaGymDelegationOptimizationRun>
  fetch: () => Promise<KhalaCodeDesktopFleetStatus>
  removeAccount: (
    accountRef: string,
  ) => Promise<{ readonly ok: boolean; readonly error?: string }>
  setAccountPaused: (
    request: { readonly accountRef: string; readonly paused: boolean },
  ) => Promise<{ readonly ok: boolean; readonly error?: string }>
  consumeResetCredit: (
    request: { readonly accountRef: string },
  ) => Promise<{ readonly ok: boolean; readonly error?: string }>
  connectAccount: (accountRef: string) => Promise<KhalaCodeDesktopConnectStart>
  openExternal: (url: string) => Promise<boolean>
  lifecycleNdjson?: () => AsyncIterable<string | Uint8Array>
  lifecycleUpdateThrottleMs?: number
}>

type Handlers = Readonly<{
  onDelegateField: (field: keyof FleetDelegateFormState, value: string | boolean) => void
  onDelegateRun: () => void
  onFleetRunField: (field: keyof FleetRunFormState, value: string) => void
  onFleetRunControl: (verb: KhalaCodeDesktopFleetRunControlRequest["verb"]) => void
  onFleetWorkerControl: (
    card: KhalaFleetWorkerCard,
    verb: KhalaCodeDesktopFleetWorkerControlRequest["verb"],
  ) => void
  onFleetRunPreview: () => void
  onFleetRunStart: () => void
  onLoadGymDemoProof: () => void
  onOptimizationStart: () => void
  onRefresh: () => void
  onRemove: (accountRef: string) => void
  onConnect: (accountRef: string) => void
  onPauseAccount: (accountRef: string, paused: boolean) => void
  onConsumeResetCredit: (accountRef: string) => void
  onOpenUrl: (url: string) => void
  onCancelConnect: () => void
}>

type ConnectView = Readonly<{
  accountRef: string
  start: KhalaCodeDesktopConnectStart | null
}>

type FleetDelegateFormState = Readonly<{
  accountRef: string
  branch: string
  commit: string
  count: string
  mode: KhalaCodeDesktopFleetDelegateRunRequest["mode"]
  noRun: boolean
  objective: string
  repo: string
  verify: string
}>

type FleetRunFormState = Readonly<{
  objective: string
  workSource: KhalaCodeDesktopFleetRunStartRequest["workSource"]["kind"]
  targetConcurrency: string
  workerKind: "codex" | "claude" | "auto"
}>

type DelegateRunView =
  | { readonly phase: "idle" }
  | { readonly phase: "loading" }
  | { readonly phase: "error"; readonly message: string }
  | { readonly phase: "ready"; readonly result: KhalaCodeDesktopFleetDelegateRunResult }

type OptimizationRunView =
  | { readonly phase: "idle" }
  | { readonly phase: "loading" }
  | { readonly phase: "error"; readonly message: string }
  | { readonly phase: "ready"; readonly result: KhalaGymDelegationOptimizationRun }

type FleetRunPreviewSlot = Readonly<{
  accountRef: string
  // Projected wave label only — real claim refs are minted by the
  // supervisor at claim time and look nothing like this.
  plannedClaimLabel: string
  slot: number
  workerKind: FleetRunFormState["workerKind"]
}>

type FleetRunView =
  | { readonly phase: "idle" }
  | { readonly phase: "preview"; readonly slots: readonly FleetRunPreviewSlot[] }
  | { readonly phase: "loading"; readonly slots: readonly FleetRunPreviewSlot[] }
  | { readonly phase: "error"; readonly message: string; readonly slots?: readonly FleetRunPreviewSlot[] }
  | {
      readonly phase: "ready"
      readonly result: KhalaCodeDesktopFleetRunStartResult
      readonly slots: readonly FleetRunPreviewSlot[]
    }

type ActiveFleetRunView = Readonly<{
  controlInFlight: KhalaCodeDesktopFleetRunControlRequest["verb"] | null
  error: string | null
  objective: string | null
  run: KhalaCodeDesktopFleetRunProjection | null
}>

type FleetView =
  | { readonly phase: "loading" }
  | { readonly phase: "error"; readonly message: string }
  | { readonly phase: "ready"; readonly data: KhalaCodeDesktopFleetStatus }

const el = <K extends keyof HTMLElementTagNameMap>(
  tag: K,
  className?: string,
  text?: string,
): HTMLElementTagNameMap[K] => {
  const node = document.createElement(tag)
  if (className !== undefined) node.className = className
  if (text !== undefined) node.textContent = text
  return node
}

const accountReadinessState = (
  readiness: string,
): "ready" | "missing" | "degraded" => {
  const value = readiness.toLowerCase()
  if (value === "ready") return "ready"
  if (value.includes("credential") || value.includes("missing")) return "missing"
  return "degraded"
}

const titleize = (value: string): string =>
  value.replace(/[_-]+/g, " ").replace(/\b\w/g, char => char.toUpperCase())

const summaryLine = (parts: ReadonlyArray<string | null>): string =>
  parts.filter((part): part is string => Boolean(part)).join("  ·  ")

const isDisplayOnlyDefaultAccountRef = (accountRef: string): boolean =>
  /^(?:\(default\)|default)$/iu.test(accountRef.trim())

const badge = (state: string, label: string): HTMLElement => {
  const node = el("span", "khala-fleet-badge")
  node.dataset.state = state
  node.append(el("span", "khala-fleet-dot"), el("span", undefined, label))
  return node
}

const detailChip = (label: string, value: string): HTMLElement => {
  const chip = el("span", "khala-fleet-chip")
  chip.append(
    el("span", "khala-fleet-chip-label", label),
    el("span", "khala-fleet-chip-value", value),
  )
  return chip
}

const iconButton = (
  label: string,
  icon: IconName,
  className = "khala-fleet-refresh",
): HTMLButtonElement => {
  const button = el("button", className) as HTMLButtonElement
  button.type = "button"
  button.append(
    iconElement(icon, {
      className: "khala-fleet-button-icon",
      dataIcon: label.toLowerCase().replace(/\s+/g, "-"),
    }),
    el("span", "khala-fleet-button-label", label),
  )
  return button
}

const setIconButtonLabel = (
  button: HTMLButtonElement,
  label: string,
): void => {
  const labelNode = button.querySelector<HTMLElement>(".khala-fleet-button-label")
  if (labelNode === null) {
    button.append(el("span", "khala-fleet-button-label", label))
    return
  }
  labelNode.textContent = label
}

const unknownNumber = (value: number | null): string =>
  value === null ? "?" : String(value)

const formatElapsedMs = (elapsedMs: number | null): string | null => {
  if (elapsedMs === null) return null
  const seconds = Math.max(0, Math.round(elapsedMs / 1000))
  const minutes = Math.floor(seconds / 60)
  const remainder = seconds % 60
  return minutes === 0 ? `${remainder}s` : `${minutes}m ${remainder}s`
}

const elapsedSince = (
  startedAt: string | null,
  updatedAt: string,
  observedAt: string | null,
): string => {
  const start = Date.parse(startedAt ?? updatedAt)
  const observed = Date.parse(observedAt ?? updatedAt)
  const updated = Date.parse(updatedAt)
  if (!Number.isFinite(start) || !Number.isFinite(observed) || !Number.isFinite(updated)) return "unknown"
  const end = Math.max(observed, updated)
  return formatElapsedMs(Math.max(0, end - start)) ?? "unknown"
}

const fleetRunStateBadge = (
  state: KhalaCodeDesktopFleetRunProjection["state"],
): "online" | "ready" | "missing" | "degraded" => {
  if (state === "running") return "online"
  if (state === "completed") return "ready"
  if (state === "stopped") return "missing"
  return "degraded"
}

const activeFleetRunSortScore = (
  state: KhalaCodeDesktopFleetRunProjection["state"],
): number => {
  if (state === "running") return 0
  if (state === "paused") return 1
  if (state === "draining") return 2
  if (state === "draft") return 3
  return 4
}

const selectActiveFleetRun = (
  runs: readonly KhalaCodeDesktopFleetRunProjection[],
): KhalaCodeDesktopFleetRunProjection | null => {
  const active = runs
    .filter(run => run.state === "running" || run.state === "paused" || run.state === "draining")
    .sort((left, right) => {
      const score = activeFleetRunSortScore(left.state) - activeFleetRunSortScore(right.state)
      if (score !== 0) return score
      return Date.parse(right.updatedAt) - Date.parse(left.updatedAt)
    })
  return active[0] ?? null
}

const runBacklogRemaining = (
  run: KhalaCodeDesktopFleetRunProjection,
): number => Math.max(
  0,
  run.counters.workUnitsTotal
    - run.counters.activeAssignments
    - run.counters.completedAssignments
    - run.counters.failedAssignments
    - run.counters.blockedAssignments,
)

const accountCapacityLabel = (
  capacity: KhalaCodeDesktopFleetAccount["capacity"],
): string | null => {
  if (capacity === null) return null
  return `${unknownNumber(capacity.available)}/${unknownNumber(capacity.ready)} free`
}

export const khalaFleetCountdownLabel = (resetsAtIso: string | null, now: Date): string | null => {
  if (resetsAtIso === null) return null
  const deltaMs = Date.parse(resetsAtIso) - now.getTime()
  if (!Number.isFinite(deltaMs)) return null
  if (deltaMs <= 0) return "now"
  const totalMinutes = Math.ceil(deltaMs / 60_000)
  const days = Math.floor(totalMinutes / 1440)
  const hours = Math.floor((totalMinutes % 1440) / 60)
  const minutes = totalMinutes % 60
  if (days > 0) return hours === 0 ? `${days}d` : `${days}d ${hours}h`
  if (hours > 0) return minutes === 0 ? `${hours}h` : `${hours}h ${minutes}m`
  return `${minutes}m`
}

const rateLimitWindowLabel = (
  window: NonNullable<KhalaCodeDesktopFleetAccount["rateLimits"]>["session"],
  now: Date,
): string | null => {
  if (window === null) return null
  const reset = khalaFleetCountdownLabel(window.resetsAtIso, now)
  return `${Math.round(window.usedPercent)}% used / ${Math.round(window.remainingPercent)}% remaining${
    reset === null ? "" : `, resets in ${reset}`
  }`
}

const sessionRoleLabel = (
  role: KhalaCodeDesktopFleetAccount["sessionRole"],
): string | null => {
  if (role === undefined) return null
  return role === "main_local_codex_session" ? "primary user session" : "worker Codex session"
}

const homeRoleLabel = (
  role: KhalaCodeDesktopFleetAccount["homeRole"],
): string | null => {
  if (role === undefined) return null
  return role === "main_user_codex_home_display_only"
    ? "primary Codex home display-only"
    : "isolated worker Codex home"
}

const queuePolicyLabel = (
  policy: KhalaCodeDesktopFleetAccount["queuePolicy"],
): string | null => {
  if (policy === undefined) return null
  return `refill ${titleize(policy.refill)}, queued ${unknownNumber(policy.queued)}`
}

const fleetTokenRateLabel = (
  tokenRate: KhalaCodeDesktopFleetStatus["tokenRate"],
): string => {
  if (tokenRate.source === "unavailable") return "not measured"
  if (tokenRate.completedStatus === "pending") return "pending exact rows"
  if (tokenRate.completedStatus === "not_measured") return "not measured"
  if (tokenRate.completedTokensPerMinute === null) return titleize(tokenRate.completedStatus)
  return `${tokenRate.completedTokensPerMinute}/min ${tokenRate.completedStatus}`
}

const fleetInFlightLabel = (
  tokenRate: KhalaCodeDesktopFleetStatus["tokenRate"],
): string | null => {
  if (tokenRate.inFlightTokens === null) return null
  const rate = tokenRate.inFlightTokensPerMinute === null
    ? ""
    : `, ${tokenRate.inFlightTokensPerMinute}/min`
  return `${tokenRate.inFlightTokens} token(s)${rate}`
}

const workerStateBadge = (
  state: KhalaFleetWorkerCard["neutralState"],
): "ready" | "online" | "missing" | "degraded" => {
  if (state === "done") return "ready"
  if (state === "working" || state === "queued") return "online"
  if (state === "failed" || state === "offline") return "missing"
  return "degraded"
}

const renderWorkerCard = (
  card: KhalaFleetWorkerCard,
  activeRun: ActiveFleetRunView,
  handlers: Handlers,
): HTMLElement => {
  const row = el("article", "khala-fleet-worker-card")
  row.dataset.state = card.neutralState
  row.dataset.workerRefHash = card.workerRefHash

  const top = el("div", "khala-fleet-worker-card-top")
  top.append(
    badge(workerStateBadge(card.neutralState), titleize(card.neutralState)),
    detailChip("worker", card.workerRefHash),
  )
  row.append(top)

  const chips = el("div", "khala-fleet-chips")
  chips.append(detailChip("work", card.claimedWorkUnit))
  appendChip(chips, "assignment", card.assignmentRefHash)
  appendChip(chips, "issue", card.issueRefHash)
  appendChip(chips, "elapsed", formatElapsedMs(card.elapsedMs))
  appendChip(chips, "tokens", card.tokenLabel)
  appendChip(chips, "closeout", card.closeoutStatus)
  if (card.blockerRefs.length > 0) {
    chips.append(detailChip("blockers", card.blockerRefs.slice(0, 3).join(", ")))
  }
  row.append(chips)

  const lifecycle = el("p", "khala-fleet-worker-lifecycle")
  lifecycle.dataset.hasFrame = card.lifecycle === null ? "false" : "true"
  lifecycle.textContent = card.lifecycle?.line ?? "No lifecycle frame received yet."
  row.append(lifecycle)

  const controls = el("div", "khala-fleet-worker-controls")
  const actionSpecs: ReadonlyArray<readonly [
    KhalaCodeDesktopFleetWorkerControlRequest["verb"],
    IconName,
  ]> = [
    ["interrupt", "Stop"],
    ["retry", "Reload"],
    ["flag", "Flag"],
  ]
  for (const [verb, icon] of actionSpecs) {
    const button = iconButton(titleize(verb), icon, verb === "interrupt" ? "khala-fleet-run khala-fleet-run-danger" : "khala-fleet-run")
    button.dataset.fleetWorkerControl = verb
    button.disabled = activeRun.run === null || (card.assignmentRef === null && verb !== "flag")
    button.addEventListener("click", () => handlers.onFleetWorkerControl(card, verb))
    controls.append(button)
  }
  row.append(controls)
  return row
}

const appendChip = (
  container: HTMLElement,
  label: string,
  value: string | null,
): void => {
  if (value !== null) container.append(detailChip(label, value))
}

const sectionHeader = (title: string, meta?: string): HTMLElement => {
  const header = el("div", "khala-fleet-section-header")
  header.append(el("h3", "khala-fleet-section-title", title))
  if (meta !== undefined) header.append(el("span", "khala-fleet-section-meta", meta))
  return header
}

const appendFleetBoard = (
  container: HTMLElement,
  data: KhalaCodeDesktopFleetStatus,
): void => {
  const projection = buildKhalaFleetBoardProjection({ status: data })
  const template = document.createElement("template")
  template.innerHTML = renderKhalaFleetBoardHtml(projection, {
    reducedMotion:
      typeof matchMedia === "function" &&
      matchMedia("(prefers-reduced-motion: reduce)").matches,
  }).html
  container.append(template.content.cloneNode(true))
}

const optionalText = (value: string): string | undefined => {
  const trimmed = value.trim()
  return trimmed.length === 0 ? undefined : trimmed
}

const delegateRunStatusLabel = (
  result: KhalaCodeDesktopFleetDelegateRunResult,
): string => `${result.acceptedCount}/${result.requestedCount} accepted`

const delegateStepState = (
  status: KhalaCodeDesktopFleetDelegateRunResult["trace"][number]["status"],
): "blocked" | "recovered" | "satisfied" => {
  if (status === "blocked") return "blocked"
  if (status === "recovered") return "recovered"
  return "satisfied"
}

const textInput = (
  name: keyof FleetDelegateFormState,
  label: string,
  value: string,
  handlers: Handlers,
  options: Readonly<{
    disabled?: boolean
    placeholder?: string
    type?: "number" | "text"
  }> = {},
): HTMLElement => {
  const wrapper = el("label", "khala-fleet-delegate-field")
  wrapper.append(el("span", "khala-fleet-chip-label", label))
  const input = el("input", "khala-fleet-delegate-input") as HTMLInputElement
  input.name = String(name)
  input.type = options.type ?? "text"
  input.value = value
  input.disabled = options.disabled === true
  if (options.placeholder !== undefined) input.placeholder = options.placeholder
  if (input.type === "number") {
    input.min = "1"
    input.max = "10"
  }
  input.addEventListener("input", () => handlers.onDelegateField(name, input.value))
  wrapper.append(input)
  return wrapper
}

const fleetRunTextInput = (
  name: keyof FleetRunFormState,
  label: string,
  value: string,
  handlers: Handlers,
  options: Readonly<{
    placeholder?: string
    type?: "number" | "text"
  }> = {},
): HTMLElement => {
  const wrapper = el("label", "khala-fleet-delegate-field")
  wrapper.append(el("span", "khala-fleet-chip-label", label))
  const input = el("input", "khala-fleet-delegate-input") as HTMLInputElement
  input.name = String(name)
  input.type = options.type ?? "text"
  input.value = value
  if (options.placeholder !== undefined) input.placeholder = options.placeholder
  if (input.type === "number") input.min = "1"
  input.addEventListener("input", () => handlers.onFleetRunField(name, input.value))
  wrapper.append(input)
  return wrapper
}

const fleetRunSelect = <T extends string>(
  name: keyof FleetRunFormState,
  label: string,
  value: T,
  options: readonly (readonly [T, string])[],
  handlers: Handlers,
): HTMLElement => {
  const wrapper = el("label", "khala-fleet-delegate-field")
  wrapper.append(el("span", "khala-fleet-chip-label", label))
  const select = el("select", "khala-fleet-delegate-input") as HTMLSelectElement
  select.name = String(name)
  for (const [optionValue, optionLabel] of options) {
    const option = el("option") as HTMLOptionElement
    option.value = optionValue
    option.textContent = optionLabel
    option.selected = value === optionValue
    select.append(option)
  }
  select.addEventListener("change", () => handlers.onFleetRunField(name, select.value))
  wrapper.append(select)
  return wrapper
}

const renderDelegateResult = (
  container: HTMLElement,
  run: DelegateRunView,
): void => {
  if (run.phase === "idle") return
  const output = el("div", "khala-fleet-delegate-output")
  if (run.phase === "loading") {
    output.dataset.state = "loading"
    output.append(
      badge("online", "Running"),
      el("p", "khala-fleet-empty", "Executing khala.fleet.delegate…"),
    )
    container.append(output)
    return
  }
  if (run.phase === "error") {
    output.dataset.state = "blocked"
    output.append(
      badge("missing", "Blocked"),
      el("p", "khala-fleet-error", run.message),
    )
    container.append(output)
    return
  }

  const { result } = run
  output.dataset.state = result.ok ? "completed" : "blocked"
  const summary = el("div", "khala-fleet-delegate-summary")
  summary.append(
    badge(result.ok ? "online" : "missing", result.delegateStatus),
    detailChip("signature", result.delegateSignature),
    detailChip("accepted", delegateRunStatusLabel(result)),
  )
  if (result.pylonRef !== null) summary.append(detailChip("pylon", result.pylonRef))
  output.append(summary)

  const steps = el("ol", "khala-fleet-delegate-steps")
  for (const step of result.trace) {
    const item = el("li", "khala-fleet-delegate-step")
    item.dataset.status = delegateStepState(step.status)
    const top = el("div", "khala-fleet-delegate-step-top")
    top.append(
      el("strong", undefined, step.module),
      el("span", "khala-fleet-section-meta", step.status),
    )
    item.append(top)
    item.append(el("p", undefined, step.summary))
    const chips = el("div", "khala-fleet-chips")
    chips.append(detailChip("precondition", step.precondition))
    if (step.fallbackModule !== null) chips.append(detailChip("fallback", step.fallbackModule))
    if (step.blockerCode !== null) chips.append(detailChip("blocker", step.blockerCode))
    if (step.refs.length > 0) chips.append(detailChip("refs", step.refs.slice(0, 3).join(", ")))
    item.append(chips)
    steps.append(item)
  }
  output.append(steps)

  if (result.results.length > 0) {
    const slots = el("div", "khala-fleet-delegate-slots")
    for (const slot of result.results) {
      const chips = el("div", "khala-fleet-chips")
      chips.append(detailChip(`slot ${slot.slot}`, slot.status))
      if (slot.accountRef !== null) chips.append(detailChip("account", slot.accountRef))
      if (slot.assignmentRef !== null) chips.append(detailChip("assignment", slot.assignmentRef))
      if (slot.closeoutStatus !== null) chips.append(detailChip("closeout", slot.closeoutStatus))
      if (slot.transcriptRef !== null) chips.append(detailChip("transcript", slot.transcriptRef))
      if (slot.blockerRefs.length > 0) chips.append(detailChip("blockers", slot.blockerRefs.slice(0, 3).join(", ")))
      if (slot.tokensVerified !== null) chips.append(detailChip("tokens", String(slot.tokensVerified)))
      slots.append(chips)
    }
    output.append(slots)
  }

  container.append(output)
}

const renderDelegateRunner = (
  container: HTMLElement,
  form: FleetDelegateFormState,
  run: DelegateRunView,
  handlers: Handlers,
): void => {
  const section = el("section", "khala-fleet-section khala-fleet-delegate")
  section.append(sectionHeader("Delegate run", "khala.fleet.delegate"))

  const formEl = el("form", "khala-fleet-delegate-form") as HTMLFormElement
  formEl.setAttribute("aria-label", "Khala fleet delegate runner")
  formEl.addEventListener("submit", event => {
    event.preventDefault()
    handlers.onDelegateRun()
  })

  const objective = el("textarea", "khala-fleet-delegate-objective") as HTMLTextAreaElement
  objective.name = "objective"
  objective.value = form.objective
  objective.rows = 2
  objective.placeholder = "Bounded objective for a worker Codex session"
  objective.setAttribute("aria-label", "Delegation objective")
  objective.addEventListener("input", () => handlers.onDelegateField("objective", objective.value))
  formEl.append(objective)

  const controls = el("div", "khala-fleet-delegate-grid")
  const mode = el("label", "khala-fleet-delegate-field")
  mode.append(el("span", "khala-fleet-chip-label", "mode"))
  const select = el("select", "khala-fleet-delegate-input") as HTMLSelectElement
  select.name = "mode"
  for (const [value, label] of [["fixture", "Fixture smoke"], ["real_work", "Real pinned"]] as const) {
    const option = el("option") as HTMLOptionElement
    option.value = value
    option.textContent = label
    option.selected = form.mode === value
    select.append(option)
  }
  select.addEventListener("change", () =>
    handlers.onDelegateField("mode", select.value === "real_work" ? "real_work" : "fixture"),
  )
  mode.append(select)
  controls.append(
    mode,
    textInput("count", "count", form.count, handlers, { placeholder: "1", type: "number" }),
    textInput("accountRef", "account", form.accountRef, handlers, { placeholder: "auto" }),
  )
  formEl.append(controls)

  const repoPinsDisabled = form.mode === "fixture"
  const pins = el("div", "khala-fleet-delegate-grid khala-fleet-delegate-pins")
  pins.dataset.disabled = repoPinsDisabled ? "true" : "false"
  pins.append(
    textInput("repo", "repo", form.repo, handlers, { disabled: repoPinsDisabled, placeholder: "OpenAgentsInc/openagents" }),
    textInput("commit", "commit", form.commit, handlers, { disabled: repoPinsDisabled, placeholder: "required SHA" }),
    textInput("branch", "branch", form.branch, handlers, { disabled: repoPinsDisabled, placeholder: "optional" }),
    textInput("verify", "verify", form.verify, handlers, { disabled: repoPinsDisabled, placeholder: "test or proof command" }),
  )
  formEl.append(pins)

  const footer = el("div", "khala-fleet-delegate-footer")
  const dryRun = el("label", "khala-fleet-delegate-check")
  const checkbox = el("input") as HTMLInputElement
  checkbox.name = "noRun"
  checkbox.type = "checkbox"
  checkbox.checked = form.noRun
  checkbox.addEventListener("change", () => handlers.onDelegateField("noRun", checkbox.checked))
  dryRun.append(checkbox, el("span", undefined, "Dry run"))
  const runButton = iconButton(run.phase === "loading" ? "Running" : "Run delegate", "Play", "khala-fleet-run")
  runButton.type = "submit"
  runButton.disabled = run.phase === "loading"
  footer.append(dryRun, runButton)
  formEl.append(footer)

  section.append(formEl)
  renderDelegateResult(section, run)
  container.append(section)
}

const renderFleetRunResult = (
  container: HTMLElement,
  run: FleetRunView,
): void => {
  if (run.phase === "idle") return
  const output = el("div", "khala-fleet-delegate-output")
  if (run.phase === "error") {
    output.dataset.state = "blocked"
    output.append(
      badge("missing", "Blocked"),
      el("p", "khala-fleet-error", run.message),
    )
  } else if (run.phase === "loading") {
    output.dataset.state = "loading"
    output.append(
      badge("online", "Starting"),
      el("p", "khala-fleet-empty", "Starting supervised FleetRun…"),
    )
  } else if (run.phase === "ready") {
    output.dataset.state = run.result.run.state
    const chips = el("div", "khala-fleet-chips")
    chips.append(
      badge("online", titleize(run.result.run.state)),
      detailChip("run", run.result.run.runRef),
      detailChip("target", String(run.result.run.targetConcurrency)),
      detailChip("source", run.result.run.workSource.kind),
      detailChip("supervisor", run.result.supervisorStarted ? "started" : "not started"),
    )
    output.append(chips)
  } else {
    output.dataset.state = "preview"
    output.append(
      badge("ready", "Dry-run preview"),
      el("p", "khala-fleet-empty", "Planned first wave before starting."),
    )
  }

  const slots = "slots" in run ? run.slots : []
  if (slots.length > 0) {
    const list = el("div", "khala-fleet-run-preview")
    for (const slot of slots) {
      const row = el("article", "khala-fleet-run-preview-slot")
      row.append(
        detailChip(`slot ${slot.slot}`, slot.accountRef),
        detailChip("planned", slot.plannedClaimLabel),
        detailChip("worker", slot.workerKind),
      )
      list.append(row)
    }
    output.append(list)
  }
  container.append(output)
}

const renderFleetRunHeader = (
  container: HTMLElement,
  activeRun: ActiveFleetRunView,
  observedAt: string | null,
  handlers: Handlers,
): void => {
  const run = activeRun.run
  if (run === null) return

  const section = el("section", "khala-fleet-section khala-fleet-run-header")
  section.dataset.state = run.state
  section.append(sectionHeader("Active FleetRun", "orchestration store"))

  const objective = activeRun.objective ?? (
    run.objectiveProjected
      ? "Objective projected by the orchestration store."
      : "Objective is not projected by the public-safe run status."
  )
  const objectiveNode = el("p", "khala-fleet-run-objective", objective)

  const chips = el("div", "khala-fleet-chips")
  chips.append(
    badge(fleetRunStateBadge(run.state), titleize(run.state)),
    detailChip("run", run.runRef),
    detailChip("target", String(run.targetConcurrency)),
    detailChip("actual", String(run.counters.activeAssignments)),
    detailChip("remaining", String(runBacklogRemaining(run))),
    detailChip("claimed", String(run.counters.activeAssignments)),
    detailChip("done", String(run.counters.completedAssignments)),
    detailChip("elapsed", elapsedSince(run.startedAt, run.updatedAt, observedAt)),
  )
  if (run.counters.blockedAssignments > 0) {
    chips.append(detailChip("blocked", String(run.counters.blockedAssignments)))
  }
  if (run.counters.failedAssignments > 0) {
    chips.append(detailChip("failed", String(run.counters.failedAssignments)))
  }

  const controls = el("div", "khala-fleet-run-controls")
  const controlSpecs: ReadonlyArray<readonly [
    KhalaCodeDesktopFleetRunControlRequest["verb"],
    IconName,
  ]> = [
    ["pause", "Pause"],
    ["resume", "Play"],
    ["drain", "Circle"],
    ["stop", "Stop"],
  ]
  for (const [verb, icon] of controlSpecs) {
    const label = titleize(verb)
    const button = iconButton(
      activeRun.controlInFlight === verb ? `${label}...` : label,
      icon,
      verb === "stop" ? "khala-fleet-run khala-fleet-run-danger" : "khala-fleet-run",
    )
    button.dataset.fleetRunControl = verb
    button.disabled = activeRun.controlInFlight !== null
    button.addEventListener("click", () => handlers.onFleetRunControl(verb))
    controls.append(button)
  }

  section.append(objectiveNode, chips)
  if (activeRun.error !== null) {
    section.append(el("p", "khala-fleet-error", activeRun.error))
  }
  section.append(controls)
  container.append(section)
}

const renderFleetRunStarter = (
  container: HTMLElement,
  form: FleetRunFormState,
  run: FleetRunView,
  handlers: Handlers,
): void => {
  const section = el("section", "khala-fleet-section khala-fleet-delegate khala-fleet-run-start")
  section.append(sectionHeader("Start fleet run", "supervised FleetRun"))

  const formEl = el("form", "khala-fleet-delegate-form") as HTMLFormElement
  formEl.setAttribute("aria-label", "Start fleet run")
  formEl.addEventListener("submit", event => {
    event.preventDefault()
    handlers.onFleetRunStart()
  })

  const objective = el("textarea", "khala-fleet-delegate-objective") as HTMLTextAreaElement
  objective.name = "objective"
  objective.value = form.objective
  objective.rows = 2
  objective.placeholder = "Fleet objective for sustained fan-out"
  objective.setAttribute("aria-label", "Fleet run objective")
  objective.addEventListener("input", () => handlers.onFleetRunField("objective", objective.value))
  formEl.append(objective)

  const controls = el("div", "khala-fleet-delegate-grid")
  controls.append(
    fleetRunSelect(
      "workSource",
      "source",
      form.workSource,
      [
        ["github_backlog", "GitHub backlog"],
        ["issue_list", "Issue list"],
        ["fixture", "Fixture"],
      ],
      handlers,
    ),
    fleetRunTextInput("targetConcurrency", "target", form.targetConcurrency, handlers, {
      placeholder: "25",
      type: "number",
    }),
    fleetRunSelect(
      "workerKind",
      "worker",
      form.workerKind,
      [
        ["codex", "Codex"],
        ["claude", "Claude"],
        ["auto", "Auto"],
      ],
      handlers,
    ),
  )
  formEl.append(controls)

  const footer = el("div", "khala-fleet-delegate-footer")
  const preview = iconButton("Preview first wave", "Eye", "khala-fleet-refresh")
  preview.addEventListener("click", handlers.onFleetRunPreview)
  const start = iconButton(run.phase === "loading" ? "Starting" : "Start fleet run", "Play", "khala-fleet-run")
  start.type = "submit"
  start.disabled = run.phase === "loading"
  footer.append(preview, start)
  formEl.append(footer)

  section.append(formEl)
  renderFleetRunResult(section, run)
  container.append(section)
}

const optimizationState = (
  phase: KhalaGymDelegationOptimizationRun["phase"],
): "missing" | "online" | "ready" => {
  if (phase === "blocked") return "missing"
  if (phase === "queued" || phase === "running") return "online"
  return "ready"
}

const appendOptimizationRunChips = (
  container: HTMLElement,
  run: KhalaGymDelegationOptimizationRun,
): void => {
  appendChip(container, "run", run.runRef)
  appendChip(container, "stage", titleize(run.stage))
  appendChip(container, "metric", run.metricValueBps === undefined ? null : `${run.metricValueBps} bps`)
  appendChip(container, "candidate", run.candidateManifestRef ?? run.candidateRef ?? null)
  appendChip(container, "admission", run.admissionDecision ?? null)
  appendChip(container, "proposal", run.actionSubmissionProposalRef ?? null)
  if (run.blockerRefs.length > 0) {
    appendChip(container, "blockers", run.blockerRefs.slice(0, 3).join(", "))
  }
}

const appendActiveParameterChips = (
  container: HTMLElement,
  run: KhalaGymDelegationOptimizationRun | null,
): void => {
  const activeParameters =
    run?.activeParameters ?? defaultKhalaFleetDelegationActiveParameters
  appendChip(container, "active source", activeParameters.source)
  appendChip(container, "parameters", activeParameters.parameterRef)
  appendChip(container, "candidate", activeParameters.candidateRef ?? null)
  appendChip(
    container,
    "proposal",
    activeParameters.actionSubmissionProposalRef ?? null,
  )
}

const renderOptimizationRunner = (
  container: HTMLElement,
  run: OptimizationRunView,
  handlers: Handlers,
): void => {
  const section = el("section", "khala-fleet-section khala-fleet-optimization")
  section.append(sectionHeader("Delegation optimization", "khala-code-delegation-gepa"))

  const actions = el("div", "khala-fleet-optimization-actions")
  const start = iconButton(
    run.phase === "loading" ? "Starting" : "Optimize delegation policy",
    "Play",
    "khala-fleet-run",
  )
  start.disabled = run.phase === "loading"
  start.addEventListener("click", handlers.onOptimizationStart)
  const load = iconButton("Load demo proof", "Eye", "khala-fleet-refresh")
  load.disabled = run.phase === "loading"
  load.addEventListener("click", handlers.onLoadGymDemoProof)
  actions.append(start, load)
  section.append(actions)

  const parameterChips = el("div", "khala-fleet-chips khala-fleet-parameter-readout")
  appendActiveParameterChips(
    parameterChips,
    run.phase === "ready" ? run.result : null,
  )
  section.append(parameterChips)

  if (run.phase === "idle") {
    section.append(
      el("p", "khala-fleet-empty", "No optimization run loaded."),
    )
  } else if (run.phase === "loading") {
    const output = el("div", "khala-fleet-delegate-output")
    output.dataset.state = "loading"
    output.append(
      badge("online", "Queued"),
      el("p", "khala-fleet-empty", "Creating khala-code-delegation-gepa run…"),
    )
    section.append(output)
  } else if (run.phase === "error") {
    const output = el("div", "khala-fleet-delegate-output")
    output.dataset.state = "blocked"
    output.append(
      badge("missing", "Blocked"),
      el("p", "khala-fleet-error", run.message),
    )
    section.append(output)
  } else {
    const output = el("div", "khala-fleet-delegate-output khala-fleet-optimization-output")
    output.dataset.state = run.result.phase
    const chips = el("div", "khala-fleet-chips")
    chips.append(
      badge(optimizationState(run.result.phase), titleize(run.result.phase)),
    )
    appendOptimizationRunChips(chips, run.result)
    section.append(output)
    output.append(chips)
    if (run.result.datasetRefs.length > 0) {
      const refs = el("div", "khala-fleet-chips")
      refs.append(detailChip("dataset", run.result.datasetRefs.slice(0, 3).join(", ")))
      output.append(refs)
    }
  }

  container.append(section)
}

const accountCard = (
  account: KhalaCodeDesktopFleetAccount,
  handlers: Handlers,
): HTMLElement => {
  const now = new Date()
  const state = accountReadinessState(account.readiness)
  const card = el("article", "khala-fleet-account")
  card.dataset.state = state
  card.dataset.accountRef = account.accountRef

  const identity = el("div", "khala-fleet-account-identity")
  const top = el("div", "khala-fleet-account-top")
  top.append(
    el(
      "strong",
      undefined,
      isDisplayOnlyDefaultAccountRef(account.accountRef) ? "default" : account.accountRef,
    ),
  )
  top.append(el("span", "khala-fleet-provider", account.provider))
  identity.append(top)
  identity.append(el("span", "khala-fleet-email", account.email ?? "not signed in"))
  card.append(identity)

  if (account.paused) {
    card.append(badge("degraded", "Paused"))
  } else if (state === "ready") {
    card.append(badge("ready", "Ready"))
  } else {
    const reconnect = iconButton("Reconnect", "Reload", "khala-fleet-reconnect")
    reconnect.dataset.state = state
    reconnect.title = `Reconnect ${account.accountRef}`
    reconnect.addEventListener("click", () => handlers.onConnect(account.accountRef))
    card.append(reconnect)
  }

  const remove = iconButton("Remove", "Trash", "khala-fleet-delete")
  remove.title = `Remove ${account.accountRef}`
  remove.setAttribute("aria-label", `Remove account ${account.accountRef}`)
  let armed = false
  let armTimer = 0
  remove.addEventListener("click", () => {
    if (!armed) {
      armed = true
      remove.replaceChildren(el("span", "khala-fleet-button-label", "Remove?"))
      remove.dataset.armed = "true"
      armTimer = window.setTimeout(() => {
        armed = false
        remove.replaceChildren(
          iconElement("Trash", {
            className: "khala-fleet-button-icon",
            dataIcon: "remove-account",
          }),
          el("span", "khala-fleet-button-label", "Remove"),
        )
        delete remove.dataset.armed
      }, 3000)
      return
    }
    window.clearTimeout(armTimer)
    handlers.onRemove(account.accountRef)
  })
  card.append(remove)

  if (!isDisplayOnlyDefaultAccountRef(account.accountRef)) {
    const pause = iconButton(
      account.paused ? "Resume planning" : "Pause account",
      account.paused ? "Play" : "Pause",
      "khala-fleet-reconnect khala-fleet-pause",
    )
    pause.title = account.paused
      ? `Include ${account.accountRef} in planning`
      : `Exclude ${account.accountRef} from planning without disconnecting`
    pause.addEventListener("click", () => handlers.onPauseAccount(account.accountRef, !account.paused))
    card.append(pause)
  }

  const details = el("div", "khala-fleet-card-details")
  if (isDisplayOnlyDefaultAccountRef(account.accountRef)) {
    appendChip(details, "routing", "default slot")
  }
  appendChip(details, "readiness", titleize(account.readiness))
  appendChip(details, "slots", accountCapacityLabel(account.capacity))
  appendChip(details, "role", sessionRoleLabel(account.sessionRole))
  appendChip(details, "home", homeRoleLabel(account.homeRole))
  appendChip(details, "queue", queuePolicyLabel(account.queuePolicy))
  appendChip(details, "cooldown", account.queuePolicy?.cooldown ?? null)
  appendChip(details, "session limit", rateLimitWindowLabel(account.rateLimits?.session ?? null, now))
  appendChip(details, "weekly limit", rateLimitWindowLabel(account.rateLimits?.weekly ?? null, now))
  if (account.rateLimits?.rateLimitResetCredits !== undefined) {
    const credits = account.rateLimits.rateLimitResetCredits
    appendChip(details, "reset credits", credits === null ? "unavailable" : String(credits.availableCount))
  }
  if (account.capacity !== null) {
    appendChip(
      details,
      "busy",
      account.capacity.busy === null ? null : String(account.capacity.busy),
    )
    appendChip(
      details,
      "queued",
      account.capacity.queued === null ? null : String(account.capacity.queued),
    )
  }
  appendChip(details, "quota", account.quotaState)
  card.append(details)

  if (
    account.rateLimits?.rateLimitResetCredits !== undefined &&
    account.rateLimits.rateLimitResetCredits !== null &&
    account.rateLimits.rateLimitResetCredits.availableCount > 0
  ) {
    const reset = iconButton("Reset credits", "Reload", "khala-fleet-reconnect")
    reset.title = "Consume an available Codex rate-limit reset credit"
    reset.addEventListener("click", () => handlers.onConsumeResetCredit(account.accountRef))
    card.append(reset)
  }

  return card
}

const renderReady = (
  container: HTMLElement,
  data: KhalaCodeDesktopFleetStatus,
  handlers: Handlers,
  activeRun: ActiveFleetRunView,
  lifecycleFrames: readonly KhalaFleetWorkerLifecycleFrame[],
): void => {
  const readyAccounts = data.accounts.filter(
    account => accountReadinessState(account.readiness) === "ready",
  ).length
  const needsReconnect = data.accounts.length - readyAccounts
  const capacity =
    data.availableCodexAssignments === null || data.maxCodexAssignments === null
      ? null
      : `${data.availableCodexAssignments}/${data.maxCodexAssignments} Codex slots free`

  appendFleetBoard(container, data)

  if (data.sessionLayers !== undefined) {
    const sessionSection = el("section", "khala-fleet-section")
    sessionSection.append(sectionHeader("Codex session boundaries", "primary user plus isolated workers"))
    const list = el("div", "khala-fleet-account-list")
    for (const layer of [data.sessionLayers.main, data.sessionLayers.workers]) {
      const row = el("article", "khala-fleet-session")
      row.dataset.role = layer.role
      const top = el("div", "khala-fleet-account-top")
      top.append(el("strong", undefined, layer.label))
      top.append(el("span", "khala-fleet-provider", titleize(layer.role)))
      row.append(top)
      const details = el("div", "khala-fleet-card-details")
      appendChip(details, "runtime", titleize(layer.runtime))
      appendChip(details, "home", titleize(layer.homeRole))
      appendChip(details, "transcripts", titleize(layer.transcriptSurface))
      appendChip(details, "policy", titleize(layer.mutationPolicy))
      row.append(details)
      list.append(row)
    }
    sessionSection.append(list)
    container.append(sessionSection)
  }

  // Pylon
  const pylonSection = el("section", "khala-fleet-section")
  pylonSection.append(sectionHeader("Pylon"))
  const pylonCard = el("article", "khala-fleet-pylon")
  const pylonState = data.pylon.status === "unavailable" ? "stale" : "online"
  pylonCard.dataset.state = pylonState
  const pylonId = el("div", "khala-fleet-pylon-identity")
  pylonId.append(
    el("strong", undefined, data.pylon.pylonRef ?? "local Pylon"),
    el("span", "khala-fleet-pylon-message", data.pylon.message),
  )
  pylonCard.append(pylonId)
  pylonCard.append(badge(pylonState, titleize(data.pylon.status)))
  const pylonDetails = el("div", "khala-fleet-card-details")
  appendChip(pylonDetails, "slots", capacity)
  appendChip(pylonDetails, "token rate", fleetTokenRateLabel(data.tokenRate))
  appendChip(pylonDetails, "in flight", fleetInFlightLabel(data.tokenRate))
  pylonCard.append(pylonDetails)
  pylonSection.append(pylonCard)
  container.append(pylonSection)

  // Accounts
  const accountsSection = el("section", "khala-fleet-section")
  accountsSection.append(
    sectionHeader(
      "Worker Codex accounts",
      summaryLine([
        `${readyAccounts} ready`,
        needsReconnect > 0 ? `${needsReconnect} need reconnect` : null,
      ]),
    ),
  )
  if (data.accounts.length === 0) {
    accountsSection.append(
      el("p", "khala-fleet-empty", "No worker Codex accounts linked yet. Connect account creates an isolated Pylon Codex home."),
    )
  } else {
    const list = el("div", "khala-fleet-account-list")
    for (const account of data.accounts) list.append(accountCard(account, handlers))
    accountsSection.append(list)
  }
  container.append(accountsSection)

  // Active assignments
  const activeSection = el("section", "khala-fleet-section")
  const workerCards = buildKhalaFleetWorkerCards(data, lifecycleFrames)
  activeSection.append(
    sectionHeader("Worker cards", `${workerCards.length} active`),
  )
  if (workerCards.length === 0) {
    activeSection.append(
      el("p", "khala-fleet-empty", "No active Codex assignments right now."),
    )
  } else {
    const list = el("div", "khala-fleet-worker-card-list")
    for (const card of workerCards) list.append(renderWorkerCard(card, activeRun, handlers))
    activeSection.append(list)
  }
  container.append(activeSection)

  if (data.processes.length > 0) {
    const procSection = el("section", "khala-fleet-section")
    procSection.append(
      sectionHeader("Codex processes", `${data.processes.length} running`),
    )
    const list = el("div", "khala-fleet-chips")
    for (const process of data.processes) {
      list.append(detailChip(`pid ${process.pid}`, process.elapsed))
    }
    procSection.append(list)
    container.append(procSection)
  }
}

const renderConnecting = (
  container: HTMLElement,
  connect: ConnectView,
  handlers: Handlers,
): void => {
  const section = el("section", "khala-fleet-connect")
  section.append(el("h3", "khala-fleet-connect-title", `Connecting ${connect.accountRef}`))

  if (connect.start === null) {
    section.append(
      el("p", "khala-fleet-empty", "Starting Codex device login…"),
    )
    container.append(section)
    return
  }

  if (!connect.start.ok) {
    section.append(
      el(
        "p",
        "khala-fleet-error",
        `Could not start device login: ${connect.start.error ?? "unknown error"}`,
      ),
    )
  } else {
    section.append(
      el(
        "p",
        "khala-fleet-connect-hint",
        "Open the link below in your browser and enter the code to sign in an isolated worker Codex home. This does not touch the primary user Codex session.",
      ),
    )
    if (connect.start.verificationUrl !== null) {
      const urlRow = el("div", "khala-fleet-connect-row")
      urlRow.append(el("span", "khala-fleet-chip-label", "url"))
      const verificationUrl = connect.start.verificationUrl
      const link = el("a", "khala-fleet-connect-url", verificationUrl)
      link.href = verificationUrl
      link.addEventListener("click", event => {
        event.preventDefault()
        handlers.onOpenUrl(verificationUrl)
      })
      urlRow.append(link)
      section.append(urlRow)
    }
    if (connect.start.userCode !== null) {
      const codeRow = el("div", "khala-fleet-connect-row")
      codeRow.append(el("span", "khala-fleet-chip-label", "code"))
      codeRow.append(el("code", "khala-fleet-connect-code", connect.start.userCode))
      section.append(codeRow)
    }
    if (connect.start.verificationUrl === null && connect.start.userCode === null) {
      section.append(
        el(
          "pre",
          "khala-fleet-connect-output",
          connect.start.output || "Waiting for the device-login prompt…",
        ),
      )
    }
    section.append(el("p", "khala-fleet-connect-status", "Waiting for authorization…"))
  }

  const close = el("button", "khala-fleet-connect-cancel", "Cancel")
  close.type = "button"
  close.addEventListener("click", handlers.onCancelConnect)
  section.append(close)
  container.append(section)
}

const render = (
  container: HTMLElement,
  view: FleetView,
  handlers: Handlers,
  activeConnect: ConnectView | null,
  delegateForm: FleetDelegateFormState,
  delegateRun: DelegateRunView,
  activeRun: ActiveFleetRunView,
  lifecycleFrames: readonly KhalaFleetWorkerLifecycleFrame[],
  fleetRunForm: FleetRunFormState,
  fleetRun: FleetRunView,
  optimizationRun: OptimizationRunView,
): void => {
  container.replaceChildren()

  const header = el("header", "khala-fleet-header")
  header.append(el("h2", "khala-fleet-title", "Fleet status"))
  const actions = el("div", "khala-fleet-actions")
  const connectBtn = iconButton("Connect account", "Plus")
  connectBtn.disabled = activeConnect !== null
  connectBtn.addEventListener("click", () => {
    // Auto-assign a short, unique ref — no name prompt.
    handlers.onConnect(`codex-${crypto.randomUUID().slice(0, 8)}`)
  })
  actions.append(connectBtn)
  const refresh = iconButton("Refresh", "Reload")
  refresh.dataset.fleetAction = "refresh"
  refresh.disabled = view.phase === "loading"
  refresh.addEventListener("click", handlers.onRefresh)
  actions.append(refresh)
  header.append(actions)
  container.append(header)

  const body = el("div", "khala-fleet-body")
  // The connect device-auth card renders inline at the top; the fleet list stays
  // visible and live below it.
  if (activeConnect !== null) renderConnecting(body, activeConnect, handlers)
  renderDelegateRunner(body, delegateForm, delegateRun, handlers)
  renderFleetRunHeader(body, activeRun, view.phase === "ready" ? view.data.observedAt : null, handlers)
  renderFleetRunStarter(body, fleetRunForm, fleetRun, handlers)
  renderOptimizationRunner(body, optimizationRun, handlers)
  if (view.phase === "loading") {
    body.append(el("p", "khala-fleet-empty", "Inspecting Codex fleet…"))
  } else if (view.phase === "error") {
    body.append(
      el("p", "khala-fleet-error", `Could not load fleet status: ${view.message}`),
    )
  } else {
    renderReady(body, view.data, handlers, activeRun, lifecycleFrames)
  }
  container.append(body)
}

export const mountFleetPanel = (
  container: HTMLElement,
  options: FleetPanelOptions,
): FleetPanelHandle => {
  let inFlight = false
  let delegateInFlight = false
  let delegateForm: FleetDelegateFormState = {
    accountRef: "",
    branch: "",
    commit: "",
    count: "1",
    mode: "fixture",
    noRun: true,
    objective: "Test delegating a bounded analysis task to one Codex worker. Do not change code.",
    repo: "",
    verify: "",
  }
  let delegateRun: DelegateRunView = { phase: "idle" }
  let fleetRunForm: FleetRunFormState = {
    objective: "Run the public-safe fixture backlog through the Codex fleet.",
    targetConcurrency: "2",
    workerKind: "codex",
    workSource: "fixture",
  }
  let fleetRun: FleetRunView = { phase: "idle" }
  let fleetRunInFlight = false
  let activeRun: ActiveFleetRunView = {
    controlInFlight: null,
    error: null,
    objective: null,
    run: null,
  }
  const objectiveByRunRef = new Map<string, string>()
  let optimizationInFlight = false
  let optimizationRun: OptimizationRunView = { phase: "idle" }
  let lastData: KhalaCodeDesktopFleetStatus | null = null
  let lifecycleFrames: readonly KhalaFleetWorkerLifecycleFrame[] = []
  let lifecycleStarted = false
  let connectPoll = 0
  let activeConnect: ConnectView | null = null
  let visible = false
  let pollTimer = 0

  const setRefreshBusy = (busy: boolean): void => {
    const buttons = container.querySelectorAll<HTMLButtonElement>('[data-fleet-action="refresh"]')
    for (const button of buttons) {
      button.disabled = busy
      setIconButtonLabel(button, busy ? "Refreshing" : "Refresh")
    }
  }

  const currentView = (): FleetView =>
    lastData !== null
      ? { phase: "ready", data: lastData }
      : { phase: "loading" }

  const paint = (): void =>
    render(
      container,
      currentView(),
      handlers,
      activeConnect,
      delegateForm,
      delegateRun,
      activeRun,
      lifecycleFrames,
      fleetRunForm,
      fleetRun,
      optimizationRun,
    )

  const fleetRunPreview = (): readonly FleetRunPreviewSlot[] | string => {
    const targetConcurrency = Number.parseInt(fleetRunForm.targetConcurrency, 10)
    if (!Number.isInteger(targetConcurrency) || targetConcurrency < 1) {
      return "Fleet run target concurrency must be a positive integer."
    }
    if (lastData === null) return "Fleet status must load before previewing a run."
    const accounts = lastData.accounts.filter(
      account => accountReadinessState(account.readiness) === "ready" && account.paused !== true,
    )
    if (accounts.length === 0) return "No ready worker accounts are available for the first wave."
    const slots: FleetRunPreviewSlot[] = []
    for (const account of accounts) {
      const available = account.capacity?.available ?? 1
      const slotCount = Math.max(0, available)
      for (let index = 0; index < slotCount && slots.length < targetConcurrency; index += 1) {
        const slot = slots.length + 1
        slots.push({
          accountRef: account.accountRef,
          plannedClaimLabel: `planned claim #${slot} (${fleetRunForm.workSource})`,
          slot,
          workerKind: fleetRunForm.workerKind,
        })
      }
      if (slots.length >= targetConcurrency) break
    }
    if (slots.length === 0) return "No available worker slots are available for the first wave."
    return slots
  }

  const fleetRunRequest = (): KhalaCodeDesktopFleetRunStartRequest | string => {
    const objective = fleetRunForm.objective.trim()
    if (objective.length === 0) return "Fleet run requires an objective."
    const targetConcurrency = Number.parseInt(fleetRunForm.targetConcurrency, 10)
    if (!Number.isInteger(targetConcurrency) || targetConcurrency < 1) {
      return "Fleet run target concurrency must be a positive integer."
    }
    if (fleetRunForm.workerKind !== "codex") {
      return `${titleize(fleetRunForm.workerKind)} FleetRun starts are accepted by the form but only Codex is wired in this build.`
    }
    return {
      objective,
      targetConcurrency,
      workerKind: "codex",
      workSource: {
        kind: fleetRunForm.workSource,
      },
    }
  }

  const delegateRequest = (): KhalaCodeDesktopFleetDelegateRunRequest | string => {
    const objective = delegateForm.objective.trim()
    if (objective.length === 0) return "Delegate run requires an objective."
    const count = Number.parseInt(delegateForm.count, 10)
    if (!Number.isInteger(count) || count < 1 || count > 10) {
      return "Delegate run count must be between 1 and 10."
    }
    const accountRef = optionalText(delegateForm.accountRef)
    const branch = optionalText(delegateForm.branch)
    const commit = optionalText(delegateForm.commit)
    const repo = optionalText(delegateForm.repo)
    const verify = optionalText(delegateForm.verify)
    const request: KhalaCodeDesktopFleetDelegateRunRequest = {
      ...(accountRef === undefined ? {} : { accountRef }),
      ...(branch === undefined ? {} : { branch }),
      ...(commit === undefined ? {} : { commit }),
      count,
      mode: delegateForm.mode,
      noRun: delegateForm.noRun,
      objective,
      ...(repo === undefined ? {} : { repo }),
      ...(verify === undefined ? {} : { verify }),
    }
    if (request.mode === "real_work") {
      const missing = [
        request.repo === undefined ? "repo" : null,
        request.commit === undefined ? "commit" : null,
        request.verify === undefined ? "verify" : null,
      ].filter((value): value is string => value !== null)
      if (missing.length > 0) {
        return `Real-work mode requires repo, commit, and verify pins before dispatch; missing ${missing.join(", ")}.`
      }
    }
    return request
  }

  const lifecycleThrottler = createKhalaFleetWorkerCardThrottler({
    intervalMs: options.lifecycleUpdateThrottleMs ?? 200,
    onUpdate: update => {
      lifecycleFrames = update.frames
      paint()
    },
  })

  const startLifecycleStream = (): void => {
    if (lifecycleStarted || options.lifecycleNdjson === undefined) return
    lifecycleStarted = true
    void consumeKhalaFleetWorkerLifecycleNdjson(
      options.lifecycleNdjson(),
      frame => lifecycleThrottler.push(frame),
    )
  }

  const handlers: Handlers = {
    onDelegateField: (field, value) => {
      if (field === "noRun") {
        delegateForm = {
          ...delegateForm,
          noRun: value === true,
        }
        return
      }
      if (typeof value !== "string") return
      delegateForm = {
        ...delegateForm,
        [field]: value,
      } as FleetDelegateFormState
      if (field === "mode") paint()
    },
    onDelegateRun: () => onDelegateRun(),
    onFleetRunField: (field, value) => {
      fleetRunForm = {
        ...fleetRunForm,
        [field]: value,
      } as FleetRunFormState
      if (fleetRun.phase !== "loading") fleetRun = { phase: "idle" }
      paint()
    },
    onFleetRunControl: verb => onFleetRunControl(verb),
    onFleetWorkerControl: (card, verb) => onFleetWorkerControl(card, verb),
    onFleetRunPreview: () => {
      const preview = fleetRunPreview()
      fleetRun = typeof preview === "string"
        ? { phase: "error", message: preview }
        : { phase: "preview", slots: preview }
      paint()
    },
    onFleetRunStart: () => onFleetRunStart(),
    onLoadGymDemoProof: () => onLoadGymDemoProof(),
    onOptimizationStart: () => onOptimizationStart(),
    onRefresh: () => void refresh(),
    onRemove: (accountRef: string) => onRemove(accountRef),
    onConnect: (accountRef: string) => onConnect(accountRef),
    onPauseAccount: (accountRef: string, paused: boolean) => onPauseAccount(accountRef, paused),
    onConsumeResetCredit: accountRef => onConsumeResetCredit(accountRef),
    onOpenUrl: (url: string) => void options.openExternal(url),
    onCancelConnect: () => {
      window.clearTimeout(connectPoll)
      activeConnect = null
      paint()
    },
  }

  const onDelegateRun = (): void => {
    if (delegateInFlight) return
    const request = delegateRequest()
    if (typeof request === "string") {
      delegateRun = { phase: "error", message: request }
      paint()
      return
    }
    delegateInFlight = true
    delegateRun = { phase: "loading" }
    paint()
    void (async () => {
      try {
        const result = await options.delegateRun(request)
        delegateRun = { phase: "ready", result }
        await refresh()
      } catch (error) {
        delegateRun = {
          phase: "error",
          message: error instanceof Error ? error.message : String(error),
        }
        paint()
      } finally {
        delegateInFlight = false
      }
    })()
  }

  const onFleetRunStart = (): void => {
    if (fleetRunInFlight) return
    const preview = fleetRunPreview()
    if (typeof preview === "string") {
      fleetRun = { phase: "error", message: preview }
      paint()
      return
    }
    const request = fleetRunRequest()
    if (typeof request === "string") {
      fleetRun = { phase: "error", message: request, slots: preview }
      paint()
      return
    }
    fleetRunInFlight = true
    fleetRun = { phase: "loading", slots: preview }
    paint()
    void (async () => {
      try {
        const result = await options.fleetRunStart(request)
        objectiveByRunRef.set(result.run.runRef, request.objective)
        activeRun = {
          controlInFlight: null,
          error: null,
          objective: request.objective,
          run: result.run,
        }
        fleetRun = { phase: "ready", result, slots: preview }
        await refresh()
      } catch (error) {
        fleetRun = {
          phase: "error",
          message: error instanceof Error ? error.message : String(error),
          slots: preview,
        }
        paint()
      } finally {
        fleetRunInFlight = false
      }
    })()
  }

  const onFleetRunControl = (
    verb: KhalaCodeDesktopFleetRunControlRequest["verb"],
  ): void => {
    if (activeRun.run === null || activeRun.controlInFlight !== null) return
    const runRef = activeRun.run.runRef
    activeRun = { ...activeRun, controlInFlight: verb, error: null }
    paint()
    void (async () => {
      try {
        const result = await options.fleetRunControl({ runRef, verb })
        activeRun = {
          controlInFlight: null,
          error: null,
          objective: objectiveByRunRef.get(result.run.runRef) ?? activeRun.objective,
          run: result.run,
        }
        await refresh()
      } catch (error) {
        activeRun = {
          ...activeRun,
          controlInFlight: null,
          error: error instanceof Error ? error.message : String(error),
        }
        paint()
      }
    })()
  }

  const onFleetWorkerControl = (
    card: KhalaFleetWorkerCard,
    verb: KhalaCodeDesktopFleetWorkerControlRequest["verb"],
  ): void => {
    void (async () => {
      try {
        await options.fleetWorkerControl({
          assignmentRef: card.assignmentRef,
          issueRef: card.issueRef,
          runRef: activeRun.run?.runRef ?? null,
          verb,
          workerRefHash: card.workerRefHash,
        })
        await refresh()
      } catch {
        // The card control is best-effort UI plumbing; the next refresh keeps
        // the visible state authoritative from Fleet status.
      }
    })()
  }

  const onLoadGymDemoProof = (): void => {
    if (optimizationInFlight) return
    void (async () => {
      try {
        const result = await options.loadGymDemoProof()
        optimizationRun = { phase: "ready", result }
        paint()
      } catch (error) {
        optimizationRun = {
          phase: "error",
          message: error instanceof Error ? error.message : String(error),
        }
        paint()
      }
    })()
  }

  const onOptimizationStart = (): void => {
    if (optimizationInFlight) return
    optimizationInFlight = true
    optimizationRun = { phase: "loading" }
    paint()
    void (async () => {
      try {
        const result = await options.startDelegationOptimization()
        optimizationRun = { phase: "ready", result }
      } catch (error) {
        optimizationRun = {
          phase: "error",
          message: error instanceof Error ? error.message : String(error),
        }
      } finally {
        optimizationInFlight = false
        paint()
      }
    })()
  }

  const onRemove = (accountRef: string): void => {
    container
      .querySelector(`[data-account-ref="${CSS.escape(accountRef)}"]`)
      ?.remove()
    void (async () => {
      const result = await options.removeAccount(accountRef)
      if (!result.ok) {
        render(
          container,
          { phase: "error", message: result.error ?? "remove failed" },
          handlers,
          activeConnect,
          delegateForm,
          delegateRun,
          activeRun,
          lifecycleFrames,
          fleetRunForm,
          fleetRun,
          optimizationRun,
        )
        return
      }
      await refresh()
    })()
  }

  const onPauseAccount = (accountRef: string, paused: boolean): void => {
    if (lastData !== null) {
      lastData = {
        ...lastData,
        accounts: lastData.accounts.map(account =>
          account.accountRef === accountRef ? { ...account, paused } : account,
        ),
      }
      paint()
    }
    void (async () => {
      const result = await options.setAccountPaused({ accountRef, paused })
      if (!result.ok) {
        render(
          container,
          { phase: "error", message: result.error ?? "pause failed" },
          handlers,
          activeConnect,
          delegateForm,
          delegateRun,
          activeRun,
          lifecycleFrames,
          fleetRunForm,
          fleetRun,
          optimizationRun,
        )
        return
      }
      await refresh()
    })()
  }

  const onConsumeResetCredit = (accountRef: string): void => {
    void (async () => {
      const result = await options.consumeResetCredit({ accountRef })
      if (!result.ok) {
        render(
          container,
          { phase: "error", message: result.error ?? "reset credit failed" },
          handlers,
          activeConnect,
          delegateForm,
          delegateRun,
          activeRun,
          lifecycleFrames,
          fleetRunForm,
          fleetRun,
          optimizationRun,
        )
        return
      }
      await refresh()
    })()
  }

  const onConnect = (accountRef: string): void => {
    window.clearTimeout(connectPoll)
    activeConnect = { accountRef, start: null }
    paint()
    void (async () => {
      const start = await options.connectAccount(accountRef)
      // The connect may have been cancelled while we awaited.
      if (activeConnect === null || activeConnect.accountRef !== accountRef) return
      activeConnect = { accountRef, start }
      paint()
      if (!start.ok) return
      const poll = async (): Promise<void> => {
        if (activeConnect === null || activeConnect.accountRef !== accountRef) return
        try {
          const data = await options.fetch()
          lastData = data
          const account = data.accounts.find(item => item.accountRef === accountRef)
          if (account !== undefined && accountReadinessState(account.readiness) === "ready") {
            activeConnect = null
            paint()
            return
          }
        } catch {
          // keep polling
        }
        paint()
        connectPoll = window.setTimeout(() => void poll(), 3000)
      }
      connectPoll = window.setTimeout(() => void poll(), 3000)
    })()
  }

  const refresh = async (): Promise<void> => {
    if (inFlight) return
    inFlight = true
    if (lastData === null && activeConnect === null) {
      paint()
    } else {
      setRefreshBusy(true)
    }
    try {
      const [data, list] = await Promise.all([
        options.fetch(),
        options.fleetRunList(),
      ])
      lastData = data
      const selected = selectActiveFleetRun(list.runs)
      activeRun = {
        controlInFlight: activeRun.controlInFlight,
        error: null,
        objective: selected === null
          ? null
          : objectiveByRunRef.get(selected.runRef) ?? (
              activeRun.run?.runRef === selected.runRef ? activeRun.objective : null
            ),
        run: selected,
      }
      paint()
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error)
      if (lastData === null) {
        render(
          container,
          { phase: "error", message },
          handlers,
          activeConnect,
          delegateForm,
          delegateRun,
          activeRun,
          lifecycleFrames,
          fleetRunForm,
          fleetRun,
          optimizationRun,
        )
      } else {
        activeRun = {
          ...activeRun,
          controlInFlight: null,
          error: message,
        }
        setRefreshBusy(false)
      }
    } finally {
      inFlight = false
    }
  }

  const setVisible = (next: boolean): void => {
    visible = next
    window.clearInterval(pollTimer)
    if (!next) return
    startLifecycleStream()
    void refresh()
    // Live updates: poll while the panel is visible (skipping in-flight loads).
    // The list stays live even during an active connect.
    pollTimer = window.setInterval(() => {
      if (visible && !inFlight) void refresh()
    }, 5000)
  }

  paint()
  return { refresh, setVisible }
}
