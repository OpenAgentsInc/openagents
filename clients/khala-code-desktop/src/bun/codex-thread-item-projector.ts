import type {
  KhalaCodeDesktopChatTurnEvent,
  KhalaCodeDesktopCodexItemCard,
  KhalaCodeDesktopMessage,
  KhalaCodeDesktopMessageRole,
} from "../shared/rpc.js"
import type { CodexAppServerNotification } from "./codex-app-server-client.js"

type JsonObject = Readonly<Record<string, unknown>>

type ProjectorOptions = {
  readonly desktopTurnId: string
  readonly renderUserMessages?: boolean
}

const MAX_BODY_CHARS = 24_000

const isObject = (value: unknown): value is JsonObject =>
  typeof value === "object" && value !== null && !Array.isArray(value)

const stringField = (value: unknown, field: string): string | null => {
  if (!isObject(value)) return null
  const candidate = value[field]
  return typeof candidate === "string" && candidate.length > 0 ? candidate : null
}

const numberField = (value: unknown, field: string): number | null => {
  if (!isObject(value)) return null
  const candidate = value[field]
  return typeof candidate === "number" && Number.isFinite(candidate) ? candidate : null
}

const objectField = (value: unknown, field: string): JsonObject | null => {
  if (!isObject(value)) return null
  const candidate = value[field]
  return isObject(candidate) ? candidate : null
}

const arrayField = (value: unknown, field: string): readonly unknown[] => {
  if (!isObject(value)) return []
  const candidate = value[field]
  return Array.isArray(candidate) ? candidate : []
}

const itemId = (item: JsonObject | null): string | null => stringField(item, "id")
const itemType = (item: JsonObject | null): string => stringField(item, "type") ?? "unknown"

const threadIdFromParams = (params: unknown): string | null => stringField(params, "threadId")
const turnIdFromParams = (params: unknown): string | null =>
  stringField(params, "turnId") ?? stringField(objectField(params, "turn"), "id")

const cardContext = (input: {
  readonly requestId?: string
  readonly subtitle?: string
  readonly threadId: string | null
  readonly turnId: string | null
}): Pick<KhalaCodeDesktopCodexItemCard, "requestId" | "subtitle" | "threadId" | "turnId"> => ({
  ...(input.requestId === undefined ? {} : { requestId: input.requestId }),
  ...(input.subtitle === undefined ? {} : { subtitle: input.subtitle }),
  ...(input.threadId === null ? {} : { threadId: input.threadId }),
  ...(input.turnId === null ? {} : { turnId: input.turnId }),
})

const normalizeStatus = (status: string | null, fallback: string): string => {
  const value = status ?? fallback
  if (value === "inProgress") return "running"
  if (value === "completed") return "completed"
  if (value === "failed") return "failed"
  if (value === "interrupted") return "interrupted"
  if (value === "cancelled" || value === "canceled") return "interrupted"
  if (value === "declined" || value === "denied") return "denied"
  if (value === "pending") return "pending"
  return value
}

const humanize = (value: string): string =>
  value
    .replace(/([a-z0-9])([A-Z])/g, "$1 $2")
    .replace(/[_./-]+/g, " ")
    .replace(/\s+/g, " ")
    .trim()
    .replace(/^./, char => char.toUpperCase())

const safeJson = (value: unknown): string => {
  try {
    return JSON.stringify(value, null, 2)
  } catch {
    return String(value)
  }
}

const bounded = (body: string): string => {
  if (body.length <= MAX_BODY_CHARS) return body
  const omitted = body.length - MAX_BODY_CHARS
  return `${body.slice(0, MAX_BODY_CHARS)}\n\n[truncated ${omitted} chars]`
}

const textFromUserInput = (input: unknown): string => {
  if (!isObject(input)) return ""
  const type = stringField(input, "type")
  if (type === "text") return stringField(input, "text") ?? ""
  if (type === "image") return `[image: ${stringField(input, "url") ?? "inline"}]`
  if (type === "localImage") return `[local image: ${stringField(input, "path") ?? "unknown"}]`
  if (type === "skill") return `[@skill ${stringField(input, "name") ?? "unknown"}]`
  if (type === "mention") return `[@${stringField(input, "name") ?? "mention"}]`
  return `[${type ?? "input"}]`
}

