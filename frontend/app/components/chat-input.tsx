import { useState } from "react"
import { Button } from "~/components/ui/button"
import {
  Select, SelectContent, SelectItem, SelectTrigger, SelectValue
} from "~/components/ui/select"
import { cn } from "~/lib/utils"

interface ChatInputProps
  extends Omit<React.ComponentProps<"form">, "onSubmit"> {
  onSubmit?: (message: string, repo: string | undefined) => void;
}

export function ChatInput({ className, onSubmit, ...props }: ChatInputProps) {
  const [message, setMessage] = useState("");
  const [repo, setRepo] = useState<string>();

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    if (message.trim()) {
      onSubmit?.(message.trim(), repo);
      setMessage("");
    }
  };

  return (
    <form
      onSubmit={handleSubmit}
      className={cn(
        "flex flex-col gap-2 items-center justify-center relative z-10",
        className
      )}
      {...props}
    >
      <div className="w-full max-w-[50rem] relative">
        <div className={cn(
          "border-input ring-ring/10 dark:ring-ring/20 dark:outline-ring/40 outline-ring/50",
          "relative w-full border bg-transparent overflow-hidden",
          "shadow-xs transition-[color,box-shadow]",
          "focus-within:ring-4 focus-within:outline-1",
          "hover:bg-accent/15 hover:text-accent-foreground",
          "pb-12 px-3"
        )}>
          <div className="relative z-10">
            <textarea
              autoFocus={true}
              value={message}
              onChange={(e) => setMessage(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === 'Enter' && !e.shiftKey) {
                  e.preventDefault();
                  if (message.trim()) {
                    onSubmit?.(message.trim(), repo);
                    setMessage("");
                  }
                }
              }}
              placeholder="Give OpenAgents a task"
              className={cn(
                "border-input placeholder:text-muted-foreground",
                "w-full px-3 bg-transparent focus:outline-none text-primary",
                "align-bottom min-h-14 py-5 my-0 mb-5 resize-none",
                "disabled:cursor-not-allowed disabled:opacity-50"
              )}
              style={{ height: "44px" }}
            />
          </div>
          <div className="flex gap-1.5 absolute inset-x-0 bottom-0 p-3">
            <div className="grow flex gap-1.5">
              <Select value={repo} onValueChange={setRepo}>
                <SelectTrigger className={cn(
                  "border-input ring-ring/10 dark:ring-ring/20",
                  "h-9 px-3.5 py-2 border bg-transparent",
                  "text-primary hover:bg-accent hover:text-accent-foreground",
                  "shadow-xs transition-[color,box-shadow]",
                  "focus-visible:ring-4 focus-visible:outline-1",
                  "!rounded-none"
                )}>
                  <SelectValue placeholder="Select a repository..." />
                </SelectTrigger>
                <SelectContent className="!rounded-none">
                  <SelectItem value="openagents">openagents</SelectItem>
                  <SelectItem value="other">other repository</SelectItem>
                </SelectContent>
              </Select>
            </div>
            <Button
              type="submit"
              disabled={!message.trim()}
              className={cn(
                "border-input ring-ring/10 dark:ring-ring/20",
                "h-9 relative aspect-square",
                "flex items-center justify-center",
                "bg-transparent text-primary",
                "shadow-xs transition-[color,box-shadow]",
                "hover:bg-accent hover:text-accent-foreground",
                "focus-visible:ring-4 focus-visible:outline-1",
                "disabled:cursor-not-allowed disabled:opacity-50"
              )}
            >
              <svg
                width="20"
                height="20"
                viewBox="0 0 24 24"
                fill="none"
                xmlns="http://www.w3.org/2000/svg"
                className="stroke-[2] relative"
              >
                <path
                  d="M5 11L12 4M12 4L19 11M12 4V21"
                  stroke="currentColor"
                />
              </svg>
            </Button>
          </div>
        </div>
      </div>
    </form>
  );
}
