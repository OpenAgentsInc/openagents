import React, { memo } from 'react';
import { ModelSelect } from '@/components/ui/model-select';
import { useModelContext } from '@/providers/ModelProvider';
import ToggleTheme from '@/components/ToggleTheme';
import { Link } from '@tanstack/react-router';
import { Button } from '@/components/ui/button';
import { SlidersHorizontal } from 'lucide-react';
import { react19 } from "@openagents/core";

// Interface for Lucide icon props
interface IconProps {
  size?: number;
  color?: string;
  className?: string;
  [key: string]: any;
}

// Make Lucide icons compatible with React 19
const SlidersHorizontalIcon = react19.icon<IconProps>(SlidersHorizontal);

export const ModelHeader = memo(function ModelHeader() {
  const { selectedModelId, handleModelChange } = useModelContext();

  return (
    <div className="flex items-center justify-between w-full overflow-hidden">
      <div></div>
      {/* <ModelSelect
        value={selectedModelId}
        onChange={handleModelChange}
        className="w-[240px]"
      /> */}
      <div className="flex items-center gap-2">
        <Link to="/settings/models">
          <Button
            size="icon"
            className="flex items-center justify-center h-8 w-8 bg-transparent text-primary hover:bg-primary/5">
            <SlidersHorizontalIcon size={20} />
          </Button>
        </Link>
        <ToggleTheme />
      </div>
    </div>
  );
});
