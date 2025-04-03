"use client"

import { Check, Copy } from "lucide-react"

import { cn } from "@/utils/tailwind"
import { useCopyToClipboard } from "@/hooks/use-copy-to-clipboard"
import { Button } from "@/components/ui/button"

export interface CopyButtonProps {
  content: string
  copyMessage?: string
  className?: string
}

export function CopyButton({ content, copyMessage = "Copied!", className }: CopyButtonProps) {
  const { isCopied, handleCopy } = useCopyToClipboard({
    text: content,
    copyMessage,
  })

  return (
    <Button
      variant="ghost"
      size="icon"
      className={cn("relative size-6", className)}
      aria-label="Copy to clipboard"
      onClick={handleCopy}
    >
      <div className="absolute inset-0 flex items-center justify-center">
        <Check
          className={cn(
            "h-4 w-4 transition-transform ease-in-out",
            isCopied ? "scale-100" : "scale-0"
          )}
        />
      </div>
      <Copy
        className={cn(
          "h-4 w-4 transition-transform ease-in-out",
          isCopied ? "scale-0" : "scale-100"
        )}
      />
    </Button>
  )
}
