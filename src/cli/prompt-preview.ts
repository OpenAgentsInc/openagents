import { readFileSync, existsSync } from "node:fs";
import { resolve } from "node:path";
import { BASE_SYSTEM_PROMPT } from "../agent/prompts.js";
import { buildSystemPromptWithContext, loadContextSources, type ContextSource } from "./context-loader.js";
import type { BundledFile, Mode } from "./parser.js";

export interface PromptPreviewOptions {
  basePrompt?: string;
  cwd?: string;
  provider?: string;
  model?: string;
  thinking?: "off" | "minimal" | "low" | "medium" | "high";
  tools?: string[];
  mode?: Mode;
  messages?: string[];
  files?: BundledFile[];
  contexts?: ContextSource[];
}

export const resolveBasePrompt = (value?: string): { prompt: string; source: string } => {
  if (!value) {
    return { prompt: BASE_SYSTEM_PROMPT, source: "default" };
  }

  const maybePath = resolve(value);
  if (existsSync(maybePath)) {
    return { prompt: readFileSync(maybePath, "utf8"), source: maybePath };
  }

  return { prompt: value, source: "inline" };
};

const formatFiles = (files: BundledFile[]): string[] => {
  if (!files.length) return ["(none)"];
  return files.map((file) => `- ${file.path} (${file.isImage ? "image" : "text"})`);
};

const formatMessages = (messages: string[]): string[] => {
  if (!messages.length) return ["(none)"];
  return messages.map((msg, idx) => `- [${idx + 1}] ${msg}`);
};

const formatContexts = (contexts: ContextSource[]): string[] => {
  if (!contexts.length) return ["(none)"];
  return contexts.map((ctx) => `- ${ctx.path}`);
};

export const renderPromptPreview = (options: PromptPreviewOptions): string => {
  const basePrompt = options.basePrompt ?? BASE_SYSTEM_PROMPT;
  const cwd = options.cwd ?? process.cwd();
  const contexts = options.contexts ?? loadContextSources(cwd);
  const systemPrompt = buildSystemPromptWithContext(basePrompt, cwd);

  const parts: string[] = [];
  parts.push("# Prompt Preview");
  parts.push("");
  parts.push(
    [
      `Mode: ${options.mode ?? "text"}`,
      `Provider: ${options.provider ?? "auto"}`,
      `Model: ${options.model ?? "auto"}`,
    ].join(" | "),
  );
  parts.push(`Thinking: ${options.thinking ?? "off"}`);
  parts.push(`Tools: ${options.tools?.length ? options.tools.join(", ") : "all"}`);
  parts.push("");
  parts.push("Context files:");
  parts.push(...formatContexts(contexts));
  parts.push("");
  parts.push("System prompt:");
  parts.push(systemPrompt);
  parts.push("");
  parts.push("Messages:");
  parts.push(...formatMessages(options.messages ?? []));
  parts.push("");
  parts.push("Attachments:");
  parts.push(...formatFiles(options.files ?? []));

  return parts.join("\n");
};
