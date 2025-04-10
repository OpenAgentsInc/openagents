import { useEffect } from "react";
import { useParams } from "react-router";
import type { LoaderFunctionArgs } from "react-router";
import type { Route } from "./+types/agent";
import { Header } from "~/components/header";
import {
  Card,
  CardHeader,
  CardTitle,
  CardDescription,
  CardContent,
} from "~/components/ui/card";
import { useOpenAgent } from "@openagents/core";
import { useState } from "react";

// Use the Agent type locally to avoid import issues
interface Agent {
  id: string;
  purpose: string;
  createdAt: number;
  messages: any[];
  setMessages: (messages: any[]) => void;
  handleSubmit: (message: string) => Promise<void>;
  infer: (token: string) => Promise<void>;
  loading: boolean;
  error: Error | null;
  setGithubToken: (token: string) => Promise<void>;
  getGithubToken: () => Promise<string>;
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

function ClientOnly({ children }: { children: React.ReactNode }) {
  const [mounted, setMounted] = useState(false);

  useEffect(() => {
    setMounted(true);
  }, []);

  if (!mounted) {
    return null;
  }

  return <>{children}</>;
}

export default function AgentDetails() {
  // Get agent ID from URL
  const { agentId } = useParams();
  const [loading, setLoading] = useState(true);

  return (
    <>
      <Header />

      <main className="w-full max-w-2xl mx-auto p-8 pt-24">
        <ClientOnly>
          <AgentContent agentId={agentId || ""} />
        </ClientOnly>
      </main>
    </>
  );
}

function AgentContent({ agentId }: { agentId: string }) {
  // Move all client-side code here
  const [loading, setLoading] = useState(true);
  const { state, messages, setMessages, handleSubmit, infer, setGithubToken, getGithubToken } = useOpenAgent(agentId);
  const [agent, setAgent] = useState<Agent | null>(null);
  const [githubToken] = useState<string>("");

  useEffect(() => {
    // Initialize agent here
    const initAgent = async () => {
      try {
        setLoading(false);
      } catch (error) {
        console.error('Failed to initialize agent:', error);
        setLoading(false);
      }
    };

    initAgent();
  }, [agentId]);

  // Agent not found view
  const NotFoundView = () => (
    <div className="text-center">
      <h1 className="text-3xl font-bold mb-6">Agent Not Found</h1>
      <p className="mb-8">We couldn't find an agent with ID: {agentId}</p>
      <a
        href="/spawn"
        className="inline-flex h-9 items-center justify-center rounded-md bg-primary px-4 py-2 text-sm font-medium text-primary-foreground shadow-xs hover:bg-primary/90"
      >
        Spawn a New Agent
      </a>
    </div>
  );

  // Loading view
  if (loading) {
    return <h1 className="text-3xl font-bold mb-6 text-center">Loading...</h1>;
  }

  // If no agent is found, show not found message
  if (!agent) {
    return <NotFoundView />;
  }

  // Format date from timestamp - on client only
  const formattedDate = new Date(agent.createdAt || 0).toLocaleString();

  return (
    <div className="space-y-6">
      <div className="mb-8">
        <h1 className="text-3xl font-bold mb-2">Agent: {agent.id}</h1>
        <p className="text-muted-foreground">Created on {formattedDate}</p>
      </div>

      <div className="space-y-6">
        <Card>
          <CardHeader>
            <CardTitle>Agent Purpose</CardTitle>
          </CardHeader>
          <CardContent>
            <p className="whitespace-pre-wrap">{agent.purpose}</p>
          </CardContent>
        </Card>

        <Card className="overflow-hidden">
          <CardHeader>
            <CardTitle>Conversation</CardTitle>
            <CardDescription>Chat with your coding agent</CardDescription>
          </CardHeader>
          <CardContent className="p-0">
            <div className="h-[600px]">
              <div className="p-4 text-center">
                Chat functionality will be available soon
              </div>
            </div>
          </CardContent>
        </Card>
      </div>
    </div>
  );
}