const textList = (values: readonly unknown[]): string =>
  values
    .map(value => (typeof value === "string" ? value : safeJson(value)))
    .filter(value => value.trim().length > 0)
    .join("\n\n")

const jsonSection = (title: string, value: unknown): string => {
  if (value === undefined || value === null) return ""
  return `\n\n${title}\n\n\`\`\`json\n${safeJson(value)}\n\`\`\``
}

const commandBody = (item: JsonObject): string => {
  const command = stringField(item, "command") ?? ""
  const cwd = stringField(item, "cwd")
  const output = stringField(item, "aggregatedOutput")
  const exitCode = numberField(item, "exitCode")
  const duration = numberField(item, "durationMs")
  return bounded([
    cwd === null ? "" : `cwd: ${cwd}`,
    command.length === 0 ? "" : `\`\`\`bash\n${command}\n\`\`\``,
    output === null || output.length === 0 ? "" : `Output\n\n\`\`\`\n${output}\n\`\`\``,
    exitCode === null ? "" : `exit: ${exitCode}`,
    duration === null ? "" : `duration: ${duration}ms`,
  ].filter(Boolean).join("\n\n"))
}

const fileChangesBody = (changes: readonly unknown[]): string => {
  if (changes.length === 0) return "No file changes are available yet."
  return bounded(changes.map(change => {
    if (!isObject(change)) return safeJson(change)
    const path = stringField(change, "path") ?? "file"
    const diff = stringField(change, "diff") ?? ""
    return [
      `### ${path}`,
      diff.length === 0 ? "No diff is available yet." : `\`\`\`diff\n${diff}\n\`\`\``,
    ].join("\n\n")
  }).join("\n\n"))
}

const statesBody = (states: JsonObject | null): string => {
  if (states === null) return ""
  const lines = Object.entries(states).map(([threadId, state]) => {
    if (!isObject(state)) return `- ${threadId}: ${safeJson(state)}`
    const status = stringField(state, "status") ?? "unknown"
    const message = stringField(state, "message")
    return `- ${threadId}: ${status}${message === null ? "" : `, ${message}`}`
  })
  return lines.length === 0 ? "" : `Agent states\n\n${lines.join("\n")}`
}

