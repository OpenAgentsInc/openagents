import type { ReactNode } from "react"
import { ScrollView, StyleSheet, View, type StyleProp, type TextStyle, type ViewStyle } from "react-native"

import { khalaMobileTheme } from "../theme/tokens"
import { ActivityIndicator } from "./activity-indicator"
import { KhalaButton } from "./khala-button"
import { KhalaText } from "./khala-text"

export type KhalaAgentMessageRole = "assistant" | "system" | "user"
export type KhalaToolStatus = "completed" | "failed" | "pending" | "running"
export type KhalaDiffLineKind = "add" | "context" | "hunk" | "remove"
export type KhalaDiagnosticTone = "danger" | "info" | "warning"
export type KhalaTodoStatus = "completed" | "in_progress" | "pending"

export type KhalaToolCardProps = Readonly<{
  detail?: string
  output?: string
  status?: KhalaToolStatus
  title: string
  type?: "command" | "file" | "search" | "tool"
}>

export type KhalaCodeBlockProps = Readonly<{
  code: string
  filename?: string
  language?: string
}>

export type KhalaDiffBlockProps = Readonly<{
  filename: string
  patch: string
}>

export type KhalaFileChangeRowProps = Readonly<{
  added?: number
  detail?: string
  path: string
  removed?: number
  status?: "created" | "deleted" | "modified" | "renamed"
}>

export type KhalaApprovalPromptProps = Readonly<{
  body: string
  decisions?: ReadonlyArray<string>
  title: string
}>

export type KhalaDiagnosticCardProps = Readonly<{
  body: string
  title: string
  tone?: KhalaDiagnosticTone
}>

export type KhalaTodoDockProps = Readonly<{
  todos: ReadonlyArray<{ label: string; status: KhalaTodoStatus }>
}>

export type KhalaConversationMessage = Readonly<{
  id: string
  meta?: string
  parts: ReadonlyArray<ReactNode>
  role: KhalaAgentMessageRole
  title?: string
}>

const statusColor: Record<KhalaToolStatus, string> = {
  completed: khalaMobileTheme.success,
  failed: khalaMobileTheme.danger,
  pending: khalaMobileTheme.warning,
  running: khalaMobileTheme.accent,
}

const roleLabel: Record<KhalaAgentMessageRole, string> = {
  assistant: "Khala",
  system: "System",
  user: "You",
}

const roleBorderColor: Record<KhalaAgentMessageRole, string> = {
  assistant: khalaMobileTheme.border,
  system: khalaMobileTheme.borderMuted,
  user: khalaMobileTheme.accent,
}

const diagnosticColor: Record<KhalaDiagnosticTone, string> = {
  danger: khalaMobileTheme.danger,
  info: khalaMobileTheme.accent,
  warning: khalaMobileTheme.warning,
}

const toolGlyph = (type: KhalaToolCardProps["type"]): string => {
  switch (type) {
    case "command":
      return ">"
    case "file":
      return "F"
    case "search":
      return "?"
    case "tool":
    default:
      return "*"
  }
}

const compactOutput = (value: string | undefined): string | undefined => {
  const line = value?.split(/\r?\n/u).map(item => item.trim()).find(Boolean)
  if (line === undefined) return undefined
  return line.length > 92 ? `${line.slice(0, 89)}...` : line
}

export const KhalaToolCard = ({
  detail,
  output,
  status = "completed",
  title,
  type = "tool",
}: KhalaToolCardProps) => (
  <View style={[styles.toolCard, { borderColor: status === "failed" ? khalaMobileTheme.danger : khalaMobileTheme.border }]}>
    <View style={styles.toolHeader}>
      <View style={[styles.toolGlyph, { borderColor: statusColor[status] }]}>
        {status === "running" ? (
          <ActivityIndicator color={statusColor[status]} size={22} strokeWidth={3} />
        ) : (
          <KhalaText style={[styles.toolGlyphText, { color: statusColor[status] }]} variant="mono">
            {toolGlyph(type)}
          </KhalaText>
        )}
      </View>
      <View style={styles.toolTitleColumn}>
        <KhalaText className="font-semibold text-text" numberOfLines={1} variant="caption">
          {title}
        </KhalaText>
        <KhalaText numberOfLines={1} variant="faint">
          {detail ?? compactOutput(output) ?? status}
        </KhalaText>
      </View>
      <KhalaText style={{ color: statusColor[status] }} variant="label">
        {status}
      </KhalaText>
    </View>
    {output === undefined ? null : (
      <ScrollView horizontal style={styles.outputScroll} contentContainerStyle={styles.outputContent}>
        <KhalaText style={styles.outputText} variant="mono">
          {output}
        </KhalaText>
      </ScrollView>
    )}
  </View>
)

