/**
 * Structured-payload cards for the conversation timeline (renderer-only).
 *
 * Instead of a raw inline JSON blob with a "Show full message" truncation, a
 * message body that is (or embeds) a JSON payload renders here as a clean,
 * collapsible card. The Full Auto mission packet gets a purpose-built mission
 * card (objective + done condition prominent, refs as quiet labels); every
 * other JSON payload gets a generic key/value tree card. Both preserve a
 * "copy raw" affordance over the canonical pretty-printed JSON.
 *
 * Presentation is className-only against the shared `oa-react-*` timeline card
 * recipes in `packages/ui/src/desktop-workbench.css` (the same tokens the plan,
 * work-entry, and notice cards use). No inline colors, fonts, or dimensions —
 * the design-conformance oracle scans this module.
 */
import { Braces, Check, ChevronRight, Copy, Target } from "lucide-react"
import { memo, useEffect, useRef, useState, type ReactElement, type ReactNode } from "react"

import type { MissionCardView, StructuredPayloadDetection } from "./structured-payload.ts"

const COPY_FEEDBACK_MS = 1000
const OBJECTIVE_PREVIEW_LIMIT = 160
const JSON_TREE_MAX_DEPTH = 6
const JSON_TREE_MAX_ARRAY = 50

const compact = (value: string, limit: number): string => {
  const normalized = value.replaceAll(/\s+/g, " ").trim()
  return normalized.length <= limit ? normalized : `${normalized.slice(0, limit - 1)}…`
}

/**
 * Clipboard write that tolerates an Electron renderer loaded from a non-secure
 * custom protocol (where `navigator.clipboard` can be undefined or reject), by
 * falling back to a hidden textarea + `execCommand` inside the click gesture.
 * Mirrors the timeline's message-copy helper; kept local to avoid an import
 * cycle with `react-timeline.tsx`.
 */
const writeClipboard = async (text: string): Promise<boolean> => {
  try {
    if (typeof navigator !== "undefined" && navigator.clipboard?.writeText !== undefined) {
      await navigator.clipboard.writeText(text)
      return true
    }
  } catch {
    // Fall through to the synchronous execCommand path.
  }
  if (typeof document === "undefined") return false
  const textarea = document.createElement("textarea")
  textarea.value = text
  textarea.setAttribute("readonly", "")
  textarea.style.position = "fixed"
  textarea.style.top = "0"
  textarea.style.left = "0"
  textarea.style.opacity = "0"
  textarea.style.pointerEvents = "none"
  document.body.appendChild(textarea)
  try {
    textarea.focus()
    textarea.select()
    return document.execCommand("copy")
  } catch {
    return false
  } finally {
    document.body.removeChild(textarea)
  }
}

const CopyRawButton = memo(({ json }: Readonly<{ json: string }>): ReactElement => {
  const [copied, setCopied] = useState(false)
  const timer = useRef<ReturnType<typeof setTimeout> | null>(null)
  useEffect(() => () => {
    if (timer.current !== null) clearTimeout(timer.current)
  }, [])
  const onCopy = (): void => {
    void writeClipboard(json).then((ok) => {
      if (!ok) return
      setCopied(true)
      if (timer.current !== null) clearTimeout(timer.current)
      timer.current = setTimeout(() => setCopied(false), COPY_FEEDBACK_MS)
    })
  }
  return (
    <button
      type="button"
      className="oa-react-payload-copy"
      aria-label="Copy raw JSON"
      disabled={copied}
      onClick={onCopy}
    >
      {copied ? <Check aria-hidden="true" /> : <Copy aria-hidden="true" />}
      <span>{copied ? "Copied" : "Copy JSON"}</span>
    </button>
  )
})

// --- Generic JSON key/value tree --------------------------------------------

const isObject = (value: unknown): value is Record<string, unknown> =>
  typeof value === "object" && value !== null && !Array.isArray(value)

const JsonScalar = ({ value }: Readonly<{ value: unknown }>): ReactElement => {
  if (typeof value === "string")
    return <span className="oa-react-payload-string">&quot;{value}&quot;</span>
  if (typeof value === "number")
    return <span className="oa-react-payload-number">{String(value)}</span>
  if (typeof value === "boolean")
    return <span className="oa-react-payload-boolean">{String(value)}</span>
  return <span className="oa-react-payload-null">null</span>
}

