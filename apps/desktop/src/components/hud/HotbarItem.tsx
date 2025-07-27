import React from "react";
import { cn } from "@/lib/utils";
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@/components/ui/tooltip";


interface HotbarItemProps {
  slotNumber: number;
  onClick?: () => void;
  children?: React.ReactNode;
  title?: string;
  isActive?: boolean;
  isPressed?: boolean;
  isGhost?: boolean;
  className?: string;
}

const isMacOs = () => {
  return navigator.platform.toUpperCase().indexOf('MAC') >= 0;
};

export const HotbarItem: React.FC<HotbarItemProps> = ({
  slotNumber,
  onClick,
  children,
  title,
  isActive,
  isPressed,
  isGhost,
  className,
}) => {
  const modifierPrefix = isMacOs() ? "⌘" : "Ctrl+";
  const shortcutText = `${modifierPrefix}${slotNumber}`;

  return (
    <Tooltip delayDuration={300}>
      <TooltipTrigger asChild>
        <button
          onClick={onClick}
          aria-label={title || `Hotbar slot ${slotNumber}`}
          className={cn(
            "border-border/80 bg-background/70 hover:bg-accent hover:border-primary focus:ring-primary relative flex h-10 w-10 items-center justify-center rounded-sm border shadow-md backdrop-blur-sm transition-all duration-150 focus:ring-1 focus:outline-none sm:h-12 sm:w-12",
            isActive && "bg-primary/20 border-primary ring-primary ring-1",
            isPressed && "bg-primary/30 border-primary scale-95 shadow-inner",
            isGhost && "cursor-default opacity-30 hover:opacity-50",
            className,
          )}
          disabled={isGhost}
        >
          {children}
          {!isGhost && (
            <div className="text-muted-foreground bg-background/50 absolute right-0.5 bottom-0.5 flex items-center rounded-sm px-0.5 text-[0.6rem] leading-none">
              <span className="font-sans">{modifierPrefix}</span>
              <span>{slotNumber}</span>
            </div>
          )}
        </button>
      </TooltipTrigger>
      {!isGhost && (
        <TooltipContent side="top" sideOffset={5}>
          <p>
            {title || `Slot ${slotNumber}`} ({shortcutText})
          </p>
        </TooltipContent>
      )}
    </Tooltip>
  );
};