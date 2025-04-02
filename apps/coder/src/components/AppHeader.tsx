import React, { memo, useCallback, useRef } from 'react';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import { PenSquare } from 'lucide-react';
import { useStableHeader } from '@/providers/StableHeaderProvider';
import { react19 } from "@openagents/core";

// Interface for Lucide icon props
interface IconProps {
  size?: number;
  color?: string;
  className?: string;
  [key: string]: any;
}

// Make Lucide icons compatible with React 19
const PenSquareIcon = react19.icon<IconProps>(PenSquare);

// The AppHeader no longer takes direct props, instead it uses the context
export const AppHeader = memo(function AppHeader() {
  // Get the stable handler from context
  const { handleCreateThread } = useStableHeader();

  // Store handler in ref to maintain stable identity
  const onClickRef = useRef(handleCreateThread);

  // Update ref when prop changes
  React.useEffect(() => {
    onClickRef.current = handleCreateThread;
  }, [handleCreateThread]);

  // Create stable handler
  const stableClickHandler = useCallback(() => {
    // First, directly dispatch the clear input event BEFORE anything else
    window.dispatchEvent(new CustomEvent('clear-chat-input'));

    // Then call the handler if it exists
    if (onClickRef.current) {
      onClickRef.current();
    }
  }, []);

  return (
    <div className="flex flex-col h-full px-3 py-3">
      <div className="flex items-center justify-center w-full pt-2">
        <span className="flex items-center text-md font-semibold">
          <span className="select-none">Coder</span>
          <Badge
            variant="outline"
            className="text-[11px] px-[4px] py-[2px] ml-2 mt-[1px]"
          >
            v0.0.1
          </Badge>
        </span>
      </div>
      <Button
        onClick={stableClickHandler}
        variant="outline"
        size="sm"
        className="w-full flex items-center justify-center py-5 mt-4 mb-10"
        data-testid="create-new-chat-button"
      >
        <PenSquareIcon size={16} className="mr-2" />
        <span>New Chat</span>
      </Button>
    </div>
  );
});
