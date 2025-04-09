// @ts-nocheck - Disabling TS checks for this example refinement
import React, { useEffect, useState, useRef, useCallback } from "react";
import { useAgent } from "agents/react";
import { useAgentChat } from "agents/ai-react";
import type { Message } from "@ai-sdk/react";
import { Moon, Sun, Trash, Bot, Send } from "lucide-react"; // Removed ArrowUp - unused
import { useApiKeyContext } from "../providers/ApiKeyProvider";

import { Button } from "@/components/ui/button";
import { Card } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Avatar, AvatarFallback } from "@/components/ui/avatar";
import { Toggle } from "@/components/ui/toggle";
import { ScrollArea } from "@/components/ui/scroll-area";

export default function ChatPage() {
  const [showDebug, setShowDebug] = useState(false);
  const [latestError, setLatestError] = useState<string | null>(null);
  const messagesEndRef = useRef<HTMLDivElement>(null);

  const scrollToBottom = useCallback(() => {
    messagesEndRef.current?.scrollIntoView({ behavior: "smooth", block: "end" });
  }, []);

  // --- Initialize Agent ---
  const agent = useAgent({ agent: "coder", name: "session-124" });

  const { apiKeys } = useApiKeyContext();

  // --- Agent Chat Hook ---
  const {
    messages: agentMessages,
    input: agentInput,
    handleInputChange: handleAgentInputChange,
    handleSubmit: handleAgentSubmit,
    clearHistory,
    error: agentError,
  } = useAgentChat({
    body: {
      githubToken: apiKeys['github'] || ''
    },
    agent,
    // maxSteps: 5,
    onError: (error) => {
      console.error("Agent chat error caught by onError:", error);
      const errorString = typeof error === 'string'
        ? error
        : error instanceof Error
          ? `${error.name}: ${error.message}` + (error.cause ? `\nCause: ${error.cause}` : '') + (error.stack ? `\nStack: ${error.stack}` : '')
          : JSON.stringify(error, Object.getOwnPropertyNames(error));

      setLatestError(`Agent Error: ${errorString}`);
    }
  });

  // --- Initial Scroll ---
  useEffect(() => {
    scrollToBottom();
  }, [scrollToBottom]);

  // --- Scroll on New Messages ---
  useEffect(() => {
    if (agentMessages.length > 0) {
      // Use a small timeout to ensure content is rendered before scrolling
      const timeoutId = setTimeout(scrollToBottom, 100);
      return () => clearTimeout(timeoutId);
    }
  }, [agentMessages, scrollToBottom]);

  // --- Update latestError state if agentError from hook changes ---
  useEffect(() => {
    if (agentError) {
      console.error("Agent error from useAgentChat hook:", agentError);
      const errorString = agentError instanceof Error
        ? `${agentError.name}: ${agentError.message}` + (agentError.stack ? `\nStack: ${agentError.stack}` : '')
        : JSON.stringify(agentError);
      setLatestError(`Hook Error: ${errorString}`);
    }
  }, [agentError]);

  const formatTime = (date: Date | string | undefined) => {
    if (!date) return '';
    try {
      const d = typeof date === 'string' ? new Date(date) : date;
      if (isNaN(d.getTime())) return ''; // Invalid date
      return d.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });
    } catch (e) {
      return ''; // Handle potential errors during date parsing/formatting
    }
  };

  const handleClearHistory = () => {
    clearHistory();
    setLatestError(null);
  };

  // Get connection status from agent
  const connectionStatus = agent?.ws?.readyState;
  const getConnectionStatusDisplay = () => {
    if (!agent?.ws) return "initializing";
    switch (agent.ws.readyState) {
      case WebSocket.CONNECTING:
        return "connecting";
      case WebSocket.OPEN:
        return "connected";
      case WebSocket.CLOSING:
      case WebSocket.CLOSED:
        return "error";
      default:
        return "error";
    }
  };

  return (
    <div className="h-[100vh] pt-[30px] w-full flex justify-center items-center bg-fixed overflow-hidden">
      {/* Outer container */}
      <div className="h-[calc(100vh-2rem)] w-full max-w-3xl mx-auto flex flex-col shadow-xl rounded-md overflow-hidden relative border border-neutral-300 dark:border-neutral-800 bg-background">
        {/* Header */}
        <div className="px-4 py-3 border-b border-neutral-300 dark:border-neutral-800 flex items-center gap-3 sticky top-0 z-10 bg-inherit">
          <div className="flex items-center justify-center h-8 w-8 text-orange-500">
            <Bot size={24} />
          </div>

          {/* Agent Title & Connection Status */}
          <div className="flex-1 flex items-center">
            <h2 className="font-semibold text-base mr-2">AI Chat Agent</h2>
            {/* Connection Status Indicator */}
            <div className="flex items-center">
              {getConnectionStatusDisplay() === "connecting" && (
                <span className="text-xs text-yellow-600 rounded-full px-2 py-0.5 bg-yellow-100 dark:bg-yellow-900/30 border border-yellow-200 dark:border-yellow-800/50">
                  Connecting...
                </span>
              )}
              {getConnectionStatusDisplay() === "error" && (
                <span className="text-xs text-red-600 rounded-full px-2 py-0.5 bg-red-100 dark:bg-red-900/30 border border-red-200 dark:border-red-800/50" title={latestError || 'Connection Error'}>
                  Error
                </span>
              )}
              {getConnectionStatusDisplay() === "connected" && (
                <span className="text-xs text-green-600 rounded-full px-2 py-0.5 bg-green-100 dark:bg-green-900/30 border border-green-200 dark:border-green-800/50">
                  Connected
                </span>
              )}
            </div>
          </div>

          {/* Controls */}
          <div className="flex items-center gap-1">
            <Toggle
              size="sm"
              pressed={showDebug}
              aria-label="Toggle debug mode"
              onClick={() => setShowDebug((prev) => !prev)}
              title="Toggle Debug View"
            >
              <span className="text-xs">Debug</span>
            </Toggle>

            <Button
              variant="ghost"
              size="icon"
              className="rounded-full h-8 w-8"
              onClick={handleClearHistory}
              title="Clear Chat History"
            >
              <Trash size={16} />
            </Button>
          </div>
        </div>

        {/* Messages Area */}
        <ScrollArea className="flex-1 p-4 pb-24 h-full">
          <div className="space-y-4 min-h-full">
            {/* Welcome Message */}
            {agentMessages.length === 0 && !latestError && (
              <div className="h-[60vh] flex items-center justify-center">
                <Card className="p-6 max-w-md mx-auto bg-neutral-100 dark:bg-neutral-900">
                  <div className="text-center space-y-4">
                    <div className="bg-orange-500/10 text-orange-500 rounded-full p-3 inline-flex">
                      <Bot size={24} />
                    </div>
                    <h3 className="font-semibold text-lg">Welcome to AI Chat</h3>
                    <p className="text-muted-foreground text-sm">
                      Start a conversation with your Coder assistant.
                    </p>
                  </div>
                </Card>
              </div>
            )}

            {/* Error Display */}
            {latestError && (
              <div className="my-4 p-3 bg-red-100 dark:bg-red-900/30 border border-red-200 dark:border-red-800 rounded-md text-red-800 dark:text-red-200">
                <h4 className="font-semibold mb-1">Error</h4>
                <pre className="text-xs overflow-auto whitespace-pre-wrap max-h-40">
                  {latestError}
                </pre>
                <button
                  className="mt-2 text-xs px-2 py-1 bg-red-200 dark:bg-red-800 rounded hover:bg-red-300 dark:hover:bg-red-700"
                  onClick={() => setLatestError(null)}
                >
                  Dismiss
                </button>
              </div>
            )}

            {/* Agent Messages */}
            {agentMessages.map((m: any, index) => {
              const isUser = m.role === "user";
              const showAvatar = index === 0 || agentMessages[index - 1]?.role !== m.role;

              return (
                <div key={m.id || index}>
                  {showDebug && (
                    <pre className="text-xs text-muted-foreground overflow-scroll">
                      {JSON.stringify(m, null, 2)}
                    </pre>
                  )}
                  <div className={`flex ${isUser ? "justify-end" : "justify-start"}`}>
                    <div className={`flex gap-2 max-w-[85%] ${isUser ? "flex-row-reverse" : "flex-row"}`}>
                      {showAvatar && !isUser ? (
                        <Avatar className="h-8 w-8">
                          <AvatarFallback>AI</AvatarFallback>
                        </Avatar>
                      ) : (
                        !isUser && <div className="w-8" />
                      )}

                      <div>
                        <Card className={`p-3 rounded-md ${isUser
                          ? "rounded-br-none bg-primary text-primary-foreground"
                          : "rounded-bl-none bg-muted"}`}
                        >
                          <p className="text-sm whitespace-pre-wrap">
                            {m.content}
                          </p>
                        </Card>
                        <p className={`text-xs text-muted-foreground mt-1 ${isUser ? "text-right" : "text-left"}`}>
                          {formatTime(new Date(m.createdAt || Date.now()))}
                        </p>
                      </div>
                    </div>
                  </div>
                </div>
              );
            })}
            <div ref={messagesEndRef} />
          </div>
        </ScrollArea>

        {/* Input Area */}
        <form
          onSubmit={handleAgentSubmit}
          className="p-3 bg-background absolute bottom-0 left-0 right-0 z-10 border-t border-neutral-300 dark:border-neutral-800"
        >
          <div className="flex items-center gap-2">
            <div className="flex-1 relative">
              <Input
                placeholder="Type your message..."
                className="pl-4 pr-10 py-2 w-full rounded-full"
                value={agentInput}
                onChange={handleAgentInputChange}
                onKeyDown={(e) => {
                  if (e.key === "Enter" && !e.shiftKey) {
                    e.preventDefault();
                    handleAgentSubmit(e as unknown as React.FormEvent);
                  }
                }}
              />
            </div>

            <Button
              type="submit"
              size="icon"
              className="rounded-full h-10 w-10 flex-shrink-0"
              disabled={!agentInput.trim()}
            >
              <Send size={16} />
            </Button>
          </div>
        </form>
      </div>
    </div>
  );
}