const projectionForItem = (
  item: JsonObject,
  fallbackStatus: string,
  context: {
    readonly threadId: string | null
    readonly turnId: string | null
  },
): KhalaCodeDesktopMessage | null => {
  const type = itemType(item)
  const id = itemId(item)
  if (id === null) return null
  const status = normalizeStatus(stringField(item, "status"), fallbackStatus)
  let role: KhalaCodeDesktopMessageRole = "tool"
  let title = humanize(type)
  let subtitle: string | undefined
  let body = ""

  switch (type) {
    case "userMessage":
      role = "user"
      title = "User message"
      body = arrayField(item, "content").map(textFromUserInput).join("\n\n")
      break
    case "hookPrompt":
      title = "Hook prompt"
      body = arrayField(item, "fragments")
        .map(fragment => isObject(fragment) ? stringField(fragment, "text") ?? "" : "")
        .filter(Boolean)
        .join("\n\n")
      break
    case "agentMessage":
      role = "assistant"
      title = "Assistant"
      body = stringField(item, "text") ?? ""
      break
    case "plan":
      role = "assistant"
      title = "Plan"
      body = stringField(item, "text") ?? ""
      break
    case "reasoning":
      role = "assistant"
      title = "Reasoning"
      body = [
        textList(arrayField(item, "summary")),
        textList(arrayField(item, "content")),
      ].filter(Boolean).join("\n\n")
      break
    case "commandExecution":
      title = stringField(item, "source") === "userShell" ? "Shell command" : "Command"
      subtitle = stringField(item, "cwd") ?? undefined
      body = commandBody(item)
      break
    case "fileChange":
      title = "File changes"
      body = fileChangesBody(arrayField(item, "changes"))
      break
    case "mcpToolCall":
      title = `MCP: ${stringField(item, "server") ?? "server"}/${stringField(item, "tool") ?? "tool"}`
      body = bounded([
        jsonSection("Arguments", item.arguments),
        jsonSection("Result", item.result),
        jsonSection("Error", item.error),
      ].join("").trim() || "MCP call is pending.")
      break
    case "dynamicToolCall":
      title = `Dynamic tool: ${stringField(item, "tool") ?? "tool"}`
      body = bounded([
        jsonSection("Arguments", item.arguments),
        jsonSection("Content", item.contentItems),
        item.success === undefined || item.success === null ? "" : `\n\nsuccess: ${String(item.success)}`,
      ].join("").trim() || "Dynamic tool call is pending.")
      break
    case "collabAgentToolCall":
      title = `Subagent: ${humanize(stringField(item, "tool") ?? "request")}`
      subtitle = arrayField(item, "receiverThreadIds").filter(value => typeof value === "string").join(", ") || undefined
      body = bounded([
        stringField(item, "prompt") ?? "",
        stringField(item, "model") === null ? "" : `model: ${stringField(item, "model")}`,
        stringField(item, "reasoningEffort") === null ? "" : `effort: ${stringField(item, "reasoningEffort")}`,
        statesBody(objectField(item, "agentsStates")),
      ].filter(Boolean).join("\n\n"))
      break
    case "subAgentActivity":
      title = `Subagent ${humanize(stringField(item, "kind") ?? "activity")}`
      subtitle = stringField(item, "agentThreadId") ?? undefined
      body = stringField(item, "agentPath") ?? ""
      break
    case "webSearch":
      title = "Web search"
      body = stringField(item, "query") ?? ""
      break
    case "imageView":
      title = "Image"
      body = stringField(item, "path") ?? ""
      break
    case "sleep":
      title = "Sleep"
      body = `${numberField(item, "durationMs") ?? 0}ms`
      break
    case "imageGeneration":
      title = "Image generation"
      body = bounded([
        stringField(item, "revisedPrompt") ?? "",
        stringField(item, "result") ?? "",
        stringField(item, "savedPath") ?? "",
      ].filter(Boolean).join("\n\n"))
      break
    case "enteredReviewMode":
      title = "Review started"
      body = stringField(item, "review") ?? ""
      break
    case "exitedReviewMode":
      title = "Review finished"
      body = stringField(item, "review") ?? ""
      break
    case "contextCompaction":
      title = "Context compaction"
      body = "Compacting conversation history."
      break
    default:
      title = `Unknown Codex item: ${type}`
      body = `Codex emitted an item variant Khala Code does not render yet.\n\n\`\`\`json\n${safeJson(item)}\n\`\`\``
      break
  }

  const codexItem: KhalaCodeDesktopCodexItemCard = {
    itemId: id,
    itemType: type,
    status,
    title,
    ...cardContext({
      ...(subtitle === undefined ? {} : { subtitle }),
      threadId: context.threadId,
      turnId: context.turnId,
    }),
  }
  return {
    id,
    role,
    body: bounded(body),
    codexItem,
  }
}

