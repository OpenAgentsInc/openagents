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
        <div className="duration-150 relative w-full ring-1 ring-input-border ring-inset overflow-hidden bg-input hover:ring-card-border-focus hover:bg-input-hover focus-within:ring-1 focus-within:ring-input-border-focus hover:focus-within:ring-input-border-focus pb-12 px-3 rounded-3xl">
          <div className="relative z-10">
            <span className="tracking-[-0.02em] absolute px-3 py-5 text-secondary pointer-events-none">
              What should we work on today?
            </span>
            <textarea
              value={message}
              onChange={(e) => setMessage(e.target.value)}
              className="w-full px-3 bg-transparent focus:outline-none text-primary align-bottom min-h-14 pt-5 my-0 mb-5 resize-none"
              style={{ height: "44px" }}
            />
          </div>
          <div className="flex gap-1.5 absolute inset-x-0 bottom-0 border-2 border-transparent p-3">
            <div className="grow flex gap-1.5">
              <Select value={repo} onValueChange={setRepo}>
                <SelectTrigger className="h-9 px-3.5 py-2 border border-toggle-border bg-transparent hover:bg-toggle-hover text-primary">
                  <SelectValue placeholder="Select a repository..." />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value="openagents">openagents</SelectItem>
                  <SelectItem value="other">other repository</SelectItem>
                </SelectContent>
              </Select>
            </div>
            <Button
              type="submit"
              disabled={!message.trim()}
              className="h-9 relative aspect-square flex items-center justify-center rounded-full ring-inset before:absolute before:inset-0 before:rounded-full before:bg-primary before:ring-0 before:transition-all duration-500 bg-button-secondary text-secondary before:[clip-path:circle(0%_at_50%_50%)] ring-0"
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
