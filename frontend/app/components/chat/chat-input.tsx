import { useRef, useState } from "react";
import { RepoSelector } from "./repo-selector";

export interface ChatInputProps {
  onSubmit: (message: string, repos?: string[]) => void;
  disabled?: boolean; // Add disabled prop
}

export function ChatInput({ onSubmit, disabled }: ChatInputProps) {
  const [message, setMessage] = useState("");
  const [repos, setRepos] = useState<string[]>([]);
  const textareaRef = useRef<HTMLTextAreaElement>(null);

  const handleSubmitMessage = () => {
    if (!message.trim()) return;
    onSubmit(message, repos.length > 0 ? repos : undefined);
    setMessage("");
    if (textareaRef.current) {
      textareaRef.current.style.height = "auto";
    }
  };

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    handleSubmitMessage();
  };

  const handleKeyDown = (e: React.KeyboardEvent<HTMLTextAreaElement>) => {
    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      handleSubmitMessage();
    }
  };

  const handleAddRepo = (repo: string) => {
    setRepos([...repos, repo]);
  };

  return (
    <div className="relative">
      <form onSubmit={handleSubmit} className="flex flex-col gap-2">
        <RepoSelector
          repos={repos}
          onAdd={handleAddRepo}
          onRemove={(index) => {
            const newRepos = [...repos];
            newRepos.splice(index, 1);
            setRepos(newRepos);
          }}
        />
        <div className="flex gap-2">
          <textarea
            ref={textareaRef}
            value={message}
            onChange={(e) => {
              setMessage(e.target.value);
              // Auto-resize textarea
              e.target.style.height = "auto";
              e.target.style.height = e.target.scrollHeight + "px";
            }}
            onKeyDown={handleKeyDown}
            placeholder="Send a message..."
            className="flex-1 resize-none rounded-md border border-input bg-background px-3 py-2 text-sm ring-offset-background placeholder:text-muted-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 disabled:cursor-not-allowed disabled:opacity-50"
            rows={1}
            disabled={disabled} // Add disabled prop
          />
          <button
            type="submit"
            className="shrink-0 rounded-md bg-primary px-3 py-2 text-sm font-semibold text-primary-foreground shadow-sm hover:bg-primary/90 focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-primary disabled:cursor-not-allowed disabled:opacity-50"
            disabled={!message.trim() || disabled} // Add disabled prop
          >
            Send
          </button>
        </div>
      </form>
    </div>
  );
}