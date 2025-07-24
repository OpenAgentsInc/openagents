import React from "react";
import { cn } from "@/lib/utils";
import { HotbarItem } from "./HotbarItem";
import { Settings, Plus, HelpCircle, PanelLeft } from "lucide-react";
import { usePaneStore } from "@/stores/pane";

interface HotbarProps {
  className?: string;
}

export const Hotbar: React.FC<HotbarProps> = ({ className }) => {
  const { toggleMetadataPane } = usePaneStore();

  const handleNewChat = () => {
    // Get data from global object and create a new session
    const data = (window as any).__openagents_data || {};
    if (data.createSession) {
      data.createSession();
    }
  };

  const handleSettings = () => {
    console.log("Settings");
  };

  const handleHelp = () => {
    console.log("Help");
  };

  return (
    <div
      className={cn(
        "bg-background/90 border-border/30 fixed bottom-4 left-1/2 z-[10000] flex -translate-x-1/2 transform space-x-1 rounded-md border p-1 shadow-lg backdrop-blur-sm",
        className,
      )}
    >
      {/* Slot 1: Metadata Panel */}
      <HotbarItem
        slotNumber={1}
        onClick={toggleMetadataPane}
        title="Sessions Panel"
      >
        <PanelLeft className="text-muted-foreground h-5 w-5" />
      </HotbarItem>

      {/* Slot 2: New Chat */}
      <HotbarItem
        slotNumber={2}
        onClick={handleNewChat}
        title="New Chat"
      >
        <Plus className="text-muted-foreground h-5 w-5" />
      </HotbarItem>

      {/* Slot 3: Empty */}
      <HotbarItem slotNumber={3} isGhost>
        <span className="h-5 w-5" />
      </HotbarItem>

      {/* Slot 4: Empty */}
      <HotbarItem slotNumber={4} isGhost>
        <span className="h-5 w-5" />
      </HotbarItem>

      {/* Slot 5: Empty */}
      <HotbarItem slotNumber={5} isGhost>
        <span className="h-5 w-5" />
      </HotbarItem>

      {/* Slot 6: Empty */}
      <HotbarItem slotNumber={6} isGhost>
        <span className="h-5 w-5" />
      </HotbarItem>

      {/* Slot 7: Empty */}
      <HotbarItem slotNumber={7} isGhost>
        <span className="h-5 w-5" />
      </HotbarItem>

      {/* Slot 8: Settings */}
      <HotbarItem
        slotNumber={8}
        onClick={handleSettings}
        title="Settings"
      >
        <Settings className="text-muted-foreground h-5 w-5" />
      </HotbarItem>

      {/* Slot 9: Help */}
      <HotbarItem
        slotNumber={9}
        onClick={handleHelp}
        title="Help"
      >
        <HelpCircle className="text-muted-foreground h-5 w-5" />
      </HotbarItem>
    </div>
  );
};