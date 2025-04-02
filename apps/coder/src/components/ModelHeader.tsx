import React, { memo } from 'react';
import { ModelSelect } from '@/components/ui/model-select';
import { useModelContext } from '@/providers/ModelProvider';
import ToggleTheme from '@/components/ToggleTheme';
import { Link } from '@tanstack/react-router';
import { Button } from '@/components/ui/button';
import { SlidersHorizontal } from 'lucide-react';

export const ModelHeader = memo(function ModelHeader() {
  const { selectedModelId, handleModelChange } = useModelContext();
  
  return (
    <div className="flex items-center justify-between w-full overflow-hidden">
      <ModelSelect
        value={selectedModelId}
        onChange={handleModelChange}
        className="w-[240px]"
      />
      <div className="flex items-center gap-2">
        <Link to="/settings/models">
          <Button 
            size="icon" 
            className="bg-transparent text-primary hover:bg-primary/5">
            <SlidersHorizontal size={16} />
          </Button>
        </Link>
        <ToggleTheme />
      </div>
    </div>
  );
});