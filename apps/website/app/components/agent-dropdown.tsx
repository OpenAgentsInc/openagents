import { useEffect, useState } from "react";
import { Link } from "react-router";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuGroup,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "~/components/ui/dropdown-menu";
import { Button } from "~/components/ui/button";
import { ChevronDown, User } from "lucide-react";
import type { Agent } from "~/lib/store";
import { useAgentStore } from "~/lib/store";

export function AgentDropdown() {
  const [agents, setAgents] = useState<Agent[]>([]);
  const [isClient, setIsClient] = useState(false);

  // Get agents on client-side only to avoid hydration mismatches
  useEffect(() => {
    if (typeof window !== 'undefined') {
      setIsClient(true);
      const storedAgents = useAgentStore.getState().agents;
      setAgents(storedAgents);
    }
  }, []);

  // Don't render if no agents or on server
  if (!isClient || agents.length === 0) {
    return null;
  }

  return (
    <DropdownMenu>
      <DropdownMenuTrigger asChild>
        <Button variant="ghost" size="sm" className="flex items-center gap-1">
          <User size={16} />
          <span>My Agents</span>
          <ChevronDown size={14} />
        </Button>
      </DropdownMenuTrigger>
      <DropdownMenuContent align="end" className="w-56">
        <DropdownMenuLabel>My Agents</DropdownMenuLabel>
        <DropdownMenuSeparator />
        <DropdownMenuGroup>
          {agents.map((agent) => (
            <DropdownMenuItem key={agent.id} asChild>
              <Link to={`/agent/${agent.id}`} className="cursor-pointer">
                {agent.purpose.length > 25
                  ? `${agent.purpose.substring(0, 25)}...`
                  : agent.purpose}
              </Link>
            </DropdownMenuItem>
          ))}
        </DropdownMenuGroup>
      </DropdownMenuContent>
    </DropdownMenu>
  );
}
