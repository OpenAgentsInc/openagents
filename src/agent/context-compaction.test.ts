/**
 * Tests for context compaction and backpressure.
 *
 * Mirrors pi-mono patterns for turn-based history management
 * and adds token-based/adaptive strategies.
 */

import { describe, test, expect } from "bun:test";
import {
  estimateTokensForString,
  estimateTokensForMessage,
  estimateTokensForMessages,
  estimateTokensForContentBlock,
  groupIntoTurns,
  turnsToMessages,
  compactByTurns,
  compactByTokens,
  compactAdaptive,
  checkBackpressure,
  compactMessages,
  needsCompaction,
  getContextSummary,
  DEFAULT_COMPACTION_CONFIG,
  type CompactionConfig,
} from "./context-compaction.js";
import type { ChatMessage, ContentBlock } from "../schemas/sdk/messages.js";

// =============================================================================
// Test Fixtures
// =============================================================================

/**
 * Create a simple text message.
 */
const createMessage = (
  role: "user" | "assistant" | "system",
  content: string
): ChatMessage => ({
  role,
  content,
});

/**
 * Create a message with content blocks.
 */
const createBlockMessage = (
  role: "user" | "assistant" | "system",
  blocks: ContentBlock[]
): ChatMessage => ({
  role,
  content: blocks,
});

/**
 * Create a typical conversation with N turns.
 * Each turn has a user message and assistant response.
 */
const createConversation = (numTurns: number, messageLength = 100): ChatMessage[] => {
  const messages: ChatMessage[] = [];
  const text = "x".repeat(messageLength);

  for (let i = 0; i < numTurns; i++) {
    messages.push(createMessage("user", `User message ${i}: ${text}`));
    messages.push(createMessage("assistant", `Assistant response ${i}: ${text}`));
  }

  return messages;
};

/**
 * Create a conversation with system message.
 */
const createConversationWithSystem = (
  numTurns: number,
  systemPrompt = "You are a helpful assistant."
): ChatMessage[] => [
  createMessage("system", systemPrompt),
  ...createConversation(numTurns),
];

// =============================================================================
// Token Estimation Tests
// =============================================================================

describe("Token Estimation", () => {
  test("estimateTokensForString returns 0 for empty string", () => {
    expect(estimateTokensForString("")).toBe(0);
  });

  test("estimateTokensForString estimates ~4 chars per token", () => {
    const text = "Hello, world!"; // 13 chars -> ~3-4 tokens
    const estimate = estimateTokensForString(text);
    expect(estimate).toBe(4); // ceil(13/4) = 4
  });

  test("estimateTokensForString handles long text", () => {
    const text = "x".repeat(1000);
    const estimate = estimateTokensForString(text);
    expect(estimate).toBe(250); // 1000/4 = 250
  });

  test("estimateTokensForContentBlock handles text blocks", () => {
    const block: ContentBlock = { type: "text", text: "Hello world" };
    const estimate = estimateTokensForContentBlock(block);
    expect(estimate).toBe(3); // ceil(11/4) = 3
  });

  test("estimateTokensForContentBlock handles thinking blocks", () => {
    const block: ContentBlock = { type: "thinking", thinking: "Let me think..." };
    const estimate = estimateTokensForContentBlock(block);
    expect(estimate).toBe(4); // ceil(15/4) = 4
  });

  test("estimateTokensForContentBlock handles tool_use blocks", () => {
    const block: ContentBlock = {
      type: "tool_use",
      id: "tool_1",
      name: "read_file",
      input: { path: "/test.txt" },
    };
    const estimate = estimateTokensForContentBlock(block);
    expect(estimate).toBeGreaterThan(0);
  });

  test("estimateTokensForContentBlock handles image blocks", () => {
    const block: ContentBlock = {
      type: "image",
      source: { type: "base64", media_type: "image/png", data: "..." },
    };
    const estimate = estimateTokensForContentBlock(block);
    expect(estimate).toBe(1000); // Images are expensive
  });

  test("estimateTokensForMessage includes role overhead", () => {
    const msg = createMessage("user", "Hi");
    const estimate = estimateTokensForMessage(msg);
    expect(estimate).toBeGreaterThan(estimateTokensForString("Hi"));
  });

  test("estimateTokensForMessages sums all messages", () => {
    const messages = [
      createMessage("user", "Hello"),
      createMessage("assistant", "Hi there!"),
    ];
    const estimate = estimateTokensForMessages(messages);
    expect(estimate).toBe(
      estimateTokensForMessage(messages[0]) + estimateTokensForMessage(messages[1])
    );
  });
});

