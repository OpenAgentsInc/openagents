import { describe, expect, it } from "bun:test";
import { mkdtempSync, writeFileSync } from "fs";
import { join } from "path";
import { buildSystemPromptWithContext, loadContextFiles } from "./context-loader.js";

describe("context loader", () => {
  it("loads AGENTS/CLAUDE files from ancestors", () => {
    const dir = mkdtempSync("/tmp/ctx-");
    const sub = join(dir, "sub");
    const { mkdirSync } = require("fs");
    mkdirSync(sub, { recursive: true });
    writeFileSync(join(dir, "AGENTS.md"), "root-context");
    mkdirSync(sub, { recursive: true });
    writeFileSync(join(sub, "CLAUDE.md"), "sub-context");

    const contexts = loadContextFiles(sub);
    expect(contexts.join("\n")).toContain("root-context");
    expect(contexts.join("\n")).toContain("sub-context");
  });

  it("builds system prompt with timestamp and cwd", () => {
    const prompt = buildSystemPromptWithContext("base", "/tmp");
    expect(prompt).toContain("Current time:");
    expect(prompt).toContain("CWD: /tmp");
    expect(prompt).toContain("base");
  });
});
