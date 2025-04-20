// src/components/agent/MinimalSolverConnector.tsx
import React, { useState, useEffect, useCallback, useRef } from "react";
import { Button } from "../button/Button";
import { Card, CardContent } from "../card/Card";
import { Spinner } from "../loader/Loader";
import { Terminal, AlertTriangle, PowerOff, CheckCircle } from "lucide-react";
import { cn } from "../../lib/utils";
import { Input } from "../input/Input";
import type { UIMessage } from "ai";
import type { MinimalAgentHook } from "../../hooks/useOpenAgent_Minimal"; // Adjust path

interface MinimalSolverConnectorProps {
  agent: MinimalAgentHook; // Use the minimal hook type
  className?: string;
}

// --- MessageContainer Component (Simplified Scroll Logic) ---
const MessageContainer = ({ children, messages }: { children: React.ReactNode, messages: UIMessage[] }) => {
  const messagesEndRef = useRef<HTMLDivElement>(null);
  useEffect(() => { messagesEndRef.current?.scrollIntoView({ behavior: "smooth" }); }, [messages]);
  return (
    <div className="flex-1 px-4 min-h-0 overflow-y-auto" style={{ overscrollBehavior: 'contain' }}>
      {children}
      <div ref={messagesEndRef} />
    </div>
  );
};
// --- End MessageContainer ---

// --- Simple Message Component ---
const MessageList = ({ messages }: { messages: UIMessage[] }) => {
  return (
    <div className="space-y-4 py-4">
      {messages.map((message) => (
        <div 
          key={message.id} 
          className={`flex ${message.role === 'user' ? 'justify-end' : 'justify-start'}`}
        >
          <div 
            className={`max-w-[80%] rounded-lg px-4 py-2 ${
              message.role === 'user' 
                ? 'bg-primary text-primary-foreground' 
                : 'bg-muted text-muted-foreground'
            }`}
          >
            {message.content}
          </div>
        </div>
      ))}
    </div>
  );
};
// --- End Simple Message Component ---

export function MinimalSolverConnector({ agent, className = "" }: MinimalSolverConnectorProps) {
  const { messages, connectionStatus, sendMessage } = agent;
  const [inputValue, setInputValue] = useState('');
  const [isHydrated, setIsHydrated] = useState(false);

  useEffect(() => { setIsHydrated(true); }, []);

  const handleFormSubmit = (e: React.FormEvent<HTMLFormElement>) => {
    e.preventDefault();
    if (!inputValue.trim() || connectionStatus !== 'connected') return;
    sendMessage(inputValue);
    setInputValue(''); // Clear input after sending
  };

  // Render logic based on connectionStatus
  const renderContent = () => {
    if (!isHydrated && typeof window !== "undefined") {
        return <div className="flex items-center justify-center h-full"><Spinner/></div>;
    }

    switch (connectionStatus) {
      case 'connecting':
        return <div className="flex flex-col items-center justify-center h-full"><Spinner className="h-8 w-8 mb-2" /><p className="text-muted-foreground text-sm">Connecting...</p></div>;
      case 'error':
        return <div className="flex flex-col items-center justify-center h-full"><AlertTriangle className="h-8 w-8 mb-2 text-destructive" /><p className="text-muted-foreground text-sm text-center px-4">Connection error.</p></div>;
      case 'disconnected':
         return <div className="flex flex-col items-center justify-center h-full"><PowerOff className="h-8 w-8 mb-2 text-muted-foreground" /><p className="text-muted-foreground text-sm text-center px-4">Agent disconnected.</p></div>;
      case 'connected':
        return (
          <div className="h-full flex flex-col"> {/* Flex container */}
            <MessageContainer messages={messages}>
              <MessageList messages={messages} />
            </MessageContainer>
            <div className="px-4 py-3 border-t flex-shrink-0">
              <form onSubmit={handleFormSubmit} className="flex items-center">
                <Input
                  type="text" name="message" autoFocus autoComplete="off"
                  placeholder="Send a message..." className="flex-1"
                  value={inputValue}
                  onChange={(e) => setInputValue(e.target.value)}
                  disabled={connectionStatus !== 'connected'}
                />
                <Button type="submit" size="sm" className="ml-2" variant="outline" disabled={connectionStatus !== 'connected' || !inputValue.trim()}>
                  Send
                </Button>
              </form>
            </div>
          </div>
        );
      default:
        return null;
    }
  };

  return (
    <Card className={cn("h-full flex flex-col py-0 overflow-hidden", className)}>
      <CardContent className="flex-1 flex flex-col overflow-hidden p-0 pt-0">
        {renderContent()}
      </CardContent>
    </Card>
  );
}