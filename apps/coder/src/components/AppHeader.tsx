import React, { memo } from 'react';
import { Badge } from '@/components/ui/badge';
import { NewChatIcon } from '@/components/NewChatIcon';

interface AppHeaderProps {
  onCreateThread: () => Promise<void>;
}

export const AppHeader = memo(function AppHeader({ onCreateThread }: AppHeaderProps) {
  return (
    <div className="flex items-center h-full justify-between px-3">
      <span className="flex items-center text-md font-semibold">
        Coder
        <Badge
          variant="outline"
          className="text-[11px] px-[4px] py-[2px] ml-2 mt-[1px]"
        >
          v0.0.1
        </Badge>
      </span>
      <NewChatIcon onClick={onCreateThread} />
    </div>
  );
});