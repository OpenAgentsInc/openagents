// src/components/agent/MinimalSolverControls.tsx
import React, { useState, useEffect, useCallback } from "react";
import { Button } from "../button/Button";
import { Card, CardHeader, CardTitle, CardContent, CardFooter } from "../card/Card";
import { Spinner } from "../loader/Loader";
import { BotIcon, PlugZap, PowerOff, CheckCircle, AlertTriangle } from "lucide-react";
import type { MinimalAgentHook } from "../../hooks/useOpenAgent_Minimal"; // Adjust path
import { cn } from "../../lib/utils";

interface MinimalSolverControlsProps {
  agent: MinimalAgentHook;
  // Required for initial connection setup if hook doesn't handle it
  // Pass necessary context/config here if needed for connect logic
}

export function MinimalSolverControls({ agent }: MinimalSolverControlsProps) {
  const { connectionStatus, disconnect } = agent;
  const [isConnecting, setIsConnecting] = useState(false); // Local connecting state
  const [isHydrated, setIsHydrated] = useState(false);

  useEffect(() => { setIsHydrated(true); }, []);

  // Update local state when hook status changes
  useEffect(() => {
      if (connectionStatus !== 'connecting') {
          setIsConnecting(false);
      }
  }, [connectionStatus]);

  // --- Minimal Connect Logic ---
  // Assumes the hook itself attempts connection when initialized or via a method
  // This button might just be visual feedback or trigger an explicit connect if needed
  const handleConnect = useCallback(() => {
      if (connectionStatus === 'connected' || isConnecting) return;
      console.log("[Controls] Requesting connection (hook should handle actual connection)...");
      setIsConnecting(true);
      // If your hook needs an explicit connect call:
      // agent.connect(); // Assuming hook provides this
  }, [connectionStatus, isConnecting /*, agent.connect */]); // Add agent.connect if needed
  // --- End Minimal Connect Logic ---

  const handleDisconnect = useCallback(() => {
    disconnect(); // Use disconnect from hook
  }, [disconnect]);

  // Button disabled states
  const isConnectButtonDisabled = isConnecting || connectionStatus === 'connected';
  const isDisconnectButtonDisabled = connectionStatus === 'disconnected' || isConnecting;

  return (
    <Card className="mb-4">
      <CardHeader className="pb-2">
        <CardTitle className="text-sm flex items-center">
          <BotIcon className="h-4 w-4 mr-2" /> Agent Status
        </CardTitle>
      </CardHeader>

      <CardContent className="space-y-2">
        <div className="flex items-center gap-2 text-xs">
          Status:
          {connectionStatus === 'connected' && <CheckCircle className="h-4 w-4 text-green-500" />}
          {connectionStatus === 'connecting' && <Spinner className="h-4 w-4" />}
          {connectionStatus === 'disconnected' && <PowerOff className="h-4 w-4 text-muted-foreground" />}
          {connectionStatus === 'error' && <AlertTriangle className="h-4 w-4 text-destructive" />}
          <span className={cn(
            connectionStatus === 'error' && 'text-destructive',
            connectionStatus === 'connected' && 'text-green-600'
          )}>
            {connectionStatus.charAt(0).toUpperCase() + connectionStatus.slice(1)}
          </span>
        </div>
         {connectionStatus === 'error' && (
            <p className="text-xs text-destructive">Connection error.</p>
         )}
      </CardContent>

      <CardFooter className="pt-0 flex gap-2 w-full">
         <Button
           variant="default" size="sm" className="flex-1"
           onClick={handleConnect} disabled={isConnectButtonDisabled}
           aria-label="Connect Agent"
         >
           {isConnecting ? <Spinner className="w-4 h-4 mr-2"/> : <PlugZap className="h-4 w-4 mr-2" />}
           {isConnecting ? "Connecting..." : "Connect"}
         </Button>
         <Button
            variant="outline" size="sm" className="flex-1"
            onClick={handleDisconnect} disabled={isDisconnectButtonDisabled}
            aria-label="Disconnect Agent"
          >
            <PowerOff className="h-4 w-4 mr-2" />
            Disconnect
          </Button>
      </CardFooter>
    </Card>
  );
}