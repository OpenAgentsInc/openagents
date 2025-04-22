// src/hooks/useOpenAgent_Minimal.ts
import React, { useState, useEffect, useCallback } from 'react';
import { useAgent as useCloudflareAgent } from "agents/react";
import type { UIMessage } from "ai";

// Minimal state needed by the frontend
type AgentState = {
  messages: UIMessage[];
  // Add other fields ONLY if the backend state includes them and UI needs them
};

type ConnectionStatus = 'connecting' | 'connected' | 'disconnected' | 'error';

export interface MinimalAgentHook {
  agentId: string;
  connectionStatus: ConnectionStatus;
  messages: UIMessage[];
  sendMessage: (content: string) => void; // Simple send function
  disconnect: () => void;
}

export function useOpenAgent_Minimal(agentId: string, agentType: string): MinimalAgentHook {
  const [messages, setMessages] = useState<UIMessage[]>([]);
  const [connectionStatus, setConnectionStatus] = useState<ConnectionStatus>('disconnected');
  const agentName = agentId.startsWith(`${agentType}-`) ? agentId : `${agentType}-${agentId}`;

  // --- Cloudflare SDK Hook ---
  const cloudflareAgent = useCloudflareAgent({
    name: agentName,
    agent: agentType,
    onStateUpdate: (newState: Partial<AgentState>) => { // Expect partial or full state
      console.log(`[Hook ${agentName}] State update received`, newState);
      // Update messages if they are present in the new state
      if (newState.messages) {
          setMessages(newState.messages);
      }
      // Update other state parts if needed
    },
    host: "agents.openagents.com" // Adjust if needed
  });
  // --- End Cloudflare SDK Hook ---

  // --- Connection Status ---
  useEffect(() => {
    if (!cloudflareAgent) return;
    
    // Use private property access for WebSocket
    // This is a workaround since getWebSocket() isn't exposed in the type
    const ws = (cloudflareAgent as any)._socket; 
    if (!ws) return;

    const handleOpen = () => setConnectionStatus('connected');
    const handleClose = () => setConnectionStatus('disconnected');
    const handleError = () => setConnectionStatus('error');

     // Use readyState initially
    if (ws.readyState === WebSocket.CONNECTING) setConnectionStatus('connecting');
    else if (ws.readyState === WebSocket.OPEN) setConnectionStatus('connected');
    else setConnectionStatus('disconnected');


    ws.addEventListener('open', handleOpen);
    ws.addEventListener('close', handleClose);
    ws.addEventListener('error', handleError);

    return () => {
      ws.removeEventListener('open', handleOpen);
      ws.removeEventListener('close', handleClose);
      ws.removeEventListener('error', handleError);
    };
  }, [cloudflareAgent]);
  // --- End Connection Status ---

  // --- Send Message ---
  const sendMessage = useCallback((content: string) => {
    if (cloudflareAgent) {
      const messagePayload = {
        type: "chat_message", // Use the type expected by the backend
        content: content,
        timestamp: new Date().toISOString()
      };
      console.log(`[Hook ${agentName}] Sending chat message:`, messagePayload);
      cloudflareAgent.send(JSON.stringify(messagePayload));
    } else {
      console.error(`[Hook ${agentName}] Cannot send message, agent not available.`);
    }
  }, [cloudflareAgent, agentName]);
  // --- End Send Message ---

  // --- Disconnect ---
  const disconnect = useCallback(() => {
    if (cloudflareAgent) {
       console.log(`[Hook ${agentName}] Disconnecting...`);
       cloudflareAgent.close();
    }
  }, [cloudflareAgent, agentName]);
  // --- End Disconnect ---

  return {
    agentId: agentName,
    connectionStatus,
    messages, // Provide the locally managed messages
    sendMessage,
    disconnect,
  };
}