export const KhalaCodeBlock = ({ code, filename, language }: KhalaCodeBlockProps) => {
  const lines = code.replace(/\r\n/g, "\n").replace(/\r/g, "\n").split("\n")
  return (
    <View style={styles.codeBlock}>
      {filename !== undefined || language !== undefined ? (
        <View style={styles.codeHeader}>
          <KhalaText className="min-w-0 flex-1" numberOfLines={1} variant="faint">
            {filename ?? "snippet"}
          </KhalaText>
          {language === undefined ? null : (
            <KhalaText className="text-accent" variant="faint">
              {language}
            </KhalaText>
          )}
        </View>
      ) : null}
      <ScrollView horizontal contentContainerStyle={styles.codeScroller}>
        <View>
          {lines.map((line, index) => (
            <View key={`${index}-${line}`} style={styles.codeLine}>
              <KhalaText style={styles.lineNumber} variant="mono">
                {String(index + 1).padStart(2, " ")}
              </KhalaText>
              <KhalaText style={styles.codeText} variant="mono">
                {line.length === 0 ? " " : line}
              </KhalaText>
            </View>
          ))}
        </View>
      </ScrollView>
    </View>
  )
}

const parseDiffRows = (patch: string): ReadonlyArray<{ kind: KhalaDiffLineKind; text: string }> =>
  patch
    .replace(/\r\n/g, "\n")
    .replace(/\r/g, "\n")
    .split("\n")
    .filter((line) => line.length > 0)
    .map((line) => {
      if (line.startsWith("@@")) return { kind: "hunk", text: line }
      if (line.startsWith("+") && !line.startsWith("+++")) return { kind: "add", text: line }
      if (line.startsWith("-") && !line.startsWith("---")) return { kind: "remove", text: line }
      return { kind: "context", text: line }
    })

const diffLineStyle = (kind: KhalaDiffLineKind): StyleProp<ViewStyle> => {
  switch (kind) {
    case "add":
      return styles.diffAdd
    case "remove":
      return styles.diffRemove
    case "hunk":
      return styles.diffHunk
    case "context":
    default:
      return styles.diffContext
  }
}

const diffSignColor = (kind: KhalaDiffLineKind): string => {
  switch (kind) {
    case "add":
      return khalaMobileTheme.success
    case "remove":
      return khalaMobileTheme.danger
    case "hunk":
      return khalaMobileTheme.accent
    case "context":
    default:
      return khalaMobileTheme.textFaint
  }
}

export const KhalaDiffBlock = ({ filename, patch }: KhalaDiffBlockProps) => {
  const rows = parseDiffRows(patch)
  const added = rows.filter(row => row.kind === "add").length
  const removed = rows.filter(row => row.kind === "remove").length
  return (
    <View style={styles.diffBlock}>
      <View style={styles.diffHeader}>
        <KhalaText className="min-w-0 flex-1" numberOfLines={1} variant="faint">
          {filename}
        </KhalaText>
        <KhalaText style={{ color: khalaMobileTheme.success }} variant="faint">
          +{added}
        </KhalaText>
        <KhalaText style={{ color: khalaMobileTheme.danger }} variant="faint">
          -{removed}
        </KhalaText>
      </View>
      <ScrollView horizontal contentContainerStyle={styles.codeScroller}>
        <View>
          {rows.map((row, index) => (
            <View key={`${index}-${row.text}`} style={[styles.diffLine, diffLineStyle(row.kind)]}>
              <KhalaText style={[styles.diffSign, { color: diffSignColor(row.kind) }]} variant="mono">
                {row.kind === "add" ? "+" : row.kind === "remove" ? "-" : row.kind === "hunk" ? "@" : " "}
              </KhalaText>
              <KhalaText style={styles.codeText} variant="mono">
                {row.text.replace(/^[+-]/, "")}
              </KhalaText>
            </View>
          ))}
        </View>
      </ScrollView>
    </View>
  )
}

