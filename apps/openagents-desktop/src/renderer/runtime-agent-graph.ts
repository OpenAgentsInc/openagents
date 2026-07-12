import {
  Badge,
  Button,
  IntentRef,
  Stack,
  StaticPayload,
  Text,
  type View,
} from "@effect-native/core"
import type {
  LiveAgentGraphPresentation,
  LiveAgentGraphPresentationRow,
  LiveAgentGraphTone,
} from "../agent-graph-presentation.ts"
import type { RuntimeChildTranscript } from "./runtime-cards.ts"

const badgeTone = (tone: LiveAgentGraphTone): "neutral" | "info" | "success" | "warn" | "danger" =>
  tone === "active"
    ? "info"
    : tone === "attention"
      ? "warn"
      : tone === "success"
        ? "success"
        : tone === "danger"
          ? "danger"
          : "neutral"

export const runtimeAgentGraphDetailFields = (
  row: LiveAgentGraphPresentationRow,
): ReadonlyArray<Readonly<{ label: string; value: string }>> => [
  { label: "Status", value: row.statusLabel },
  { label: "Provider", value: row.providerLabel },
  { label: "Runtime", value: row.runtimeLabel },
  { label: "Session", value: row.sessionLabel },
  { label: "Worktree", value: row.worktreeLabel },
  { label: "Elapsed", value: row.elapsedLabel },
  { label: "Tokens", value: row.tokensLabel },
  ...(row.toolLabel === null ? [] : [{ label: "Current action", value: row.toolLabel }]),
  ...(row.attentionLabel === null ? [] : [{ label: "Attention", value: row.attentionLabel }]),
  ...(row.terminalLabel === null ? [] : [{ label: "Terminal", value: row.terminalLabel }]),
]

const accessibilityLabel = (row: LiveAgentGraphPresentationRow): string =>
  [
    row.label,
    row.statusLabel,
    row.toolLabel,
    row.attentionLabel,
    row.terminalLabel,
    row.elapsedLabel,
    `Tokens ${row.tokensLabel}`,
    "Show agent details",
  ].filter((value): value is string => value !== null).join(". ")

const depthSpacing = (depth: number) =>
  (["0", "2", "4", "6", "8", "10"] as const)[Math.min(depth, 5)] ?? "0"

const transcriptRoleLabel = (role: RuntimeChildTranscript[number]["role"]): string =>
  role === "user" ? "You" : role === "assistant" ? "Codex" : "Activity"

const agentInspector = (
  row: LiveAgentGraphPresentationRow,
  transcript: RuntimeChildTranscript | null,
): View => {
  const fields = runtimeAgentGraphDetailFields(row)
  return Stack(
    {
      key: `runtime-agent-inspector-${row.agentRef}`,
      direction: "column",
      gap: "2",
      style: {
        marginLeft: depthSpacing(row.depth + 1),
        padding: "3",
        borderColor: "border",
        borderWidth: 1,
        borderRadius: "md",
      },
      a11y: { role: "region", label: `Agent details, ${row.label}` },
    },
    [
      ...(transcript === null ? [] : [Stack(
        {
          key: `runtime-agent-transcript-${row.agentRef}`,
          direction: "column",
          gap: "3",
          style: { width: "full" },
          a11y: { role: "region", label: `Transcript for ${row.label}` },
        },
        [
          Text({ key: `runtime-agent-transcript-title-${row.agentRef}`, content: "Transcript", variant: "label", color: "textPrimary" }),
          ...transcript.map((entry, index) => Stack(
            {
              key: `runtime-agent-transcript-entry-${row.agentRef}-${index}`,
              direction: "column",
              gap: "1",
              style: { width: "full" },
            },
            [
              Text({
                key: `runtime-agent-transcript-role-${row.agentRef}-${index}`,
                content: transcriptRoleLabel(entry.role),
                variant: "caption",
                color: entry.role === "assistant" ? "accent" : "textMuted",
              }),
              Text({
                key: `runtime-agent-transcript-text-${row.agentRef}-${index}`,
                content: entry.text,
                variant: "body",
                color: entry.role === "system" ? "textMuted" : "textPrimary",
              }),
            ],
          )),
        ],
      )]),
      Stack(
        { key: `runtime-agent-fields-${row.agentRef}`, direction: "column", gap: "2", style: { width: "full" } },
        fields.map((field, index) => Stack(
          { key: `runtime-agent-field-row-${row.agentRef}-${index}`, direction: "column", gap: "1", style: { width: "full" } },
          [
            Text({ key: `runtime-agent-field-${row.agentRef}-${index}`, content: field.label, variant: "caption", color: "textMuted" }),
            Text({ key: `runtime-agent-value-${row.agentRef}-${index}`, content: field.value, variant: "body", color: "textPrimary" }),
          ],
        )),
      ),
      ...(row.canControl ? [Button({
        key: `runtime-agent-focus-${row.agentRef}`,
        label: "Focus agent",
        variant: "secondary",
        onPress: IntentRef("DesktopAgentAction", StaticPayload({
          kind: "focus_agent",
          agentRef: row.agentRef,
        })),
        a11y: { label: `Focus ${row.label}` },
      })] : []),
    ],
  )
}

