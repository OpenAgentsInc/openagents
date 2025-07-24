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
      user: "bg-input-terminal border-border",
      assistant: "bg-input-terminal border-border",
      tool_use: "bg-input-terminal border-border",
      error: "bg-input-terminal border-border",
      summary: "bg-input-terminal border-border",
      thinking: "bg-input-terminal border-border italic",
      system: "bg-input-terminal border-border",
    };

    const style = messageTypeStyles[msg.message_type] || "bg-input-terminal border-border";

    // Terminal-style message formatting
    const getMessagePrefix = () => {
      switch (msg.message_type) {
        case 'user':
          return <span className="text-foreground">{'> '}</span>;
        case 'assistant':
          return <span className="text-model-claude">[Claude] </span>;
        case 'tool_use':
          return (
            <span className="text-tool-name">
              {msg.tool_info?.tool_name ? `[${msg.tool_info.tool_name}] ` : '[Tool] '}
            </span>
          );
        case 'error':
          return <span className="text-terminal-error">[ERROR] </span>;
        case 'thinking':
          return <span className="text-muted-foreground">[Thinking] </span>;
        default:
          return <span className="text-muted-foreground">{`[${msg.message_type.toUpperCase()}] `}</span>;
      }
    };

    return (
      <div key={msg.id} className={`border p-4 overflow-hidden ${style} font-mono`}>
        <div className="text-sm overflow-hidden">
          <div className="whitespace-pre-wrap break-all overflow-x-auto overflow-y-hidden">
            {getMessagePrefix()}
            <span className={msg.message_type === 'error' ? 'text-terminal-error' : 'text-foreground'}>
              {msg.content}
            </span>
          </div>
          {msg.tool_info && msg.tool_info.output && (
            <details className="mt-2">
              <summary className="cursor-pointer text-xs font-medium hover:underline text-tool-name">
                ▶ Tool Output
              </summary>
              <div className="mt-2 text-xs opacity-80 whitespace-pre-wrap break-all overflow-x-auto overflow-y-hidden">
                <span className="text-terminal-success">OUTPUT: </span>
                <span className="text-foreground">{msg.tool_info.output}</span>
              </div>
            </details>
          )}
          {msg.tool_info && msg.tool_info.input && (
            <div className="mt-2 text-xs opacity-60">
              <span className="text-muted-foreground">INPUT: </span>
              <span className="text-muted-foreground">{JSON.stringify(msg.tool_info.input, null, 2)}</span>
            </div>
          )}
        </div>
      </div>
    );
  };

  return (
    <div className="flex flex-col h-full">
      {/* Messages */}
      <ScrollArea className="flex-1 -mx-4 px-4">
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
                <p className="text-xs text-muted-foreground mt-1">You can start typing below</p>
              </div>
            </div>
          </div>
        ) : messages.length === 0 ? (
          <p className="text-sm text-muted-foreground">
            No messages yet. Send a message to start the conversation.
          </p>
        ) : (
          <div className="space-y-4">
            {messages.map(renderMessage)}
          </div>
        )}
      </ScrollArea>

      {/* Input - Always shown */}
      <div className="mt-4 -mx-4 px-4 pt-4 border-t border-border bg-input-terminal">
        <div className="flex gap-2 items-center">
          <span className="text-foreground font-mono text-sm">{'>'}</span>
          <Input
            type="text"
            value={inputMessage}
            onChange={(e) => updateSessionInput?.(sessionId, e.target.value)}
            onKeyPress={(e) => {
              if (e.key === "Enter" && !isInitializing) {
                sendMessage?.(sessionId);
              }
            }}
            placeholder={isInitializing ? "Start typing (initializing...)..." : "Type your message..."}
            disabled={isLoading}
            className="flex-1 text-sm font-mono bg-transparent border-none focus-visible:ring-0 focus-visible:border-none px-2"
            autoFocus
          />
          <Button
            onClick={() => sendMessage?.(sessionId)}
            disabled={isLoading || isInitializing || !inputMessage.trim()}
            size="sm"
            className="bg-primary text-primary-foreground hover:bg-primary/90"
          >
            Send
          </Button>
        </div>
      </div>
    </div>
  );
};
