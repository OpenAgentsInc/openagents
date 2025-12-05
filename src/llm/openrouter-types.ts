import type { Tool } from "../tools/schema.js";

export type LogLevel = "debug" | "info" | "warn" | "error";

export type ContentBlock =
  | { type: "text"; text: string }
  | { type: "image"; data: string; mimeType: string };

export interface ChatMessage {
  role: "system" | "user" | "assistant" | "tool";
  content: string | ContentBlock[];
  tool_call_id?: string;
  name?: string;
}

export interface ChatRequest {
  model?: string;
  messages: ChatMessage[];
  tools?: Tool<any>[];
  temperature?: number;
  maxTokens?: number;
  toolChoice?: "auto" | "required" | { type: "function"; function: { name: string } };
  baseUrl?: string;
  apiKey?: string;
  headers?: Record<string, string>;
  retry?: Partial<import("./retry.js").RetryConfig>;
  timeoutMs?: number;
  logLevel?: LogLevel;
  logger?: OpenRouterLogger;
}

export interface ChatToolCall {
  id: string;
  name: string;
  arguments: string;
}

export interface ChatChoice {
  message: {
    role: "assistant";
    content: string | null;
    tool_calls?: ChatToolCall[];
  };
}

export interface ChatResponse {
  id: string;
  choices: ChatChoice[];
  usage?: {
    prompt_tokens?: number;
    completion_tokens?: number;
    total_tokens?: number;
  };
}

export interface OpenRouterLogger {
  level: LogLevel;
  debug: (message: string) => void;
  info: (message: string) => void;
  warn: (message: string) => void;
  error: (message: string) => void;
}
