import React, { memo } from 'react';
import { Link } from '@tanstack/react-router';
import { AlertCircle } from 'lucide-react';
import { MODELS } from '@openagents/core';
import { useModelContext } from '@/providers/ModelProvider';

export const ModelWarningBanner = memo(function ModelWarningBanner() {
  const { isModelAvailable, modelWarning, selectedModelId } = useModelContext();
  
  if (isModelAvailable || !modelWarning) return null;
  
  const selectedModelProvider = MODELS.find(m => m.id === selectedModelId)?.provider;
  
  return (
    <div className="mb-2 p-2 text-sm text-yellow-600 dark:text-yellow-400 border border-yellow-400 rounded-md bg-yellow-50 dark:bg-yellow-900/20">
      <div className="flex items-center">
        <AlertCircle className="h-4 w-4 mr-2 flex-shrink-0" />
        <span>{modelWarning}</span>
      </div>
      <div className="mt-1 ml-6">
        {selectedModelProvider === 'ollama' ? (
          <Link to="/settings/local-models" className="underline">Configure Ollama</Link>
        ) : modelWarning?.includes("LMStudio") ? (
          <Link to="/settings/local-models" className="underline">Configure LMStudio</Link>
        ) : (
          <Link to="/settings/models" className="underline">Add API Key</Link>
        )}
      </div>
    </div>
  );
});