import { ArrowUp } from "lucide-react"
import {
  ChangeEvent, KeyboardEvent, RefObject, useCallback, useEffect, useState
} from "react"
import { Textarea } from "@/components/ui/textarea"
import { Button } from "../ui/button"

interface ChatInputProps {
  initialContent: string;
  onContentSubmit: (contentOrEvent: string | ChangeEvent<HTMLTextAreaElement>) => void;
  handleKeyDown: (e: KeyboardEvent<HTMLTextAreaElement>) => void;
  handleSubmit: () => void;
  textareaRef: RefObject<HTMLTextAreaElement>;
  isStreaming: boolean;
}

export function ChatInput({ initialContent, onContentSubmit, handleKeyDown, textareaRef, handleSubmit, isStreaming }: ChatInputProps) {
  const [localContent, setLocalContent] = useState(initialContent);

  useEffect(() => {
    setLocalContent(initialContent);
  }, [initialContent]);

  const handleContentChange = (e: ChangeEvent<HTMLTextAreaElement>) => {
    setLocalContent(e.target.value);
    onContentSubmit(e);
  };

  const handleLocalSubmit = (e: React.MouseEvent<HTMLButtonElement> | KeyboardEvent<HTMLTextAreaElement>) => {
    e.preventDefault();
    handleSubmit();
  };

  return (
    <div className="bg-background">
      <div className="max-w-3xl mx-auto px-4 py-2">
        <div className="relative">
          <Textarea
            ref={textareaRef}
            placeholder={isStreaming ? "Waiting for response..." : "Message OpenAgents"}
            rows={3}
            value={localContent}
            onChange={handleContentChange}
            onKeyDown={(e) => {
              if (e.key === "Enter" && !e.shiftKey && !isStreaming) {
                handleLocalSubmit(e);
              } else {
                handleKeyDown(e);
              }
            }}
            className={`resize-none rounded-md pr-10 ${isStreaming ? 'opacity-50' : ''}`}
          />

          <Button
            onClick={handleLocalSubmit}
            variant="ghost"
            className="absolute top-1 right-1 p-2 focus:outline-none"
          >
            <ArrowUp size={20} />
          </Button>
        </div>
        <p className="text-xs text-zinc-500 mt-2 text-center">Messages visible only to you</p>
      </div>
    </div>
  );
}