const approvalMessage = (
  notification: CodexAppServerNotification,
  title: string,
): KhalaCodeDesktopMessage | null => {
  const params = notification.params
  if (!isObject(params)) return null
  const item = stringField(params, "itemId") ?? String(notification.id ?? "approval")
  const requestId = notification.id === undefined ? undefined : String(notification.id)
  const status = "pending"
  const body = bounded([
    stringField(params, "reason") ?? "",
    stringField(params, "cwd") === null ? "" : `cwd: ${stringField(params, "cwd")}`,
    stringField(params, "command") === null ? "" : `\`\`\`bash\n${stringField(params, "command")}\n\`\`\``,
    jsonSection("Network", params.networkApprovalContext),
    jsonSection("Permissions", params.permissions ?? params.additionalPermissions),
    jsonSection("Available decisions", params.availableDecisions),
  ].filter(Boolean).join("\n\n") || "Codex is waiting for approval.")
  return {
    id: `approval-${requestId ?? item}`,
    role: "tool",
    body,
    codexItem: {
      itemId: item,
      itemType: "approval",
      status,
      title,
      ...cardContext({
        ...(requestId === undefined ? {} : { requestId }),
        threadId: threadIdFromParams(params),
        turnId: turnIdFromParams(params),
      }),
    },
  }
}

export type CodexThreadItemEventProjector = Readonly<{
  accept: (notification: CodexAppServerNotification) => readonly KhalaCodeDesktopChatTurnEvent[]
  messages: () => readonly KhalaCodeDesktopMessage[]
}>

