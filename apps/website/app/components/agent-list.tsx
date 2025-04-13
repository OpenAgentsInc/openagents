import { useEffect, useState } from "react";
import { Link } from "react-router";
import { Bot, PlusCircle } from "lucide-react";
import type { Agent } from "~/lib/store";
import { useAgentStore } from "~/lib/store";
import { Button } from "~/components/ui/button";

export function AgentList({ currentAgentId }: { currentAgentId: string }) {
  const [agents, setAgents] = useState<Agent[]>([]);
  const [isClient, setIsClient] = useState(false);

  // Get agents on client-side only to avoid hydration mismatches
  useEffect(() => {
    if (typeof window !== 'undefined') {
      setIsClient(true);
      const storedAgents = useAgentStore.getState().agents;
      
      // Sort agents by createdAt timestamp (newest first)
      const sortedAgents = [...storedAgents].sort((a, b) => b.createdAt - a.createdAt);
      setAgents(sortedAgents);
      
      // Subscribe to store changes
      const unsubscribe = useAgentStore.subscribe(
        (state) => state.agents,
        (agents) => {
          // Sort agents by createdAt timestamp (newest first)
          const sortedAgents = [...agents].sort((a, b) => b.createdAt - a.createdAt);
          setAgents(sortedAgents);
        }
      );
      
      return () => unsubscribe();
    }
  }, []);

  // Don't render if on server
  if (!isClient) {
    return null;
  }

  return (
    <div className="py-2">
      <div className="px-4 pb-2 flex items-center justify-between">
        <div className="text-xs font-medium text-muted-foreground uppercase tracking-wider">
          My Agents
        </div>
        <Button 
          asChild 
          variant="ghost" 
          size="sm" 
          className="h-auto px-2 py-1 text-xs"
        >
          <Link to="/spawn" className="flex items-center gap-1.5">
            <PlusCircle size={12} />
            New Agent
          </Link>
        </Button>
      </div>
      
      <div className="space-y-1 px-1">
        {agents.length > 0 ? (
          agents.map((agent) => (
            <Link
              key={agent.id}
              to={`/agent/${agent.id}`}
              className={`flex items-center gap-2 px-3 py-2 text-sm rounded-md transition-colors ${
                agent.id === currentAgentId
                  ? "bg-primary text-primary-foreground"
                  : "hover:bg-muted"
              }`}
            >
              <Bot size={14} />
              <span className="truncate">
                {agent.purpose.length > 25
                  ? `${agent.purpose.substring(0, 25)}...`
                  : agent.purpose}
              </span>
            </Link>
          ))
        ) : (
          <div className="px-3 py-4 text-center text-xs text-muted-foreground">
            No agents yet. Create your first agent.
          </div>
        )}
      </div>
    </div>
  );
}