export const runtimeAgentGraphView = (input: Readonly<{
  graph: LiveAgentGraphPresentation
  expanded: boolean
  selectedAgentRef: string | null
  selectedTranscript?: RuntimeChildTranscript | null
}>): View => {
  const { graph, expanded, selectedAgentRef, selectedTranscript = null } = input
  const summary = `${graph.totalCount} agent${graph.totalCount === 1 ? "" : "s"} · ${graph.activeCount} active` +
    (graph.attentionCount === 0 ? "" : ` · ${graph.attentionCount} need attention`)
  return Stack(
    {
      key: "runtime-agent-graph",
      direction: "column",
      gap: "2",
      style: {
        width: "full",
        minWidth: 0,
        padding: "2",
        borderColor: "border",
        borderWidth: 1,
        borderRadius: "lg",
      },
      a11y: { role: "region", label: `${graph.authorityLabel} agent graph` },
      interactions: {
        onKey: [{
          key: "Escape",
          preventDefault: true,
          intent: IntentRef("DesktopAgentAction", StaticPayload({
            kind: "inspect_agent",
            agentRef: "",
          })),
        }],
      },
    },
    [
      Stack({ key: "runtime-agent-summary-row", direction: "column", gap: "1", align: "start", style: { width: "full" } }, [
        Badge({
          key: "runtime-agent-authority",
          label: graph.authorityLabel,
          tone: graph.authority === "live" ? "info" : "neutral",
        }),
        Button({
          key: "runtime-agent-toggle",
          label: `Agent stack · ${summary}`,
          variant: "ghost",
          style: { width: "full", padding: "1", borderWidth: 0, textAlign: "left" },
          onPress: IntentRef("DesktopAgentGraphToggled"),
          a11y: { label: `${expanded ? "Collapse" : "Expand"} agent stack. ${summary}` },
        }),
      ]),
      ...(expanded ? graph.rows.flatMap(row => {
        const selected = row.agentRef === selectedAgentRef
        return [
          Stack(
            {
              key: `runtime-agent-row-${row.agentRef}`,
              direction: "column",
              gap: "1",
              align: "start",
              style: { width: "full", minWidth: 0, paddingLeft: depthSpacing(row.depth) },
            },
            [
              Badge({
                key: `runtime-agent-status-${row.agentRef}`,
                label: row.statusLabel,
                tone: badgeTone(row.tone),
              }),
              Button({
                key: `runtime-agent-select-${row.agentRef}`,
                label: row.label,
                variant: selected ? "secondary" : "ghost",
                style: { width: "full", padding: "1", textAlign: "left" },
                onPress: IntentRef("DesktopAgentAction", StaticPayload({
                  kind: "inspect_agent",
                  agentRef: row.agentRef,
                })),
                a11y: { label: accessibilityLabel(row) },
              }),
              ...(row.toolLabel === null ? [] : [Text({
                key: `runtime-agent-tool-${row.agentRef}`,
                content: row.toolLabel,
                variant: "caption",
                color: "textMuted",
              })]),
            ],
          ),
          ...(selected ? [agentInspector(row, selectedTranscript)] : []),
        ]
      }) : []),
      ...(expanded && graph.hiddenCount > 0 ? [Text({
        key: "runtime-agent-overflow",
        content: `${graph.hiddenCount} more agents hidden by the Desktop safety bound`,
        variant: "caption",
        color: "textMuted",
      })] : []),
    ],
  )
}
