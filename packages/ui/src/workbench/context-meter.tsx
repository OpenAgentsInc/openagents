import type { ReactElement } from "react"

/**
 * Context/usage meter (#8868, epic #8857 T11): the Autopilot quantized
 * block-progress motif over `thread/tokenUsage/updated` (context-window
 * composition) and `AccountRateLimitsUpdated` (rolling rate-limit windows).
 *
 * Design rule (never violate): every number rendered here is an EXACT wire
 * value. A field that is `undefined` renders as "—" or is omitted entirely —
 * it is NEVER coerced to a fake `0`, and no context-window ceiling is ever
 * guessed from a model name. `usage.contextWindowTokens` is accepted so a
 * caller that genuinely knows the ceiling (e.g. from account/model config)
 * can render a "used / ceiling" fill; without it the block bar instead shows
 * the real composition of the tokens that ARE known (cached/input/reasoning/
 * output, in that left-to-right order) so the visual never implies a fill
 * level nobody actually reported.
 */

/** One rate-limit window from `AccountRateLimitsUpdated` (primary/secondary). */
export type ContextMeterRateLimitWindow = Readonly<{
  /** Caller-supplied window label, e.g. "PRIMARY" / "SECONDARY" — never invented. */
  label: string
  /** Exact wire `usedPercent` (0-100). */
  usedPercent: number
  /** Exact wire `resetsAt` (unix seconds), when the server reported one. */
  resetsAt?: number
  /** Exact wire `windowDurationMins`, when the server reported one. */
  windowDurationMins?: number
}>

/** Exact token-usage fields from `thread/tokenUsage/updated`. */
export type ContextMeterUsage = Readonly<{
  inputTokens?: number
  cachedInputTokens?: number
  outputTokens?: number
  reasoningTokens?: number
  totalTokens?: number
  /** The model's context-window ceiling, ONLY when a caller actually knows it. */
  contextWindowTokens?: number
}>

export type ContextMeterProps = Readonly<{
  itemKey?: string
  usage?: ContextMeterUsage
  rateLimits?: ReadonlyArray<ContextMeterRateLimitWindow>
  /**
   * `true` for the dispatch.tsx timeline/inspector rendering of a past
   * snapshot; `false` (default) for the live header/rail mount. Historical
   * snapshots never claim to be "live" in their labeling.
   */
  historical?: boolean
  /** Epoch ms "now", for deterministic reset countdowns in tests. */
  now?: number
}>

const BLOCK_COUNT = 20

const formatCount = (value: number): string => value.toLocaleString("en-US")

/** Compact duration label from real minutes — formatting, never invention. */
const formatDuration = (mins: number): string => {
  if (!Number.isFinite(mins) || mins <= 0) return "—"
  // Long server windows are human calendar windows. Countdown sampling can
  // land a few seconds/minutes shy of an exact boundary, so normalize 48h+
  // to the nearest day instead of rendering the same weekly window as
  // "7D" in one lane and "168.0H" in another.
  if (mins >= 48 * 60) return `${Math.max(1, Math.round(mins / (24 * 60)))}D`
  if (mins % 60 === 0) return `${mins / 60}H`
  if (mins >= 60) return `${(mins / 60).toFixed(1)}H`
  return `${Math.round(mins)}M`
}

/** "RESETS IN 3H" from an exact unix-seconds `resetsAt`, or "—" if absent. */
const formatResetCountdown = (resetsAt: number | undefined, nowMs: number): string => {
  if (resetsAt === undefined) return "—"
  const remainingMins = Math.round((resetsAt * 1000 - nowMs) / 60_000)
  if (remainingMins <= 0) return "RESETTING"
  return `RESETS IN ${formatDuration(remainingMins)}`
}

const quantizeBlocks = (fraction: number): number =>
  Math.max(0, Math.min(BLOCK_COUNT, Math.round(fraction * BLOCK_COUNT)))

/** Sentinel `dangerAt` meaning "never render the danger tone" — no block
 * index reaches it, so the field stays a plain required `number` instead of
 * an optional one under `exactOptionalPropertyTypes`. */
const NO_DANGER = BLOCK_COUNT

/** One row of `BLOCK_COUNT` discrete rectangles — never a smooth/eased fill. */
const QuantizedBlocks = ({ segments, dangerAt }: Readonly<{
  segments: ReadonlyArray<Readonly<{ blocks: number; tone: "dim" | "medium" | "bright" }>>
  /** Blocks at/after this index render in the danger tone (near-limit state). */
  dangerAt: number
}>): ReactElement => {
  const cells: Array<{ tone: "dim" | "medium" | "bright" | "empty" }> = []
  for (const segment of segments) {
    for (let i = 0; i < segment.blocks && cells.length < BLOCK_COUNT; i++) cells.push({ tone: segment.tone })
  }
  while (cells.length < BLOCK_COUNT) cells.push({ tone: "empty" })
  return <span className="oa-react-meter-blocks">
    {cells.map((cell, index) => <i
      data-danger={index >= dangerAt && cell.tone !== "empty" ? "true" : "false"}
      data-tone={cell.tone}
      key={index}
    />)}
  </span>
}

const usageHasAnyField = (usage: ContextMeterUsage | undefined): usage is ContextMeterUsage =>
  usage !== undefined && (
    usage.inputTokens !== undefined ||
    usage.cachedInputTokens !== undefined ||
    usage.outputTokens !== undefined ||
    usage.reasoningTokens !== undefined ||
    usage.totalTokens !== undefined
  )

