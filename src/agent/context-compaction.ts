/**
 * Context Compaction & Backpressure for Long Sessions
 *
 * Provides intelligent context management to prevent token limit overruns
 * during long-running agent sessions. Based on pi-mono patterns.
 *
 * Strategies:
 * - turn-based: Keep last N conversation turns (pi-mono default: 50)
 * - token-based: Keep messages within a token budget
 * - adaptive: Combine both, prioritizing important messages
 *
 * @module
 */

import type { ChatMessage, ContentBlock } from "../schemas/sdk/messages.js";

// =============================================================================
// Types
// =============================================================================

/**
 * Compaction strategy options.
 */
export type CompactionStrategy = "turns" | "tokens" | "adaptive";

/**
 * Configuration for context compaction.
 */
export interface CompactionConfig {
  /** Strategy to use for compaction */
  strategy: CompactionStrategy;

  /** Maximum number of turns to keep (for turns/adaptive strategies) */
  maxTurns?: number;

  /** Maximum tokens to allow (for tokens/adaptive strategies) */
  maxTokens?: number;

  /**
   * Model's context window size.
   * Used to calculate backpressure thresholds.
   */
  contextWindow?: number;

  /**
   * Threshold (0-1) at which to trigger compaction.
   * Default: 0.8 (80% of context window)
   */
  backpressureThreshold?: number;

  /**
   * Whether to preserve system messages during compaction.
   * Default: true
   */
  preserveSystemMessages?: boolean;

  /**
   * Whether to preserve tool results (they provide important context).
   * Default: true for the most recent N tool results
   */
  preserveRecentToolResults?: number;

  /**
   * Minimum messages to always keep (prevents over-compaction).
   * Default: 4
   */
  minMessages?: number;
}

/**
 * Result of a compaction operation.
 */
export interface CompactionResult {
  /** Compacted messages */
  messages: ChatMessage[];

  /** Number of messages before compaction */
  originalCount: number;

  /** Number of messages after compaction */
  compactedCount: number;

  /** Estimated tokens before compaction */
  originalTokens: number;

  /** Estimated tokens after compaction */
  compactedTokens: number;

  /** Whether compaction was performed */
  wasCompacted: boolean;

  /** Reason for compaction (or why it wasn't needed) */
  reason: string;
}

/**
 * Backpressure status for monitoring.
 */
export interface BackpressureStatus {
  /** Current estimated tokens */
  currentTokens: number;

  /** Maximum allowed tokens */
  maxTokens: number;

  /** Usage percentage (0-1) */
  usagePercent: number;

  /** Whether backpressure threshold is exceeded */
  thresholdExceeded: boolean;

  /** Recommended action */
  action: "none" | "warn" | "compact";
}

/**
 * A conversation turn (user + assistant pair).
 */
export interface ConversationTurn {
  /** Index of the user message in the original array */
  userIndex: number;
  /** The user message */
  userMessage: ChatMessage;
  /** Index of the assistant message in the original array (if present) */
  assistantIndex?: number;
  /** The assistant message (if present) */
  assistantMessage?: ChatMessage;
  /** Tool result messages associated with this turn */
  toolResults: Array<{ index: number; message: ChatMessage }>;
  /** Estimated tokens for this turn */
  tokenEstimate: number;
}

// =============================================================================
// Token Estimation
// =============================================================================

/**
 * Estimate tokens for a string.
 * Uses a simple heuristic: ~4 characters per token for English text.
 * This is conservative and works across most models.
 */
export const estimateTokensForString = (text: string): number => {
  if (!text) return 0;
  // ~4 chars per token is a safe estimate for English
  // Actual varies by model (GPT: ~4, Claude: ~3.5, etc.)
  return Math.ceil(text.length / 4);
};

/**
 * Estimate tokens for a content block.
 */
