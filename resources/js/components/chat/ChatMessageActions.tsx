import { Button } from "@/components/ui/button"
import { IconCheck, IconCopy } from "@/components/ui/icons"
import { useCopyToClipboard } from "@/lib/useCopyToClipboard"
import { cn } from "@/lib/utils"

export function ChatMessageActions({
  message,
  className,
  ...props
}: any) {
  const { isCopied, copyToClipboard } = useCopyToClipboard({ timeout: 2000 })

  const onCopy = () => {
    if (isCopied) return
    copyToClipboard(message.content)
  }

  return (
    <div
      className={cn(
        'absolute right-0 top-0 flex items-center justify-end transition-opacity group-hover:opacity-100 opacity-0 -mr-10',
        className
      )}
      {...props}
    >
      <Button variant="ghost" size="icon" onClick={onCopy} className="h-8 w-8">
        {isCopied ? <IconCheck className="h-4 w-4" /> : <IconCopy className="h-4 w-4" />}
        <span className="sr-only">Copy message</span>
      </Button>
    </div>
  )
}