export const KhalaFileChangeRow = ({
  added = 0,
  detail,
  path,
  removed = 0,
  status = "modified",
}: KhalaFileChangeRowProps) => (
  <View style={styles.fileRow}>
    <View style={styles.fileBadge}>
      <KhalaText className="text-accent" variant="faint">
        {status === "created" ? "A" : status === "deleted" ? "D" : status === "renamed" ? "R" : "M"}
      </KhalaText>
    </View>
    <View style={styles.fileText}>
      <KhalaText numberOfLines={1} variant="caption">
        {path}
      </KhalaText>
      {detail === undefined ? null : (
        <KhalaText numberOfLines={1} variant="faint">
          {detail}
        </KhalaText>
      )}
    </View>
    <View style={styles.fileStats}>
      <KhalaText style={{ color: khalaMobileTheme.success }} variant="faint">
        +{added}
      </KhalaText>
      <KhalaText style={{ color: khalaMobileTheme.danger }} variant="faint">
        -{removed}
      </KhalaText>
    </View>
  </View>
)

export const KhalaApprovalPrompt = ({
  body,
  decisions = ["Allow once", "Allow session", "Deny"],
  title,
}: KhalaApprovalPromptProps) => (
  <View style={styles.approvalCard}>
    <View style={styles.promptHeader}>
      <KhalaText className="text-warning" variant="label">
        Permission
      </KhalaText>
      <KhalaText className="font-semibold text-text" variant="caption">
        {title}
      </KhalaText>
    </View>
    <KhalaText variant="muted">{body}</KhalaText>
    <View style={styles.promptActions}>
      {decisions.map((decision, index) => (
        <KhalaButton
          key={decision}
          className="min-h-0 px-3 py-2"
          text={decision}
          textClassName="text-[12px] leading-[16px]"
          variant={index === 0 ? "primary" : index === decisions.length - 1 ? "danger" : "secondary"}
        />
      ))}
    </View>
  </View>
)

export const KhalaDiagnosticCard = ({ body, title, tone = "info" }: KhalaDiagnosticCardProps) => (
  <View style={[styles.diagnosticCard, { borderColor: diagnosticColor[tone] }]}>
    <KhalaText style={{ color: diagnosticColor[tone] }} variant="label">
      {tone}
    </KhalaText>
    <KhalaText className="font-semibold text-text" variant="caption">
      {title}
    </KhalaText>
    <KhalaText className="mt-1" variant="muted">
      {body}
    </KhalaText>
  </View>
)

export const KhalaTodoDock = ({ todos }: KhalaTodoDockProps) => {
  const completed = todos.filter(todo => todo.status === "completed").length
  return (
    <View style={styles.todoDock}>
      <View style={styles.todoHeader}>
        <KhalaText className="text-text" variant="caption">
          {completed}/{todos.length} tasks
        </KhalaText>
        <KhalaText variant="faint">Plan</KhalaText>
      </View>
      <View style={styles.todoList}>
        {todos.map(todo => (
          <View key={todo.label} style={styles.todoRow}>
            <View
              style={[
                styles.todoCheck,
                todo.status === "completed"
                  ? styles.todoCompleted
                  : todo.status === "in_progress"
                    ? styles.todoActive
                    : styles.todoPending,
              ]}
            >
              <KhalaText style={styles.todoCheckText} variant="faint">
                {todo.status === "completed" ? "✓" : todo.status === "in_progress" ? "•" : ""}
              </KhalaText>
            </View>
            <KhalaText
              className={todo.status === "completed" ? "text-textFaint" : "text-text"}
              numberOfLines={2}
              variant="caption"
            >
              {todo.label}
            </KhalaText>
          </View>
        ))}
      </View>
    </View>
  )
}

