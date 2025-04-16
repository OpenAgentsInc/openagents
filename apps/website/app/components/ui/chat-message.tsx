"use client"

import React, { useMemo, useState } from "react"
import { cva, type VariantProps } from "class-variance-authority"

import { cn } from "@/lib/utils"
import { FilePreview } from "@/components/ui/file-preview"
import { MarkdownRenderer } from "@/components/ui/markdown-renderer"
import { ToolCall } from "@/components/ui/tool-call"
import type {
  Message,
  Attachment,
  ToolInvocation,
  MessagePart,
  TextUIPart,
  ReasoningUIPart,
  ToolInvocationUIPart
} from "@/lib/types"
import { CopyButton } from "@/components/ui/copy-button"
import { Button } from "@/components/ui/button"
import { ThumbsUp, ThumbsDown } from "lucide-react"

const chatBubbleVariants = cva(
  "group/message relative break-words rounded-lg p-3 text-sm sm:max-w-[85%]",
  {
    variants: {
      isUser: {
        // true: "bg-primary text-primary-foreground",
        true: "border-muted-foreground border bg-muted text-foreground",
        false: "bg-muted text-foreground",
      },
      animation: {
        none: "",
        slide: "",
        scale: "",
        fade: "",
      },
    },
    compoundVariants: [
      {
        isUser: true,
        animation: "slide",
        class: "",
      },
      {
        isUser: false,
        animation: "slide",
        class: "",
      },
      {
        isUser: true,
        animation: "scale",
        class: "",
      },
      {
        isUser: false,
        animation: "scale",
        class: "",
      },
    ],
  }
)

type Animation = VariantProps<typeof chatBubbleVariants>["animation"]

export interface ChatMessageProps extends Message {
  showTimeStamp?: boolean
  animation?: Animation
  actions?: React.ReactNode
  className?: string
  experimental_attachments?: Attachment[]
  onRateResponse?: (messageId: string, rating: string) => void
}

