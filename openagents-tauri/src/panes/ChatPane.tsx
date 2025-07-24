import React from "react"
import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import { ScrollArea } from "@/components/ui/scroll-area"
import { Pane } from "@/types/pane"

interface ChatPaneProps {
  pane: Pane;
  session?: any;
  sendMessage?: (sessionId: string) => void;
  updateSessionInput?: (sessionId: string, value: string) => void;
}

// This will be replaced with actual session data from the parent component
interface Message {
  id: string;
  message_type: string;
  content: string;
  timestamp: string;
  tool_info?: {
    tool_name: string;
    tool_use_id: string;
    input: Record<string, any>;
    output?: string;
  };
}

export const ChatPane: React.FC<ChatPaneProps> = ({ pane, session, sendMessage, updateSessionInput }) => {
  const sessionId = pane.content?.sessionId as string;

  if (!session) {
    return <div>Session not found</div>;
  }

  const messages = session.messages || [];
  const inputMessage = session.inputMessage || "";
  const isLoading = session.isLoading || false;
  const isInitializing = session.isInitializing || false;

  const renderMessage = (msg: Message) => {
    const messageTypeStyles: Record<string, string> = {
      user: "bg-primary/10 border-primary/20",
      assistant: "bg-secondary/10 border-secondary/20",
      tool_use: "bg-accent/10 border-accent/20",
      error: "bg-destructive/10 border-destructive/20 text-destructive",
      summary: "bg-muted/50 border-muted",
      thinking: "bg-muted/30 border-muted italic",
      system: "bg-muted/20 border-muted",
    };

    const style = messageTypeStyles[msg.message_type] || "bg-muted/10 border-muted";

    return (
      <div key={msg.id} className={`border p-4 overflow-hidden ${style}`}>
        <div className="text-xs font-semibold uppercase opacity-70 mb-1">
          {msg.message_type}
        </div>
        <div className="text-sm overflow-hidden">
          <div className="whitespace-pre-wrap break-all font-mono overflow-x-auto overflow-y-hidden">
            {msg.content}
          </div>
          {msg.tool_info && msg.tool_info.output && (
            <details className="mt-2">
              <summary className="cursor-pointer text-xs font-medium hover:underline">
                Tool Output
              </summary>
              <div className="mt-2 text-xs opacity-80 whitespace-pre-wrap break-all font-mono overflow-x-auto overflow-y-hidden">
                {msg.tool_info.output}
              </div>
            </details>
          )}
        </div>
      </div>
    );
  };

  return (
    <div className="flex flex-col h-full">
      {isInitializing ? (
        // Loading state while initializing Claude
        <div className="flex-1 flex items-center justify-center">
          <div className="text-center space-y-4">
            <div className="animate-pulse">
              <div className="w-16 h-16 mx-auto bg-primary/20 rounded-full flex items-center justify-center">
                <div className="w-12 h-12 bg-primary/30 rounded-full flex items-center justify-center">
                  <div className="w-8 h-8 bg-primary/40 rounded-full"></div>
                </div>
              </div>
            </div>
            <div>
              <p className="text-sm font-medium">Initializing Claude Code...</p>
              <p className="text-xs text-muted-foreground mt-1">This may take a moment</p>
            </div>
          </div>
        </div>
      ) : (
        <>
          {/* Messages */}
          <ScrollArea className="flex-1 -mx-4 px-4">
            {messages.length === 0 ? (
              <p className="text-sm text-muted-foreground">
                No messages yet. Send a message to start the conversation.
              </p>
            ) : (
              <div className="space-y-4">
                {messages.map(renderMessage)}
              </div>
            )}
          </ScrollArea>

          {/* Input */}
          <div className="mt-4 -mx-4 px-4 pt-4 border-t border-border">
            <div className="flex gap-2">
              <Input
                type="text"
                value={inputMessage}
                onChange={(e) => updateSessionInput?.(sessionId, e.target.value)}
                onKeyPress={(e) => {
                  if (e.key === "Enter") {
                    sendMessage?.(sessionId);
                  }
                }}
                placeholder="Type your message..."
                disabled={isLoading}
                className="flex-1 text-sm"
                autoFocus
              />
              <Button
                onClick={() => sendMessage?.(sessionId)}
                disabled={isLoading || !inputMessage.trim()}
                size="sm"
              >
                Send
              </Button>
            </div>
          </div>
        </>
      )}
    </div>
  );
};
