import React, { useCallback, useRef } from "react"
import { ScrollArea } from "@/components/ui/scroll-area"
import { Pane } from "@/types/pane"
import { ProseMirrorInput, ProseMirrorInputRef } from "@/components/ProseMirrorInput"

interface ChatPaneProps {
  pane: Pane;
  session?: any;
  sendMessage?: (sessionId: string, messageContent?: string) => void;
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

export const ChatPane: React.FC<ChatPaneProps> = ({ pane, session, sendMessage }) => {
  const sessionId = pane.content?.sessionId as string;
  const inputRef = useRef<ProseMirrorInputRef>(null);

  // Get session data with defaults to avoid conditional hooks
  const messages = session?.messages || [];
  const isLoading = session?.isLoading || false;
  const isInitializing = session?.isInitializing || false;

  const handleSubmit = useCallback(
    async (content: string) => {
      if (content.trim() && !isLoading && !isInitializing && session && sendMessage) {
        try {
          // Pass the message content directly to sendMessage
          await sendMessage(sessionId, content);
          // Clear the input after successful send
          inputRef.current?.clear();
        } catch (error) {
          console.error('Failed to send message:', error);
        }
      }
    },
    [sessionId, sendMessage, isLoading, isInitializing, session]
  );

  if (!session) {
    return <div>Session not found</div>;
  }

  const renderContentWithReasoning = (content: string) => {
    // Handle <thinking> tags and similar reasoning content
    const thinkingRegex = /<(?:thinking|antml:thinking)>([\s\S]*?)<\/(?:thinking|antml:thinking)>/g;
    const parts = [];
    let lastIndex = 0;
    let match;

    while ((match = thinkingRegex.exec(content)) !== null) {
      // Add content before the thinking block
      if (match.index > lastIndex) {
        parts.push(
          <span key={`content-${lastIndex}`}>
            {content.slice(lastIndex, match.index)}
          </span>
        );
      }
      
      // Add the thinking block with darker, italic styling
      parts.push(
        <span key={`thinking-${match.index}`} className="text-zinc-500 italic">
          {match[1]}
        </span>
      );
      
      lastIndex = match.index + match[0].length;
    }
    
    // Add remaining content after the last thinking block
    if (lastIndex < content.length) {
      parts.push(
        <span key={`content-${lastIndex}`}>
          {content.slice(lastIndex)}
        </span>
      );
    }
    
    return parts.length > 0 ? parts : content;
  };

  const renderMessage = (msg: Message) => {
    const isUser = msg.message_type === 'user';
    const isThinking = msg.message_type === 'thinking';
    
    return (
      <div key={msg.id} className={`text-left font-mono ${isUser ? 'mb-4' : 'mb-2'}`}>
        <div className="text-xs">
          {isUser ? (
            <div className="text-zinc-400">
              <span>&gt; </span>
              <span className="whitespace-pre-wrap">{msg.content}</span>
            </div>
          ) : isThinking ? (
            <div className="pl-2 text-zinc-300 opacity-50 italic">
              <div className="whitespace-pre-wrap">{msg.content}</div>
            </div>
          ) : (
            <div className="pl-2 text-zinc-300">
              <div className="whitespace-pre-wrap">{renderContentWithReasoning(msg.content)}</div>
              
              {/* Tool info rendering */}
              {msg.tool_info && (
                <div className="mt-2 space-y-1">
                  <div className="flex items-center gap-1">
                    <span className="text-zinc-500">[TOOL: </span>
                    <span className="text-cyan-400">{msg.tool_info.tool_name}</span>
                    <span className="text-zinc-500">]</span>
                    <span className="text-yellow-400">●</span>
                  </div>
                  
                  {msg.tool_info.input && (
                    <div className="ml-2">
                      <div className="text-zinc-600">INPUT:</div>
                      <div className="ml-2 text-zinc-400">
                        <pre className="whitespace-pre-wrap text-xs">
                          {typeof msg.tool_info.input === 'string' 
                            ? msg.tool_info.input 
                            : JSON.stringify(msg.tool_info.input, null, 2)
                          }
                        </pre>
                      </div>
                    </div>
                  )}
                  
                  {msg.tool_info.output && (
                    <div className="ml-2">
                      <div className="text-zinc-600">OUTPUT:</div>
                      <div className="ml-2 text-zinc-400">
                        <pre className="whitespace-pre-wrap text-xs">{msg.tool_info.output}</pre>
                      </div>
                    </div>
                  )}
                </div>
              )}
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
