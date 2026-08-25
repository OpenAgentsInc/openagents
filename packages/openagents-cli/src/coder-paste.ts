/**
 * Types and utilities for compact paste placeholders in the coder composer.
 */

import {
  expandImageRefsForModel,
  formatImageRef,
  mimeTypeForImage,
  parseDroppedImagePaths,
  type PastedImageContent,
} from "./coder-image.js";
import * as path from "node:path";
import * as fs from "node:fs";

export const PASTE_TEXT_THRESHOLD = 800;
export const PASTE_MAX_LINES = 1;

export interface PastedTextContent {
  readonly id: number;
  readonly content: string;
}

/**
 * Return line count difference or line count for reference tokens.
 * "line1\nline2" has 1 additional line (+1 line).
 */
export function getPastedTextRefNumLines(text: string): number {
  return (text.match(/\r\n|\r|\n/g) ?? []).length;
}

/**
 * Format reference token: `[Pasted text #1]` or `[Pasted text #1 +10 lines]`.
 */
export function formatPastedTextRef(id: number, numLines: number): string {
  if (numLines <= 0) {
    return `[Pasted text #${id}]`;
  }
  return `[Pasted text #${id} +${numLines} ${numLines === 1 ? "line" : "lines"}]`;
}

/**
 * Regex matching reference tokens: `[Pasted text #1]`, `[Pasted text #1 +1 line]`, `[Pasted text #1 +10 lines]`.
 */
export const PASTED_TEXT_REF_REGEX = /\[Pasted text #(\d+)(?: \+\d+ lines?)?\]/g;

/**
 * Replace all `[Pasted text #N]` references with their actual content.
 */
export function expandPastedTextRefs(
  input: string,
  pastedContents: ReadonlyMap<number, PastedTextContent> | Record<number, PastedTextContent>,
): string {
  const isMap = pastedContents instanceof Map;
  const getEntry = (id: number): PastedTextContent | undefined => {
    if (isMap) {
      return (pastedContents as ReadonlyMap<number, PastedTextContent>).get(id);
    }
    return (pastedContents as Record<number, PastedTextContent>)[id];
  };

  const matches = [...input.matchAll(PASTED_TEXT_REF_REGEX)];
  let expanded = input;
  for (let i = matches.length - 1; i >= 0; i--) {
    const match = matches[i];
    if (match === undefined || match.index === undefined) continue;
    const id = parseInt(match[1] ?? "0", 10);
    const entry = getEntry(id);
    if (entry !== undefined) {
      expanded =
        expanded.slice(0, match.index) +
        entry.content +
        expanded.slice(match.index + match[0].length);
    }
  }
  return expanded;
}

/**
 * Replace both text and image references for model submission.
 */
export function expandComposerPrompt(
  input: string,
  pastedText: ReadonlyMap<number, PastedTextContent> | Record<number, PastedTextContent>,
  pastedImages: ReadonlyMap<number, PastedImageContent> | Record<number, PastedImageContent>,
): string {
  const withText = expandPastedTextRefs(input, pastedText);
  return expandImageRefsForModel(withText, pastedImages);
}

/**
 * Check if a pasted string qualifies for compact placeholder representation.
 */
export function shouldCollapsePaste(text: string, maxLines = PASTE_MAX_LINES): boolean {
  const numLines = getPastedTextRefNumLines(text);
  return text.length > PASTE_TEXT_THRESHOLD || numLines > maxLines;
}

/**
 * Process a pasted raw string chunk from bracketed paste or normal paste.
 * If the paste consists of dropped image file paths, creates image entries and returns token placeholders.
 */
export function handleIncomingPasteChunk(
  text: string,
  state: {
    nextTextId: number;
    nextImageId: number;
    pastedText: Map<number, PastedTextContent>;
    pastedImages: Map<number, PastedImageContent>;
  },
): string {
  const imagePaths = parseDroppedImagePaths(text);
  if (imagePaths.length > 0) {
    const tokens: string[] = [];
    for (const imgPath of imagePaths) {
      const id = state.nextImageId++;
      let sizeBytes = 0;
      try {
        sizeBytes = fs.statSync(imgPath).size;
      } catch {
        // ignore
      }
      const entry: PastedImageContent = {
        id,
        path: imgPath,
        filename: path.basename(imgPath),
        mime: mimeTypeForImage(imgPath),
        sizeBytes,
      };
      state.pastedImages.set(id, entry);
      tokens.push(formatImageRef(id));
    }
    return tokens.join(" ");
  }

  if (shouldCollapsePaste(text)) {
    const pasteId = state.nextTextId++;
    state.pastedText.set(pasteId, { id: pasteId, content: text });
    const numLines = getPastedTextRefNumLines(text);
    return formatPastedTextRef(pasteId, numLines);
  }

  return text;
}

/**
 * Remove trailing reference token (text or image) if the backspace lands on it.
 */
export function backspaceComposer(composer: string): string {
  const trailingPastedToken = /\[Pasted text #\d+(?: \+\d+ lines?)?\]$/.exec(composer);
  if (trailingPastedToken !== null) {
    return composer.slice(0, -trailingPastedToken[0].length);
  }
  const trailingImageToken = /\[Image #\d+\]$/.exec(composer);
  if (trailingImageToken !== null) {
    return composer.slice(0, -trailingImageToken[0].length);
  }
  return composer.slice(0, -1);
}
