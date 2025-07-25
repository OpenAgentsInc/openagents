import React from "react";
import { Separator } from "@/components/ui/separator";

interface SettingsPaneProps {
  // Initial implementation can be minimal
  // Add settings-specific props as features are added
}

export const SettingsPane: React.FC<SettingsPaneProps> = ({}) => {
  return (
    <div className="flex flex-col h-full">
      {/* Header */}
      <div className="text-center select-none mb-4">
        <h1 className="text-xl font-bold mb-1">OpenAgents</h1>
        <p className="text-muted-foreground text-xs">Claude Code Commander</p>
      </div>

      <Separator className="my-4" />

      {/* Settings Section */}
      <div className="space-y-4">
        <h3 className="text-sm font-semibold text-primary uppercase tracking-wide">Settings</h3>
        
        {/* Application Settings */}
        <div className="space-y-3">
          <div className="space-y-2">
            <h4 className="text-xs font-medium text-muted-foreground">Application</h4>
            <div className="pl-2 space-y-2">
              <p className="text-xs text-muted-foreground">Theme settings coming soon...</p>
              <p className="text-xs text-muted-foreground">Keyboard shortcuts coming soon...</p>
            </div>
          </div>

          <Separator />

          <div className="space-y-2">
            <h4 className="text-xs font-medium text-muted-foreground">About</h4>
            <div className="pl-2 space-y-1">
              <p className="text-xs text-muted-foreground">Version: 1.0.0</p>
              <p className="text-xs text-muted-foreground">Built with Tauri + React</p>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
};