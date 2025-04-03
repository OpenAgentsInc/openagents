"use client"

import * as React from "react"
import { memo, useCallback, useRef } from "react"
import { PenSquare } from "lucide-react"
import { react19 } from "@openagents/core"

interface NewChatIconProps {
  onClick?: (e: React.MouseEvent) => void;
}

// Interface for Lucide icon props
interface IconProps {
  size?: number;
  color?: string;
  className?: string;
  [key: string]: any;
}

// Make Lucide icons compatible with React 19
const PenSquareIcon = react19.icon<IconProps>(PenSquare);

// Use memoization for the entire component
export const NewChatIcon = memo(function NewChatIcon({ onClick }: NewChatIconProps) {
  // Store handler in ref to maintain stable identity
  const onClickRef = useRef(onClick);
  
  // Update ref when prop changes
  React.useEffect(() => {
    onClickRef.current = onClick;
  }, [onClick]);
  
  // Create stable handler
  const stableClickHandler = useCallback((e: React.MouseEvent) => {
    // First, directly dispatch the clear input event BEFORE anything else
    window.dispatchEvent(new CustomEvent('clear-chat-input'));
    
    // Then call the handler if it exists
    if (onClickRef.current) {
      onClickRef.current(e);
    }
  }, []);
  
  return (
    <button
      aria-label="New chat"
      data-testid="create-new-chat-button"
      className="cursor-pointer flex items-center justify-center h-8 w-8 rounded-md text-foreground bg-transparent hover:bg-primary/5 focus-visible:outline-0"
      onClick={stableClickHandler}
    >
      <PenSquareIcon size={20} />
    </button>
  );
});