export const estimateTokensForContentBlock = (block: ContentBlock): number => {
  switch (block.type) {
    case "text":
      return estimateTokensForString(block.text);
    case "thinking":
      // Thinking blocks are included in context
      return estimateTokensForString(block.thinking);
    case "redacted_thinking":
      // Redacted thinking is typically small
      return 10;
    case "tool_use":
      // Tool name + JSON input
      return (
        estimateTokensForString(block.name) +
        estimateTokensForString(JSON.stringify(block.input))
      );
    case "tool_result":
      // Tool results can be large
      if (!block.content) return 10;
      return block.content.reduce((sum, c) => {
        if (c.type === "text") return sum + estimateTokensForString(c.text);
        if (c.type === "image") return sum + 1000; // Images are expensive
        return sum + 10;
      }, 0);
    case "image":
      // Images are expensive (roughly 1000 tokens for medium images)
      return 1000;
    default:
      return 10;
  }
};

/**
 * Estimate tokens for a message.
 */
export const estimateTokensForMessage = (message: ChatMessage): number => {
  // Role overhead (~4 tokens for role + formatting)
  let tokens = 4;

  if (typeof message.content === "string") {
    tokens += estimateTokensForString(message.content);
  } else if (Array.isArray(message.content)) {
    tokens += message.content.reduce(
      (sum, block) => sum + estimateTokensForContentBlock(block),
      0
    );
  }

  return tokens;
};

/**
 * Estimate total tokens for a message array.
 */
export const estimateTokensForMessages = (messages: ChatMessage[]): number =>
  messages.reduce((sum, msg) => sum + estimateTokensForMessage(msg), 0);

// =============================================================================
// Turn-Based Compaction
// =============================================================================

/**
 * Group messages into conversation turns.
 * A turn is a user message followed by assistant response(s) and tool results.
 */
export const groupIntoTurns = (messages: ChatMessage[]): ConversationTurn[] => {
  const turns: ConversationTurn[] = [];
  let currentTurn: ConversationTurn | null = null;

  for (let i = 0; i < messages.length; i++) {
    const msg = messages[i];

    if (msg.role === "user") {
      // Start a new turn
      if (currentTurn) {
        turns.push(currentTurn);
      }
      currentTurn = {
        userIndex: i,
        userMessage: msg,
        toolResults: [],
        tokenEstimate: estimateTokensForMessage(msg),
      };
    } else if (msg.role === "assistant" && currentTurn) {
      currentTurn.assistantIndex = i;
      currentTurn.assistantMessage = msg;
      currentTurn.tokenEstimate += estimateTokensForMessage(msg);
    } else if (msg.role === "system") {
      // System messages are handled separately (usually preserved)
      // But if we encounter one mid-conversation, treat it as part of current turn
      if (currentTurn) {
        currentTurn.toolResults.push({ index: i, message: msg });
        currentTurn.tokenEstimate += estimateTokensForMessage(msg);
      }
    }
  }

  // Don't forget the last turn
  if (currentTurn) {
    turns.push(currentTurn);
  }

  return turns;
};

/**
 * Reconstruct messages from turns.
 */
export const turnsToMessages = (
  turns: ConversationTurn[],
  systemMessages: ChatMessage[] = []
): ChatMessage[] => {
  const messages: ChatMessage[] = [...systemMessages];

  for (const turn of turns) {
    messages.push(turn.userMessage);
    if (turn.assistantMessage) {
      messages.push(turn.assistantMessage);
    }
    for (const tr of turn.toolResults) {
      messages.push(tr.message);
    }
  }

  return messages;
};

/**
 * Compact messages by keeping only the last N turns.
 * System messages are always preserved at the beginning.
 */
export const compactByTurns = (
  messages: ChatMessage[],
  maxTurns: number,
  preserveSystemMessages = true
): ChatMessage[] => {
  // Extract system messages
  const systemMessages = preserveSystemMessages
    ? messages.filter((m) => m.role === "system")
    : [];
  const nonSystemMessages = preserveSystemMessages
    ? messages.filter((m) => m.role !== "system")
    : messages;

  // Group into turns
  const turns = groupIntoTurns(nonSystemMessages);

  // Keep only the last N turns
  const keptTurns = turns.slice(-maxTurns);

  // Reconstruct
  return turnsToMessages(keptTurns, systemMessages);
};

