import * as Context from "effect/Context";
import { Effect, Layer } from "effect";
import type { ChatRequest, ChatResponse } from "./openrouter.js";
import { OpenRouterClient } from "./openrouter.js";
import { AnthropicClient } from "./anthropic.js";
import { OpenAIClient } from "./openai.js";

export interface ChatProvider {
  chat: (request: ChatRequest) => Effect.Effect<ChatResponse, Error>;
}

export class ChatProviderTag extends Context.Tag("ChatProvider")<
  ChatProviderTag,
  ChatProvider
>() {}

export const fromClient = (client: { chat: (request: ChatRequest) => Effect.Effect<ChatResponse, Error> }): ChatProvider => ({
  chat: (request: ChatRequest) => client.chat(request),
});

export const openRouterProviderLayer = Layer.effect(
  ChatProviderTag,
  Effect.gen(function* () {
    const client = yield* OpenRouterClient;
    return fromClient(client);
  }),
);

export const anthropicProviderLayer = Layer.effect(
  ChatProviderTag,
  Effect.gen(function* () {
    const client = yield* AnthropicClient;
    return fromClient(client);
  }),
);

export const openAIProviderLayer = Layer.effect(
  ChatProviderTag,
  Effect.gen(function* () {
    const client = yield* OpenAIClient;
    return fromClient(client);
  }),
);