// =============================================================================
// Turn Grouping Tests
// =============================================================================

describe("Turn Grouping", () => {
  test("groupIntoTurns groups user+assistant pairs", () => {
    const messages = createConversation(3);
    const turns = groupIntoTurns(messages);

    expect(turns.length).toBe(3);
    expect(turns[0].userMessage.role).toBe("user");
    expect(turns[0].assistantMessage?.role).toBe("assistant");
  });

  test("groupIntoTurns handles incomplete final turn", () => {
    const messages = [
      createMessage("user", "Question 1"),
      createMessage("assistant", "Answer 1"),
      createMessage("user", "Question 2"),
      // No assistant response yet
    ];
    const turns = groupIntoTurns(messages);

    expect(turns.length).toBe(2);
    expect(turns[1].userMessage.content).toBe("Question 2");
    expect(turns[1].assistantMessage).toBeUndefined();
  });

  test("groupIntoTurns calculates token estimates", () => {
    const messages = createConversation(2);
    const turns = groupIntoTurns(messages);

    expect(turns[0].tokenEstimate).toBeGreaterThan(0);
    expect(turns[1].tokenEstimate).toBeGreaterThan(0);
  });

  test("turnsToMessages reconstructs original order", () => {
    const messages = createConversation(3);
    const turns = groupIntoTurns(messages);
    const reconstructed = turnsToMessages(turns);

    expect(reconstructed.length).toBe(messages.length);
    for (let i = 0; i < messages.length; i++) {
      expect(reconstructed[i].role).toBe(messages[i].role);
    }
  });

  test("turnsToMessages preserves system messages", () => {
    const systemMsg = createMessage("system", "System prompt");
    const conversationMsgs = createConversation(2);
    const turns = groupIntoTurns(conversationMsgs);
    const reconstructed = turnsToMessages(turns, [systemMsg]);

    expect(reconstructed[0].role).toBe("system");
    expect(reconstructed.length).toBe(conversationMsgs.length + 1);
  });
});

// =============================================================================
// Turn-Based Compaction Tests (pi-mono style)
// =============================================================================

describe("Turn-Based Compaction", () => {
  test("compactByTurns keeps last N turns", () => {
    const messages = createConversation(10);
    const compacted = compactByTurns(messages, 5);

    // 5 turns = 10 messages (user + assistant each)
    expect(compacted.length).toBe(10);

    // Should keep the last 5 turns
    const turns = groupIntoTurns(compacted);
    expect(turns.length).toBe(5);
  });

  test("compactByTurns preserves system messages", () => {
    const messages = createConversationWithSystem(10, "Important system prompt");
    const compacted = compactByTurns(messages, 5, true);

    expect(compacted[0].role).toBe("system");
    expect(compacted[0].content).toBe("Important system prompt");
  });

  test("compactByTurns does nothing if under limit", () => {
    const messages = createConversation(3);
    const compacted = compactByTurns(messages, 5);

    expect(compacted.length).toBe(messages.length);
  });

  test("compactByTurns handles maxTurns=50 (pi-mono default)", () => {
    const messages = createConversation(100);
    const compacted = compactByTurns(messages, 50);

    const turns = groupIntoTurns(compacted);
    expect(turns.length).toBe(50);
  });
});

// =============================================================================
// Token-Based Compaction Tests
// =============================================================================

describe("Token-Based Compaction", () => {
  test("compactByTokens keeps messages under budget", () => {
    const messages = createConversation(20, 200); // Each message ~50+ tokens
    const maxTokens = 500;
    const compacted = compactByTokens(messages, maxTokens);

    const estimate = estimateTokensForMessages(compacted);
    expect(estimate).toBeLessThanOrEqual(maxTokens);
  });

  test("compactByTokens keeps minimum messages", () => {
    const messages = createConversation(10, 1000); // Very long messages
    const compacted = compactByTokens(messages, 100, { minMessages: 4 });

    expect(compacted.length).toBeGreaterThanOrEqual(4);
  });

  test("compactByTokens preserves system messages", () => {
    const messages = createConversationWithSystem(10);
    const compacted = compactByTokens(messages, 1000, { preserveSystemMessages: true });

    expect(compacted[0].role).toBe("system");
  });

  test("compactByTokens keeps most recent messages", () => {
    const messages = createConversation(10);
    const compacted = compactByTokens(messages, 500);

    // Last message should always be kept
    const lastOriginal = messages[messages.length - 1];
    const lastCompacted = compacted[compacted.length - 1];
    expect(lastCompacted.content).toBe(lastOriginal.content);
  });
});

