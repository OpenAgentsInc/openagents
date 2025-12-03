import { describe, expect, it } from "bun:test";
import { bundleFiles, parseArgs } from "./parser.js";
import { writeFileSync } from "fs";
import { tmpdir } from "os";
import { join } from "path";

describe("parseArgs", () => {
  it("parses mode, provider, model, tools, and messages", () => {
    const parsed = parseArgs([
      "--mode",
      "json",
      "--provider",
      "openai",
      "--model",
      "gpt-5",
      "--tools",
      "read,write",
      "hello",
      "world",
    ]);
    expect(parsed.mode).toBe("json");
    expect(parsed.provider).toBe("openai");
    expect(parsed.model).toBe("gpt-5");
    expect(parsed.tools).toEqual(["read", "write"]);
    expect(parsed.messages).toEqual(["hello", "world"]);
  });

  it("collects @file args separately", () => {
    const parsed = parseArgs(["@a.txt", "@b.png", "msg"]);
    expect(parsed.files).toEqual(["a.txt", "b.png"]);
    expect(parsed.messages).toEqual(["msg"]);
  });
});

describe("bundleFiles", () => {
  it("bundles text and image files with metadata", () => {
    const dir = tmpdir();
    const textPath = join(dir, "test.txt");
    const imgPath = join(dir, "img.png");
    writeFileSync(textPath, "hello");
    writeFileSync(imgPath, Buffer.from([0, 1, 2]));

    const bundled = bundleFiles([textPath, imgPath]);
    const textFile = bundled.find((b) => b.path === textPath)!;
    expect(textFile.isImage).toBe(false);
    expect(textFile.content).toBe("hello");

    const imgFile = bundled.find((b) => b.path === imgPath)!;
    expect(imgFile.isImage).toBe(true);
    expect(imgFile.mimeType).toBe("image/png");
    expect(typeof imgFile.content).toBe("string");
  });
});
