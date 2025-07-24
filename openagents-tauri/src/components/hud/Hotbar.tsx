import React from "react";
import { cn } from "@/lib/utils";
import { HotbarItem } from "./HotbarItem";
import { Plus, PanelLeft, Hand } from "lucide-react";
import { usePaneStore } from "@/stores/pane";

interface HotbarProps {
  className?: string;
  onNewChat?: () => void;
  isHandTrackingActive?: boolean;
  onToggleHandTracking?: () => void;
}

export const Hotbar: React.FC<HotbarProps> = ({ 
  className, 
  onNewChat,
  isHandTrackingActive,
  onToggleHandTracking 
}) => {
  const { toggleMetadataPane } = usePaneStore();

  const handleNewChat = () => {
    if (onNewChat) {
      onNewChat();
    }
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

      {/* Slot 7: Settings (disabled for now) */}
      {/* <HotbarItem
        slotNumber={7}
        onClick={handleSettings}
        title="Settings"
      >
        <Settings className="text-muted-foreground h-5 w-5" />
      </HotbarItem> */}
      <HotbarItem slotNumber={7} isGhost>
        <span className="h-5 w-5" />
      </HotbarItem>

      {/* Slot 8: Help (disabled for now) */}
      {/* <HotbarItem
        slotNumber={8}
        onClick={handleHelp}
        title="Help"
      >
        <HelpCircle className="text-muted-foreground h-5 w-5" />
      </HotbarItem> */}
      <HotbarItem slotNumber={8} isGhost>
        <span className="h-5 w-5" />
      </HotbarItem>

      {/* Slot 9: Hand Tracking */}
      <HotbarItem
        slotNumber={9}
        onClick={onToggleHandTracking}
        title="Hand Tracking"
        isActive={isHandTrackingActive}
      >
        <Hand className="text-muted-foreground h-5 w-5" />
      </HotbarItem>
    </div>
  );
};