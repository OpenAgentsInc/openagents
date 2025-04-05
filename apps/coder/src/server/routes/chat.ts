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
  ProviderType,
  ToolError
} from '../errors';
import { transformToolError } from '@openagents/core/src/chat/errors';
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
    console.log('[Server] 🔍🔍🔍 FULL REQUEST BODY:', JSON.stringify(body, null, 2));
    console.log('[Server] 🔍🔍🔍 Body Type:', typeof body);
    console.log('[Server] 🔍🔍🔍 Has selectedToolIds property:', body.hasOwnProperty('selectedToolIds'));
    
    // Check if body properties might be nested
    for (const key of Object.keys(body)) {
      console.log(`[Server] 🔍 Body key: ${key}, type: ${typeof body[key]}`);
      if (typeof body[key] === 'object' && body[key] !== null) {
        console.log(`[Server] 🔍 Properties of ${key}:`, Object.keys(body[key]));
        if (body[key] && body[key].hasOwnProperty('selectedToolIds')) {
          console.log(`[Server] 🔍 Found selectedToolIds in ${key}:`, body[key].selectedToolIds);
        }
      }
    }
    
    // Extract the selected tool IDs from the request body, considering all possible locations
    // This handles both the direct body.selectedToolIds and the nested options.body.selectedToolIds formats
    let selectedToolIds: string[] = [];
    
    // First check if it's directly in the body (simple case)
    if (Array.isArray(body.selectedToolIds)) {
      selectedToolIds = body.selectedToolIds;
      console.log('[Server] ✅ Found selectedToolIds directly in body');
    } 
    // Next check various nested structures that might be used by different clients
    else if (body.options && body.options.body && Array.isArray(body.options.body.selectedToolIds)) {
      selectedToolIds = body.options.body.selectedToolIds;
      console.log('[Server] ✅ Found selectedToolIds in options.body');
    }
    // Finally, look in any other object properties that might contain it
    else {
      for (const key of Object.keys(body)) {
        if (typeof body[key] === 'object' && body[key] !== null) {
          if (body[key].selectedToolIds && Array.isArray(body[key].selectedToolIds)) {
            selectedToolIds = body[key].selectedToolIds;
            console.log(`[Server] ✅ Found selectedToolIds in body.${key}`);
            break;
          }
          // Check one level deeper if needed
          for (const nestedKey of Object.keys(body[key] || {})) {
            if (typeof body[key][nestedKey] === 'object' && body[key][nestedKey] !== null) {
              if (body[key][nestedKey].selectedToolIds && Array.isArray(body[key][nestedKey].selectedToolIds)) {
                selectedToolIds = body[key][nestedKey].selectedToolIds;
                console.log(`[Server] ✅ Found selectedToolIds in body.${key}.${nestedKey}`);
                break;
              }
            }
          }
        }
      }
    }
    console.log('[Server] Selected tool IDs from request body:', selectedToolIds);
    
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
    
    // Handle special case: if selectedToolIds is explicitly an empty array ([]), the user wants NO tools
    const isExplicitEmptySelection = 
      body.hasOwnProperty('selectedToolIds') && 
      Array.isArray(selectedToolIds) && 
      selectedToolIds.length === 0;
    
    // Filter tools based on user selection for this request
    if (isExplicitEmptySelection) {
      // User explicitly selected NO tools - we should disable all tools
      console.log('===============================================================');
      console.log(`[Server] 🚫🚫🚫 ALL TOOLS DISABLED - User explicitly selected ZERO tools`);
      console.log('===============================================================');
      
      // Clear all tools
      combinedTools = {};
      
      console.log(`[Server] 🚫 Removed ALL tools from this request`);
      console.log('===============================================================');
      console.log(`[Server] 🔧 FINAL TOOL SELECTION: NONE (all tools disabled)`);
      console.log('===============================================================');
    }
    else if (selectedToolIds && selectedToolIds.length > 0) {
      // User selected specific tools
      console.log('===============================================================');
      console.log(`[Server] 🔧 TOOL FILTERING ACTIVE - User selected ${selectedToolIds.length} specific tools:`);
      console.log(`[Server] 🔧 Selected tools: ${JSON.stringify(selectedToolIds)}`);
      console.log('===============================================================');
      
      // Start with empty tools object
      const filteredTools: Record<string, any> = {};
      
      // For any MCP tools that aren't in combinedTools, check if we need to add them directly from MCP
      // This is important - we need to get the tools directly from MCP for selected tools that might
      // not have been added to combinedTools yet
      const allMCPTools = getMCPTools();
      
      // Only include tools that were explicitly selected by the user
      for (const toolId of selectedToolIds) {
        // First check if the tool is already in our combined tools
        if (combinedTools[toolId]) {
          console.log(`[Server] ✅ Including selected tool from combinedTools: ${toolId}`);
          filteredTools[toolId] = combinedTools[toolId];
        } 
        // Next check if it's a valid MCP tool that we need to add
        else if (allMCPTools[toolId]) {
          console.log(`[Server] ✅ Including selected tool directly from MCP: ${toolId}`);
          // Get the raw MCP tool
          const rawTool = allMCPTools[toolId];
          // Wrap it with error handling
          const wrappedTool = {
            ...rawTool,
            // Add error handling to execute method
            execute: async (...args: any[]) => {
              try {
                return await rawTool.execute(...args);
              } catch (error) {
                const toolError = error instanceof ToolError
                  ? error
                  : transformToolError(error, toolId);
                console.error(`Error executing MCP tool ${toolId}:`, toolError);
                throw toolError;
              }
            }
          };
          // Add to filtered tools
          filteredTools[toolId] = wrappedTool;
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
        
        // First, get all available MCP tools to use as a reference
        const mcpTools = getMCPTools();
        const allPossibleToolIds = ['shell_command', ...Object.keys(mcpTools)];
        
        console.log(`[Server] Found ${allPossibleToolIds.length} possible tools (1 built-in + ${Object.keys(mcpTools).length} MCP tools)`);
        
        // Try to fetch from settings repository, but with better error handling
        try {
          // Import settings dynamically to avoid circular dependencies
          // Use a safer try/catch approach to handle ESM vs CJS issues
          let settingsRepository;
          try {
            const settingsModule = require('@openagents/core');
            settingsRepository = settingsModule.settingsRepository;
          } catch (requireError) {
            console.warn('[Server] Could not require @openagents/core directly:', requireError);
            // If that failed, try alternative approaches
            try {
              const { settingsRepository: repo } = require('@openagents/core/src/db/repositories/settings-repository');
              settingsRepository = repo;
            } catch (e) {
              console.warn('[Server] Alternative import also failed:', e);
            }
          }
          
          if (settingsRepository && typeof settingsRepository.getEnabledToolIds === 'function') {
            console.log('[Server] Fetching enabled tool IDs from settings repository');
            const enabledIds = await settingsRepository.getEnabledToolIds();
            
            // If we got valid enabled IDs from settings, use those
            if (Array.isArray(enabledIds) && enabledIds.length > 0) {
              console.log('[Server] Using enabled tool IDs from settings:', enabledIds);
              
              // But include any new tools that weren't in settings yet
              // This ensures new MCP tools show up even if the settings haven't been updated
              const knownToolIds = new Set(enabledIds);
              const missingTools = allPossibleToolIds.filter(id => !knownToolIds.has(id));
              
              if (missingTools.length > 0) {
                console.log(`[Server] Adding ${missingTools.length} new tools that weren't in settings yet:`, missingTools);
                
                // Add missing tools to the settings repository if possible
                if (typeof settingsRepository.enableTool === 'function') {
                  try {
                    for (const toolId of missingTools) {
                      await settingsRepository.enableTool(toolId);
                      console.log(`[Server] Auto-enabled new tool: ${toolId}`);
                    }
                  } catch (e) {
                    console.warn('[Server] Error auto-enabling new tools:', e);
                  }
                }
                
                // Return the combined list
                const combinedTools = [...enabledIds, ...missingTools];
                console.log(`[Server] Using combined tool list with ${combinedTools.length} tools`);
                return combinedTools;
              }
              
              return enabledIds;
            } else {
              console.log('[Server] No enabled tool IDs found in settings, using default');
            }
          } else {
            console.warn('[Server] Settings repository not found or missing getEnabledToolIds method');
          }
        } catch (settingsError) {
          console.warn('[Server] Could not fetch tool IDs from settings repository:', settingsError);
        }
        
        // If we got here, either there were no enabled tools in settings or we couldn't access settings
        // Include all available tools by default
        console.log(`[Server] Using enhanced default tool set with all available tools (${allPossibleToolIds.length} tools)`);
        console.log(`[Server] Available tools: ${allPossibleToolIds.join(', ')}`);
        return allPossibleToolIds;
      } catch (error) {
        console.error("Error getting enabled tool IDs:", error);
        
        // Even in error case, try to get MCP tools
        try {
          const mcpTools = getMCPTools();
          const fallbackTools = ['shell_command', ...Object.keys(mcpTools)];
          console.log(`[Server] Using ${fallbackTools.length} tools as error fallback`);
          return fallbackTools;
        } catch (e) {
          console.error("Error getting fallback tool list:", e);
          return ['shell_command']; // Absolute minimum fallback
        }
      }
    }
    
    // Configure streaming options
    const streamOptions = {
      // Only include tools if the model supports them AND we have tools to include
      ...(provider.supportsTools && Object.keys(combinedTools).length > 0 
        ? { tools: combinedTools, toolCallStreaming: true } 
        : {}),
      temperature: body.temperature || 0.7
    };
    
    console.log(`[Server] 🔧🔧🔧 Final stream options with tools:`, JSON.stringify({
      ...streamOptions,
      tools: Object.keys(streamOptions.tools || {})
    }, null, 2));
    
    // Extra debug information
    console.log(`[Server] 🔧 Provider supports tools: ${provider.supportsTools}`);
    console.log(`[Server] 🔧 Combined tools count: ${Object.keys(combinedTools).length}`);
    
    if (Object.keys(combinedTools).length > 0) {
      console.log(`[Server] 🔧 Combined tools: ${Object.keys(combinedTools).join(', ')}`);
    } else {
      console.log(`[Server] ❌ No tools included in stream options!`);
    }
    
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