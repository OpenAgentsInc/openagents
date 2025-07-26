import React from "react";
import { Separator } from "@/components/ui/separator";
import { Input } from "@/components/ui/input";
import { Button } from "@/components/ui/button";
import { UnifiedHistoryList } from "@/components/session/UnifiedHistoryList";

// This will be replaced with actual data from the parent component
interface Session {
  id: string;
  projectPath: string;
  messages: any[];
  isLoading: boolean;
}

interface HistoryPaneProps {
  sessions?: Session[];
  newProjectPath?: string;
  isDiscoveryLoading?: boolean;
  setNewProjectPath?: (value: string) => void;
  createSession?: () => void;
  stopSession?: (sessionId: string) => void;
}

export const HistoryPane: React.FC<HistoryPaneProps> = ({
  sessions = [],
  newProjectPath = "",
  isDiscoveryLoading = false,
  setNewProjectPath,
  createSession,
  stopSession,
}) => {

  return (
    <div className="flex flex-col h-full">
      {/* Sessions Section */}
      <div className="flex-1 flex flex-col space-y-4 min-h-0">
        <h3 className="text-sm font-semibold text-primary uppercase tracking-wide">Sessions</h3>
        
        {/* Create New Session */}
        <div className="space-y-2">
          <Input
            type="text"
            value={newProjectPath}
            onChange={(e) => setNewProjectPath?.(e.target.value)}
            onKeyPress={(e) => {
              if (e.key === "Enter") {
                createSession?.();
              }
            }}
            placeholder="Project path"
            className="text-xs"
          />
          <Button 
            onClick={() => createSession?.()}
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
                    onClick={() => stopSession?.(session.id)}
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

        <Separator />

        {/* Session History */}
        <div className="flex-1 min-h-0">
          <UnifiedHistoryList limit={50} />
        </div>
      </div>
    </div>
  );
};