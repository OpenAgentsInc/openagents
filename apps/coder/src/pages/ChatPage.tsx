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

  const agent = useAgent({
    agent: "coderagent",
  });

  const {
    messages: agentMessages,
    input: agentInput,
    handleInputChange: handleAgentInputChange,
    handleSubmit: handleAgentSubmit,
    addToolResult,
    clearHistory,
  } = useAgentChat({
    agent,
    maxSteps: 5,
  });

  // Scroll to bottom when messages change
  useEffect(() => {
    agentMessages.length > 0 && scrollToBottom();
  }, [agentMessages, scrollToBottom]);

  const formatTime = (date: Date) => {
    return date.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });
  };

  return (
    <div className="h-[100vh] w-full p-4 flex justify-center items-center bg-fixed overflow-hidden">
      <div className="h-[calc(100vh-2rem)] w-full mx-auto max-w-lg flex flex-col shadow-xl rounded-md overflow-hidden relative border border-neutral-300 dark:border-neutral-800">
        <div className="px-4 py-3 border-b border-neutral-300 dark:border-neutral-800 flex items-center gap-3 sticky top-0 z-10 bg-background">
          <div className="flex items-center justify-center h-8 w-8 text-orange-500">
            <Bot size={24} />
          </div>

          <div className="flex-1">
            <h2 className="font-semibold text-base">AI Chat Agent</h2>
          </div>

          <div className="flex items-center gap-2">
            <Toggle
              pressed={showDebug}
              aria-label="Toggle debug mode"
              onClick={() => setShowDebug((prev) => !prev)}
            >
              <span className="text-xs">Debug</span>
            </Toggle>
          </div>

          <Button
            variant="ghost"
            size="icon"
            className="rounded-full h-9 w-9"
            onClick={toggleTheme}
          >
            {theme === "dark" ? <Sun size={20} /> : <Moon size={20} />}
          </Button>

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
