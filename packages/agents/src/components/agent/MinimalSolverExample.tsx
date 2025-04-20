// src/components/agent/MinimalSolverExample.tsx
import React from "react";
import { useOpenAgent_Minimal } from "../../hooks/useOpenAgent_Minimal";
import { MinimalSolverConnector } from "./MinimalSolverConnector";
import { MinimalSolverControls } from "./MinimalSolverControls";

interface MinimalSolverExampleProps {
  agentId?: string; // Allow custom agent ID, or use a generated one
}

/**
 * Example component that demonstrates how to use the minimal Solver agent
 */
export function MinimalSolverExample({ agentId = `minimal-${Date.now()}` }: MinimalSolverExampleProps) {
  // Initialize the agent hook
  const agent = useOpenAgent_Minimal(agentId, "solver");

  return (
    <div className="flex flex-col h-full max-h-screen">
      <div className="flex-none p-4">
        <MinimalSolverControls agent={agent} />
      </div>
      
      <div className="flex-1 p-4 min-h-0">
        <MinimalSolverConnector agent={agent} className="h-full" />
      </div>
    </div>
  );
}

/**
 * This is how you would use the MinimalSolverExample in your application:
 * 
 * function SomePageComponent() {
 *   return (
 *     <div className="h-screen">
 *       <MinimalSolverExample agentId="my-custom-agent-id" />
 *     </div>
 *   );
 * }
 */