// =============================================================================
// Token-Based Compaction
// =============================================================================

/**
 * Compact messages by removing oldest messages until under token budget.
 * Preserves system messages and the most recent messages.
 */
export const compactByTokens = (
  messages: ChatMessage[],
  maxTokens: number,
  config: Partial<CompactionConfig> = {}
): ChatMessage[] => {
  const { preserveSystemMessages = true, minMessages = 4 } = config;

  // Extract system messages (always preserved)
  const systemMessages = preserveSystemMessages
    ? messages.filter((m) => m.role === "system")
    : [];
  const nonSystemMessages = preserveSystemMessages
    ? messages.filter((m) => m.role !== "system")
    : messages;

  // Calculate system message tokens
  const systemTokens = estimateTokensForMessages(systemMessages);
  const availableTokens = maxTokens - systemTokens;

  if (availableTokens <= 0) {
    // System messages alone exceed budget - return just the most essential
    return [systemMessages[0] || nonSystemMessages[nonSystemMessages.length - 1]].filter(Boolean);
  }

  // Work backwards from most recent, adding messages until we hit the budget
  const keptMessages: ChatMessage[] = [];
  let currentTokens = 0;

  for (let i = nonSystemMessages.length - 1; i >= 0; i--) {
    const msg = nonSystemMessages[i];
    const msgTokens = estimateTokensForMessage(msg);

    if (currentTokens + msgTokens <= availableTokens || keptMessages.length < minMessages) {
      keptMessages.unshift(msg);
      currentTokens += msgTokens;
    } else {
      break;
    }
  }

  return [...systemMessages, ...keptMessages];
};

// =============================================================================
// Adaptive Compaction
// =============================================================================

/**
 * Adaptive compaction: combines turn-based and token-based strategies.
 * First applies turn limit, then trims further if still over token budget.
 */
export const compactAdaptive = (
  messages: ChatMessage[],
  config: CompactionConfig
): ChatMessage[] => {
  const { maxTurns = 50, maxTokens, preserveSystemMessages = true } = config;

  // Step 1: Apply turn-based compaction
  let result = compactByTurns(messages, maxTurns, preserveSystemMessages);

  // Step 2: If we have a token budget and still exceed it, trim further
  if (maxTokens) {
    const currentTokens = estimateTokensForMessages(result);
    if (currentTokens > maxTokens) {
      result = compactByTokens(result, maxTokens, config);
    }
  }

  return result;
};

// =============================================================================
// Backpressure Detection
// =============================================================================

/**
 * Check backpressure status for a message array.
 */
export const checkBackpressure = (
  messages: ChatMessage[],
  config: CompactionConfig
): BackpressureStatus => {
  const { contextWindow = 128000, backpressureThreshold = 0.8, maxTokens } = config;

  // Use the smaller of maxTokens or contextWindow
  const effectiveMax = maxTokens ?? contextWindow;
  const currentTokens = estimateTokensForMessages(messages);
  const usagePercent = currentTokens / effectiveMax;
  const thresholdExceeded = usagePercent >= backpressureThreshold;

  let action: BackpressureStatus["action"] = "none";
  if (usagePercent >= 0.95) {
    action = "compact";
  } else if (thresholdExceeded) {
    action = "warn";
  }

  return {
    currentTokens,
    maxTokens: effectiveMax,
    usagePercent,
    thresholdExceeded,
    action,
  };
};

// =============================================================================
// Main API
// =============================================================================

/**
 * Default compaction configuration.
 * Based on pi-mono patterns (50 turns, 80% backpressure threshold).
 */
export const DEFAULT_COMPACTION_CONFIG: CompactionConfig = {
  strategy: "adaptive",
  maxTurns: 50,
  contextWindow: 128000,
  backpressureThreshold: 0.8,
  preserveSystemMessages: true,
  preserveRecentToolResults: 10,
  minMessages: 4,
};

