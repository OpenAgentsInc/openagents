import * as fs from "node:fs";
import * as path from "node:path";
import * as os from "node:os";
import { describe, expect, it } from "vitest";
import {
  asImageFilePath,
  expandImageRefsForModel,
  formatImageRef,
  mimeTypeForImage,
  parseDroppedImagePaths,
  removeOuterQuotes,
  stripBackslashEscapes,
} from "../src/coder-image.js";
import { handleIncomingPasteChunk, expandComposerPrompt, backspaceComposer } from "../src/coder-paste.js";

describe("coder-image utilities", () => {
  it("removes outer quotes", () => {
    expect(removeOuterQuotes('"image.png"')).toBe("image.png");
    expect(removeOuterQuotes("'image.png'")).toBe("image.png");
    expect(removeOuterQuotes("image.png")).toBe("image.png");
  });

  it("strips shell escape backslashes on non-windows platforms", () => {
    if (process.platform !== "win32") {
      expect(stripBackslashEscapes("/path/to/my\\ screenshot\\ (1).png")).toBe(
        "/path/to/my screenshot (1).png",
      );
    }
  });

  it("recognizes valid image file paths", () => {
    expect(asImageFilePath("/path/to/test.PNG")).toBe("/path/to/test.PNG");
    expect(asImageFilePath("/path/to/test.jpeg")).toBe("/path/to/test.jpeg");
    expect(asImageFilePath("/path/to/test.webp")).toBe("/path/to/test.webp");
    expect(asImageFilePath("/path/to/test.txt")).toBe(null);
  });

  it("infers mime types", () => {
    expect(mimeTypeForImage("test.jpg")).toBe("image/jpeg");
    expect(mimeTypeForImage("test.jpeg")).toBe("image/jpeg");
    expect(mimeTypeForImage("test.png")).toBe("image/png");
    expect(mimeTypeForImage("test.webp")).toBe("image/webp");
    expect(mimeTypeForImage("test.gif")).toBe("image/gif");
  });

  it("parses single and multiple dropped image paths that exist on disk", () => {
    const tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), "openagents-img-test-"));
    const img1 = path.join(tmpDir, "screen1.png");
    const img2 = path.join(tmpDir, "screen2.jpg");
    fs.writeFileSync(img1, "fake png content");
    fs.writeFileSync(img2, "fake jpg content");

    try {
      const single = parseDroppedImagePaths(img1);
      expect(single).toEqual([img1]);

      const multiple = parseDroppedImagePaths(`${img1} ${img2}`);
      expect(multiple).toEqual([img1, img2]);

      const withQuotes = parseDroppedImagePaths(`"${img1}" "${img2}"`);
      expect(withQuotes).toEqual([img1, img2]);
    } finally {
      fs.rmSync(tmpDir, { recursive: true, force: true });
    }
  });

  it("handles incoming paste chunk for dropped images", () => {
    const tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), "openagents-img-test-"));
    const img = path.join(tmpDir, "diagram.png");
    fs.writeFileSync(img, "fake content");

    try {
      const state = {
        nextTextId: 1,
        nextImageId: 1,
        pastedText: new Map(),
        pastedImages: new Map(),
      };

      const result = handleIncomingPasteChunk(img, state);
      expect(result).toBe("[Image #1]");
      expect(state.pastedImages.has(1)).toBe(true);
      expect(state.pastedImages.get(1)?.filename).toBe("diagram.png");
    } finally {
      fs.rmSync(tmpDir, { recursive: true, force: true });
    }
  });

  it("expands image references and prompt text for model submission", () => {
    const tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), "openagents-img-test-"));
    const img = path.join(tmpDir, "chart.png");
    fs.writeFileSync(img, "fake chart");

    try {
      const pastedText = new Map([[1, { id: 1, content: "pasted multi\nline text" }]]);
      const pastedImages = new Map([
        [
          1,
          {
            id: 1,
            path: img,
            filename: "chart.png",
            mime: "image/png",
            sizeBytes: 10,
          },
        ],
      ]);

      const composer = "Please analyze [Image #1] and read [Pasted text #1 +1 line].";
      const expanded = expandComposerPrompt(composer, pastedText, pastedImages);

      expect(expanded).toBe(
        `Please analyze ![chart.png](${img}) and read pasted multi\nline text.`,
      );
    } finally {
      fs.rmSync(tmpDir, { recursive: true, force: true });
    }
  });

  it("removes [Image #N] atomically on backspace", () => {
    const composer = "Look at this [Image #1]";
    expect(backspaceComposer(composer)).toBe("Look at this ");
  });
});
