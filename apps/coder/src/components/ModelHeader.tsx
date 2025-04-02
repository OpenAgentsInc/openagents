import React, { memo } from 'react';
import { ModelSelect } from '@/components/ui/model-select';
import { useModelContext } from '@/providers/ModelProvider';

export const ModelHeader = memo(function ModelHeader() {
  const { selectedModelId, handleModelChange } = useModelContext();
  
  return (
    <div className="flex items-center gap-2 overflow-hidden">
      <ModelSelect
        value={selectedModelId}
        onChange={handleModelChange}
        className="w-[240px]"
      />
      <div className="flex items-center ml-auto">
        {/* Status display if needed */}
      </div>
    </div>
  );
});