/**
 * Compact messages using the specified strategy.
 *
 * @example
 * ```ts
 * // Turn-based compaction (pi-mono style)
 * const result = compactMessages(messages, {
 *   strategy: "turns",
 *   maxTurns: 50,
 * });
 *
 * // Token-based compaction
 * const result = compactMessages(messages, {
 *   strategy: "tokens",
 *   maxTokens: 100000,
 * });
 *
 * // Adaptive (recommended)
 * const result = compactMessages(messages, {
 *   strategy: "adaptive",
 *   maxTurns: 50,
 *   maxTokens: 100000,
 * });
 * ```
 */
export const compactMessages = (
  messages: ChatMessage[],
  config: Partial<CompactionConfig> = {}
): CompactionResult => {
  const fullConfig: CompactionConfig = { ...DEFAULT_COMPACTION_CONFIG, ...config };
  const { strategy, maxTurns = 50, maxTokens } = fullConfig;

  const originalCount = messages.length;
  const originalTokens = estimateTokensForMessages(messages);

  // Check if compaction is needed
  const backpressure = checkBackpressure(messages, fullConfig);
  if (backpressure.action === "none" && messages.length <= (maxTurns * 2 + 10)) {
    return {
      messages,
      originalCount,
      compactedCount: originalCount,
      originalTokens,
      compactedTokens: originalTokens,
      wasCompacted: false,
      reason: `No compaction needed (${backpressure.usagePercent.toFixed(1)}% of context)`,
    };
  }

  // Apply compaction strategy
  let compacted: ChatMessage[];
  let reason: string;

  switch (strategy) {
    case "turns":
      compacted = compactByTurns(messages, maxTurns, fullConfig.preserveSystemMessages);
      reason = `Turn-based compaction to ${maxTurns} turns`;
      break;

    case "tokens":
      if (!maxTokens) {
        return {
          messages,
          originalCount,
          compactedCount: originalCount,
          originalTokens,
          compactedTokens: originalTokens,
          wasCompacted: false,
          reason: "Token-based strategy requires maxTokens config",
        };
      }
      compacted = compactByTokens(messages, maxTokens, fullConfig);
      reason = `Token-based compaction to ${maxTokens} tokens`;
      break;

    case "adaptive":
    default:
      compacted = compactAdaptive(messages, fullConfig);
      reason = `Adaptive compaction (${maxTurns} turns, ${maxTokens ?? "unlimited"} tokens)`;
      break;
  }

  const compactedTokens = estimateTokensForMessages(compacted);

  return {
    messages: compacted,
    originalCount,
    compactedCount: compacted.length,
    originalTokens,
    compactedTokens,
    wasCompacted: compacted.length < originalCount,
    reason,
  };
};

/**
 * Check if messages need compaction based on backpressure.
 * Useful for monitoring without performing compaction.
 */
export const needsCompaction = (
  messages: ChatMessage[],
  config: Partial<CompactionConfig> = {}
): boolean => {
  const fullConfig: CompactionConfig = { ...DEFAULT_COMPACTION_CONFIG, ...config };
  const backpressure = checkBackpressure(messages, fullConfig);
  return backpressure.action === "compact";
};

/**
 * Get a summary string for logging/debugging.
 */
export const getContextSummary = (
  messages: ChatMessage[],
  config: Partial<CompactionConfig> = {}
): string => {
  const fullConfig: CompactionConfig = { ...DEFAULT_COMPACTION_CONFIG, ...config };
  const backpressure = checkBackpressure(messages, fullConfig);
  const turns = groupIntoTurns(messages.filter((m) => m.role !== "system"));

  return [
    `Messages: ${messages.length}`,
    `Turns: ${turns.length}`,
    `Tokens: ${backpressure.currentTokens}/${backpressure.maxTokens}`,
    `Usage: ${(backpressure.usagePercent * 100).toFixed(1)}%`,
    `Status: ${backpressure.action}`,
  ].join(" | ");
};
