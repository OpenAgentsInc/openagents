// src/components/agent/MinimalSolverControls.tsx
import React, { useState, useEffect, useCallback } from "react";
import type { MinimalAgentHook } from "../../hooks/useOpenAgent_Minimal";

// Define a simplified Card component
const Card = ({ className = "", children }: { className?: string, children: React.ReactNode }) => (
  <div className={`rounded-lg border bg-card text-card-foreground shadow-sm ${className}`}>
    {children}
  </div>
);

// Define simplified Card subcomponents
const CardHeader = ({ className = "", children }: { className?: string, children: React.ReactNode }) => (
  <div className={`flex flex-col space-y-1.5 p-6 ${className}`}>
    {children}
  </div>
);

const CardTitle = ({ className = "", children }: { className?: string, children: React.ReactNode }) => (
  <h3 className={`text-lg font-semibold leading-none tracking-tight ${className}`}>
    {children}
  </h3>
);

const CardContent = ({ className = "", children }: { className?: string, children: React.ReactNode }) => (
  <div className={`p-6 pt-0 ${className}`}>
    {children}
  </div>
);

const CardFooter = ({ className = "", children }: { className?: string, children: React.ReactNode }) => (
  <div className={`flex items-center p-6 pt-0 ${className}`}>
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

// Define a simplified Spinner component
const Spinner = ({ className = "", size = "default" }: { className?: string, size?: "default" | "sm" }) => (
  <div className={`animate-spin rounded-full border-b-2 border-primary ${size === "sm" ? "h-3 w-3" : "h-4 w-4"} ${className}`}></div>
);

// Icons as SVG for simplicity
const BotIcon = () => (
  <svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
    <rect width="18" height="14" x="3" y="8" rx="2"></rect>
    <path d="M7 8v0a5 5 0 0 1 5-5h0a5 5 0 0 1 5 5v0"></path>
    <circle cx="12" cy="16" r="1"></circle>
    <path d="M17 11v2"></path>
    <path d="M7 11v2"></path>
  </svg>
);

const PlugZap = () => (
  <svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
    <path d="M6.3 20.3a2.4 2.4 0 0 0 3.4 0L12 18l-6-6-2.3 2.3a2.4 2.4 0 0 0 0 3.4Z"></path>
    <path d="m2 22 3-3"></path>
    <path d="M7.5 13.5 10 11"></path>
    <path d="M10.5 16.5 13 14"></path>
    <path d="m18 3-4 4h6v-6l-4 4"></path>
    <path d="M15 9 6 18"></path>
  </svg>
);

const PowerOff = () => (
  <svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
    <path d="M18.36 6.64a9 9 0 1 1-12.73 0"></path>
    <line x1="12" y1="2" x2="12" y2="12"></line>
  </svg>
);

const CheckCircle = () => (
  <svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
    <path d="M22 11.08V12a10 10 0 1 1-5.93-9.14"></path>
    <polyline points="22 4 12 14.01 9 11.01"></polyline>
  </svg>
);

const AlertTriangle = () => (
  <svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
    <path d="m21.73 18-8-14a2 2 0 0 0-3.48 0l-8 14A2 2 0 0 0 4 21h16a2 2 0 0 0 1.73-3Z"></path>
    <path d="M12 9v4"></path>
    <path d="M12 17h.01"></path>
  </svg>
);

// Utility function for class names
const cn = (...classes: (string | undefined | boolean)[]) => {
  return classes.filter(Boolean).join(' ');
};

interface MinimalSolverControlsProps {
  agent: MinimalAgentHook;
}

export function MinimalSolverControls({ agent }: MinimalSolverControlsProps) {
  const { connectionStatus, disconnect } = agent;
  const [isConnecting, setIsConnecting] = useState(false);
  const [isHydrated, setIsHydrated] = useState(false);

  useEffect(() => { setIsHydrated(true); }, []);

  // Update local state when hook status changes
  useEffect(() => {
      if (connectionStatus !== 'connecting') {
          setIsConnecting(false);
      }
  }, [connectionStatus]);

  // Connect button handler
  // In a minimal implementation, just provide visual feedback
  // The hook should handle actual connection via the SDK
  const handleConnect = useCallback(() => {
      if (connectionStatus === 'connected' || isConnecting) return;
      console.log("[Controls] Requesting connection (hook should handle actual connection)...");
      setIsConnecting(true);
  }, [connectionStatus, isConnecting]);

  // Disconnect handler
  const handleDisconnect = useCallback(() => {
    disconnect();
  }, [disconnect]);

  // Button disabled states
  const isConnectButtonDisabled = isConnecting || connectionStatus === 'connected';
  const isDisconnectButtonDisabled = connectionStatus === 'disconnected' || isConnecting;

  return (
    <Card className="mb-4">
      <CardHeader className="pb-2">
        <CardTitle className="text-sm flex items-center">
          <BotIcon /> Agent Status
        </CardTitle>
      </CardHeader>

      <CardContent className="space-y-2">
        <div className="flex items-center gap-2 text-xs">
          Status:
          {connectionStatus === 'connected' && <CheckCircle />}
          {connectionStatus === 'connecting' && <Spinner />}
          {connectionStatus === 'disconnected' && <PowerOff />}
          {connectionStatus === 'error' && <AlertTriangle />}
          <span className={cn(
            connectionStatus === 'error' && 'text-red-500',
            connectionStatus === 'connected' && 'text-green-600'
          )}>
            {connectionStatus.charAt(0).toUpperCase() + connectionStatus.slice(1)}
          </span>
        </div>
         {connectionStatus === 'error' && (
            <p className="text-xs text-red-500">Connection error.</p>
         )}
      </CardContent>

      <CardFooter className="pt-0 flex gap-2 w-full">
         <Button
           variant="primary"
           size="sm"
           className="flex-1"
           onClick={handleConnect}
           disabled={isConnectButtonDisabled}
         >
           {isConnecting ? <Spinner size="sm" /> : <PlugZap />}
           {isConnecting ? "Connecting..." : "Connect"}
         </Button>
         <Button
            variant="outline"
            size="sm"
            className="flex-1"
            onClick={handleDisconnect}
            disabled={isDisconnectButtonDisabled}
          >
            <PowerOff />
            Disconnect
          </Button>
      </CardFooter>
    </Card>
  );
}