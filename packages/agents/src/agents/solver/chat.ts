import { Effect, Layer } from "effect";
import type { UIMessage } from "ai";
import { FetchHttpClient } from "@effect/platform";
import type { SolverState, TextUIPart } from "./types";
import { ChatError, AnthropicConfig } from "./types";
import { env } from "cloudflare:workers";

// --- Service Implementation (Layers) ---

// Layer for the Anthropic Client using Fetch
const AnthropicClientLive = Layer.succeed(
  AnthropicConfig,
  {
    apiKey: env.ANTHROPIC_API_KEY || "",
    fetch: globalThis.fetch,
    model: "claude-3-5-sonnet-latest"
  } as AnthropicConfig
);

// Combined AI chat layers with FetchHttpClient
export const AiChatLayers = Layer.merge(
  AnthropicClientLive,
  FetchHttpClient.layer
);

// --- Chat Effect Creation ---

/**
 * Creates an Effect that handles a chat interaction
 */
export const createChatEffect = (
  currentState: SolverState,
  userMessageContent: string
): Effect.Effect<UIMessage, ChatError, AnthropicConfig> =>
  Effect.gen(function* (_) {
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
        anthropicConfig = yield* _(AnthropicConfig);
      } catch (e) {
        // Provide default config if it's not in the context
        anthropicConfig = {
          apiKey: env.ANTHROPIC_API_KEY || "",
          fetch: globalThis.fetch,
          model: "claude-3-5-sonnet-latest"
        };
        yield* _(Effect.logWarning("Using default AnthropicConfig"));
      }

      const apiKey = anthropicConfig.apiKey;
      const model = anthropicConfig.model || "claude-3-5-sonnet-latest";

      // For now, simulate the response - this would be replaced with actual API call
      // const responseContent = `I understand you're working on ${currentState.currentIssue?.title}. How can I help?`;

      // Now implement the actual API call

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
      yield* _(Effect.logError("AI completion failed", { error }));
      return yield* _(Effect.fail(new ChatError({ cause: error })));
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
