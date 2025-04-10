import { useEffect } from "react";
import { Link, useLoaderData, useParams } from "react-router";
import type { LoaderFunctionArgs } from "react-router";
import type { Route } from "./+types/agent";
import { useAgentStore } from "~/lib/store";

// Use the Agent type locally to avoid import issues
interface Agent {
  id: string;
  purpose: string;
  createdAt: number;
}

export function meta({ params, data }: Route.MetaArgs) {
  const agent = data as Agent | null;
  return [
    { title: agent ? `Agent: ${agent.id}` : "Agent Not Found - OpenAgents" },
    { name: "description", content: agent?.purpose || "Agent details not found" },
  ];
}

// Load agent data
export async function loader({ params }: LoaderFunctionArgs) {
  const { agentId } = params;
  
  // Note: In a real app, you would fetch agent data from an API
  // For now, we'll return null and let the client-side handle it
  return null;
}

export default function AgentDetails() {
  // Get agent ID from URL
  const { agentId } = useParams();
  const loaderData = useLoaderData<Agent | null>();
  
  // Get agent from store
  const { getAgent, agents } = useAgentStore();
  
  // Find agent in store (will be available client-side)
  const agent = getAgent(agentId || "");
  
  // If no agent is found, show not found message
  if (!agent) {
    return (
      <>
        <header className="w-full p-4 border-b">
          <div className="max-w-7xl mx-auto flex items-center">
            <Link to="/" className="text-lg font-semibold hover:text-primary transition-colors">
              OpenAgents
            </Link>
          </div>
        </header>
        
        <main className="w-full max-w-2xl mx-auto p-8 pt-16 text-center">
          <h1 className="text-4xl font-bold mb-6">Agent Not Found</h1>
          <p className="mb-8">We couldn't find an agent with ID: {agentId}</p>
          <Link 
            to="/spawn" 
            className="inline-flex h-9 items-center justify-center rounded-md bg-primary px-4 py-2 text-sm font-medium text-primary-foreground shadow-xs transition-colors hover:bg-primary/90 focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring disabled:pointer-events-none disabled:opacity-50"
          >
            Spawn a New Agent
          </Link>
        </main>
      </>
    );
  }

  // Format date from timestamp
  const formattedDate = new Date(agent.createdAt).toLocaleString();
  
  return (
    <>
      <header className="w-full p-4 border-b">
        <div className="max-w-7xl mx-auto flex items-center">
          <Link to="/" className="text-lg font-semibold hover:text-primary transition-colors">
            OpenAgents
          </Link>
        </div>
      </header>
      
      <main className="w-full max-w-2xl mx-auto p-8 pt-16">
        <div className="flex justify-between items-start mb-8">
          <div>
            <h1 className="text-4xl font-bold mb-2">Agent: {agent.id}</h1>
            <p className="text-muted-foreground">Created on {formattedDate}</p>
          </div>
          <Link 
            to="/spawn" 
            className="inline-flex h-9 items-center justify-center rounded-md bg-primary px-4 py-2 text-sm font-medium text-primary-foreground shadow-xs transition-colors hover:bg-primary/90 focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring disabled:pointer-events-none disabled:opacity-50"
          >
            New Agent
          </Link>
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