const UsageRow = ({ usage }: Readonly<{ usage: ContextMeterUsage }>): ReactElement => {
  const cached = usage.cachedInputTokens
  const input = usage.inputTokens
  const reasoning = usage.reasoningTokens
  const output = usage.outputTokens
  const total = usage.totalTokens
  const ceiling = usage.contextWindowTokens
  // Denominator for quantizing: the real ceiling when known, else the sum of
  // whatever fields ARE known (never a guessed max) so blocks are always a
  // true proportion of tokens that were actually reported.
  const knownSum = (cached ?? 0) + (input ?? 0) + (reasoning ?? 0) + (output ?? 0)
  const denominator = ceiling ?? (total ?? knownSum)
  const blocksFor = (value: number | undefined): number =>
    value === undefined || denominator <= 0 ? 0 : quantizeBlocks(value / denominator)
  const segments = [
    { blocks: blocksFor(cached), tone: "dim" as const },
    { blocks: blocksFor(input), tone: "medium" as const },
    { blocks: blocksFor(reasoning), tone: "medium" as const },
    { blocks: blocksFor(output), tone: "bright" as const },
  ]
  const nearLimit = ceiling !== undefined && ceiling > 0 && total !== undefined && total / ceiling >= 0.85
  const dangerAt = nearLimit ? quantizeBlocks(0.85) : NO_DANGER
  return <div className="oa-react-meter-row" data-meter-row="usage">
    <span className="oa-react-meter-label">CONTEXT</span>
    <QuantizedBlocks dangerAt={dangerAt} segments={segments} />
    <span className="oa-react-meter-total" data-near-limit={nearLimit ? "true" : "false"}>
      {total === undefined ? "—" : formatCount(total)}
      {ceiling === undefined ? "" : ` / ${formatCount(ceiling)}`}
      {" TOKENS"}
    </span>
  </div>
}

const UsageBreakdown = ({ usage }: Readonly<{ usage: ContextMeterUsage }>): ReactElement =>
  <div className="oa-react-meter-breakdown">
    <span data-field="input"><small>INPUT</small>{usage.inputTokens === undefined ? "—" : formatCount(usage.inputTokens)}</span>
    <span data-field="cached"><small>CACHED</small>{usage.cachedInputTokens === undefined ? "—" : formatCount(usage.cachedInputTokens)}</span>
    <span data-field="output"><small>OUTPUT</small>{usage.outputTokens === undefined ? "—" : formatCount(usage.outputTokens)}</span>
    <span data-field="reasoning"><small>REASONING</small>{usage.reasoningTokens === undefined ? "—" : formatCount(usage.reasoningTokens)}</span>
  </div>

const RateLimitRow = ({ window, nowMs }: Readonly<{ window: ContextMeterRateLimitWindow; nowMs: number }>): ReactElement => {
  const fraction = Math.max(0, Math.min(100, window.usedPercent)) / 100
  const rateLimited = window.usedPercent >= 100
  const nearLimit = window.usedPercent >= 85
  const dangerAt = nearLimit ? quantizeBlocks(0.85) : NO_DANGER
  return <div className="oa-react-meter-row" data-meter-row="rate-limit" data-rate-limited={rateLimited ? "true" : "false"}>
    <span className="oa-react-meter-label">{window.label.toLocaleUpperCase()}</span>
    <QuantizedBlocks dangerAt={dangerAt} segments={[{ blocks: quantizeBlocks(fraction), tone: rateLimited ? "bright" : "medium" }]} />
    <span className="oa-react-meter-total" data-near-limit={nearLimit ? "true" : "false"}>
      {Math.round(window.usedPercent)}% USED
      {window.resetsAt === undefined
        ? window.windowDurationMins === undefined ? "" : ` · ${formatDuration(window.windowDurationMins)} WINDOW`
        : ` · ${formatResetCountdown(window.resetsAt, nowMs)}`}
    </span>
  </div>
}

/**
 * The context/usage meter. Renders nothing fabricated: an empty `usage` and
 * empty `rateLimits` together render one honest "NO DATA" row rather than a
 * misleading empty bar.
 */
export const ContextMeter = ({
  itemKey,
  usage,
  rateLimits,
  historical = false,
  now,
}: ContextMeterProps): ReactElement => {
  const rateLimitWindows = rateLimits ?? []
  const hasUsage = usageHasAnyField(usage)
  const hasRateLimits = rateLimitWindows.length > 0
  const nowMs = now ?? Date.now()
  if (!hasUsage && !hasRateLimits) {
    return <div
      className="oa-react-meter oa-react-meter-empty"
      data-historical={historical ? "true" : "false"}
      data-timeline-key={itemKey}
    >
      <span className="oa-react-meter-label">CONTEXT</span>
      <span className="oa-react-meter-empty-state">NO DATA</span>
    </div>
  }
  return <div className="oa-react-meter" data-historical={historical ? "true" : "false"} data-timeline-key={itemKey}>
    {usageHasAnyField(usage) ? <UsageRow usage={usage} /> : null}
    {usageHasAnyField(usage) ? <UsageBreakdown usage={usage} /> : null}
    {hasRateLimits ? rateLimitWindows.map((window, index) => <RateLimitRow key={`${window.label}:${index}`} nowMs={nowMs} window={window} />) : null}
  </div>
}
