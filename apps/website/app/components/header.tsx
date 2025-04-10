import { Link } from "react-router";
import { Button } from "~/components/ui/button";
import { Plus } from "lucide-react";
import { AgentDropdown } from "~/components/agent-dropdown";

export function Header({ showNewAgentButton = true }: { showNewAgentButton?: boolean }) {
  return (
    <header className="w-full p-4 border-b fixed top-0 z-10 bg-background h-16 flex items-center">
      <div className="max-w-7xl mx-auto flex items-center justify-between w-full">
        <Link to="/" className="text-lg font-semibold hover:text-primary transition-colors">
          OpenAgents
        </Link>
        
        <div className="h-full flex items-center gap-2">
          <AgentDropdown />
          
          {showNewAgentButton ? (
            <Button variant="outline" asChild>
              <Link to="/spawn" className="flex items-center gap-2">
                <Plus size={16} />
                <span>Spawn coding agent</span>
              </Link>
            </Button>
          ) : (
            <div className="h-9"></div> // Placeholder to maintain header height
          )}
        </div>
      </div>
    </header>
  );
}