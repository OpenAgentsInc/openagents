import { useEffect } from "react";
import { Link, useParams } from "react-router";
import type { LoaderFunctionArgs } from "react-router";
import type { Route } from "./+types/agent";
import { useAgentStore } from "~/lib/store";
import { Header } from "~/components/header";

// Use the Agent type locally to avoid import issues
interface Agent {
  id: string;
  purpose: string;
  createdAt: number;
}

export function meta({ params }: Route.MetaArgs) {
  return [
    { title: `Agent: ${params.agentId}` },
    { name: "description", content: "View agent details" },
  ];
}

// Load agent data - server-side only returns ID for safety
export async function loader({ params }: LoaderFunctionArgs) {
  const { agentId } = params;
  
  // For security, don't try to load agents on the server
  // Just return the ID and let client-side handle data lookup
  return { id: agentId };
}

export default function AgentDetails() {
  // Get agent ID from URL
  const { agentId } = useParams();
  
  // Client-side only state
  const [agent, setAgent] = useState<Agent | null>(null);
  const [loading, setLoading] = useState(true);
  
  // Get agent from store - client-side only
  useEffect(() => {
    if (typeof window !== 'undefined') {
      const { getAgent } = useAgentStore.getState();
      const foundAgent = getAgent(agentId || "");
      setAgent(foundAgent || null);
      setLoading(false);
    }
  }, [agentId]);
  
  // Agent not found view (shared between server and client)
  const NotFoundView = () => (
    <>
      <Header />
      
      <main className="w-full max-w-2xl mx-auto p-8 pt-24 text-center">
        <h1 className="text-3xl font-bold mb-6">Agent Not Found</h1>
        <p className="mb-8">We couldn't find an agent with ID: {agentId}</p>
        <a 
          href="/spawn" 
          className="inline-flex h-9 items-center justify-center rounded-md bg-primary px-4 py-2 text-sm font-medium text-primary-foreground shadow-xs hover:bg-primary/90"
        >
          Spawn a New Agent
        </a>
      </main>
    </>
  );

  // Loading view
  if (loading) {
    return (
      <>
        <Header />
        
        <main className="w-full max-w-2xl mx-auto p-8 pt-24 text-center">
          <h1 className="text-3xl font-bold mb-6">Loading...</h1>
        </main>
      </>
    );
  }
  
  // If no agent is found, show not found message
  if (!agent) {
    return <NotFoundView />;
  }

  // Format date from timestamp - on client only
  const formattedDate = new Date(agent.createdAt || 0).toLocaleString();
  
  return (
    <>
      <Header />
      
      <main className="w-full max-w-2xl mx-auto p-8 pt-24">
        <div className="mb-8">
          <h1 className="text-3xl font-bold mb-2">Agent: {agent.id}</h1>
          <p className="text-muted-foreground">Created on {formattedDate}</p>
        </div>
        
        <div className="space-y-6">
          <div className="p-6 border rounded-lg bg-card">
            <h2 className="text-xl font-semibold mb-4">Agent Purpose</h2>
            <p className="whitespace-pre-wrap">{agent.purpose}</p>
          </div>
          
          <div className="p-6 border rounded-lg bg-card">
            <h2 className="text-xl font-semibold mb-4">Actions</h2>
            <div className="space-y-4">
              <button className="w-full flex items-center justify-center h-10 px-4 py-2 bg-secondary text-secondary-foreground rounded-md hover:bg-secondary/90 transition-colors">
                Start Conversation
              </button>
              <button className="w-full flex items-center justify-center h-10 px-4 py-2 bg-secondary text-secondary-foreground rounded-md hover:bg-secondary/90 transition-colors">
                Manage Repositories
              </button>
            </div>
          </div>
        </div>
      </main>
    </>
  );
}

// Use React hooks only in component body
import { useState } from "react";