export const ChatMessage: React.FC<ChatMessageProps> = ({
  id,
  role = "assistant",
  content = "",  // Default to empty string to prevent errors
  createdAt,
  showTimeStamp = false,
  animation = "scale",
  actions,
  className,
  experimental_attachments,
  toolInvocations = [],
  parts = [],
  onRateResponse,
}) => {
  const files = useMemo(() => {
    return experimental_attachments?.map((attachment: Attachment) => {
      const dataArray = dataUrlToUint8Array(attachment.url)
      const file = new File([dataArray], attachment.name ?? "Unknown")
      return file
    })
  }, [experimental_attachments])

  const isUser = role === "user"

  // Convert createdAt to Date if it's a string
  const createdAtDate = useMemo(() => {
    if (!createdAt) return null;
    try {
      return createdAt instanceof Date ? createdAt : new Date(createdAt);
    } catch (e) {
      console.error("Error parsing date:", e);
      return null;
    }
  }, [createdAt]);

  // Format the time
  const formattedTime = useMemo(() => {
    if (!createdAtDate) return "";
    try {
      return createdAtDate.toLocaleTimeString("en-US", {
        hour: "2-digit",
        minute: "2-digit",
      });
    } catch (e) {
      console.error("Error formatting time:", e);
      return "";
    }
  }, [createdAtDate]);

  // Get ISO string for dateTime attribute
  const dateTimeISO = useMemo(() => {
    if (!createdAtDate) return "";
    try {
      return createdAtDate.toISOString();
    } catch (e) {
      console.error("Error generating ISO string:", e);
      return "";
    }
  }, [createdAtDate]);

  // Generate message parts from content and toolInvocations if parts is not provided
  const messageParts = useMemo(() => {
    // One simple log of the entire message structure
    // console.log('[ChatMessage] Message structure:', {
    //   id,
    //   role,
    //   content: content ? (content.length > 50 ? content.substring(0, 50) + '...' : content) : null,
    //   hasTools: !!toolInvocations?.length,
    //   toolCount: toolInvocations?.length || 0,
    //   hasParts: !!parts?.length,
    //   partCount: parts?.length || 0
    // });

    // CRITICAL: Always prioritize the parts array if it exists and has content
    // This is the format from the AI SDK and must be preserved
    if (parts && Array.isArray(parts) && parts.length > 0) {
      // Log parts for debugging
      // console.log('[ChatMessage] Using parts from message:',
      //   parts.map(p => ({ type: p.type, tool: p.type === 'tool-invocation' ? p.toolInvocation?.toolName : null }))
      // );

      // Don't sort - preserve the original order of parts
      // as they should already be in the correct order from the AI SDK
      return parts;
    }

    // If no parts array exists, we need to generate one from toolInvocations + content
    const generatedParts: MessagePart[] = [];

    // Only add tool invocations if available (and ONLY ONE per message)
    if (toolInvocations && Array.isArray(toolInvocations) && toolInvocations.length > 0) {
      // Only ever add the first tool to avoid duplicates
      const firstTool = toolInvocations[0];

      if (firstTool) {
        // Log for debugging
        // console.log('[ChatMessage] Generated part from toolInvocation:', firstTool.toolName);

        generatedParts.push({
          type: "tool-invocation",
          toolInvocation: firstTool
        });
      }
    }

    // Only add content part if it's not empty
    if (content && content.trim() !== '') {
      generatedParts.push({
        type: "text",
        text: content
      });
    }

    // If we have no parts at all, but we do have content, add it
    if (generatedParts.length === 0 && content) {
      return [{
        type: "text",
        text: content
      }];
    }

    return generatedParts;
  }, [parts, content, toolInvocations]);

  return (
    <div className={cn("flex flex-col", isUser ? "items-end" : "items-start")}>
      {files ? (
        <div className="mb-1 flex flex-wrap gap-2">
          {files.map((file: File, index: number) => {
            return <FilePreview file={file} key={index} />
          })}
        </div>
      ) : null}

      <div className={cn(chatBubbleVariants({ isUser, animation }), className, "p-0 mb-6 max-w-full overflow-hidden")}>
        {isUser ? (
          <div className="group w-full overflow-hidden">
            <div className="prose prose-zinc prose-sm prose-invert max-w-none prose-pre:m-0 prose-pre:bg-transparent prose-pre:p-0 text-muted-foreground overflow-hidden">
              {/* Render message parts in sequence */}
              {messageParts.map((part, index) => {
                if (part.type === "text" && "text" in part) {
                  return (
                    <div key={`text-${index}`} className="px-3 py-2">
                      <MarkdownRenderer>{part.text}</MarkdownRenderer>
                    </div>
                  );
                } else if (part.type === "tool-invocation" && "toolInvocation" in part) {
                  return (
                    <div key={`tool-${index}`} className="my-2">
                      <ToolCall toolInvocations={[part.toolInvocation]} />
                    </div>
                  );
                } else if (part.type === "reasoning" && "reasoning" in part) {
                  return (
                    <div key={`reasoning-${index}`} className="p-3 pl-4 border-l-2 border-muted-foreground/30 bg-muted/30 text-xs opacity-50 italic">
                      <MarkdownRenderer>{part.reasoning}</MarkdownRenderer>
                    </div>
                  );
                } else {
                  return null;
                }
              })}
            </div>
            <CopyButton content={content} copyMessage="Copied to clipboard" isUser={true} />
          </div>
        ) : (
          <div className="group relative w-full max-w-full break-words overflow-hidden">
            <div className="prose prose-zinc prose-sm prose-invert max-w-none prose-pre:m-0 prose-pre:bg-transparent prose-pre:p-0 text-muted-foreground overflow-hidden">
              {/* Render message parts in sequence */}
              {messageParts.map((part, index) => {
                if (part.type === "text" && "text" in part) {
                  return (
                    <div key={`text-${index}`} className="px-3 py-2">
                      <MarkdownRenderer>{part.text}</MarkdownRenderer>
                    </div>
                  );
                } else if (part.type === "tool-invocation" && "toolInvocation" in part) {
                  return (
                    <div key={`tool-${index}`} className="my-2">
                      <ToolCall toolInvocations={[part.toolInvocation]} />
                    </div>
                  );
                } else if (part.type === "reasoning" && "reasoning" in part) {
                  return (
                    <div key={`reasoning-${index}`} className="p-3 pl-4 border-l-2 border-muted-foreground/30 bg-muted/30 text-xs opacity-50 italic">
                      <MarkdownRenderer>{part.reasoning}</MarkdownRenderer>
                    </div>
                  );
                } else {
                  return null;
                }
              })}
            </div>
            <CopyButton content={content} copyMessage="Copied to clipboard" isUser={false} />
          </div>
        )}

        {actions && onRateResponse && !isUser ? (
          <div className="absolute -top-4 right-5 flex space-x-1 rounded-lg border bg-background p-1 text-foreground opacity-0 transition-opacity group-hover/message:opacity-100">
            <Button
              size="icon"
              variant="ghost"
              className="h-6 w-6"
              onClick={() => onRateResponse(id!, "thumbs-up")}
            >
              <ThumbsUp className="h-4 w-4" />
            </Button>
            <Button
              size="icon"
              variant="ghost"
              className="h-6 w-6"
              onClick={() => onRateResponse(id!, "thumbs-down")}
            >
              <ThumbsDown className="h-4 w-4" />
            </Button>
          </div>
        ) : null}
      </div>

      {showTimeStamp && createdAtDate && dateTimeISO ? (
        <time
          dateTime={dateTimeISO}
          className="mt-1 block px-1 text-xs opacity-50"
        >
          {formattedTime}
        </time>
      ) : null}
    </div>
  )
}

function dataUrlToUint8Array(data: string) {
  const base64 = data.split(",")[1]
  const buf = Buffer.from(base64, "base64")
  return new Uint8Array(buf)
}
