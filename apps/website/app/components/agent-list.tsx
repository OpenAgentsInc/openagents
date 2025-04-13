import { useEffect, useState } from "react";
import { Link } from "react-router";
import { Bot } from "lucide-react";
import type { Agent } from "~/lib/store";
import { useAgentStore } from "~/lib/store";

export function AgentList({ currentAgentId }: { currentAgentId: string }) {
  const [agents, setAgents] = useState<Agent[]>([]);
  const [isClient, setIsClient] = useState(false);

  // Get agents on client-side only to avoid hydration mismatches
  useEffect(() => {
    if (typeof window !== 'undefined') {
      setIsClient(true);
      const storedAgents = useAgentStore.getState().agents;
      setAgents(storedAgents);
      
      // Subscribe to store changes
      const unsubscribe = useAgentStore.subscribe(
        (state) => state.agents,
        (agents) => setAgents([...agents])
      );
      
      return () => unsubscribe();
    }
  }, []);

  // Don't render if no agents or on server
  if (!isClient || agents.length === 0) {
    return null;
  }

  return (
    <div className="py-2">
      <div className="px-4 py-2 text-xs font-medium text-muted-foreground uppercase tracking-wider">
        My Agents
      </div>
      <div className="space-y-1 px-1">
        {agents.map((agent) => (
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
        ))}
      </div>
    </div>
  );
}