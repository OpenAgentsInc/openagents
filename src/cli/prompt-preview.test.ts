import { describe, expect, it } from "bun:test";
import { mkdtempSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { renderPromptPreview, resolveBasePrompt } from "./prompt-preview.js";

describe("prompt preview", () => {
  it("renders preview with contexts, tools, and messages", () => {
    const preview = renderPromptPreview({
      basePrompt: "Base prompt",
      provider: "openai",
      model: "gpt-4o",
      thinking: "medium",
      tools: ["read", "write"],
      mode: "text",
      messages: ["First question", "Second question"],
      files: [{ path: "/tmp/a.txt", content: "hello", isImage: false }],
      contexts: [{ path: "/tmp/AGENTS.md", content: "agent rules" }],
    });

    expect(preview).toContain("openai");
    expect(preview).toContain("gpt-4o");
    expect(preview).toContain("read, write");
    expect(preview).toContain("Base prompt");
    expect(preview).toContain("/tmp/AGENTS.md");
    expect(preview).toContain("First question");
    expect(preview).toContain("/tmp/a.txt");
  });

  it("resolves base prompt from file path", () => {
    const dir = mkdtempSync("/tmp/prompt-");
    const promptPath = join(dir, "prompt.txt");
    writeFileSync(promptPath, "From file");

    const resolved = resolveBasePrompt(promptPath);
    expect(resolved.prompt).toBe("From file");
    expect(resolved.source).toBe(promptPath);
  });
});