const JsonNode = ({ value, depth }: Readonly<{ value: unknown; depth: number }>): ReactElement => {
  if (!isObject(value) && !Array.isArray(value)) return <JsonScalar value={value} />
  if (depth >= JSON_TREE_MAX_DEPTH)
    return <span className="oa-react-payload-collapsed">{Array.isArray(value) ? "[…]" : "{…}"}</span>

  const entries: ReadonlyArray<readonly [string, unknown]> = Array.isArray(value)
    ? value.slice(0, JSON_TREE_MAX_ARRAY).map((item, index) => [String(index), item] as const)
    : Object.entries(value)
  const overflow = Array.isArray(value) ? Math.max(0, value.length - JSON_TREE_MAX_ARRAY) : 0

  return (
    <div className="oa-react-payload-nest" data-json-kind={Array.isArray(value) ? "array" : "object"}>
      {entries.map(([key, child]) => {
        const branch = isObject(child) || Array.isArray(child)
        return (
          <div className="oa-react-payload-entry" key={key} data-branch={branch ? "true" : "false"}>
            <span className="oa-react-payload-key">{Array.isArray(value) ? key : `"${key}"`}</span>
            <span className="oa-react-payload-punct">:</span>
            {branch ? (
              <JsonNode value={child} depth={depth + 1} />
            ) : (
              <JsonScalar value={child} />
            )}
          </div>
        )
      })}
      {overflow > 0 ? (
        <div className="oa-react-payload-entry oa-react-payload-overflow">+{overflow} more</div>
      ) : null}
    </div>
  )
}

const jsonPreview = (value: unknown, schemaLabel: string | null): string => {
  if (Array.isArray(value)) return `${value.length} ${value.length === 1 ? "item" : "items"}`
  if (isObject(value)) {
    const keys = Object.keys(value).filter((key) => key !== "schema")
    const head = keys.slice(0, 4).join(", ")
    const label = keys.length === 0 ? "empty object" : keys.length > 4 ? `${head}, …` : head
    return schemaLabel === null ? label : label
  }
  return String(value)
}

// --- Cards -------------------------------------------------------------------

const PayloadShell = ({
  chipIcon,
  chipLabel,
  preview,
  kind,
  itemKey,
  defaultOpen,
  children,
}: Readonly<{
  chipIcon: ReactNode
  chipLabel: string
  preview: string
  kind: string
  itemKey: string
  defaultOpen: boolean
  children: ReactNode
}>): ReactElement => (
  <details
    className="oa-react-payload"
    data-kind={kind}
    data-timeline-key={itemKey}
    role="listitem"
    {...(defaultOpen ? { open: true } : {})}
  >
    <summary className="oa-react-payload-summary">
      <span className="oa-react-payload-chip">
        {chipIcon}
        <span>{chipLabel}</span>
      </span>
      <span className="oa-react-payload-preview">{preview}</span>
      <ChevronRight aria-hidden="true" className="oa-react-payload-caret" />
    </summary>
    <div className="oa-react-payload-body">{children}</div>
  </details>
)

const MissionMetaRow = ({
  term,
  children,
}: Readonly<{ term: string; children: ReactNode }>): ReactElement => (
  <div className="oa-react-mission-meta-row">
    <dt>{term}</dt>
    <dd>{children}</dd>
  </div>
)

const MissionRef = ({
  term,
  value,
}: Readonly<{ term: string; value: string | null | undefined }>): ReactElement | null =>
  value === null || value === undefined || value === ""
    ? null
    : (
      <span className="oa-react-payload-ref">
        <span className="oa-react-payload-ref-term">{term}</span>
        <span className="oa-react-payload-ref-value">{value}</span>
      </span>
    )