export function createCodexThreadItemEventProjector(
  options: ProjectorOptions,
): CodexThreadItemEventProjector {
  const messages = new Map<string, KhalaCodeDesktopMessage>()
  const doneMessages = new Set<string>()

  const upsert = (
    message: KhalaCodeDesktopMessage,
    done = false,
  ): KhalaCodeDesktopChatTurnEvent[] => {
    const previous = messages.get(message.id)
    messages.set(message.id, message)
    const events: KhalaCodeDesktopChatTurnEvent[] = [
      previous === undefined
        ? { message, turnId: options.desktopTurnId, type: "message_start" }
        : { message, turnId: options.desktopTurnId, type: "message_replace" },
    ]
    if (done && !doneMessages.has(message.id)) {
      doneMessages.add(message.id)
      events.push({ messageId: message.id, turnId: options.desktopTurnId, type: "message_done" })
    }
    return events
  }

  const appendDelta = (
    item: {
      readonly itemId: string
      readonly itemType: string
      readonly role: KhalaCodeDesktopMessageRole
      readonly title: string
    },
    delta: string,
    notification: CodexAppServerNotification,
  ): KhalaCodeDesktopChatTurnEvent[] => {
    const previous = messages.get(item.itemId)
    if (previous === undefined) {
      const message: KhalaCodeDesktopMessage = {
        id: item.itemId,
        role: item.role,
        body: "",
        codexItem: {
          itemId: item.itemId,
          itemType: item.itemType,
          status: "running",
          title: item.title,
          ...cardContext({
            threadId: threadIdFromParams(notification.params),
            turnId: turnIdFromParams(notification.params),
          }),
        },
      }
      messages.set(item.itemId, {
        ...message,
        body: bounded(delta),
      })
      return [
        { message, turnId: options.desktopTurnId, type: "message_start" },
        { delta, messageId: item.itemId, turnId: options.desktopTurnId, type: "message_delta" },
      ]
    }
    messages.set(item.itemId, {
      ...previous,
      body: bounded(`${previous.body}${delta}`),
    })
    if (item.itemType === "agentMessage") {
      return [{ delta, messageId: item.itemId, turnId: options.desktopTurnId, type: "message_delta" }]
    }
    return upsert(messages.get(item.itemId) ?? previous)
  }

  const accept = (notification: CodexAppServerNotification): readonly KhalaCodeDesktopChatTurnEvent[] => {
    const params = notification.params
    if (!isObject(params)) return []

    if (notification.method === "item/started" || notification.method === "item/completed") {
      const item = objectField(params, "item")
      if (item === null) return []
      if (itemType(item) === "userMessage" && options.renderUserMessages !== true) return []
      const message = projectionForItem(item, notification.method === "item/completed" ? "completed" : "running", {
        threadId: threadIdFromParams(params),
        turnId: turnIdFromParams(params),
      })
      return message === null ? [] : upsert(message, notification.method === "item/completed")
    }

    const delta = stringField(params, "delta") ?? ""
    const streamedItemId = stringField(params, "itemId")
    if (streamedItemId !== null && delta.length > 0) {
      if (notification.method === "item/agentMessage/delta") {
        return appendDelta({
          itemId: streamedItemId,
          itemType: "agentMessage",
          role: "assistant",
          title: "Assistant",
        }, delta, notification)
      }
      if (
        notification.method === "item/reasoning/summaryTextDelta" ||
        notification.method === "item/reasoning/textDelta"
      ) {
        return appendDelta({
          itemId: streamedItemId,
          itemType: "reasoning",
          role: "assistant",
          title: "Reasoning",
        }, delta, notification)
      }
      if (notification.method === "item/plan/delta") {
        return appendDelta({
          itemId: streamedItemId,
          itemType: "plan",
          role: "assistant",
          title: "Plan",
        }, delta, notification)
      }
      if (notification.method === "item/commandExecution/outputDelta") {
        return appendDelta({
          itemId: streamedItemId,
          itemType: "commandExecution",
          role: "tool",
          title: "Command output",
        }, delta, notification)
      }
      if (notification.method === "item/fileChange/outputDelta") {
        return appendDelta({
          itemId: streamedItemId,
          itemType: "fileChange",
          role: "tool",
          title: "File changes",
        }, delta, notification)
      }
    }

    if (notification.method === "item/fileChange/patchUpdated") {
      const id = stringField(params, "itemId")
      if (id === null) return []
      return upsert({
        id,
        role: "tool",
        body: fileChangesBody(arrayField(params, "changes")),
        codexItem: {
          itemId: id,
          itemType: "fileChange",
          status: "running",
          title: "File changes",
          ...cardContext({
            threadId: threadIdFromParams(params),
            turnId: turnIdFromParams(params),
          }),
        },
      })
    }

    if (notification.method === "item/mcpToolCall/progress") {
      const id = stringField(params, "itemId")
      const message = stringField(params, "message")
      if (id === null || message === null) return []
      return appendDelta({
        itemId: id,
        itemType: "mcpToolCall",
        role: "tool",
        title: "MCP progress",
      }, `${message}\n`, notification)
    }

    if (
      notification.method === "item/commandExecution/requestApproval" ||
      notification.method === "item/fileChange/requestApproval" ||
      notification.method === "item/permissions/requestApproval"
    ) {
      const title =
        notification.method === "item/commandExecution/requestApproval"
          ? "Command approval"
          : notification.method === "item/fileChange/requestApproval"
            ? "File change approval"
            : "Permission approval"
      const message = approvalMessage(notification, title)
      return message === null ? [] : upsert(message)
    }

    if (
      notification.method === "item/autoApprovalReview/started" ||
      notification.method === "item/autoApprovalReview/completed"
    ) {
      const reviewId = stringField(params, "reviewId") ?? "auto-review"
      return upsert({
        id: `approval-review-${reviewId}`,
        role: "tool",
        body: bounded(jsonSection("Review", params.review).trim() || "Auto-approval review is running."),
        codexItem: {
          itemId: stringField(params, "targetItemId") ?? reviewId,
          itemType: "approvalReview",
          status: notification.method.endsWith("/completed") ? "completed" : "running",
          title: "Approval review",
          ...cardContext({
            requestId: reviewId,
            threadId: threadIdFromParams(params),
            turnId: turnIdFromParams(params),
          }),
        },
      }, notification.method.endsWith("/completed"))
    }

    if (notification.method === "serverRequest/resolved") {
      const requestId = String(params.requestId ?? "")
      if (requestId.length === 0) return []
      const existing = [...messages.values()].find(message => message.codexItem?.requestId === requestId)
      if (existing === undefined || existing.codexItem === undefined) return []
      return upsert({
        ...existing,
        body: existing.body.length === 0 ? "Approval resolved." : `${existing.body}\n\nApproval resolved.`,
        codexItem: {
          ...existing.codexItem,
          status: "completed",
        },
      }, true)
    }

    return []
  }

  return {
    accept,
    messages: () => [...messages.values()],
  }
}