export const KhalaTerminalPanel = ({ output, title = "Terminal" }: Readonly<{ output: string; title?: string }>) => (
  <View style={styles.terminalPanel}>
    <View style={styles.codeHeader}>
      <KhalaText className="text-accent" variant="faint">
        {title}
      </KhalaText>
      <KhalaText variant="faint">zsh</KhalaText>
    </View>
    <ScrollView horizontal contentContainerStyle={styles.outputContent}>
      <KhalaText style={styles.outputText} variant="mono">
        {output}
      </KhalaText>
    </ScrollView>
  </View>
)

export const KhalaConversationBubble = ({
  meta,
  parts,
  role,
  title,
}: Omit<KhalaConversationMessage, "id">) => (
  <View style={[styles.bubble, { borderColor: roleBorderColor[role] }, role === "user" ? styles.userBubble : null]}>
    <View style={styles.bubbleHeader}>
      <KhalaText className={role === "user" ? "text-accent" : "text-text"} variant="label">
        {title ?? roleLabel[role]}
      </KhalaText>
      {meta === undefined ? null : (
        <KhalaText variant="faint">
          {meta}
        </KhalaText>
      )}
    </View>
    <View style={styles.bubbleParts}>{parts.map((part, index) => <View key={index}>{part}</View>)}</View>
  </View>
)

export const KhalaCodingConversation = ({ messages }: Readonly<{ messages: ReadonlyArray<KhalaConversationMessage> }>) => (
  <ScrollView style={styles.conversation} contentContainerStyle={styles.conversationContent}>
    {messages.map(message => (
      <KhalaConversationBubble
        key={message.id}
        meta={message.meta}
        parts={message.parts}
        role={message.role}
        title={message.title}
      />
    ))}
  </ScrollView>
)

