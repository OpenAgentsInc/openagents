"use client"

import React, { useMemo, useState } from "react"
import { cva, type VariantProps } from "class-variance-authority"
import { motion } from "framer-motion"
import { ChevronRight } from "lucide-react"

import { cn } from "@/utils/tailwind"
import {
  Collapsible,
  CollapsibleContent,
  CollapsibleTrigger,
} from "@/components/ui/collapsible"
import { FilePreview } from "@/components/ui/file-preview"
import { MarkdownRenderer, StreamedMarkdownRenderer } from "@/components/ui/markdown-renderer"
import { ToolCall } from "@/components/ui/tool-call"

type Animation = "none" | "fade" | "scale" | null | undefined

const chatBubbleVariants = cva(
  "group/message relative break-words rounded-lg p-3 text-sm",
  {
    variants: {
      isUser: {
        true: "border-muted-foreground border bg-transparent text-foreground sm:max-w-[70%]",
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

const ReasoningBlock = ({ part }: { part: ReasoningPart }) => {
  const [isOpen, setIsOpen] = useState(false)

  return (
    <div className="mb-2 flex flex-col items-start sm:max-w-[70%]">
      <Collapsible
        open={isOpen}
        onOpenChange={setIsOpen}
        className="group w-full overflow-hidden rounded-lg border bg-muted/50"
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
                {part.reasoning}
              </div>
            </div>
          </motion.div>
        </CollapsibleContent>
      </Collapsible>
    </div>
  )
}

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
  
  // Use StreamedMarkdownRenderer for assistant messages (which are likely streaming)
  // and regular MarkdownRenderer for user/system messages (which are not streaming)
  const messageContent = useMemo(() => {
    // Assistant messages use the streamed renderer for better performance with incremental updates
    return role === 'assistant' 
      ? <StreamedMarkdownRenderer>{content}</StreamedMarkdownRenderer>
      : <MarkdownRenderer>{content}</MarkdownRenderer>;
  }, [content, role])

  if (isSystem) {
    // System messages (like errors) use a special style
    // Log to console first for debugging
    console.log("SYSTEM MESSAGE CONTENT:", content);

    // For context overflow errors, show content directly (without markdown processing)
    // Also check for the special hardcoded error
    const isContextOverflowError = content.includes('context the overflows') || 
                                   content.includes('context length of only') ||
                                   content.includes('Trying to keep the first');
    
    console.log("RENDERING SYSTEM MESSAGE:", {
      content,
      isContextOverflowError,
      hasOverflows: content.includes('context the overflows'),
      hasTrying: content.includes('Trying to keep the first')
    });
    
    return (
      <div className="flex flex-col items-center w-full">
        <div className={cn(chatBubbleVariants({ isUser: false, isSystem: true, animation }))}>
          {isContextOverflowError ? (
            <div className="whitespace-pre-wrap font-mono text-sm p-1">{content}</div>
          ) : (
            messageContent
          )}
        </div>

        {showTimeStamp && createdAt ? (
          <time
            dateTime={createdAt.toISOString()}
            className={cn("mt-1 block px-1 text-xs opacity-50")}
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

        <div className={cn(chatBubbleVariants({ isUser, isSystem: false, animation }))}>
          {messageContent}
        </div>

        {showTimeStamp && createdAt ? (
          <time
            dateTime={createdAt.toISOString()}
            className={cn(
              "mt-1 block px-1 text-xs opacity-50"
            )}
          >
            {formattedTime}
          </time>
        ) : null}
      </div>
    )
  }

  if (parts && parts.length > 0) {
    // Use useMemo to memoize the mapped parts
    const renderedParts = useMemo(() => {
      return parts.map((part, index) => {
        if (part.type === "text") {
          // Use StreamedMarkdownRenderer for assistant messages which are likely streaming
          const partContent = role === 'assistant'
            ? <StreamedMarkdownRenderer>{part.text}</StreamedMarkdownRenderer>
            : <MarkdownRenderer>{part.text}</MarkdownRenderer>;
          
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
                {actions ? (
                  <div className="absolute -bottom-4 right-2 flex space-x-1 rounded-lg border bg-background p-1 text-foreground opacity-0 transition-opacity group-hover/message:opacity-100">
                    {actions}
                  </div>
                ) : null}
              </div>

              {showTimeStamp && createdAt ? (
                <time
                  dateTime={createdAt.toISOString()}
                  className={cn(
                    "mt-1 block px-1 text-xs opacity-50"
                  )}
                >
                  {formattedTime}
                </time>
              ) : null}
            </div>
          );
        } else if (part.type === "reasoning") {
          return <ReasoningBlock key={`reasoning-${index}`} part={part} />;
        } else if (part.type === "tool-invocation") {
          return (
            <ToolCall
              key={`tool-${index}`}
              toolInvocations={[part.toolInvocation]}
            />
          );
        }
        return null;
      });
    }, [parts, isUser, animation, actions, showTimeStamp, createdAt, formattedTime]);
    
    return renderedParts;
  }

  if (toolInvocations && toolInvocations.length > 0) {
    return <ToolCall toolInvocations={toolInvocations} />
  }

  return (
    <div className={cn("flex flex-col", isUser ? "items-end" : "items-start")}>
      <div className={cn(chatBubbleVariants({ isUser, isSystem: false, animation }))}>
        {messageContent}
        {actions ? (
          <div className="absolute -bottom-4 right-2 flex space-x-1 rounded-lg border bg-background p-1 text-foreground opacity-0 transition-opacity group-hover/message:opacity-100">
            {actions}
          </div>
        ) : null}
      </div>

      {showTimeStamp && createdAt ? (
        <time
          dateTime={createdAt.toISOString()}
          className={cn(
            "mt-1 block px-1 text-xs opacity-50"
          )}
        >
          {formattedTime}
        </time>
      ) : null}
    </div>
  )
});