const MissionCard = ({
  mission,
  json,
  itemKey,
}: Readonly<{ mission: MissionCardView; json: string; itemKey: string }>): ReactElement => {
  const turnCap = mission.turnCap
  const remaining = mission.remainingTurnsIncludingThisOne
  const turnLabel =
    turnCap === undefined
      ? null
      : remaining === undefined
        ? `${turnCap} cap`
        : `${Math.max(0, turnCap - remaining) + 1} of ${turnCap}`
  return (
    <PayloadShell
      chipIcon={<Target aria-hidden="true" />}
      chipLabel="MISSION"
      preview={compact(mission.objective, OBJECTIVE_PREVIEW_LIMIT)}
      kind="full_auto_mission"
      itemKey={itemKey}
      defaultOpen={false}
    >
      <div className="oa-react-mission-field">
        <span className="oa-react-payload-label">Objective</span>
        <p className="oa-react-mission-objective">{mission.objective}</p>
      </div>
      <div className="oa-react-mission-field">
        <span className="oa-react-payload-label">Done condition</span>
        <p className="oa-react-mission-text">{mission.doneCondition}</p>
      </div>
      {mission.planBrief === undefined ? null : (
        <div className="oa-react-mission-field">
          <span className="oa-react-payload-label">
            Plan
            {mission.planBrief.done !== undefined && mission.planBrief.total !== undefined
              ? ` · ${mission.planBrief.done} of ${mission.planBrief.total}`
              : ""}
          </span>
          <p className="oa-react-mission-text">{mission.planBrief.text}</p>
        </div>
      )}
      <dl className="oa-react-mission-meta">
        {mission.currentLane === undefined ? null : (
          <MissionMetaRow term="Lane">
            <span className="oa-react-payload-mono">{mission.currentLane}</span>
          </MissionMetaRow>
        )}
        {turnLabel === null ? null : <MissionMetaRow term="Turn">{turnLabel}</MissionMetaRow>}
        {mission.continuationOrdinal === undefined ? null : (
          <MissionMetaRow term="Attempt">{String(mission.continuationOrdinal)}</MissionMetaRow>
        )}
        {mission.objectiveSource === undefined ? null : (
          <MissionMetaRow term="Source">{mission.objectiveSource.replaceAll("_", " ")}</MissionMetaRow>
        )}
      </dl>
      {mission.runRef || mission.threadRef || mission.workspaceRef || mission.accountRef ? (
        <div className="oa-react-mission-refs">
          <MissionRef term="run" value={mission.runRef} />
          <MissionRef term="thread" value={mission.threadRef} />
          <MissionRef term="workspace" value={mission.workspaceRef} />
          <MissionRef term="account" value={mission.accountRef} />
        </div>
      ) : null}
      <div className="oa-react-payload-actions">
        <CopyRawButton json={json} />
        <details className="oa-react-payload-raw">
          <summary>Raw packet</summary>
          <pre>
            <code>{json}</code>
          </pre>
        </details>
      </div>
    </PayloadShell>
  )
}

const JsonPayloadCard = ({
  value,
  json,
  schemaLabel,
  itemKey,
}: Readonly<{
  value: unknown
  json: string
  schemaLabel: string | null
  itemKey: string
}>): ReactElement => (
  <PayloadShell
    chipIcon={<Braces aria-hidden="true" />}
    chipLabel={schemaLabel ?? "STRUCTURED PAYLOAD"}
    preview={jsonPreview(value, schemaLabel)}
    kind="structured_payload"
    itemKey={itemKey}
    defaultOpen={false}
  >
    <div className="oa-react-payload-tree">
      <JsonNode value={value} depth={0} />
    </div>
    <div className="oa-react-payload-actions">
      <CopyRawButton json={json} />
    </div>
  </PayloadShell>
)

/**
 * The single entry point the timeline uses: render a detected payload as its
 * mission card or generic JSON card. `itemKey` is the timeline record key so
 * the card carries the same stable `data-timeline-key` as every other row.
 */
export const StructuredPayloadCard = ({
  detection,
  itemKey,
}: Readonly<{
  detection: StructuredPayloadDetection
  itemKey: string
}>): ReactElement =>
  detection.kind === "mission" ? (
    <MissionCard mission={detection.mission} json={detection.json} itemKey={itemKey} />
  ) : (
    <JsonPayloadCard
      value={detection.value}
      json={detection.json}
      schemaLabel={detection.schemaLabel}
      itemKey={itemKey}
    />
  )
