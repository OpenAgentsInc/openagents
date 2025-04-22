import { Data, Context, Effect } from "effect";
import type { UIMessage } from "ai";
import type { BaseIssue, BaseProject, BaseTeam } from "@openagents/core";

// --- UI Part Types ---
export type TextUIPart = {
  type: 'text';
  text: string;
};

export type UIPart = TextUIPart;

// --- State Definition ---
export type SolverState = {
  messages: UIMessage[];
  currentIssue?: BaseIssue;
  currentProject?: BaseProject;
  currentTeam?: BaseTeam;
  githubToken?: string;
};

// --- Error Definitions ---
export class ParseError extends Data.TaggedError("ParseError")<{ cause: unknown }> { }
export class StateUpdateError extends Data.TaggedError("StateUpdateError")<{ cause: unknown }> { }
export class ChatError extends Data.TaggedError("ChatError")<{ cause: unknown }> { }

export type HandleMessageError = ParseError | StateUpdateError;

// --- Service Definitions ---
export interface AnthropicConfig {
  readonly apiKey: string;
  readonly fetch: typeof fetch;
  readonly model?: string;
}

export class AnthropicConfig extends Context.Tag("AnthropicConfig")<AnthropicConfig, AnthropicConfig>() { }

export interface AiClientService {
  readonly getApiKey: () => Effect.Effect<string>;
}

export class AiClientService extends Context.Tag("AiClientService")<
  AiClientService,
  AiClientService
>() { }
