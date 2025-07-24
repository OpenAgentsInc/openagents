import React, { useCallback, useRef } from "react"
import { ScrollArea } from "@/components/ui/scroll-area"
import { Pane } from "@/types/pane"
import { ProseMirrorInput, ProseMirrorInputRef } from "@/components/ProseMirrorInput"

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
  const inputRef = useRef<ProseMirrorInputRef>(null);

  // Get session data with defaults to avoid conditional hooks
  const messages = session?.messages || [];
  const isLoading = session?.isLoading || false;
  const isInitializing = session?.isInitializing || false;

  const handleSubmit = useCallback(
    async (content: string) => {
      if (content.trim() && !isLoading && !isInitializing && session) {
        // Update session input with the content
        updateSessionInput?.(sessionId, content);
        
        // Call sendMessage which will handle the actual sending
        try {
          await sendMessage?.(sessionId);
          // Clear the input after successful send
          inputRef.current?.clear();
        } catch (error) {
          console.error('Failed to send message:', error);
        }
      }
    },
    [sessionId, sendMessage, updateSessionInput, isLoading, isInitializing, session]
  );

  if (!session) {
    return <div>Session not found</div>;
  }

  const renderMessage = (msg: Message) => {
    return (
      <div key={msg.id} className="p-4 overflow-hidden font-mono text-sm text-foreground">
        <div className="overflow-hidden">
          <div className="whitespace-pre-wrap break-all overflow-x-auto overflow-y-hidden">
            {msg.content}
          </div>
          {msg.tool_info && msg.tool_info.output && (
            <details className="mt-2">
              <summary className="cursor-pointer text-xs font-medium hover:underline">
                Tool Output
              </summary>
              <div className="mt-2 text-xs opacity-80 whitespace-pre-wrap break-all overflow-x-auto overflow-y-hidden">
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
      <div className="mt-4 -mx-4 px-4 pt-4">
        <ProseMirrorInput
          ref={inputRef}
          placeholder={isInitializing ? "Start typing (initializing...)..." : "Type your message..."}
          onSubmit={handleSubmit}
          disabled={isLoading || isInitializing}
        />
      </div>
    </div>
  );
};