const styles = StyleSheet.create({
  approvalCard: {
    backgroundColor: "rgba(237, 190, 89, 0.08)",
    borderColor: khalaMobileTheme.warning,
    borderRadius: 8,
    borderWidth: 1,
    gap: 10,
    padding: 12,
  },
  bubble: {
    backgroundColor: khalaMobileTheme.surface,
    borderRadius: 8,
    borderWidth: 1,
    gap: 10,
    padding: 12,
  },
  bubbleHeader: {
    alignItems: "center",
    flexDirection: "row",
    justifyContent: "space-between",
  },
  bubbleParts: { gap: 10 },
  codeBlock: {
    backgroundColor: "#050a12",
    borderColor: khalaMobileTheme.border,
    borderRadius: 8,
    borderWidth: 1,
    overflow: "hidden",
  },
  codeHeader: {
    alignItems: "center",
    backgroundColor: khalaMobileTheme.surfaceRaised,
    borderBottomColor: khalaMobileTheme.border,
    borderBottomWidth: 1,
    flexDirection: "row",
    gap: 10,
    justifyContent: "space-between",
    paddingHorizontal: 10,
    paddingVertical: 8,
  },
  codeLine: {
    flexDirection: "row",
    minHeight: 22,
  },
  codeScroller: {
    padding: 10,
  },
  codeText: {
    color: khalaMobileTheme.textBody,
    fontSize: 12,
    lineHeight: 20,
  } satisfies TextStyle,
  conversation: { flex: 1 },
  conversationContent: {
    gap: 12,
    padding: 16,
    paddingBottom: 28,
  },
  diagnosticCard: {
    backgroundColor: khalaMobileTheme.surfaceRaised,
    borderRadius: 8,
    borderWidth: 1,
    padding: 12,
  },
  diffAdd: { backgroundColor: "rgba(53, 208, 127, 0.11)" },
  diffBlock: {
    backgroundColor: "#050a12",
    borderColor: khalaMobileTheme.border,
    borderRadius: 8,
    borderWidth: 1,
    overflow: "hidden",
  },
  diffContext: { backgroundColor: "transparent" },
  diffHeader: {
    alignItems: "center",
    backgroundColor: khalaMobileTheme.surfaceRaised,
    borderBottomColor: khalaMobileTheme.border,
    borderBottomWidth: 1,
    flexDirection: "row",
    gap: 10,
    paddingHorizontal: 10,
    paddingVertical: 8,
  },
  diffHunk: { backgroundColor: "rgba(79, 208, 255, 0.1)" },
  diffLine: {
    flexDirection: "row",
    minHeight: 22,
    paddingRight: 8,
  },
  diffRemove: { backgroundColor: "rgba(228, 90, 90, 0.12)" },
  diffSign: {
    fontSize: 12,
    lineHeight: 20,
    textAlign: "center",
    width: 22,
  } satisfies TextStyle,
  fileBadge: {
    alignItems: "center",
    borderColor: khalaMobileTheme.border,
    borderRadius: 6,
    borderWidth: 1,
    height: 28,
    justifyContent: "center",
    width: 28,
  },
  fileRow: {
    alignItems: "center",
    backgroundColor: khalaMobileTheme.surfaceRaised,
    borderColor: khalaMobileTheme.border,
    borderRadius: 8,
    borderWidth: 1,
    flexDirection: "row",
    gap: 10,
    padding: 10,
  },
  fileStats: {
    alignItems: "flex-end",
    gap: 2,
  },
  fileText: {
    flex: 1,
    minWidth: 0,
  },
  lineNumber: {
    color: khalaMobileTheme.textFaint,
    fontSize: 12,
    lineHeight: 20,
    paddingRight: 10,
    textAlign: "right",
    width: 34,
  } satisfies TextStyle,
  outputContent: {
    padding: 10,
  },
  outputScroll: {
    borderTopColor: khalaMobileTheme.borderMuted,
    borderTopWidth: 1,
    maxHeight: 138,
  },
  outputText: {
    color: khalaMobileTheme.textBody,
    fontSize: 12,
    lineHeight: 19,
  } satisfies TextStyle,
  promptActions: {
    flexDirection: "row",
    flexWrap: "wrap",
    gap: 8,
  },
  promptHeader: { gap: 2 },
  terminalPanel: {
    backgroundColor: "#050a12",
    borderColor: khalaMobileTheme.border,
    borderRadius: 8,
    borderWidth: 1,
    overflow: "hidden",
  },
  todoActive: {
    backgroundColor: "rgba(79, 208, 255, 0.16)",
    borderColor: khalaMobileTheme.accent,
  },
  todoCheck: {
    alignItems: "center",
    borderRadius: 999,
    borderWidth: 1,
    height: 22,
    justifyContent: "center",
    width: 22,
  },
  todoCheckText: {
    color: khalaMobileTheme.accent,
    fontSize: 12,
    lineHeight: 16,
  } satisfies TextStyle,
  todoCompleted: {
    backgroundColor: "rgba(53, 208, 127, 0.12)",
    borderColor: khalaMobileTheme.success,
  },
  todoDock: {
    backgroundColor: khalaMobileTheme.surfaceRaised,
    borderColor: khalaMobileTheme.border,
    borderRadius: 8,
    borderWidth: 1,
    gap: 10,
    padding: 12,
  },
  todoHeader: {
    alignItems: "center",
    flexDirection: "row",
    justifyContent: "space-between",
  },
  todoList: { gap: 8 },
  todoPending: {
    backgroundColor: khalaMobileTheme.surface,
    borderColor: khalaMobileTheme.border,
  },
  todoRow: {
    alignItems: "center",
    flexDirection: "row",
    gap: 10,
  },
  toolCard: {
    backgroundColor: khalaMobileTheme.surfaceRaised,
    borderRadius: 8,
    borderWidth: 1,
    overflow: "hidden",
  },
  toolGlyph: {
    alignItems: "center",
    borderRadius: 6,
    borderWidth: 1,
    height: 32,
    justifyContent: "center",
    width: 32,
  },
  toolGlyphText: {
    fontSize: 15,
    lineHeight: 20,
  } satisfies TextStyle,
  toolHeader: {
    alignItems: "center",
    flexDirection: "row",
    gap: 10,
    padding: 10,
  },
  toolTitleColumn: {
    flex: 1,
    minWidth: 0,
  },
  userBubble: {
    backgroundColor: "rgba(79, 208, 255, 0.08)",
  },
})
