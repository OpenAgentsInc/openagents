import React from "react";
import { Separator } from "@/components/ui/separator";
import { Input } from "@/components/ui/input";
import { Button } from "@/components/ui/button";

// This will be replaced with actual data from the parent component
interface Session {
  id: string;
  projectPath: string;
  messages: any[];
  isLoading: boolean;
}

export const MetadataPane: React.FC = () => {
  // Get data from global object (temporary solution)
  const data = (window as any).__openagents_data || {};
  const claudeStatus = data.claudeStatus || "Ready";
  const sessions: Session[] = data.sessions || [];
  const newProjectPath = data.newProjectPath || "";
  const isDiscoveryLoading = data.isDiscoveryLoading || false;

  return (
    <div className="flex flex-col h-full">
      {/* Header */}
      <div className="text-center select-none mb-4">
        <h1 className="text-xl font-bold mb-1">OpenAgents</h1>
        <p className="text-muted-foreground text-xs">Claude Code Commander</p>
      </div>

      {/* Status Section */}
      <div className="space-y-2">
        <h3 className="text-sm font-semibold text-primary uppercase tracking-wide">Status</h3>
        <p className="text-xs text-muted-foreground">
          Sessions: {sessions.length} • {isDiscoveryLoading ? "Loading..." : "Ready"}
        </p>
        <p className="text-xs break-all">{claudeStatus}</p>
      </div>

      <Separator className="my-4" />

      {/* Sessions Section */}
      <div className="flex-1 flex flex-col space-y-4 min-h-0">
        <h3 className="text-sm font-semibold text-primary uppercase tracking-wide">Sessions</h3>
        
        {/* Create New Session */}
        <div className="space-y-2">
          <Input
            type="text"
            value={newProjectPath}
            onChange={(e) => data.setNewProjectPath?.(e.target.value)}
            onKeyPress={(e) => {
              if (e.key === "Enter") {
                data.createSession?.();
              }
            }}
            placeholder="Project path"
            className="text-xs"
          />
          <Button 
            onClick={() => data.createSession?.()}
            disabled={isDiscoveryLoading}
            size="sm"
            className="w-full"
          >
            Create Session
          </Button>
        </div>

        <Separator />

        {/* Active Sessions List */}
        <div className="space-y-2 flex-1 overflow-y-auto">
          {sessions.length === 0 ? (
            <p className="text-xs text-muted-foreground">No active sessions</p>
          ) : (
            sessions.map((session) => (
              <div key={session.id} className="p-2 border border-border/20 bg-muted/10">
                <div className="flex justify-between items-start mb-1">
                  <p className="text-xs font-mono break-all flex-1">
                    {session.projectPath.split('/').pop()}
                  </p>
                  <Button
                    onClick={() => data.stopSession?.(session.id)}
                    disabled={session.isLoading}
                    variant="destructive"
                    size="sm"
                    className="h-5 px-2 text-xs ml-2"
                  >
                    ×
                  </Button>
                </div>
                <p className="text-xs text-muted-foreground truncate">
                  {session.messages.length} messages
                </p>
              </div>
            ))
          )}
        </div>
      </div>
    </div>
  );
};