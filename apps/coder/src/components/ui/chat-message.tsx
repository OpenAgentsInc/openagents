import React, { useMemo, useState, useCallback } from "react"
import { cva, type VariantProps } from "class-variance-authority"
import { motion } from "framer-motion"
import { ChevronRight } from "lucide-react"

import { cn } from "@/utils/tailwind"
import {
  Collapsible,
  CollapsibleContent,
  CollapsibleTrigger,
} from "@/components/ui/collapsible"
import { CopyButton } from "@/components/ui/copy-button"
import { FilePreview } from "@/components/ui/file-preview"
import { MarkdownRenderer, StreamedMarkdownRenderer } from "@/components/ui/markdown-renderer"
import { ToolCall } from "@/components/ui/tool-call"

type Animation = "none" | "fade" | "scale" | null | undefined

const chatBubbleVariants = cva(
  "group/message relative break-words p-3 text-sm",
  {
    variants: {
      isUser: {
        true: "border border-secondary/50 bg-secondary/50 text-foreground sm:max-w-[80%]",
        false: "text-foreground w-full",
      },
      isSystem: {
        true: "border-yellow-500 border bg-yellow-50 dark:bg-yellow-900/20 text-yellow-800 dark:text-yellow-200 w-full",
        false: "",
      },
      animation: {
        none: "",
        fade: "",
        scale: "duration-500 scale-in-95",
      },
    },
  }
)

interface Attachment {
  name?: string
  contentType?: string
  url: string
}

interface PartialToolCall {
  state: "partial-call"
  toolName: string
}

interface ToolCallState {
  state: "call"
  toolName: string
}

interface ToolResult {
  state: "result"
  toolName: string
  result: {
    __cancelled?: boolean
    [key: string]: any
  }
}

type ToolInvocation = PartialToolCall | ToolCallState | ToolResult

interface ReasoningPart {
  type: "reasoning"
  reasoning: string
}

interface ToolInvocationPart {
  type: "tool-invocation"
  toolInvocation: ToolInvocation
}

interface TextPart {
  type: "text"
  text: string
}

interface SourcePart {
  type: "source"
}

type MessagePart = TextPart | ReasoningPart | ToolInvocationPart | SourcePart

export interface Message {
  id: string
  role: "user" | "assistant" | (string & {})
  content: string
  createdAt?: Date
  experimental_attachments?: Attachment[]
  toolInvocations?: ToolInvocation[]
  parts?: MessagePart[]
}

export interface ChatMessageProps extends Message {
  showTimeStamp?: boolean
  animation?: 'none' | 'fade' | 'scale'
  actions?: React.ReactNode
}

function dataUrlToUint8Array(data: string) {
  const base64 = data.split(",")[1]
  const buf = Buffer.from(base64, "base64")
  return new Uint8Array(buf)
}

// Memoize the ReasoningBlock component to prevent unnecessary rerenders
const ReasoningBlock = React.memo(function ReasoningBlock({ part }: { part: ReasoningPart }) {
  const [isOpen, setIsOpen] = useState(false)

  // Memoize the onOpenChange handler to maintain reference stability
  const handleOpenChange = useCallback((open: boolean) => {
    setIsOpen(open);
  }, []);

  // Memoize the reasoning content to prevent rendering on parent rerenders
  const reasoningContent = useMemo(() => part.reasoning, [part.reasoning]);

  return (
    <div className="mb-2 flex flex-col items-start sm:max-w-[70%]">
      <Collapsible
        open={isOpen}
        onOpenChange={handleOpenChange}
        className="group w-full overflow-hidden rounded-xl border bg-muted/50"
      >
        <CollapsibleTrigger asChild>
          <button className="w-full">
            <div className="flex items-center p-2">
              <div className="flex items-center gap-2 text-sm text-muted-foreground hover:text-foreground">
                <ChevronRight className="h-4 w-4 transition-transform group-data-[state=open]:rotate-90" />
                <span>Thinking</span>
              </div>
            </div>
          </button>
        </CollapsibleTrigger>
        <CollapsibleContent forceMount>
          <motion.div
            initial={false}
            animate={isOpen ? "open" : "closed"}
            variants={{
              open: { height: "auto", opacity: 1 },
              closed: { height: 0, opacity: 0 },
            }}
            transition={{ duration: 0.3, ease: [0.04, 0.62, 0.23, 0.98] }}
            className="border-t"
          >
            <div className="p-2">
              <div className="whitespace-pre-wrap text-xs">
                {reasoningContent}
              </div>
            </div>
          </motion.div>
        </CollapsibleContent>
      </Collapsible>
    </div>
  )
});

