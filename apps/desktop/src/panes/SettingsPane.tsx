import React from "react";
import { Separator } from "@/components/ui/separator";

interface Session {
  id: string;
  projectPath: string;
  messages: any[];
  isLoading: boolean;
}

interface SettingsPaneProps {
  claudeStatus?: string;
  sessions?: Session[];
  isDiscoveryLoading?: boolean;
}

export const SettingsPane: React.FC<SettingsPaneProps> = ({
  claudeStatus = "Ready",
  sessions = [],
  isDiscoveryLoading = false,
}) => {
  return (
    <div className="flex flex-col h-full">
      {/* Header */}
      <div className="text-center select-none mb-4">
        <h1 className="text-xl font-bold mb-1">OpenAgents</h1>
        <p className="text-muted-foreground text-xs">Claude Code Commander</p>
      </div>

      <Separator className="my-4" />

      {/* Status Section */}
      <div className="space-y-2">
        <h3 className="text-sm font-semibold text-primary uppercase tracking-wide">Status</h3>
        <p className="text-xs text-muted-foreground">
          Sessions: {sessions.length} • {isDiscoveryLoading ? "Loading..." : "Ready"}
        </p>
        <p className="text-xs break-all">{claudeStatus}</p>
      </div>

      <Separator className="my-4" />

      {/* About Section */}
      <div className="space-y-2">
        <h3 className="text-sm font-semibold text-primary uppercase tracking-wide">About</h3>
        <div className="space-y-1">
          <p className="text-xs text-muted-foreground">Version: 1.0.0</p>
          <p className="text-xs text-muted-foreground">Built with Tauri + React</p>
        </div>
      </div>
    </div>
  );
};