// =============================================================================
// Adaptive Compaction Tests
// =============================================================================

describe("Adaptive Compaction", () => {
  test("compactAdaptive applies turn limit first", () => {
    const messages = createConversation(100);
    const config: CompactionConfig = {
      strategy: "adaptive",
      maxTurns: 50,
    };
    const compacted = compactAdaptive(messages, config);

    const turns = groupIntoTurns(compacted);
    expect(turns.length).toBeLessThanOrEqual(50);
  });

  test("compactAdaptive applies token limit if still over", () => {
    const messages = createConversation(100, 500); // Long messages
    const config: CompactionConfig = {
      strategy: "adaptive",
      maxTurns: 50,
      maxTokens: 5000, // Will need further trimming
    };
    const compacted = compactAdaptive(messages, config);

    const estimate = estimateTokensForMessages(compacted);
    expect(estimate).toBeLessThanOrEqual(5000);
  });

  test("compactAdaptive preserves system messages", () => {
    const messages = createConversationWithSystem(100);
    const config: CompactionConfig = {
      strategy: "adaptive",
      maxTurns: 10,
      preserveSystemMessages: true,
    };
    const compacted = compactAdaptive(messages, config);

    expect(compacted[0].role).toBe("system");
  });
});

// =============================================================================
// Backpressure Detection Tests
// =============================================================================

describe("Backpressure Detection", () => {
  test("checkBackpressure returns correct status when under threshold", () => {
    const messages = createConversation(5);
    const status = checkBackpressure(messages, {
      ...DEFAULT_COMPACTION_CONFIG,
      contextWindow: 128000,
    });

    expect(status.thresholdExceeded).toBe(false);
    expect(status.action).toBe("none");
  });

  test("checkBackpressure returns warn when near threshold", () => {
    const messages = createConversation(100, 400); // ~10k tokens per turn
    const config: CompactionConfig = {
      ...DEFAULT_COMPACTION_CONFIG,
      contextWindow: 10000,
      backpressureThreshold: 0.5,
    };
    const status = checkBackpressure(messages, config);

    expect(status.thresholdExceeded).toBe(true);
  });

  test("checkBackpressure returns compact when critical", () => {
    const messages = createConversation(100, 400);
    const config: CompactionConfig = {
      ...DEFAULT_COMPACTION_CONFIG,
      maxTokens: 1000, // Very small budget
    };
    const status = checkBackpressure(messages, config);

    expect(status.usagePercent).toBeGreaterThan(0.95);
    expect(status.action).toBe("compact");
  });

  test("checkBackpressure calculates percentage correctly", () => {
    const messages = createConversation(10); // ~80-100 tokens per message
    const config: CompactionConfig = {
      ...DEFAULT_COMPACTION_CONFIG,
      contextWindow: 10000,
    };
    const status = checkBackpressure(messages, config);

    expect(status.usagePercent).toBeGreaterThan(0);
    expect(status.usagePercent).toBeLessThan(1);
    expect(status.currentTokens).toBe(estimateTokensForMessages(messages));
  });
});

// =============================================================================
// Main API Tests
// =============================================================================

describe("compactMessages (Main API)", () => {
  test("compactMessages returns original when no compaction needed", () => {
    const messages = createConversation(5);
    const result = compactMessages(messages, {
      maxTurns: 50,
      contextWindow: 128000,
    });

    expect(result.wasCompacted).toBe(false);
    expect(result.compactedCount).toBe(result.originalCount);
  });

  test("compactMessages performs turn-based compaction", () => {
    const messages = createConversation(100);
    const result = compactMessages(messages, {
      strategy: "turns",
      maxTurns: 20,
    });

    expect(result.wasCompacted).toBe(true);
    expect(result.compactedCount).toBeLessThan(result.originalCount);
    expect(result.reason).toContain("Turn-based");
  });

  test("compactMessages performs token-based compaction", () => {
    const messages = createConversation(50, 200);
    const result = compactMessages(messages, {
      strategy: "tokens",
      maxTokens: 2000,
    });

    expect(result.wasCompacted).toBe(true);
    expect(result.compactedTokens).toBeLessThanOrEqual(2000);
    expect(result.reason).toContain("Token-based");
  });

  test("compactMessages performs adaptive compaction", () => {
    const messages = createConversation(100, 200);
    const result = compactMessages(messages, {
      strategy: "adaptive",
      maxTurns: 30,
      maxTokens: 5000,
    });

    expect(result.wasCompacted).toBe(true);
    expect(result.reason).toContain("Adaptive");
  });

  test("compactMessages includes stats in result", () => {
    const messages = createConversation(50);
    const result = compactMessages(messages, {
      strategy: "turns",
      maxTurns: 10,
    });

    expect(result.originalCount).toBe(100); // 50 turns * 2 messages
    expect(result.originalTokens).toBeGreaterThan(0);
    expect(result.compactedTokens).toBeGreaterThan(0);
    expect(result.compactedTokens).toBeLessThan(result.originalTokens);
  });
});

