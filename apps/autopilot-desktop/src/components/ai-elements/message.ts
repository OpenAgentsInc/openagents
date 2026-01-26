import { html } from "../../effuse/template/html.js"
import type { TemplateResult } from "../../effuse/template/types.js"
import type { CodexMessage, CodexMessageRole } from "../../types/codex.js"
import { renderMarkdown } from "./markdown.js"

export type MessageRole = CodexMessageRole

export type MessageProps = Pick<CodexMessage, "role" | "text"> & {
  readonly isStreaming?: boolean
}

const roleWrapperClasses: Record<string, string> = {
  user: "ml-auto items-end",
  assistant: "mr-auto items-start",
  system: "mr-auto items-start",
}

const roleBubbleClasses: Record<string, string> = {
  user:
    "border border-white/30 text-white/90 bg-transparent",
  assistant:
    "border border-white/10 text-white/70 bg-transparent",
  system:
    "border border-white/10 text-white/50 bg-transparent",
}

export const Message = ({
  role,
  text,
  isStreaming = false,
}: MessageProps): TemplateResult => html`
  <article class="flex w-full max-w-[90%] flex-col gap-2 ${roleWrapperClasses[role] ?? roleWrapperClasses.assistant}">
    <div class="flex flex-col gap-2 px-3 py-2 text-sm leading-relaxed ${roleBubbleClasses[role] ?? roleBubbleClasses.assistant}">
      <div class="break-words">${renderMarkdown(text)}</div>
      ${
        isStreaming
          ? html`<span class="text-[10px] uppercase text-white/30">Streaming</span>`
          : ""
      }
    </div>
  </article>
`
