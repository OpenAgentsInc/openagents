import React from "react";
import { cn } from "@/lib/utils";
import { HotbarItem } from "./HotbarItem";
import {
  Plus as PlusIcon,
  History as HistoryIcon,
  Hand as HandIcon,
  Settings as SettingsIcon,
  LayoutGrid as LayoutGridIcon,
  BarChart as BarChartIcon,
} from "lucide-react";

import { usePaneStore } from "@/stores/pane";
import { useHotbarStore } from "@/stores/hotbar";

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
  const { panes, toggleMetadataPane, toggleSettingsPane, toggleStatsPane, organizePanes } = usePaneStore();
  const { pressedSlots } = useHotbarStore();
  
  // Check which panes are open
  const isMetadataPaneOpen = panes.some(p => p.id === 'metadata');
  const isSettingsPaneOpen = panes.some(p => p.id === 'settings');
  const isStatsPaneOpen = panes.some(p => p.id === 'stats');

  const handleNewChat = () => {
    if (onNewChat) {
      onNewChat();
    }
  };

  return (
    <div
      className={cn(
        "bg-background/90 border-border/60 fixed bottom-4 left-1/2 z-[10000] flex -translate-x-1/2 transform space-x-1 rounded-md border p-1 shadow-lg backdrop-blur-sm",
        className,
      )}
    >
      {/* Slot 1: New Chat */}
      <HotbarItem
        slotNumber={1}
        onClick={handleNewChat}
        title="New Chat"
        isPressed={pressedSlots.includes(1)}
      >
        <PlusIcon className="text-muted-foreground h-5 w-5" />
      </HotbarItem>

      {/* Slot 2: Organize Panes */}
      <HotbarItem
        slotNumber={2}
        onClick={organizePanes}
        title="Organize Panes"
        isPressed={pressedSlots.includes(2)}
      >
        <LayoutGridIcon className="text-muted-foreground h-5 w-5" />
      </HotbarItem>

      {/* Slot 3: History Panel */}
      <HotbarItem
        slotNumber={3}
        onClick={toggleMetadataPane}
        title="History"
        isActive={isMetadataPaneOpen}
        isPressed={pressedSlots.includes(3)}
      >
        <HistoryIcon className="text-muted-foreground h-5 w-5" />
      </HotbarItem>

      {/* Slot 4: Stats Panel */}
      <HotbarItem
        slotNumber={4}
        onClick={toggleStatsPane}
        title="APM Statistics"
        isActive={isStatsPaneOpen}
        isPressed={pressedSlots.includes(4)}
      >
        <BarChartIcon className="text-muted-foreground h-5 w-5" />
      </HotbarItem>

      {/* Slot 5: Empty */}
      <HotbarItem slotNumber={5} isGhost>
        <span className="h-5 w-5" />
      </HotbarItem>

      {/* Slot 6: Empty */}
      <HotbarItem slotNumber={6} isGhost>
        <span className="h-5 w-5" />
      </HotbarItem>

      {/* Slot 7: Settings */}
      <HotbarItem
        slotNumber={7}
        onClick={toggleSettingsPane}
        title="Settings"
        isActive={isSettingsPaneOpen}
        isPressed={pressedSlots.includes(7)}
      >
        <SettingsIcon className="text-muted-foreground h-5 w-5" />
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
        isPressed={pressedSlots.includes(9)}
      >
        <HandIcon className="text-muted-foreground h-5 w-5" />
      </HotbarItem>
    </div>
  );
};