describe("needsCompaction", () => {
  test("returns false for small conversations", () => {
    const messages = createConversation(5);
    expect(needsCompaction(messages)).toBe(false);
  });

  test("returns true when over context limit", () => {
    const messages = createConversation(100, 400);
    expect(needsCompaction(messages, { maxTokens: 1000 })).toBe(true);
  });
});

describe("getContextSummary", () => {
  test("returns human-readable summary", () => {
    const messages = createConversation(10);
    const summary = getContextSummary(messages);

    expect(summary).toContain("Messages:");
    expect(summary).toContain("Turns:");
    expect(summary).toContain("Tokens:");
    expect(summary).toContain("Usage:");
    expect(summary).toContain("Status:");
  });
});

// =============================================================================
// Edge Cases
// =============================================================================

describe("Edge Cases", () => {
  test("handles empty message array", () => {
    const result = compactMessages([], { maxTurns: 50 });
    expect(result.messages).toEqual([]);
    expect(result.wasCompacted).toBe(false);
  });

  test("handles single message", () => {
    const messages = [createMessage("user", "Hello")];
    const result = compactMessages(messages, { maxTurns: 50 });
    expect(result.messages.length).toBe(1);
  });

  test("handles system-only messages", () => {
    const messages = [createMessage("system", "System prompt")];
    const result = compactMessages(messages, { maxTurns: 50 });
    expect(result.messages.length).toBe(1);
    expect(result.messages[0].role).toBe("system");
  });

  test("handles very long single message", () => {
    const longContent = "x".repeat(100000);
    const messages = [createMessage("user", longContent)];
    const result = compactMessages(messages, { maxTokens: 1000 });

    // Should still include the message (can't compact below minMessages)
    expect(result.messages.length).toBeGreaterThanOrEqual(1);
  });

  test("handles content block arrays", () => {
    const messages: ChatMessage[] = [
      createBlockMessage("user", [
        { type: "text", text: "Hello" },
        { type: "text", text: "World" },
      ]),
      createBlockMessage("assistant", [
        { type: "thinking", thinking: "Let me think..." },
        { type: "text", text: "Hi there!" },
      ]),
    ];

    const estimate = estimateTokensForMessages(messages);
    expect(estimate).toBeGreaterThan(0);

    const result = compactMessages(messages, { maxTurns: 50 });
    expect(result.messages.length).toBe(2);
  });
});

// =============================================================================
// Integration Tests (pi-mono behavior)
// =============================================================================

describe("pi-mono Compatibility", () => {
  test("default config matches pi-mono (50 turns)", () => {
    expect(DEFAULT_COMPACTION_CONFIG.maxTurns).toBe(50);
  });

  test("preserves last 50 turns like pi-mono", () => {
    const messages = createConversation(100);
    const result = compactMessages(messages, {
      strategy: "turns",
      maxTurns: 50,
    });

    const turns = groupIntoTurns(result.messages);
    expect(turns.length).toBe(50);

    // Verify we kept the LAST 50 turns, not the first 50
    const lastUserMsg = turns[turns.length - 1].userMessage;
    expect(lastUserMsg.content).toContain("User message 99");
  });

  test("system prompt is always first after compaction", () => {
    const messages = createConversationWithSystem(100, "You are a helpful assistant.");
    const result = compactMessages(messages, {
      strategy: "turns",
      maxTurns: 10,
    });

    expect(result.messages[0].role).toBe("system");
    expect(result.messages[0].content).toBe("You are a helpful assistant.");
  });
});
