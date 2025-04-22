// src/components/agent/MinimalSolverConnector.tsx
import React, { useState, useEffect, useCallback, useRef } from "react";
import type { UIMessage } from "ai";
import type { MinimalAgentHook } from "../../hooks/useOpenAgent_Minimal";

// Define a simplified Card component
const Card = ({ className = "", children }: { className?: string, children: React.ReactNode }) => (
  <div className={`rounded-lg border bg-card text-card-foreground shadow-sm ${className}`}>
    {children}
  </div>
);

// Define a simplified CardContent component
const CardContent = ({ className = "", children }: { className?: string, children: React.ReactNode }) => (
  <div className={`p-6 ${className}`}>
    {children}
  </div>
);

// Define a simplified Button component
const Button = ({ 
  className = "", 
  children, 
  disabled = false, 
  type = "button", 
  size = "default",
  variant = "default",
  onClick
}: { 
  className?: string, 
  children: React.ReactNode, 
  disabled?: boolean, 
  type?: "button" | "submit" | "reset",
  size?: "default" | "sm" | "lg" | "icon",
  variant?: "default" | "primary" | "secondary" | "ghost" | "destructive" | "tertiary" | "outline",
  onClick?: () => void
}) => (
  <button 
    className={`inline-flex items-center justify-center rounded-md font-medium transition-colors 
    ${variant === "outline" ? "border border-input bg-background hover:bg-accent hover:text-accent-foreground" : ""}
    ${size === "sm" ? "h-9 px-3 text-xs" : "h-10 px-4 py-2"}
    ${disabled ? "opacity-50 cursor-not-allowed" : ""}
    ${className}`}
    disabled={disabled}
    type={type}
    onClick={onClick}
  >
    {children}
  </button>
);

// Define a simplified Input component
const Input = ({ 
  className = "", 
  type = "text", 
  placeholder = "", 
  value = "", 
  onChange,
  name = "",
  autoFocus = false,
  autoComplete = "off",
  disabled = false
}: { 
  className?: string, 
  type?: string, 
  placeholder?: string, 
  value?: string, 
  onChange?: (e: React.ChangeEvent<HTMLInputElement>) => void,
  name?: string,
  autoFocus?: boolean,
  autoComplete?: string,
  disabled?: boolean
}) => (
  <input 
    className={`flex h-10 w-full rounded-md border border-input bg-background px-3 py-2 text-sm ${className}`}
    type={type}
    placeholder={placeholder}
    value={value}
    onChange={onChange}
    name={name}
    autoFocus={autoFocus}
    autoComplete={autoComplete}
    disabled={disabled}
  />
);

// Define a simplified Spinner component
const Spinner = ({ className = "" }: { className?: string }) => (
  <div className={`animate-spin rounded-full h-4 w-4 border-b-2 border-primary ${className}`}></div>
);

// Define utility function for class names
const cn = (...classes: (string | undefined)[]) => {
  return classes.filter(Boolean).join(' ');
};

// Icons as SVG for simplicity
const Terminal = () => (
  <svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
    <polyline points="4 17 10 11 4 5"></polyline>
    <line x1="12" y1="19" x2="20" y2="19"></line>
  </svg>
);

const AlertTriangle = () => (
  <svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
    <path d="m21.73 18-8-14a2 2 0 0 0-3.48 0l-8 14A2 2 0 0 0 4 21h16a2 2 0 0 0 1.73-3Z"></path>
    <line x1="12" y1="9" x2="12" y2="13"></line>
    <line x1="12" y1="17" x2="12.01" y2="17"></line>
  </svg>
);

const PowerOff = () => (
  <svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
    <path d="M18.36 6.64a9 9 0 1 1-12.73 0"></path>
    <line x1="12" y1="2" x2="12" y2="12"></line>
  </svg>
);

// Simple message component
const MessageListItem = ({ message }: { message: UIMessage }) => {
  return (
    <div className={`flex ${message.role === 'user' ? 'justify-end' : 'justify-start'} mb-4`}>
      <div className={`max-w-[80%] rounded-lg px-4 py-2 ${
        message.role === 'user' 
          ? 'bg-blue-500 text-white' 
          : 'bg-gray-200 dark:bg-gray-700 text-gray-900 dark:text-white'
      }`}>
        {message.content}
      </div>
    </div>
  );
};

const MessageList = ({ messages }: { messages: UIMessage[] }) => (
  <div className="space-y-4">
    {messages.map((message) => (
      <MessageListItem key={message.id} message={message} />
    ))}
  </div>
);

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
        return <div className="flex flex-col items-center justify-center h-full"><AlertTriangle /><p className="text-muted-foreground text-sm text-center px-4">Connection error.</p></div>;
      case 'disconnected':
         return <div className="flex flex-col items-center justify-center h-full"><PowerOff /><p className="text-muted-foreground text-sm text-center px-4">Agent disconnected.</p></div>;
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
                <Button type="submit" variant="primary" className="ml-2" disabled={connectionStatus !== 'connected' || !inputValue.trim()}>
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