import { Effect, Layer } from "effect";
import type { UIMessage } from "ai";
import { FetchHttpClient } from "@effect/platform";
import type { SolverState, TextUIPart } from "./types";
import { ChatError, AnthropicConfig } from "./types";
import { env } from "cloudflare:workers";

// --- Service Implementation (Layers) ---

// Layer for the Anthropic Config 
const AnthropicConfigLive = Layer.succeed(
  AnthropicConfig,
  {
    apiKey: env.ANTHROPIC_API_KEY || process.env.ANTHROPIC_API_KEY || "",
    fetch: globalThis.fetch,
    model: "claude-3-5-sonnet-latest"
  } as AnthropicConfig
);

// FetchHttpClient layer for making HTTP requests
export const FetchHttpClientLayer = FetchHttpClient.layer;

// --- Chat Effect Creation ---

/**
 * Creates an Effect that handles a chat interaction
 */
export const createChatEffect = (
  currentState: SolverState,
  userMessageContent: string
): Effect.Effect<UIMessage, ChatError, AnthropicConfig> =>
  Effect.gen(function* (eff) {
    // Create user message with proper parts
    const userTextPart: TextUIPart = {
      type: 'text',
      text: userMessageContent
    };

    const userMessage: UIMessage = {
      id: crypto.randomUUID(),
      role: 'user',
      content: userMessageContent,
      createdAt: new Date(),
      parts: [userTextPart]
    };

    // Combine messages for context
    const messagesForLLM = [...currentState.messages, userMessage];

    // Generate system prompt based on current context
    const systemPrompt = `You are Solver Agent, helping with issue: ${currentState.currentIssue?.title ?? 'Unknown'
      }. Project: ${currentState.currentProject?.name ?? 'Unknown'
      }. Team: ${currentState.currentTeam?.name ?? 'Unknown'
      }.`;

    try {
      // Preparation for actual Anthropic API call
      // Get API key and client from Effect context, or use defaults for testing
      let anthropicConfig;
      try {
        anthropicConfig = yield* eff(AnthropicConfig);
      } catch (e) {
        // Provide default config if it's not in the context
        anthropicConfig = {
          apiKey: env.ANTHROPIC_API_KEY || process.env.ANTHROPIC_API_KEY || "",
          fetch: globalThis.fetch,
          model: "claude-3-5-sonnet-latest"
        };
        yield* eff(Effect.logWarning("Using default AnthropicConfig"));
      }

      const apiKey = anthropicConfig.apiKey;
      const model = anthropicConfig.model || "claude-3-5-sonnet-latest";

      // Implement the actual API call using Effect AI
      yield* eff(Effect.logInfo("Making Anthropic API call", { model }));
      
      // Initialize responseContent variable to hold either API response or fallback
      let responseContent = '';
      
      // Direct Anthropic API call using fetch
      try {
        // Configure the API request
        const apiUrl = "https://api.anthropic.com/v1/messages";
        const headers = {
          "Content-Type": "application/json",
          "x-api-key": apiKey,
          "anthropic-version": "2023-06-01"
        };
        
        // Prepare messages in Anthropic format
        const formattedMessages = messagesForLLM.map(msg => {
          if (msg.role === "user" || msg.role === "assistant") {
            return {
              role: msg.role,
              content: msg.content
            };
          } else {
            // Default to user for any other role types
            return {
              role: "user",
              content: msg.content
            };
          }
        });
        
        // Build request body
        const requestBody = {
          model: model || "claude-3-5-sonnet-latest",
          messages: formattedMessages,
          system: systemPrompt,
          max_tokens: 1000,
          temperature: 0.7
        };
        
        // Log the request for debugging
        yield* eff(Effect.logInfo("Sending Anthropic API request", { 
          model: requestBody.model,
          messageCount: formattedMessages.length,
          systemPromptLength: systemPrompt.length
        }));
        
        // Make the API request
        const response = yield* eff(Effect.tryPromise({
          try: () => fetch(apiUrl, {
            method: "POST",
            headers,
            body: JSON.stringify(requestBody)
          }),
          catch: error => new ChatError({ cause: error })
        }));
        
        // Check for HTTP errors
        if (!response.ok) {
          const errorText = yield* eff(Effect.tryPromise({
            try: () => response.text(),
            catch: error => new ChatError({ cause: error })
          }));
          
          yield* eff(Effect.logError("Anthropic API error", { 
            status: response.status, 
            statusText: response.statusText,
            body: errorText
          }));
          
          throw new ChatError({ 
            cause: new Error(`Anthropic API error: ${response.status} ${response.statusText}`) 
          });
        }
        
        // Parse the response
        const jsonData = yield* eff(Effect.tryPromise({
          try: () => response.json(),
          catch: error => new ChatError({ cause: error })
        }));
        
        // Type assertion for the response format
        const responseData = jsonData as { content: Array<{ type: string, text: string }> };
        
        // Extract content from the Anthropic response
        responseContent = responseData.content[0].text;
        
        yield* eff(Effect.logInfo("Received Anthropic API response", { 
          responseLength: responseContent.length,
          preview: responseContent.substring(0, 100) + (responseContent.length > 100 ? "..." : "")
        }));
      } catch (error) {
        // Fallback response if the API call fails
        yield* eff(Effect.logError("Failed to call Anthropic API, using fallback response", { 
          error: error instanceof Error ? error.message : String(error)
        }));
        
        // Generate a fallback response
        if (currentState.currentIssue) {
          responseContent = `I'm your Solver agent, ready to help with issue #${currentState.currentIssue.number}: "${currentState.currentIssue.title}".
          
Based on the issue description, I can assist with analyzing requirements, researching solutions, planning implementation steps, and providing guidance throughout the development process.

What specific aspect of this issue would you like me to help with first?`;
        } else {
          responseContent = `I'm your Solver agent, ready to help. What project issue would you like assistance with?`;
        }
      }
      
      yield* eff(Effect.logInfo("Generated response for the user", { 
        responseLength: responseContent.length,
        preview: responseContent.substring(0, 100) + (responseContent.length > 100 ? "..." : "")
      }));

      // Format the AI response with proper parts
      const assistantTextPart: TextUIPart = {
        type: 'text',
        text: responseContent
      };

      const assistantMessage: UIMessage = {
        id: crypto.randomUUID(),
        role: 'assistant',
        content: responseContent,
        createdAt: new Date(),
        parts: [assistantTextPart]
        // model is not part of UIMessage interface, so we remove it
      };

      return assistantMessage;
    } catch (error) {
      yield* eff(Effect.logError("AI completion failed", { error }));
      return yield* eff(Effect.fail(new ChatError({ cause: error })));
    }
  }).pipe(
    Effect.withLogSpan("agentChatCompletion")
  );

/**
 * Creates the initial chat message effect after context is set
 */
export const createInitialChatEffect = (
  currentState: SolverState
): Effect.Effect<UIMessage, ChatError, AnthropicConfig> => {
  const initialMessage = `Context loaded for issue #${currentState.currentIssue?.number}: "${currentState.currentIssue?.title}". How can I assist?`;
  return createChatEffect(currentState, initialMessage);
};
