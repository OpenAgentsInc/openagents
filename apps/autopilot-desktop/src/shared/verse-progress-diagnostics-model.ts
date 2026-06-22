export type VerseRunStepKind = "assignment" | "trace" | "replay" | "verdict" | "settle"

export type VerseRunLifecycleFacts = Readonly<{
  assignmentRef?: string
  traceRef?: string
  replayRef?: string
  verdictRef?: string
  settlementRef?: string
  failedStep?: VerseRunStepKind
}>

export type VerseRunProgressStep = Readonly<{
  kind: VerseRunStepKind
  label: string
  status: "blocked" | "current" | "done" | "failed"
  progress: number
  sourceRef: string | null
}>

export type VersePortraitChipInput = Readonly<{
  avatarRef: string
  label: string
  anchorX: number
  anchorY: number
}>

export type VersePortraitChip = Readonly<{
  avatarRef: string
  label: string
  cssX: number
  cssY: number
  cssSize: number
  backingPx: number
  overscanPx: number
  visible: boolean
}>

export type VersePerfDiagnosticsMode = "development" | "production" | "smoke"

export type VersePerfDiagnostics = Readonly<{
  available: boolean
  showOverlay: boolean
  artifact: {
    schema: "openagents.autopilot_desktop.verse_webgl_diagnostics.v1"
    drawCalls: number
    entityCount: number
    frameCount: number
    frameMsAverage: number
    frameMsP95: number
    frameMsMax: number
    fpsEstimate: number
    sourceRefs: ReadonlyArray<string>
  } | null
}>

const runSteps: ReadonlyArray<{
  kind: VerseRunStepKind
  label: string
  refKey: keyof Omit<VerseRunLifecycleFacts, "failedStep">
}> = [
  { kind: "assignment", label: "Assignment", refKey: "assignmentRef" },
  { kind: "trace", label: "Trace", refKey: "traceRef" },
  { kind: "replay", label: "Replay", refKey: "replayRef" },
  { kind: "verdict", label: "Verdict", refKey: "verdictRef" },
  { kind: "settle", label: "Settle", refKey: "settlementRef" },
]

const clamp = (value: number, min: number, max: number): number =>
  Math.min(max, Math.max(min, Number.isFinite(value) ? value : min))

const round = (value: number, digits = 2): number => {
  const scale = 10 ** digits
  return Math.round(value * scale) / scale
}

const percentile = (values: ReadonlyArray<number>, p: number): number => {
  if (values.length === 0) return 0
  const sorted = [...values].sort((a, b) => a - b)
  const index = Math.min(sorted.length - 1, Math.ceil(sorted.length * p) - 1)
  return sorted[index] ?? 0
}

const isPublicRef = (value: string): boolean =>
  /^[A-Za-z0-9._:/#-]{1,96}$/.test(value)

export const projectVerseRunStepProgress = (
  facts: VerseRunLifecycleFacts,
): ReadonlyArray<VerseRunProgressStep> => {
  const failedIndex = facts.failedStep === undefined
    ? -1
    : runSteps.findIndex(step => step.kind === facts.failedStep)
  const firstMissingIndex = runSteps.findIndex(step => facts[step.refKey] === undefined)
  const currentIndex =
    failedIndex >= 0 ? failedIndex
      : firstMissingIndex >= 0 ? firstMissingIndex
        : runSteps.length - 1

  return runSteps.map((step, index) => {
    const sourceRef = facts[step.refKey] ?? null
    const status =
      failedIndex === index ? "failed"
        : sourceRef !== null && (failedIndex < 0 || index < failedIndex) ? "done"
          : index === currentIndex ? "current"
            : "blocked"
    return {
      kind: step.kind,
      label: step.label,
      status,
      progress: status === "done" ? 1 : status === "failed" ? 1 : status === "current" ? 0.5 : 0,
      sourceRef,
    }
  })
}

export const projectVersePortraitChips = (input: {
  readonly avatars: ReadonlyArray<VersePortraitChipInput>
  readonly viewportCssWidth: number
  readonly viewportCssHeight: number
  readonly devicePixelRatio?: number
  readonly chipCssSize?: number
  readonly overscanRatio?: number
}): ReadonlyArray<VersePortraitChip> => {
  const width = Math.max(1, Math.floor(input.viewportCssWidth))
  const height = Math.max(1, Math.floor(input.viewportCssHeight))
  const cssSize = Math.max(24, Math.floor(input.chipCssSize ?? 48))
  const dpr = clamp(input.devicePixelRatio ?? 1, 1, 3)
  const overscanPx = Math.round(cssSize * clamp(input.overscanRatio ?? 0.18, 0, 0.5))
  const minX = -overscanPx
  const minY = -overscanPx
  const maxX = width - cssSize + overscanPx
  const maxY = height - cssSize + overscanPx

  return input.avatars.map(avatar => {
    const rawX = avatar.anchorX * width - cssSize / 2
    const rawY = avatar.anchorY * height - cssSize / 2
    const cssX = round(clamp(rawX, minX, maxX))
    const cssY = round(clamp(rawY, minY, maxY))
    return {
      avatarRef: avatar.avatarRef,
      label: avatar.label,
      cssX,
      cssY,
      cssSize,
      backingPx: Math.round(cssSize * dpr),
      overscanPx,
      visible: rawX >= minX && rawX <= maxX && rawY >= minY && rawY <= maxY,
    }
  })
}

export const projectVerseWebglDiagnostics = (input: {
  readonly mode: VersePerfDiagnosticsMode
  readonly enabled?: boolean
  readonly frameTimesMs?: ReadonlyArray<number>
  readonly drawCalls?: number
  readonly entityCount?: number
  readonly sourceRefs?: ReadonlyArray<string>
}): VersePerfDiagnostics => {
  const enabled = input.enabled === true
  if (!enabled || input.mode === "production") {
    return { available: false, showOverlay: false, artifact: null }
  }

  const frames = (input.frameTimesMs ?? [])
    .filter(value => Number.isFinite(value) && value >= 0)
    .slice(-120)
  const average = frames.length === 0
    ? 0
    : frames.reduce((sum, value) => sum + value, 0) / frames.length
  const p95 = percentile(frames, 0.95)
  const max = frames.length === 0 ? 0 : Math.max(...frames)
  const fpsEstimate = average <= 0 ? 0 : 1000 / average

  return {
    available: true,
    showOverlay: input.mode === "development",
    artifact: {
      schema: "openagents.autopilot_desktop.verse_webgl_diagnostics.v1",
      drawCalls: Math.max(0, Math.floor(input.drawCalls ?? 0)),
      entityCount: Math.max(0, Math.floor(input.entityCount ?? 0)),
      frameCount: frames.length,
      frameMsAverage: round(average),
      frameMsP95: round(p95),
      frameMsMax: round(max),
      fpsEstimate: round(fpsEstimate, 1),
      sourceRefs: (input.sourceRefs ?? []).filter(isPublicRef).slice(0, 8),
    },
  }
}
