import { Github } from "lucide-react"
import { useState } from "react"
import TextareaAutosize from "react-textarea-autosize"
import { RepoSelector } from "~/components/repo-selector"
import { Button } from "~/components/ui/button"
import {
  Popover, PopoverContent, PopoverTrigger
} from "~/components/ui/popover"
import { cn } from "~/lib/utils"

interface Repo {
  owner: string
  name: string
  branch: string
}

interface ChatInputProps
  extends Omit<React.ComponentProps<"form">, "onSubmit"> {
  onSubmit?: (message: string, repos: Repo[]) => void;
}

export function ChatInput({ className, onSubmit, ...props }: ChatInputProps) {
  const [message, setMessage] = useState("");
  const [selectedRepos, setSelectedRepos] = useState<Repo[]>([]);
  const [isAddingRepo, setIsAddingRepo] = useState(false);

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    if (message.trim()) {
      onSubmit?.(message.trim(), selectedRepos);
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
          "pb-[4.5rem] px-3"
        )}>
          <div className="relative z-10">
            <TextareaAutosize
              autoFocus={true}
              value={message}
              onChange={(e) => setMessage(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === 'Enter' && !e.shiftKey) {
                  e.preventDefault();
                  if (message.trim()) {
                    onSubmit?.(message.trim(), selectedRepos);
                    setMessage("");
                  }
                }
              }}
              minRows={1}
              maxRows={12}
              placeholder="Give OpenAgents a task"
              className={cn(
                "border-input placeholder:text-muted-foreground",
                "w-full px-3 bg-transparent focus:outline-none text-primary",
                "align-bottom min-h-14 py-5 my-0 mb-5 resize-none",
                "disabled:cursor-not-allowed disabled:opacity-50"
              )}
            />
          </div>
          <div className="absolute inset-x-0 bottom-0 p-3">
            <div className="flex items-center gap-1.5">
              <Popover open={isAddingRepo} onOpenChange={setIsAddingRepo}>
                <PopoverTrigger asChild>
                  <Button
                    type="button"
                    className={cn(
                      "border-input ring-ring/10 dark:ring-ring/20",
                      "h-9 px-3.5 py-2 border bg-transparent",
                      "text-primary hover:bg-accent hover:text-accent-foreground",
                      "shadow-xs transition-[color,box-shadow]",
                      "focus-visible:ring-4 focus-visible:outline-1",
                      "!rounded-none shrink-0"
                    )}
                  >
                    <Github className="w-4 h-4" />
                  </Button>
                </PopoverTrigger>
                <PopoverContent sideOffset={4}>
                  <form onSubmit={(e: React.FormEvent) => {
                    e.preventDefault();
                    const form = e.target as HTMLFormElement;
                    const owner = (form.elements.namedItem('owner') as HTMLInputElement).value;
                    const name = (form.elements.namedItem('name') as HTMLInputElement).value;
                    const branch = (form.elements.namedItem('branch') as HTMLInputElement).value || 'main';
                    setSelectedRepos([...selectedRepos, { owner, name, branch }]);
                    setIsAddingRepo(false);
                    form.reset();
                  }} className="space-y-2">
                    <input
                      type="text"
                      name="owner"
                      placeholder="Owner"
                      className={cn(
                        "w-full p-2 border rounded text-sm",
                        "bg-background dark:bg-background",
                        "text-foreground dark:text-foreground",
                        "border-input dark:border-input",
                        "focus:outline-none focus:border-ring dark:focus:border-ring",
                        "placeholder:text-muted-foreground dark:placeholder:text-muted-foreground"
                      )}
                      autoComplete="off"
                    />
                    <input
                      type="text"
                      name="name"
                      placeholder="Repo name"
                      className={cn(
                        "w-full p-2 border rounded text-sm",
                        "bg-background dark:bg-background",
                        "text-foreground dark:text-foreground",
                        "border-input dark:border-input",
                        "focus:outline-none focus:border-ring dark:focus:border-ring",
                        "placeholder:text-muted-foreground dark:placeholder:text-muted-foreground"
                      )}
                      autoComplete="off"
                    />
                    <input
                      type="text"
                      name="branch"
                      placeholder="Branch (defaults to main)"
                      className={cn(
                        "w-full p-2 border rounded text-sm",
                        "bg-background dark:bg-background",
                        "text-foreground dark:text-foreground",
                        "border-input dark:border-input",
                        "focus:outline-none focus:border-ring dark:focus:border-ring",
                        "placeholder:text-muted-foreground dark:placeholder:text-muted-foreground"
                      )}
                      autoComplete="off"
                    />
                    <Button
                      type="submit"
                      className="w-full"
                    >
                      Add Repository
                    </Button>
                  </form>
                </PopoverContent>
              </Popover>
              <div className="flex-1 overflow-x-auto hide-scrollbar">
                <RepoSelector
                  selectedRepos={selectedRepos}
                  onReposChange={setSelectedRepos}
                  className="flex items-center gap-1.5"
                  showOnlyAddButton={true}
                />
              </div>
              <Button
                type="submit"
                disabled={!message.trim() && !selectedRepos.length}
                className={cn(
                  "border-input ring-ring/10 dark:ring-ring/20",
                  "h-9 relative aspect-square",
                  "flex items-center justify-center",
                  "bg-transparent text-primary",
                  "shadow-xs transition-[color,box-shadow]",
                  "hover:bg-accent hover:text-accent-foreground",
                  "focus-visible:ring-4 focus-visible:outline-1",
                  "disabled:cursor-not-allowed disabled:opacity-50",
                  "shrink-0",
                  "border !rounded-none"
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
      </div>
    </form>
  );
}
