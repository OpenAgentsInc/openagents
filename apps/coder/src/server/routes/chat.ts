/**
 * Chat API routes
 */

import { Hono } from 'hono';
import type { Message } from 'ai';
import { 
  validateChatRequest, 
  sanitizeMessages, 
  normalizeSystemMessages, 
  filterInvalidMessages,
  getApiKeys,
  validateApiKey,
  getProviderOptions,
  findModelInfo,
  validateModelProviderMatch,
} from '../utils';
import { 
  ChatError, 
  transformUnknownError, 
  formatErrorForStream,
  ProviderType
} from '../errors';
import { Provider, createProvider, detectProviderFromModel } from '../providers';
import { streamManager } from '../streaming';
import { createShellCommandTool } from '../tools/shell-command';
import { getMCPTools, wrapMCPToolsWithErrorHandling } from '../tools/mcp-tools';

// Create chat router
const chatRoutes = new Hono();

/**
 * Main chat endpoint
 */
chatRoutes.post('/chat', async (c) => {
  console.log('[Server] Received chat request');
  
  try {
    // Parse request body
    const body = await c.req.json();
    
    // Validate request
    validateChatRequest(body);
    
    // Extract API keys from request if provided
    const apiKeys = getApiKeys(body.apiKeys || {});
    
    // Get the model ID
    const modelId = body.model;
    
    // Extract the preferred provider if specified (for ambiguous models)
    const preferredProvider = body.preferredProvider;
    
    // Extract selected tool IDs if provided
    const selectedToolIds = body.selectedToolIds || [];
    
    // Get model info
    const modelInfo = findModelInfo(modelId, preferredProvider);
    
    // Get the provider type
    const providerType = modelInfo.provider as ProviderType;
    
    // Validate API key for the provider
    validateApiKey(providerType, apiKeys);
    
    // Validate that the provider is appropriate for the model
    validateModelProviderMatch(modelId, providerType);
    
    // Get API key for the provider (handle different provider types gracefully)
    let apiKey = '';
    if (providerType !== 'unknown') {
      // Cast to any to avoid TypeScript error with provider types
      const keys = apiKeys as Record<string, string>;
      apiKey = keys[providerType] || '';
    }
    
    // Get provider options
    const providerOptions = getProviderOptions(providerType, apiKeys);
    
    // Create the provider
    const provider: Provider = createProvider(
      providerType,
      modelId,
      apiKey,
      providerOptions
    );
    
    // Process messages
    let messages: Message[] = body.messages || [];
    
    // 1. Sanitize messages to ensure valid content and parts
    messages = sanitizeMessages(messages);
    
    // 2. Handle system messages, especially for models that require them at the beginning
    messages = normalizeSystemMessages(messages);
    
    // 3. Filter out any invalid messages
    messages = filterInvalidMessages(messages);
    
    // Check if there are any messages left after filtering
    if (messages.length === 0) {
      throw new Error("No valid messages found after filtering");
    }
    
    // 4. Check for system prompt in request and add if needed
    const systemPrompt = body.systemPrompt;
    if (systemPrompt && typeof systemPrompt === 'string' && systemPrompt.trim() !== '') {
      console.log('[Server] Using custom system prompt');
      
      // Add system message at the beginning if it doesn't already exist
      const hasSystemMessage = messages.some(msg => msg.role === 'system');
      
      if (!hasSystemMessage) {
        messages = [
          { role: 'system', content: systemPrompt, id: `system-${Date.now()}` },
          ...messages
        ];
      }
    }
    
    // 5. If model context length is too small, only use the most recent user message
    if (provider.contextWindowSize < 10000) {
      messages = [messages[messages.length - 1]];
    }
    
    // Get MCP tools
    const mcpTools = getMCPTools();
    
    // Wrap MCP tools with error handling
    const wrappedMCPTools = wrapMCPToolsWithErrorHandling(mcpTools);
    
    // Create shell command tool
    const shellCommandTool = createShellCommandTool();
    
    // Get all enabled tool IDs
    const enabledToolIds = await getEnabledToolIds();
    
    // Combine all enabled tools
    let combinedTools: Record<string, any> = {};
    
    // First add shell_command if it's enabled
    if (enabledToolIds.includes('shell_command')) {
      combinedTools.shell_command = shellCommandTool;
    }
    
    // Then add enabled MCP tools
    for (const [toolId, tool] of Object.entries(wrappedMCPTools)) {
      if (enabledToolIds.includes(toolId)) {
        combinedTools[toolId] = tool;
      }
    }
    
    // Filter tools based on user selection for this request
    if (selectedToolIds && selectedToolIds.length > 0) {
      console.log('===============================================================');
      console.log(`[Server] 🔧 TOOL FILTERING ACTIVE - User selected ${selectedToolIds.length} specific tools:`);
      console.log(`[Server] 🔧 Selected tools: ${JSON.stringify(selectedToolIds)}`);
      console.log('===============================================================');
      
      // Start with empty tools object
      const filteredTools: Record<string, any> = {};
      
      // Only include tools that were explicitly selected by the user
      for (const toolId of selectedToolIds) {
        if (combinedTools[toolId]) {
          console.log(`[Server] ✅ Including selected tool: ${toolId}`);
          filteredTools[toolId] = combinedTools[toolId];
        } else {
          console.log(`[Server] ❌ Selected tool not available: ${toolId}`);
        }
      }
      
      // Log which tools are being excluded
      const excludedTools = Object.keys(combinedTools).filter(id => !selectedToolIds.includes(id));
      if (excludedTools.length > 0) {
        console.log(`[Server] 🚫 Excluding ${excludedTools.length} tools that were not selected:`);
        console.log(`[Server] 🚫 Excluded tools: ${JSON.stringify(excludedTools)}`);
      }
      
      // This is the important part - ONLY use the tools explicitly selected by the user
      combinedTools = filteredTools;
      
      console.log('===============================================================');
      console.log(`[Server] 🔧 FINAL TOOL SELECTION: ${Object.keys(combinedTools).join(', ')}`);
      console.log('===============================================================');
    } else {
      console.log('[Server] No specific tools selected by user, using all enabled tools');
      console.log(`[Server] Available tools: ${Object.keys(combinedTools).join(', ')}`);
    }
    
    // Helper to get enabled tool IDs
    async function getEnabledToolIds(): Promise<string[]> {
      try {
        console.log('[Server] Attempting to fetch enabled tool IDs');
        
        // For server-side implementation, we need to respect the user's settings
        // rather than automatically including all MCP tools
        
        // Try to fetch from settings repository
        try {
          // Import settings dynamically to avoid circular dependencies
          const settingsModule = require('@openagents/core');
          if (settingsModule && settingsModule.settingsRepository && typeof settingsModule.settingsRepository.getEnabledToolIds === 'function') {
            console.log('[Server] Fetching enabled tool IDs from settings repository');
            const enabledIds = await settingsModule.settingsRepository.getEnabledToolIds();
            
            // If we got valid enabled IDs from settings, use those
            if (Array.isArray(enabledIds) && enabledIds.length > 0) {
              console.log('[Server] Using enabled tool IDs from settings:', enabledIds);
              return enabledIds;
            } else {
              console.log('[Server] No enabled tool IDs found in settings, using default');
            }
          }
        } catch (settingsError) {
          console.warn('[Server] Could not fetch tool IDs from settings repository:', settingsError);
        }
        
        // At a minimum, always include shell_command if available
        const baseTools = ['shell_command'];
        
        console.log('[Server] Using default tool set (shell_command only)');
        return baseTools;
      } catch (error) {
        console.error("Error getting enabled tool IDs:", error);
        return ['shell_command']; // Default fallback
      }
    }
    
    // Configure streaming options
    const streamOptions = {
      // Only include tools if the model supports them
      ...(provider.supportsTools && Object.keys(combinedTools).length > 0 
        ? { tools: combinedTools, toolCallStreaming: true } 
        : {}),
      temperature: body.temperature || 0.7
    };
    
    try {
      // Create the stream
      const streamResult = await streamManager.createStream(provider, messages, streamOptions);
      
      // Return the stream response
      return streamManager.createStreamResponse(c, streamResult);
    } catch (streamSetupError) {
      console.error("🚨 streamText setup failed:", streamSetupError);
      
      // Handle specific error
      const chatError = streamSetupError instanceof ChatError 
        ? streamSetupError 
        : transformUnknownError(streamSetupError);
      
      // Return error stream
      return streamManager.handleStreamError(chatError, c);
    }
  } catch (error) {
    console.error("💥 Chat endpoint error:", error);
    
    // Convert to ChatError
    const chatError = error instanceof ChatError 
      ? error 
      : transformUnknownError(error);
    
    // Set SSE headers
    c.header('Content-Type', 'text/event-stream; charset=utf-8');
    c.header('Cache-Control', 'no-cache');
    c.header('Connection', 'keep-alive');
    c.header('X-Vercel-AI-Data-Stream', 'v1');
    
    // Return error as SSE
    const errorResponse = formatErrorForStream(chatError);
    return new Response(errorResponse);
  }
});

export default chatRoutes;