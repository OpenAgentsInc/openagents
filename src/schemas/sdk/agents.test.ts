import { describe, expect, test } from "bun:test";
import * as S from "effect/Schema";
import {
  AgentDefinition,
  AgentModel,
  AgentRegistry,
  ResumeStrategy,
  SessionMetadata,
  SubagentConfig,
  SubagentResult,
  hasClaudeCodeSession,
  hasModelOverride,
  hasToolRestriction,
  isFailedResult,
  isSuccessfulResult,
} from "./agents.js";

describe("Agent Schemas", () => {
  describe("AgentModel", () => {
    test("accepts valid model values", () => {
      const models = ["sonnet", "opus", "haiku", "inherit"];

      for (const model of models) {
        expect(() => S.decodeUnknownSync(AgentModel)(model)).not.toThrow();
      }
    });

    test("rejects invalid model values", () => {
      expect(() => S.decodeUnknownSync(AgentModel)("gpt-4")).toThrow();
    });
  });

  describe("AgentDefinition", () => {
    test("decodes minimal valid agent definition", () => {
      const def = {
        description: "Expert code reviewer",
        prompt: "You are a code review specialist...",
      };

      const decoded = S.decodeUnknownSync(AgentDefinition)(def);
      expect(decoded.description).toBe("Expert code reviewer");
      expect(decoded.prompt).toBe("You are a code review specialist...");
    });

    test("decodes agent definition with tools restriction", () => {
      const def = {
        description: "Read-only analyzer",
        prompt: "Analyze code without modifications",
        tools: ["Read", "Grep", "Glob"],
      };

      const decoded = S.decodeUnknownSync(AgentDefinition)(def);
      expect(decoded.tools).toEqual(["Read", "Grep", "Glob"]);
      expect(hasToolRestriction(decoded)).toBe(true);
    });

    test("decodes agent definition with model override", () => {
      const def = {
        description: "Fast task executor",
        prompt: "Complete tasks quickly",
        model: "haiku",
      };

      const decoded = S.decodeUnknownSync(AgentDefinition)(def);
      expect(decoded.model).toBe("haiku");
      expect(hasModelOverride(decoded)).toBe(true);
    });

    test("decodes complete agent definition", () => {
      const def = {
        description: "Full-featured agent",
        prompt: "Do everything",
        tools: ["Read", "Write", "Edit", "Bash"],
        model: "sonnet" as const,
      };

      const decoded = S.decodeUnknownSync(AgentDefinition)(def);
      expect(decoded).toEqual(def);
    });
  });

  describe("SubagentConfig", () => {
    test("decodes minimal subagent config", () => {
      const config = {
        description: "Fix the login bug",
        cwd: "/workspace",
        tools: ["Read", "Edit", "Bash"],
      };

      const decoded = S.decodeUnknownSync(SubagentConfig)(config);
      expect(decoded.description).toBe("Fix the login bug");
      expect(decoded.cwd).toBe("/workspace");
      expect(decoded.tools).toEqual(["Read", "Edit", "Bash"]);
    });

    test("decodes config with all optional fields", () => {
      const config = {
        description: "Refactor authentication",
        cwd: "/workspace",
        tools: ["Read", "Write", "Edit"],
        model: "sonnet",
        permissionMode: "bypassPermissions",
        maxTurns: 30,
        resumeSessionId: "sess-123",
        forkSession: true,
        systemPromptAppend: "Be extra careful with security",
      };

      const decoded = S.decodeUnknownSync(SubagentConfig)(config);
      expect(decoded.model).toBe("sonnet");
      expect(decoded.permissionMode).toBe("bypassPermissions");
      expect(decoded.maxTurns).toBe(30);
      expect(decoded.resumeSessionId).toBe("sess-123");
      expect(decoded.forkSession).toBe(true);
      expect(decoded.systemPromptAppend).toBe("Be extra careful with security");
    });
  });

  describe("ResumeStrategy", () => {
    test("accepts valid resume strategies", () => {
      expect(() => S.decodeUnknownSync(ResumeStrategy)("continue")).not.toThrow();
      expect(() => S.decodeUnknownSync(ResumeStrategy)("fork")).not.toThrow();
    });

    test("rejects invalid resume strategies", () => {
      expect(() => S.decodeUnknownSync(ResumeStrategy)("restart")).toThrow();
    });
  });

  describe("SessionMetadata", () => {
    test("decodes minimal session metadata", () => {
      const metadata = {};

      const decoded = S.decodeUnknownSync(SessionMetadata)(metadata);
      expect(decoded).toEqual({});
    });

    test("decodes session metadata with session IDs", () => {
      const metadata = {
        sessionId: "sess-123",
        forkedFromSessionId: "sess-100",
        resumeStrategy: "fork",
      };

      const decoded = S.decodeUnknownSync(SessionMetadata)(metadata);
      expect(decoded.sessionId).toBe("sess-123");
      expect(decoded.forkedFromSessionId).toBe("sess-100");
      expect(decoded.resumeStrategy).toBe("fork");
    });

    test("decodes complete session metadata", () => {
      const metadata = {
        sessionId: "sess-456",
        toolsUsed: { Edit: 5, Bash: 3 },
        blockers: ["Rate limit hit"],
        suggestedNextSteps: ["Wait 60s then retry"],
        summary: "Made good progress on auth refactor",
        usage: {
          inputTokens: 1000,
          outputTokens: 500,
          cacheReadInputTokens: 200,
          cacheCreationInputTokens: 50,
        },
        totalCostUsd: 0.0123,
      };

      const decoded = S.decodeUnknownSync(SessionMetadata)(metadata);
      expect(decoded.sessionId).toBe("sess-456");
      expect(decoded.toolsUsed).toEqual({ Edit: 5, Bash: 3 });
      expect(decoded.blockers).toEqual(["Rate limit hit"]);
      expect(decoded.usage?.inputTokens).toBe(1000);
      expect(decoded.totalCostUsd).toBe(0.0123);
    });
  });

  describe("SubagentResult", () => {
    test("decodes minimal successful result", () => {
      const result = {
        success: true,
        subtaskId: "sub-1",
        filesModified: [],
        turns: 5,
      };

      const decoded = S.decodeUnknownSync(SubagentResult)(result);
      expect(decoded.success).toBe(true);
      expect(decoded.subtaskId).toBe("sub-1");
      expect(isSuccessfulResult(decoded)).toBe(true);
    });

    test("decodes failed result with error", () => {
      const result = {
        success: false,
        subtaskId: "sub-2",
        filesModified: [],
        turns: 3,
        error: "Timeout after 3 turns",
      };

      const decoded = S.decodeUnknownSync(SubagentResult)(result);
      expect(decoded.success).toBe(false);
      expect(decoded.error).toBe("Timeout after 3 turns");
      expect(isFailedResult(decoded)).toBe(true);
    });

    test("decodes result with Claude Code session", () => {
      const result = {
        success: true,
        subtaskId: "sub-3",
        filesModified: ["src/auth.ts", "src/login.ts"],
        turns: 10,
        agent: "claude-code",
        claudeCodeSessionId: "sess-789",
      };

      const decoded = S.decodeUnknownSync(SubagentResult)(result);
      expect(decoded.agent).toBe("claude-code");
      expect(decoded.claudeCodeSessionId).toBe("sess-789");
      expect(hasClaudeCodeSession(decoded)).toBe(true);
    });

    test("decodes result with forked session", () => {
      const result = {
        success: true,
        subtaskId: "sub-4",
        filesModified: [],
        turns: 7,
        claudeCodeSessionId: "sess-new",
        claudeCodeForkedFromSessionId: "sess-old",
      };

      const decoded = S.decodeUnknownSync(SubagentResult)(result);
      expect(decoded.claudeCodeForkedFromSessionId).toBe("sess-old");
    });

    test("decodes result with complete metadata", () => {
      const result = {
        success: true,
        subtaskId: "sub-5",
        filesModified: ["test.ts"],
        turns: 8,
        agent: "minimal",
        tokenUsage: { input: 2000, output: 1000 },
        verificationOutputs: ["All tests passed"],
        sessionMetadata: {
          sessionId: "sess-complete",
          toolsUsed: { Write: 2, Bash: 1 },
          summary: "Completed successfully",
          usage: { inputTokens: 2000, outputTokens: 1000 },
        },
      };

      const decoded = S.decodeUnknownSync(SubagentResult)(result);
      expect(decoded.tokenUsage?.input).toBe(2000);
      expect(decoded.verificationOutputs).toEqual(["All tests passed"]);
      expect(decoded.sessionMetadata?.sessionId).toBe("sess-complete");
    });
  });

  describe("AgentRegistry", () => {
    test("decodes agent registry", () => {
      const registry = {
        agents: {
          "code-reviewer": {
            description: "Reviews code",
            prompt: "Review this code...",
            tools: ["Read", "Grep"],
          },
          "test-runner": {
            description: "Runs tests",
            prompt: "Run the tests...",
            tools: ["Bash"],
            model: "haiku",
          },
        },
      };

      const decoded = S.decodeUnknownSync(AgentRegistry)(registry);
      expect(Object.keys(decoded.agents)).toEqual(["code-reviewer", "test-runner"]);
      expect(decoded.agents["code-reviewer"].description).toBe("Reviews code");
      expect(decoded.agents["test-runner"].model).toBe("haiku");
    });
  });

  describe("Type guards", () => {
    test("hasToolRestriction correctly identifies agents with tool restrictions", () => {
      const withTools = S.decodeUnknownSync(AgentDefinition)({
        description: "Restricted",
        prompt: "Use only these tools",
        tools: ["Read"],
      });

      const withoutTools = S.decodeUnknownSync(AgentDefinition)({
        description: "Unrestricted",
        prompt: "Use any tools",
      });

      expect(hasToolRestriction(withTools)).toBe(true);
      expect(hasToolRestriction(withoutTools)).toBe(false);
    });

    test("hasModelOverride correctly identifies agents with model overrides", () => {
      const withModel = S.decodeUnknownSync(AgentDefinition)({
        description: "Fast",
        prompt: "Be quick",
        model: "haiku",
      });

      const withoutModel = S.decodeUnknownSync(AgentDefinition)({
        description: "Standard",
        prompt: "Use default model",
      });

      expect(hasModelOverride(withModel)).toBe(true);
      expect(hasModelOverride(withoutModel)).toBe(false);
    });

    test("isSuccessfulResult and isFailedResult are mutually exclusive", () => {
      const success = S.decodeUnknownSync(SubagentResult)({
        success: true,
        subtaskId: "sub-1",
        filesModified: [],
        turns: 5,
      });

      const failure = S.decodeUnknownSync(SubagentResult)({
        success: false,
        subtaskId: "sub-2",
        filesModified: [],
        turns: 3,
        error: "Failed",
      });

      expect(isSuccessfulResult(success)).toBe(true);
      expect(isFailedResult(success)).toBe(false);

      expect(isSuccessfulResult(failure)).toBe(false);
      expect(isFailedResult(failure)).toBe(true);
    });

    test("hasClaudeCodeSession correctly identifies results with sessions", () => {
      const withSession = S.decodeUnknownSync(SubagentResult)({
        success: true,
        subtaskId: "sub-1",
        filesModified: [],
        turns: 5,
        claudeCodeSessionId: "sess-123",
      });

      const withoutSession = S.decodeUnknownSync(SubagentResult)({
        success: true,
        subtaskId: "sub-2",
        filesModified: [],
        turns: 5,
      });

      expect(hasClaudeCodeSession(withSession)).toBe(true);
      expect(hasClaudeCodeSession(withoutSession)).toBe(false);
    });
  });
});
