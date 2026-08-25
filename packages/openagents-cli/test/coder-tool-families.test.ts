import { describe, expect, it } from "vitest";

import { declaredDescription, toolFamilyOf } from "../src/coder-tool-families.js";
import type { CoderTool } from "../src/coder-tools.js";

const tool = (name: string): CoderTool => ({
  name,
  description: "Base description.",
  parameters: { type: "object", properties: {} },
  run: async () => "",
});

describe("toolFamilyOf", () => {
  it("names the gemini family by model prefix", () => {
    expect(toolFamilyOf("gemini-3.7-flash")).toBe("gemini");
    expect(toolFamilyOf("gemini-2.0-flash")).toBe("gemini");
    expect(toolFamilyOf("gemini-3.5-flash")).toBe("gemini");
  });

  it("names the local family by the ollama model shape", () => {
    expect(toolFamilyOf("ollama:qwen3.8:27b-mtp-q8_0")).toBe("local");
  });

  it("defaults everything else, including absence", () => {
    expect(toolFamilyOf("gpt-5.6-luna")).toBe("default");
    expect(toolFamilyOf("ox-alpha")).toBe("default");
    expect(toolFamilyOf("claude-3-5-sonnet")).toBe("default");
    expect(toolFamilyOf(undefined)).toBe("default");
  });
});

describe("declaredDescription", () => {
  it("adds batching and ranged-read emphasis to shell for the gemini family", () => {
    const declared = declaredDescription(tool("shell"), "gemini");
    expect(declared.startsWith("Base description.")).toBe(true);
    expect(declared).toContain("batch independent commands into ONE call");
    expect(declared).toContain("offset/limit ranged reads");
  });

  it("adds the latency emphasis to shell for the local family", () => {
    expect(declaredDescription(tool("shell"), "local")).toContain("generates slowly");
  });

  it("leaves the default family and unlisted tools at the base", () => {
    expect(declaredDescription(tool("shell"), "default")).toBe("Base description.");
    expect(declaredDescription(tool("skill"), "gemini")).toBe("Base description.");
    expect(declaredDescription(tool("unknown_tool"), "gemini")).toBe("Base description.");
    expect(declaredDescription(tool("shell"), "gemini")).not.toBe("Base description.");
  });
});
