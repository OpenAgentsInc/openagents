export type ChatRole = "user" | "assistant"

export type ChatTextPart = {
  readonly type: "text"
  readonly text: string
  readonly state?: "streaming" | "done"
}

export type ChatToolPart = {
  readonly type: `tool-${string}` | "dynamic-tool"
  readonly toolName?: string
  readonly toolCallId: string
  readonly state: string
  readonly input?: unknown
  readonly output?: unknown
  readonly errorText?: string
  readonly preliminary?: boolean
  readonly approval?: { readonly id: string; readonly approved?: boolean; readonly reason?: string }
  readonly rawInput?: unknown
}

export type ChatPart =
  | ChatTextPart
  | ChatToolPart
  | { readonly type: string; readonly [k: string]: unknown }

export type ChatMessage = {
  readonly id: string
  readonly role: ChatRole
  readonly parts: ReadonlyArray<ChatPart>
}
