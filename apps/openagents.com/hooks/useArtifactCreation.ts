'use client'

import { useCallback } from 'react'
import { useArtifactOperations } from '@/components/artifacts/ArtifactsContext'
import { Message } from 'ai'

// Code block patterns to detect in AI responses
const CODE_BLOCK_PATTERNS = [
  /```(?:tsx?|jsx?|javascript|typescript|react)\n([\s\S]*?)```/g,
  /```\n([\s\S]*?)```/g // Generic code blocks
]

// Extract title from code or message
function extractTitle(code: string, message: string): string {
  // Try to extract from export default function ComponentName
  const exportFunctionMatch = code.match(/export\s+default\s+function\s+(\w+)/);
  if (exportFunctionMatch && exportFunctionMatch[1] !== 'App') {
    return exportFunctionMatch[1];
  }
  
  // Try to extract from export default ComponentName
  const componentMatch = code.match(/export\s+default\s+(\w+)/);
  if (componentMatch && componentMatch[1] !== 'function' && componentMatch[1] !== 'App') {
    return componentMatch[1];
  }
  
  // Try to extract from function ComponentName
  const functionMatch = code.match(/function\s+(\w+)\s*\(/);
  if (functionMatch && functionMatch[1] !== 'App') {
    return functionMatch[1];
  }
  
  // Try to extract from user's request
  const requestPatterns = [
    /create\s+(?:a\s+)?([^.!?]+)/i,
    /build\s+(?:a\s+)?([^.!?]+)/i,
    /make\s+(?:a\s+)?([^.!?]+)/i,
  ];
  
  for (const pattern of requestPatterns) {
    const match = message.match(pattern);
    if (match) {
      const extracted = match[1].trim()
        .split(' ')
        .slice(0, 3)
        .map(word => word.charAt(0).toUpperCase() + word.slice(1))
        .join(' ');
      return extracted;
    }
  }
  
  // If we found "App" but no user message, use it
  if (exportFunctionMatch && exportFunctionMatch[1] === 'App') {
    return 'App';
  }
  if (componentMatch && componentMatch[1] === 'App') {
    return 'App';
  }
  if (functionMatch && functionMatch[1] === 'App') {
    return 'App';
  }
  
  return 'Generated Component';
}

// Extract description from code comments or structure
function extractDescription(code: string): string {
  // Look for JSDoc comments
  const jsdocMatch = code.match(/\/\*\*\s*\n\s*\*\s*(.+?)\n/);
  if (jsdocMatch) {
    return jsdocMatch[1].trim();
  }
  
  // Look for first line comment
  const commentMatch = code.match(/^\/\/\s*(.+)$/m);
  if (commentMatch) {
    return commentMatch[1].trim();
  }
  
  // Analyze code structure
  if (code.includes('useState') && code.includes('useEffect')) {
    return 'Interactive React component with state and effects';
  } else if (code.includes('useState')) {
    return 'React component with state management';
  } else if (code.includes('fetch') || code.includes('async')) {
    return 'Component with data fetching';
  }
  
  return 'React component';
}

export function useArtifactCreation() {
  const { addArtifact } = useArtifactOperations()

  const extractCodeFromMessage = useCallback((content: string): { code: string; language: string } | null => {
    for (const pattern of CODE_BLOCK_PATTERNS) {
      const matches = Array.from(content.matchAll(pattern));
      if (matches.length > 0) {
        // Get the last/most complete code block
        const lastMatch = matches[matches.length - 1];
        const code = lastMatch[1].trim();
        
        // Detect language from the code block marker or content
        let language = 'tsx'; // default
        const blockStart = lastMatch[0].split('\n')[0];
        if (blockStart.includes('javascript')) language = 'javascript';
        else if (blockStart.includes('typescript')) language = 'typescript';
        else if (blockStart.includes('jsx')) language = 'jsx';
        else if (blockStart.includes('tsx')) language = 'tsx';
        
        return { code, language };
      }
    }
    return null;
  }, []);

  const createArtifactFromMessage = useCallback((message: Message, userMessage?: string) => {
    if (message.role !== 'assistant') return null;

    const codeData = extractCodeFromMessage(message.content);
    if (!codeData) return null;

    const { code } = codeData;
    const title = extractTitle(code, userMessage || '');
    const description = extractDescription(code);

    // Ensure the code is a complete React component
    const isCompleteComponent = 
      code.includes('export default') || 
      code.includes('function') || 
      code.includes('const') && code.includes('=>');

    if (!isCompleteComponent) return null;

    // Create the artifact
    const artifactId = addArtifact({
      title,
      description,
      type: 'code',
      content: code,
      conversationId: message.id,
      messageId: message.id
    });

    return artifactId;
  }, [extractCodeFromMessage, addArtifact]);

  return {
    extractCodeFromMessage,
    createArtifactFromMessage
  };
}