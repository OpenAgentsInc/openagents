import { Effect, Layer } from "effect";
import type { UIMessage } from "ai";
import { FetchHttpClient } from "@effect/platform";
import type { SolverState, TextUIPart } from "./types";
import { ChatError, OpenAIConfig } from "./types";
import { env } from "cloudflare:workers";

// --- Service Implementation (Layers) ---

// Layer for the OpenAI Client using Fetch
const OpenAiClientLive = Layer.succeed(
  OpenAIConfig,
  {
    apiKey: env.OPENAI_API_KEY || "",
    fetch: globalThis.fetch
  } as OpenAIConfig
);

// Combined AI chat layers with FetchHttpClient
export const AiChatLayers = Layer.merge(
  OpenAiClientLive,
  FetchHttpClient.layer
);

// --- Chat Effect Creation ---

/**
 * Creates an Effect that handles a chat interaction
 */
export const createChatEffect = (
  currentState: SolverState,
  userMessageContent: string
): Effect.Effect<UIMessage, ChatError, never> =>
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
      // Simulate AI response for now - replace with actual implementation
      const responseContent = `I understand you're working on ${currentState.currentIssue?.title}. How can I help?`;

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
): Effect.Effect<UIMessage, ChatError, never> => {
  const initialMessage = `Context loaded for issue #${currentState.currentIssue?.number}: "${currentState.currentIssue?.title}". How can I assist?`;
  return createChatEffect(currentState, initialMessage);
};
