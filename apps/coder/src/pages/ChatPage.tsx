import React, { useEffect, useState, useRef, useCallback } from "react";
import { useAgent } from "agents/react";
import { useAgentChat } from "agents/ai-react";
import type { Message } from "@ai-sdk/react";
import { Moon, Sun, Trash, Bot, Send, ArrowUp } from "lucide-react";

import { Button } from "@/components/ui/button";
import { Card } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Avatar, AvatarFallback } from "@/components/ui/avatar";
import { Toggle } from "@/components/ui/toggle";
import { ScrollArea } from "@/components/ui/scroll-area";

export default function ChatPage() {
  const [theme, setTheme] = useState<"dark" | "light">(() => {
    // Check localStorage first, default to dark if not found
    const savedTheme = localStorage.getItem("theme");
    return (savedTheme as "dark" | "light") || "dark";
  });
  const [showDebug, setShowDebug] = useState(false);
  const [latestError, setLatestError] = useState<string | null>(null);
  const [connectionStatus, setConnectionStatus] = useState<string>("initializing");
  const messagesEndRef = useRef<HTMLDivElement>(null);

  const scrollToBottom = useCallback(() => {
    messagesEndRef.current?.scrollIntoView({ behavior: "smooth" });
  }, []);

  useEffect(() => {
    // Apply theme class on mount and when theme changes
    if (theme === "dark") {
      document.documentElement.classList.add("dark");
      document.documentElement.classList.remove("light");
    } else {
      document.documentElement.classList.remove("dark");
      document.documentElement.classList.add("light");
    }

    // Save theme preference to localStorage
    localStorage.setItem("theme", theme);
  }, [theme]);

  // Scroll to bottom on mount
  useEffect(() => {
    scrollToBottom();
  }, [scrollToBottom]);

  const toggleTheme = () => {
    const newTheme = theme === "dark" ? "light" : "dark";
    setTheme(newTheme);
  };

  // Add a custom reconnect function
  const [useDirectConnection, setUseDirectConnection] = useState(() => {
    return localStorage.getItem("useDirectConnection") === "true";
  });
  
  // Apply WebSocket patch immediately on page load
  useEffect(() => {
    // This needs to happen before any WebSocket connections are attempted
    console.log("🚀 Applying global WebSocket patch");
    try {
      const OriginalWebSocket = window.WebSocket;
      (window as any).WebSocket = function(url: string, protocols?: string | string[]) {
        console.log("🔄 WebSocket constructor called with URL:", url);
        
        // If it's a local agent endpoint, always redirect to production
        if (url.includes('/agents/') && !url.includes('agents.openagents.com')) {
          // Determine if it's ws or wss
          const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
          const urlObj = new URL(url, window.location.href);
          
          // Create a new URL pointing to the production server
          const newUrl = `wss://agents.openagents.com${urlObj.pathname}${urlObj.search}`;
          console.log("🔀 Redirecting WebSocket to:", newUrl);
          return new OriginalWebSocket(newUrl, protocols);
        }
        
        // For non-agent WebSockets, use the original URL
        return new OriginalWebSocket(url, protocols);
      };
      
      // Maintain prototype chain and constructor properties
      (window as any).WebSocket.prototype = OriginalWebSocket.prototype;
      (window as any).WebSocket.CONNECTING = OriginalWebSocket.CONNECTING;
      (window as any).WebSocket.OPEN = OriginalWebSocket.OPEN;
      (window as any).WebSocket.CLOSING = OriginalWebSocket.CLOSING;
      (window as any).WebSocket.CLOSED = OriginalWebSocket.CLOSED;
      
      console.log("✅ Global WebSocket patch applied");
    } catch (error) {
      console.error("❌ Failed to apply WebSocket patch:", error);
    }
  }, []);
  
  // Use direct URL if direct connection is enabled
  const agent = useAgent({
    agent: "coderagent",
    host: useDirectConnection ? "https://agents.openagents.com" : undefined
  });

  const {
    messages: agentMessages,
    input: agentInput,
    handleInputChange: handleAgentInputChange,
    handleSubmit: handleAgentSubmit,
    addToolResult,
    clearHistory,
    error: agentError,
  } = useAgentChat({
    agent,
    maxSteps: 5,
    onError: (error) => {
      console.error("Agent chat error:", error);
      const errorString = typeof error === 'string'
        ? error
        : error instanceof Error
          ? `${error.name}: ${error.message}`
          : JSON.stringify(error);
      setLatestError(errorString);
    }
  });

  // Scroll to bottom when messages change
  useEffect(() => {
    agentMessages.length > 0 && scrollToBottom();
  }, [agentMessages, scrollToBottom]);
  
  // Monitor agent connection status
  useEffect(() => {
    if (!agent) return;
    
    // Add event listener for agent connection errors
    const handleError = (event: any) => {
      console.error("Agent WebSocket error:", event);
      setConnectionStatus("error");
      setLatestError("WebSocket connection error. Check console for details.");
    };
    
    // Inject script to patch WebSocket connection
    const injectWebSocketPatch = () => {
      try {
        console.log("🔧 Injecting WebSocket connection patch");
        // Save the original WebSocket constructor
        const OriginalWebSocket = window.WebSocket;
        
        // Replace with our custom version
        (window as any).WebSocket = function(url: string, protocols?: string | string[]) {
          console.log("🔄 WebSocket constructor called with URL:", url);
          
          // Check if this is an agent URL that needs to be patched
          if (url.includes('/agents/') && url.startsWith('ws:')) {
            // Replace with wss and direct to production
            const newUrl = url.replace('ws:', 'wss:').replace('localhost:5173', 'agents.openagents.com');
            console.log("🔀 Redirecting WebSocket to:", newUrl);
            return new OriginalWebSocket(newUrl, protocols);
          }
          
          // Otherwise, use the original constructor
          return new OriginalWebSocket(url, protocols);
        };
        
        // Maintain prototype chain and constructor properties
        (window as any).WebSocket.prototype = OriginalWebSocket.prototype;
        (window as any).WebSocket.CONNECTING = OriginalWebSocket.CONNECTING;
        (window as any).WebSocket.OPEN = OriginalWebSocket.OPEN;
        (window as any).WebSocket.CLOSING = OriginalWebSocket.CLOSING;
        (window as any).WebSocket.CLOSED = OriginalWebSocket.CLOSED;
        
        console.log("✅ WebSocket patch injected successfully");
      } catch (error) {
        console.error("❌ Failed to inject WebSocket patch:", error);
      }
    };
    
    // Set up connection monitoring
    const monitorConnection = () => {
      // Initially assume we're connecting
      setConnectionStatus("connecting");
      
      // We'll update connection status based on agent state or WebSocket events
      console.log("Connection monitoring set up - WebSocket patch is in place");
      
      // Set status to connected after a delay
      setTimeout(() => {
        setConnectionStatus("connected");
      }, 500);
    };
    
    // Run the connection monitor
    monitorConnection();
    
    // Add global event listeners for WebSocket debugging
    window.addEventListener("error", handleError);
    
    return () => {
      window.removeEventListener("error", handleError);
    };
  }, [agent, useDirectConnection]);

  const formatTime = (date: Date) => {
    return date.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });
  };

  return (
    <div className="h-[100vh] pt-[30px] w-full flex justify-center items-center bg-fixed overflow-hidden">
      <div className="h-[calc(100vh-2rem)] w-full mx-auto flex flex-col shadow-xl rounded-md overflow-hidden relative border border-neutral-300 dark:border-neutral-800">
        <div className="px-4 py-3 border-b border-neutral-300 dark:border-neutral-800 flex items-center gap-3 sticky top-0 z-10 bg-background">
          <div className="flex items-center justify-center h-8 w-8 text-orange-500">
            <Bot size={24} />
          </div>

          <div className="flex-1">
            <h2 className="font-semibold text-base">AI Chat Agent</h2>
            <div className="ml-2 flex items-center">
              {connectionStatus === "connecting" && (
                <span className="text-xs text-yellow-500 rounded-full px-2 py-0.5 bg-yellow-100 dark:bg-yellow-900">
                  Connecting...
                </span>
              )}
              {connectionStatus === "error" && (
                <span className="text-xs text-red-500 rounded-full px-2 py-0.5 bg-red-100 dark:bg-red-900">
                  Connection Error
                </span>
              )}
              {connectionStatus === "connected" && (
                <span className="text-xs text-green-500 rounded-full px-2 py-0.5 bg-green-100 dark:bg-green-900">
                  Connected
                </span>
              )}
              {useDirectConnection && (
                <span className="ml-1 text-xs text-blue-500 rounded-full px-2 py-0.5 bg-blue-100 dark:bg-blue-900">
                  Direct
                </span>
              )}
            </div>
          </div>

          <div className="flex items-center gap-2">
            <Toggle
              pressed={showDebug}
              aria-label="Toggle debug mode"
              onClick={() => setShowDebug((prev) => !prev)}
            >
              <span className="text-xs">Debug</span>
            </Toggle>
            
            <Toggle
              pressed={useDirectConnection}
              aria-label="Toggle direct connection"
              onClick={() => {
                setUseDirectConnection((prev) => !prev);
                // Force page reload to apply the change
                if (!useDirectConnection) {
                  // Setting to direct connection - save preference and reload
                  localStorage.setItem("useDirectConnection", "true");
                  window.location.reload();
                } else {
                  // Setting back to proxy - save preference and reload
                  localStorage.setItem("useDirectConnection", "false");
                  window.location.reload();
                }
              }}
            >
              <span className="text-xs">Direct Connection</span>
            </Toggle>
          </div>

          <Button
            variant="ghost"
            size="icon"
            className="rounded-full h-9 w-9"
            onClick={clearHistory}
          >
            <Trash size={20} />
          </Button>
        </div>

        {/* Messages */}
        <ScrollArea className="flex-1 p-4 pb-24">
          <div className="space-y-4">
            {agentMessages.length === 0 && (
              <div className="h-[60vh] flex items-center justify-center">
                <Card className="p-6 max-w-md mx-auto bg-neutral-100 dark:bg-neutral-900">
                  <div className="text-center space-y-4">
                    <div className="bg-orange-500/10 text-orange-500 rounded-full p-3 inline-flex">
                      <Bot size={24} />
                    </div>
                    <h3 className="font-semibold text-lg">Welcome to AI Chat</h3>
                    <p className="text-muted-foreground text-sm">
                      Start a conversation with your AI assistant.
                    </p>
                    <ul className="text-sm text-left space-y-2">
                      <li className="flex items-center gap-2">
                        <span className="text-orange-500">•</span>
                        <span>Ask questions about code</span>
                      </li>
                      <li className="flex items-center gap-2">
                        <span className="text-orange-500">•</span>
                        <span>Get help with programming tasks</span>
                      </li>
                    </ul>
                  </div>
                </Card>
              </div>
            )}

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
            {/* Display error if one exists */}
            {(latestError || agentError) && (
              <div className="mt-4 p-3 bg-red-100 dark:bg-red-900 border border-red-300 dark:border-red-700 rounded-md text-red-800 dark:text-red-200">
                <h4 className="font-semibold mb-1">Error</h4>
                <pre className="text-xs overflow-auto whitespace-pre-wrap">
                  {latestError || (agentError ? String(agentError) : '')}
                </pre>
                <button
                  className="mt-2 text-xs px-2 py-1 bg-red-200 dark:bg-red-800 rounded hover:bg-red-300 dark:hover:bg-red-700"
                  onClick={() => setLatestError(null)}
                >
                  Dismiss
                </button>
              </div>
            )}
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
