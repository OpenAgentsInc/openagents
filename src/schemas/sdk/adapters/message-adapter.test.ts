import { describe, test, expect } from "bun:test";
import {
  internalContentToSdk,
  sdkContentToInternal,
  internalContentToToolContent,
  toolCallToToolUseBlock,
  toolUseBlockToToolCall,
  internalMessageToSdkChat,
  sdkChatToInternalMessage,
  createToolResultBlock,
  createInternalToolResult,
  extractToolCalls,
} from "./message-adapter.js";
import type { ContentBlock as InternalContentBlock, ChatToolCall } from "../../../llm/openrouter.js";
import type { ContentBlock as SDKContentBlock, ToolUseBlock } from "../messages.js";

describe("message-adapter", () => {
  describe("content block conversion", () => {
    test("converts text content internal → SDK", () => {
      const internal: InternalContentBlock = { type: "text", text: "hello" };
      const sdk = internalContentToSdk(internal);
      expect(sdk).toEqual({ type: "text", text: "hello" });
    });

    test("converts image content internal → SDK", () => {
      const internal: InternalContentBlock = {
        type: "image",
        data: "base64data",
        mimeType: "image/png",
      };
      const sdk = internalContentToSdk(internal);
      expect(sdk).toEqual({
        type: "image",
        source: {
          type: "base64",
          media_type: "image/png",
          data: "base64data",
        },
      });
    });

    test("converts text content SDK → internal", () => {
      const sdk: SDKContentBlock = { type: "text", text: "hello" };
      const internal = sdkContentToInternal(sdk);
      expect(internal).toEqual({ type: "text", text: "hello" });
    });

    test("converts image content SDK → internal", () => {
      const sdk: SDKContentBlock = {
        type: "image",
        source: {
          type: "base64",
          media_type: "image/jpeg",
          data: "base64data",
        },
      };
      const internal = sdkContentToInternal(sdk);
      expect(internal).toEqual({
        type: "image",
        data: "base64data",
        mimeType: "image/jpeg",
      });
    });

    test("converts thinking block to text", () => {
      const sdk: SDKContentBlock = { type: "thinking", thinking: "reasoning..." };
      const internal = sdkContentToInternal(sdk);
      expect(internal).toEqual({ type: "text", text: "[thinking] reasoning..." });
    });
  });

  describe("tool content conversion", () => {
    test("converts text to ToolContent", () => {
      const internal: InternalContentBlock = { type: "text", text: "result" };
      const toolContent = internalContentToToolContent(internal);
      expect(toolContent).toEqual({ type: "text", text: "result" });
    });

    test("converts image to ToolContent", () => {
      const internal: InternalContentBlock = {
        type: "image",
        data: "imgdata",
        mimeType: "image/png",
      };
      const toolContent = internalContentToToolContent(internal);
      expect(toolContent).toEqual({
        type: "image",
        data: "imgdata",
        mimeType: "image/png",
      });
    });
  });

  describe("tool call conversion", () => {
    test("converts ChatToolCall → ToolUseBlock", () => {
      const toolCall: ChatToolCall = {
        id: "call_123",
        name: "read_file",
        arguments: '{"path":"/tmp/file.txt"}',
      };
      const block = toolCallToToolUseBlock(toolCall);
      expect(block).toEqual({
        type: "tool_use",
        id: "call_123",
        name: "read_file",
        input: { path: "/tmp/file.txt" },
      });
    });

    test("converts ToolUseBlock → ChatToolCall", () => {
      const block: ToolUseBlock = {
        type: "tool_use",
        id: "call_456",
        name: "write_file",
        input: { path: "/tmp/out.txt", content: "hello" },
      };
      const toolCall = toolUseBlockToToolCall(block);
      expect(toolCall).toEqual({
        id: "call_456",
        name: "write_file",
        arguments: '{"path":"/tmp/out.txt","content":"hello"}',
      });
    });

    test("extracts tool calls from content blocks", () => {
      const blocks: SDKContentBlock[] = [
        { type: "text", text: "Let me help" },
        { type: "tool_use", id: "t1", name: "read", input: {} },
        { type: "tool_use", id: "t2", name: "write", input: { x: 1 } },
      ];
      const calls = extractToolCalls(blocks);
      expect(calls).toHaveLength(2);
      expect(calls[0].id).toBe("t1");
      expect(calls[1].id).toBe("t2");
    });
  });

  describe("message conversion", () => {
    test("converts internal user message → SDK chat message", () => {
      const internal = { role: "user" as const, content: "Hello" };
      const sdk = internalMessageToSdkChat(internal);
      expect(sdk).toEqual({ role: "user", content: "Hello" });
    });

    test("converts internal assistant message → SDK chat message", () => {
      const internal = { role: "assistant" as const, content: "Hi there" };
      const sdk = internalMessageToSdkChat(internal);
      expect(sdk).toEqual({ role: "assistant", content: "Hi there" });
    });

    test("converts tool role to user for SDK", () => {
      const internal = {
        role: "tool" as const,
        content: "result",
        tool_call_id: "call_123",
      };
      const sdk = internalMessageToSdkChat(internal);
      expect(sdk.role).toBe("user");
      expect(sdk.tool_call_id).toBe("call_123");
    });

    test("converts SDK chat message → internal", () => {
      const sdk = { role: "user" as const, content: "Test" };
      const internal = sdkChatToInternalMessage(sdk);
      expect(internal).toEqual({ role: "user", content: "Test" });
    });

    test("preserves optional fields", () => {
      const sdk = {
        role: "user" as const,
        content: "Test",
        name: "user1",
        tool_call_id: "tc1",
      };
      const internal = sdkChatToInternalMessage(sdk);
      expect(internal.name).toBe("user1");
      expect(internal.tool_call_id).toBe("tc1");
    });

    test("roundtrips multi-block content arrays", () => {
      const internal = {
        role: "user" as const,
        content: [
          { type: "text", text: "hi" },
          { type: "image", data: "img", mimeType: "image/png" },
        ] satisfies InternalContentBlock[],
      };

      const sdk = internalMessageToSdkChat(internal);
      expect(Array.isArray(sdk.content)).toBe(true);
      const roundtrip = sdkChatToInternalMessage(sdk);
      expect(roundtrip.content).toEqual([
        { type: "text", text: "hi" },
        { type: "image", data: "img", mimeType: "image/png" },
      ]);
    });
  });

  describe("tool result helpers", () => {
    test("creates tool result block from string", () => {
      const block = createToolResultBlock("call_123", "Success!");
      expect(block.type).toBe("tool_result");
      expect(block.tool_use_id).toBe("call_123");
      expect(block.content).toEqual([{ type: "text", text: "Success!" }]);
      expect(block.is_error).toBe(false);
    });

    test("creates tool result block with error flag", () => {
      const block = createToolResultBlock("call_123", "Error occurred", true);
      expect(block.is_error).toBe(true);
    });

    test("creates internal tool result message", () => {
      const msg = createInternalToolResult("call_123", "output", "read_file");
      expect(msg.role).toBe("tool");
      expect(msg.content).toBe("output");
      expect(msg.tool_call_id).toBe("call_123");
      expect(msg.name).toBe("read_file");
    });

    test("creates internal tool result without name", () => {
      const msg = createInternalToolResult("call_123", "output");
      expect(msg.role).toBe("tool");
      expect(msg.name).toBeUndefined();
    });
  });
});
