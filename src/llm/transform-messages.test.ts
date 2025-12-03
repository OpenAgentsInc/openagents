import { describe, test, expect } from "bun:test";
import {
  transformMessagesForProvider,
  transformMessagesForModel,
  getApiCapabilities,
  getModelCapabilities,
  thinkingBlockToXml,
  redactedThinkingToText,
  parseThinkingFromXml,
  transformContentBlock,
  mergeAdjacentTextBlocks,
  hasThinkingBlocks,
  hasXmlThinkingTags,
} from "./transform-messages.js";
import type { Api, Model } from "./model-types.js";
import type {
  ChatMessage as SDKChatMessage,
  ContentBlock as SDKContentBlock,
  ThinkingBlock,
  TextBlock,
} from "../schemas/sdk/messages.js";

describe("transform-messages", () => {
  describe("getApiCapabilities", () => {
    test("anthropic supports native thinking", () => {
      const caps = getApiCapabilities("anthropic-messages");
      expect(caps.supportsNativeThinking).toBe(true);
      expect(caps.supportsImages).toBe(true);
      expect(caps.supportsTools).toBe(true);
    });

    test("openai does not support native thinking", () => {
      const caps = getApiCapabilities("openai-completions");
      expect(caps.supportsNativeThinking).toBe(false);
      expect(caps.supportsImages).toBe(true);
    });

    test("google does not support native thinking", () => {
      const caps = getApiCapabilities("google-generative-ai");
      expect(caps.supportsNativeThinking).toBe(false);
      expect(caps.supportsImages).toBe(true);
    });
  });

  describe("getModelCapabilities", () => {
    test("derives capabilities from model", () => {
      const model: Model = {
        id: "claude-3",
        name: "Claude 3",
        api: "anthropic-messages",
        provider: "anthropic",
        baseUrl: "https://api.anthropic.com",
        reasoning: true,
        input: ["text", "image"],
        cost: { input: 0, output: 0, cacheRead: 0, cacheWrite: 0 },
        contextWindow: 200000,
        maxTokens: 4096,
      };
      const caps = getModelCapabilities(model);
      expect(caps.supportsNativeThinking).toBe(true);
      expect(caps.supportsImages).toBe(true);
    });

    test("respects model input capabilities", () => {
      const model: Model = {
        id: "gpt-4-turbo",
        name: "GPT-4 Turbo",
        api: "openai-completions",
        provider: "openai",
        baseUrl: "https://api.openai.com",
        reasoning: false,
        input: ["text"], // No image support
        cost: { input: 0, output: 0, cacheRead: 0, cacheWrite: 0 },
        contextWindow: 128000,
        maxTokens: 4096,
      };
      const caps = getModelCapabilities(model);
      expect(caps.supportsImages).toBe(false);
    });
  });

  describe("thinkingBlockToXml", () => {
    test("converts thinking block to XML", () => {
      const block: ThinkingBlock = {
        type: "thinking",
        thinking: "Let me analyze this step by step...",
      };
      const result = thinkingBlockToXml(block);
      expect(result.type).toBe("text");
      expect(result.text).toBe(
        "<thinking>\nLet me analyze this step by step...\n</thinking>"
      );
    });

    test("uses custom tag name", () => {
      const block: ThinkingBlock = { type: "thinking", thinking: "reasoning" };
      const result = thinkingBlockToXml(block, "reasoning");
      expect(result.text).toBe("<reasoning>\nreasoning\n</reasoning>");
    });
  });

  describe("redactedThinkingToText", () => {
    test("converts redacted thinking to placeholder", () => {
      const block = { type: "redacted_thinking" as const, data: "encrypted" };
      const result = redactedThinkingToText(block);
      expect(result).toEqual({ type: "text", text: "[redacted thinking]" });
    });
  });

  describe("parseThinkingFromXml", () => {
    test("extracts thinking from XML tags", () => {
      const block: TextBlock = {
        type: "text",
        text: "<thinking>\nMy reasoning here\n</thinking>",
      };
      const result = parseThinkingFromXml(block);
      expect(result).toHaveLength(1);
      expect(result[0].type).toBe("thinking");
      expect((result[0] as ThinkingBlock).thinking).toBe("My reasoning here");
    });

    test("handles text before and after thinking", () => {
      const block: TextBlock = {
        type: "text",
        text: "Before <thinking>\nthinking\n</thinking> after",
      };
      const result = parseThinkingFromXml(block);
      expect(result).toHaveLength(3);
      expect(result[0]).toEqual({ type: "text", text: "Before" });
      expect(result[1]).toEqual({ type: "thinking", thinking: "thinking" });
      expect(result[2]).toEqual({ type: "text", text: "after" });
    });

    test("handles multiple thinking blocks", () => {
      const block: TextBlock = {
        type: "text",
        text: "<thinking>\nfirst\n</thinking> middle <thinking>\nsecond\n</thinking>",
      };
      const result = parseThinkingFromXml(block);
      expect(result).toHaveLength(3);
      expect((result[0] as ThinkingBlock).thinking).toBe("first");
      expect((result[1] as TextBlock).text).toBe("middle");
      expect((result[2] as ThinkingBlock).thinking).toBe("second");
    });

    test("returns original block if no tags found", () => {
      const block: TextBlock = { type: "text", text: "No thinking here" };
      const result = parseThinkingFromXml(block);
      expect(result).toEqual([block]);
    });

    test("uses custom tag name", () => {
      const block: TextBlock = {
        type: "text",
        text: "<reasoning>\ncustom\n</reasoning>",
      };
      const result = parseThinkingFromXml(block, "reasoning");
      expect(result).toHaveLength(1);
      expect((result[0] as ThinkingBlock).thinking).toBe("custom");
    });
  });

  describe("transformContentBlock", () => {
    const noThinkingCaps = {
      supportsNativeThinking: false,
      supportsImages: true,
      supportsTools: true,
    };

    const thinkingCaps = {
      supportsNativeThinking: true,
      supportsImages: true,
      supportsTools: true,
    };

    test("converts thinking to XML for non-thinking provider", () => {
      const block: SDKContentBlock = {
        type: "thinking",
        thinking: "My reasoning",
      };
      const result = transformContentBlock(block, noThinkingCaps);
      expect(result).toHaveLength(1);
      expect(result[0].type).toBe("text");
      expect((result[0] as TextBlock).text).toContain("<thinking>");
    });

    test("preserves thinking for provider that supports it", () => {
      const block: SDKContentBlock = {
        type: "thinking",
        thinking: "My reasoning",
      };
      const result = transformContentBlock(block, thinkingCaps, {
        preserveNativeThinking: true,
      });
      expect(result).toEqual([block]);
    });

    test("converts redacted thinking to text", () => {
      const block: SDKContentBlock = {
        type: "redacted_thinking",
        data: "encrypted",
      };
      const result = transformContentBlock(block, noThinkingCaps);
      expect(result).toEqual([{ type: "text", text: "[redacted thinking]" }]);
    });

    test("excludes redacted thinking when option set", () => {
      const block: SDKContentBlock = {
        type: "redacted_thinking",
        data: "encrypted",
      };
      const result = transformContentBlock(block, noThinkingCaps, {
        includeRedactedThinking: false,
      });
      expect(result).toEqual([]);
    });

    test("passes through text blocks", () => {
      const block: SDKContentBlock = { type: "text", text: "Hello" };
      const result = transformContentBlock(block, noThinkingCaps);
      expect(result).toEqual([block]);
    });

    test("parses XML thinking tags when preserveNativeThinking is true", () => {
      const block: SDKContentBlock = {
        type: "text",
        text: "<thinking>\nparsed\n</thinking>",
      };
      const result = transformContentBlock(block, thinkingCaps, {
        preserveNativeThinking: true,
      });
      expect(result).toHaveLength(1);
      expect(result[0].type).toBe("thinking");
    });

    test("converts unsupported images to placeholder", () => {
      const block: SDKContentBlock = {
        type: "image",
        source: { type: "base64", media_type: "image/png", data: "..." },
      };
      const noImageCaps = { ...noThinkingCaps, supportsImages: false };
      const result = transformContentBlock(block, noImageCaps);
      expect(result).toHaveLength(1);
      expect(result[0].type).toBe("text");
      expect((result[0] as TextBlock).text).toContain("not supported");
    });

    test("passes through tool_use blocks", () => {
      const block: SDKContentBlock = {
        type: "tool_use",
        id: "t1",
        name: "read",
        input: {},
      };
      const result = transformContentBlock(block, noThinkingCaps);
      expect(result).toEqual([block]);
    });
  });

  describe("mergeAdjacentTextBlocks", () => {
    test("merges adjacent text blocks", () => {
      const blocks: SDKContentBlock[] = [
        { type: "text", text: "First" },
        { type: "text", text: "Second" },
        { type: "text", text: "Third" },
      ];
      const result = mergeAdjacentTextBlocks(blocks);
      expect(result).toHaveLength(1);
      expect((result[0] as TextBlock).text).toBe("First\n\nSecond\n\nThird");
    });

    test("preserves non-text blocks between text", () => {
      const blocks: SDKContentBlock[] = [
        { type: "text", text: "Before" },
        { type: "tool_use", id: "t1", name: "read", input: {} },
        { type: "text", text: "After" },
      ];
      const result = mergeAdjacentTextBlocks(blocks);
      expect(result).toHaveLength(3);
      expect(result[0].type).toBe("text");
      expect(result[1].type).toBe("tool_use");
      expect(result[2].type).toBe("text");
    });

    test("handles empty array", () => {
      expect(mergeAdjacentTextBlocks([])).toEqual([]);
    });
  });

  describe("transformMessagesForProvider", () => {
    test("transforms thinking blocks to XML for OpenAI", () => {
      const messages: SDKChatMessage[] = [
        {
          role: "assistant",
          content: [
            { type: "thinking", thinking: "Let me think..." },
            { type: "text", text: "Here's my answer" },
          ],
        },
      ];

      const result = transformMessagesForProvider(messages, "openai-completions");
      expect(result).toHaveLength(1);
      expect(typeof result[0].content).not.toBe("string");
      const content = result[0].content as SDKContentBlock[];
      // Text blocks should be merged
      expect(content).toHaveLength(1);
      expect(content[0].type).toBe("text");
      expect((content[0] as TextBlock).text).toContain("<thinking>");
      expect((content[0] as TextBlock).text).toContain("Here's my answer");
    });

    test("preserves thinking blocks for Anthropic with option", () => {
      const messages: SDKChatMessage[] = [
        {
          role: "assistant",
          content: [{ type: "thinking", thinking: "reasoning" }],
        },
      ];

      const result = transformMessagesForProvider(messages, "anthropic-messages", {
        preserveNativeThinking: true,
      });
      const content = result[0].content as SDKContentBlock[];
      expect(content[0].type).toBe("thinking");
    });

    test("handles string content unchanged", () => {
      const messages: SDKChatMessage[] = [{ role: "user", content: "Hello" }];
      const result = transformMessagesForProvider(messages, "openai-completions");
      expect(result[0].content).toBe("Hello");
    });

    test("transforms for Google Generative AI", () => {
      const messages: SDKChatMessage[] = [
        {
          role: "assistant",
          content: [{ type: "thinking", thinking: "reasoning" }],
        },
      ];

      const result = transformMessagesForProvider(
        messages,
        "google-generative-ai"
      );
      const content = result[0].content as SDKContentBlock[];
      expect(content[0].type).toBe("text");
      expect((content[0] as TextBlock).text).toContain("<thinking>");
    });
  });

  describe("transformMessagesForModel", () => {
    test("uses model capabilities for transformation", () => {
      const model: Model = {
        id: "gpt-4",
        name: "GPT-4",
        api: "openai-completions",
        provider: "openai",
        baseUrl: "https://api.openai.com",
        reasoning: false,
        input: ["text", "image"],
        cost: { input: 0, output: 0, cacheRead: 0, cacheWrite: 0 },
        contextWindow: 128000,
        maxTokens: 4096,
      };

      const messages: SDKChatMessage[] = [
        {
          role: "assistant",
          content: [{ type: "thinking", thinking: "reasoning" }],
        },
      ];

      const result = transformMessagesForModel(messages, model);
      const content = result[0].content as SDKContentBlock[];
      expect(content[0].type).toBe("text");
      expect((content[0] as TextBlock).text).toContain("<thinking>");
    });
  });

  describe("hasThinkingBlocks", () => {
    test("detects thinking blocks", () => {
      const messages: SDKChatMessage[] = [
        {
          role: "assistant",
          content: [{ type: "thinking", thinking: "..." }],
        },
      ];
      expect(hasThinkingBlocks(messages)).toBe(true);
    });

    test("detects redacted thinking blocks", () => {
      const messages: SDKChatMessage[] = [
        {
          role: "assistant",
          content: [{ type: "redacted_thinking", data: "..." }],
        },
      ];
      expect(hasThinkingBlocks(messages)).toBe(true);
    });

    test("returns false for no thinking", () => {
      const messages: SDKChatMessage[] = [
        { role: "user", content: "Hello" },
        {
          role: "assistant",
          content: [{ type: "text", text: "Hi" }],
        },
      ];
      expect(hasThinkingBlocks(messages)).toBe(false);
    });

    test("handles string content", () => {
      const messages: SDKChatMessage[] = [{ role: "user", content: "Hello" }];
      expect(hasThinkingBlocks(messages)).toBe(false);
    });
  });

  describe("hasXmlThinkingTags", () => {
    test("detects XML thinking tags in string content", () => {
      const messages: SDKChatMessage[] = [
        { role: "assistant", content: "<thinking>test</thinking>" },
      ];
      expect(hasXmlThinkingTags(messages)).toBe(true);
    });

    test("detects XML thinking tags in content blocks", () => {
      const messages: SDKChatMessage[] = [
        {
          role: "assistant",
          content: [{ type: "text", text: "<thinking>test</thinking>" }],
        },
      ];
      expect(hasXmlThinkingTags(messages)).toBe(true);
    });

    test("returns false when no tags present", () => {
      const messages: SDKChatMessage[] = [
        { role: "assistant", content: "No thinking here" },
      ];
      expect(hasXmlThinkingTags(messages)).toBe(false);
    });

    test("uses custom tag name", () => {
      const messages: SDKChatMessage[] = [
        { role: "assistant", content: "<reasoning>test</reasoning>" },
      ];
      expect(hasXmlThinkingTags(messages, "reasoning")).toBe(true);
      expect(hasXmlThinkingTags(messages, "thinking")).toBe(false);
    });
  });

  describe("cross-provider workflow scenarios", () => {
    test("escalate from cheap model to Claude: XML → native thinking", () => {
      // Simulate messages with XML thinking tags from a cheap model
      const cheapModelMessages: SDKChatMessage[] = [
        { role: "user", content: "Analyze this code" },
        {
          role: "assistant",
          content: [
            { type: "text", text: "<thinking>\nFirst analysis\n</thinking>\nResult: OK" },
          ],
        },
      ];

      // Transform for Claude with native thinking
      const claudeMessages = transformMessagesForProvider(
        cheapModelMessages,
        "anthropic-messages",
        { preserveNativeThinking: true }
      );

      const content = claudeMessages[1].content as SDKContentBlock[];
      // Should have parsed thinking block
      expect(content.some((b) => b.type === "thinking")).toBe(true);
      // Should have text result
      expect(content.some((b) => b.type === "text")).toBe(true);
    });

    test("de-escalate from Claude to GPT: native → XML thinking", () => {
      // Simulate messages with native thinking from Claude
      const claudeMessages: SDKChatMessage[] = [
        { role: "user", content: "Quick question" },
        {
          role: "assistant",
          content: [
            { type: "thinking", thinking: "Deep reasoning..." },
            { type: "text", text: "Answer: 42" },
          ],
        },
      ];

      // Transform for GPT
      const gptMessages = transformMessagesForProvider(
        claudeMessages,
        "openai-completions"
      );

      const content = gptMessages[1].content as SDKContentBlock[];
      // Should be merged text with XML tags
      expect(content).toHaveLength(1);
      expect(content[0].type).toBe("text");
      const text = (content[0] as TextBlock).text;
      expect(text).toContain("<thinking>");
      expect(text).toContain("Deep reasoning...");
      expect(text).toContain("Answer: 42");
    });

    test("round-trip preserves thinking content", () => {
      const original: SDKChatMessage[] = [
        {
          role: "assistant",
          content: [
            { type: "thinking", thinking: "Important reasoning" },
            { type: "text", text: "Final answer" },
          ],
        },
      ];

      // Claude → GPT (native → XML)
      const toGpt = transformMessagesForProvider(original, "openai-completions");

      // GPT → Claude (XML → native)
      const backToClaude = transformMessagesForProvider(toGpt, "anthropic-messages", {
        preserveNativeThinking: true,
      });

      const content = backToClaude[0].content as SDKContentBlock[];
      const thinking = content.find((b) => b.type === "thinking") as ThinkingBlock;
      expect(thinking).toBeDefined();
      expect(thinking.thinking).toBe("Important reasoning");
    });
  });
});