// Memoize the entire ChatMessage component to prevent unnecessary renders
export const ChatMessage = React.memo(function ChatMessage({
  role,
  content,
  createdAt,
  showTimeStamp = false,
  animation = 'scale',
  actions,
  experimental_attachments,
  toolInvocations,
  parts,
}: ChatMessageProps) {
  // Memoize file processing to prevent unnecessary processing on re-renders
  const files = useMemo(() => {
    return experimental_attachments?.map((attachment) => {
      const dataArray = dataUrlToUint8Array(attachment.url)
      const file = new File([dataArray], attachment.name ?? "Unknown")
      return file
    })
  }, [experimental_attachments])

  // Memoize these values to prevent recalculations
  const isUser = useMemo(() => role === "user", [role])
  const isSystem = useMemo(() => role === "system", [role])

  // Memoize the formatted time to avoid recalculations
  const formattedTime = useMemo(() => {
    return createdAt?.toLocaleTimeString("en-US", {
      hour: "2-digit",
      minute: "2-digit",
    })
  }, [createdAt])

  // Use regular markdown renderer for all messages
  // The previous attempt to optimize with StreamedMarkdownRenderer was causing issues
  const messageContent = useMemo(() => {
    // All messages now use the standard renderer
    return <MarkdownRenderer>{content}</MarkdownRenderer>;
  }, [content])

  if (isSystem) {
    // System messages (like errors) use a special style
    // Log to console first for debugging
    console.log("SYSTEM MESSAGE CONTENT:", content);

    // For context overflow errors and tool execution errors, show content directly (without markdown processing)
    // Also check for the special hardcoded error
    const isSpecialErrorFormat = 
      content.includes('context the overflows') ||
      content.includes('context length of only') ||
      content.includes('Trying to keep the first') ||
      content.includes('Error executing tool') ||
      content.includes('Authentication Failed: Bad credentials');

    // Add more detailed console logging for debugging
    console.log("RENDERING SYSTEM MESSAGE - FULL CONTENT:", JSON.stringify(content));
    console.log("RENDERING SYSTEM MESSAGE:", {
      firstChars: content.substring(0, 50),
      length: content.length,
      isSpecialErrorFormat,
      hasOverflows: content.includes('context the overflows'),
      hasTrying: content.includes('Trying to keep the first'),
      hasToolError: content.includes('Error executing tool'),
      hasAuthError: content.includes('Authentication Failed: Bad credentials'),
      hasAI_ToolExecutionError: content.includes('AI_ToolExecutionError')
    });

    return (
      <div className="flex flex-col items-center w-full">
        <div className={cn(chatBubbleVariants({ isUser: false, isSystem: true, animation }))}>
          {isSpecialErrorFormat ? (
            <div className="whitespace-pre-wrap font-mono text-sm p-1">{content}</div>
          ) : (
            messageContent
          )}
        </div>

        {showTimeStamp && createdAt ? (
          <time
            dateTime={createdAt.toISOString()}
            className={cn(
              "mt-1 block px-1 text-xs text-secondary-foreground"
            )}
          >
            {formattedTime}
          </time>
        ) : null}
      </div>
    )
  }

  if (isUser) {
    return (
      <div
        className={cn("flex flex-col", isUser ? "items-end" : "items-start")}
      >
        {files ? (
          <div className="mb-1 flex flex-wrap gap-2">
            {files.map((file, index) => {
              return <FilePreview file={file} key={index} />
            })}
          </div>
        ) : null}

        <div className={cn(chatBubbleVariants({ isUser, isSystem: false, animation }), "group/message")}>
          {messageContent}
          <div className="absolute -bottom-4 right-2 flex items-center gap-1 opacity-0 group-hover/message:opacity-100">
            <CopyButton
              content={content}
              copyMessage="Copied to clipboard"
              className="size-6 rounded-md bg-transparent p-1 hover:bg-muted-foreground/10"
            />
          </div>
        </div>

        {showTimeStamp && createdAt ? (
          <time
            dateTime={createdAt.toISOString()}
            className={cn(
              "mt-1 block px-1 text-xs text-secondary-foreground/80"
            )}
          >
            {formattedTime}
          </time>
        ) : null}
      </div>
    )
  }

  if (parts && parts.length > 0) {
    // Extract and memoize reasoning parts separately
    const reasoningParts = useMemo(() =>
      parts.filter(part => part.type === "reasoning") as ReasoningPart[],
      [parts]);

    // Extract and memoize tool invocation parts separately
    const toolInvocationParts = useMemo(() =>
      parts.filter(part => part.type === "tool-invocation") as ToolInvocationPart[],
      [parts]);

    // Extract and memoize text parts separately
    const textParts = useMemo(() =>
      parts.filter(part => part.type === "text") as TextPart[],
      [parts]);

    // Render text parts
    const renderedTextParts = useMemo(() => {
      return textParts.map((part, index) => {
        // Use standard MarkdownRenderer for all parts
        const partContent = <MarkdownRenderer>{part.text}</MarkdownRenderer>;

        return (
          <div
            className={cn(
              "flex flex-col",
              isUser ? "items-end" : "items-start"
            )}
            key={`text-${index}`}
          >
            <div className={cn(chatBubbleVariants({ isUser, animation }))}>
              {partContent}
              <div className="absolute -bottom-4 right-2 flex items-center gap-1 opacity-0 group-hover/message:opacity-100">
                {actions ? actions : (
                  <CopyButton
                    content={part.text}
                    copyMessage="Copied to clipboard"
                    className="size-6 rounded-md bg-transparent p-1 hover:bg-muted-foreground/10"
                  />
                )}
              </div>
            </div>

            {showTimeStamp && createdAt ? (
              <time
                dateTime={createdAt.toISOString()}
                className={cn(
                  "mt-1 block px-1 text-xs text-secondary-foreground/80"
                )}
              >
                {formattedTime}
              </time>
            ) : null}
          </div>
        );
      });
    }, [textParts, role, isUser, animation, actions, showTimeStamp, createdAt, formattedTime]);

    // Render reasoning parts
    const renderedReasoningParts = useMemo(() => {
      return reasoningParts.map((part, index) => (
        <ReasoningBlock key={`reasoning-${index}`} part={part} />
      ));
    }, [reasoningParts]);

    // Render tool invocation parts
    const renderedToolParts = useMemo(() => {
      return toolInvocationParts.map((part, index) => (
        <ToolCall
          key={`tool-${index}`}
          toolInvocations={[part.toolInvocation]}
        />
      ));
    }, [toolInvocationParts]);

    // Combine all rendered parts in proper order
    const renderedParts = useMemo(() => {
      // Create an array to hold all rendered parts in their original order
      const result: React.ReactNode[] = [];

      // Map through original parts to maintain order
      parts.forEach((part, index) => {
        if (part.type === "text") {
          const textIndex = textParts.findIndex(p => p === part);
          if (textIndex !== -1) {
            result.push(renderedTextParts[textIndex]);
          }
        } else if (part.type === "reasoning") {
          const reasoningIndex = reasoningParts.findIndex(p => p === part);
          if (reasoningIndex !== -1) {
            result.push(renderedReasoningParts[reasoningIndex]);
          }
        } else if (part.type === "tool-invocation") {
          const toolIndex = toolInvocationParts.findIndex(p => p === part);
          if (toolIndex !== -1) {
            result.push(renderedToolParts[toolIndex]);
          }
        }
      });

      return result;
    }, [parts, textParts, reasoningParts, toolInvocationParts, renderedTextParts, renderedReasoningParts, renderedToolParts]);

    return <>{renderedParts}</>;
  }

  if (toolInvocations && toolInvocations.length > 0) {
    return <ToolCall toolInvocations={toolInvocations} />
  }

  return (
    <div className={cn("flex flex-col", isUser ? "items-end" : "items-start")}>
      <div className={cn(chatBubbleVariants({ isUser, isSystem: false, animation }))}>
        {messageContent}
        <div className="absolute -bottom-4 right-2 flex items-center gap-1 opacity-0 group-hover/message:opacity-100">
          {actions ? actions : (
            <CopyButton
              content={content}
              copyMessage="Copied to clipboard"
              className="size-6 rounded-md bg-secondary p-1 hover:bg-muted-foreground/10"
            />
          )}
        </div>
      </div>

      {showTimeStamp && createdAt ? (
        <time
          dateTime={createdAt.toISOString()}
          className={cn(
            "mt-1 block px-1 text-xs text-secondary-foreground/80"
          )}
        >
          {formattedTime}
        </time>
      ) : null}
    </div>
  )
});
