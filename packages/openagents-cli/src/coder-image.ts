import * as fs from "node:fs";
import * as path from "node:path";
import * as crypto from "node:crypto";

export const IMAGE_EXTENSION_REGEX = /\.(png|jpe?g|gif|webp)$/i;

export interface PastedImageContent {
  readonly id: number;
  readonly path: string;
  readonly filename: string;
  readonly mime: string;
  readonly sizeBytes: number;
  readonly base64?: string;
}

/**
 * Format image reference token: `[Image #1]`.
 */
export function formatImageRef(id: number): string {
  return `[Image #${id}]`;
}

export const IMAGE_REF_REGEX = /\[Image #(\d+)\]/g;

/**
 * Remove outer single or double quotes from a path.
 */
export function removeOuterQuotes(text: string): string {
  const trimmed = text.trim();
  if (
    (trimmed.startsWith('"') && trimmed.endsWith('"')) ||
    (trimmed.startsWith("'") && trimmed.endsWith("'"))
  ) {
    return trimmed.slice(1, -1);
  }
  return trimmed;
}

/**
 * Remove shell escape backslashes from a path.
 */
export function stripBackslashEscapes(filePath: string): string {
  if (process.platform === "win32") {
    return filePath;
  }
  const salt = crypto.randomBytes(8).toString("hex");
  const placeholder = `__DBL_BS_${salt}__`;
  const withPlaceholder = filePath.replace(/\\\\/g, placeholder);
  const withoutEscapes = withPlaceholder.replace(/\\(.)/g, "$1");
  return withoutEscapes.replace(new RegExp(placeholder, "g"), "\\");
}

/**
 * Normalize and test if text represents an image file path.
 */
export function asImageFilePath(text: string): string | null {
  const cleaned = removeOuterQuotes(text);
  const unescaped = stripBackslashEscapes(cleaned);
  if (IMAGE_EXTENSION_REGEX.test(unescaped)) {
    return unescaped;
  }
  return null;
}

/**
 * Split dropped text into path segments, handling quotes, spaces, and newlines.
 */
export function splitDroppedPaths(text: string): string[] {
  const trimmed = text.trim();
  if (trimmed.length === 0) return [];

  const tokens: string[] = [];
  let current = "";
  let inDoubleQuote = false;
  let inSingleQuote = false;
  let escaped = false;

  for (let i = 0; i < trimmed.length; i++) {
    const char = trimmed[i];

    if (escaped) {
      current += char;
      escaped = false;
      continue;
    }

    if (char === "\\") {
      escaped = true;
      current += char;
      continue;
    }

    if (char === '"' && !inSingleQuote) {
      inDoubleQuote = !inDoubleQuote;
      current += char;
      continue;
    }

    if (char === "'" && !inDoubleQuote) {
      inSingleQuote = !inSingleQuote;
      current += char;
      continue;
    }

    if ((char === " " || char === "\n" || char === "\r" || char === "\t") && !inDoubleQuote && !inSingleQuote) {
      if (current.trim().length > 0) {
        tokens.push(current.trim());
        current = "";
      }
      continue;
    }

    current += char;
  }

  if (current.trim().length > 0) {
    tokens.push(current.trim());
  }

  return tokens;
}

/**
 * Check whether a string consists of one or more dropped image file paths.
 * Returns array of parsed and existing file paths, or empty array if not images.
 */
export function parseDroppedImagePaths(text: string): string[] {
  const rawParts = splitDroppedPaths(text);
  if (rawParts.length === 0) return [];

  const parsedPaths: string[] = [];
  for (const part of rawParts) {
    const candidate = asImageFilePath(part);
    if (candidate !== null && fs.existsSync(candidate)) {
      try {
        const stat = fs.statSync(candidate);
        if (stat.isFile()) {
          parsedPaths.push(candidate);
        }
      } catch {
        // ignore invalid files
      }
    }
  }
  return parsedPaths;
}

/**
 * Detect MIME type from extension.
 */
export function mimeTypeForImage(filePath: string): string {
  const ext = path.extname(filePath).toLowerCase();
  switch (ext) {
    case ".jpg":
    case ".jpeg":
      return "image/jpeg";
    case ".png":
      return "image/png";
    case ".gif":
      return "image/gif";
    case ".webp":
      return "image/webp";
    default:
      return "application/octet-stream";
  }
}

/**
 * Replace `[Image #N]` references with clean file path references for model submission.
 * Avoids redundant repetition like `![filename](path)` by formatting as `[Image: path]`.
 */
export function expandImageRefsForModel(
  input: string,
  images: ReadonlyMap<number, PastedImageContent> | Record<number, PastedImageContent>,
): string {
  const isMap = images instanceof Map;
  const getEntry = (id: number): PastedImageContent | undefined => {
    if (isMap) {
      return (images as ReadonlyMap<number, PastedImageContent>).get(id);
    }
    return (images as Record<number, PastedImageContent>)[id];
  };

  const matches = [...input.matchAll(IMAGE_REF_REGEX)];
  let expanded = input;
  for (let i = matches.length - 1; i >= 0; i--) {
    const match = matches[i];
    if (match === undefined || match.index === undefined) continue;
    const id = parseInt(match[1] ?? "0", 10);
    const entry = getEntry(id);
    if (entry !== undefined) {
      const replacement = `[Image: ${entry.path}]`;
      expanded =
        expanded.slice(0, match.index) +
        replacement +
        expanded.slice(match.index + match[0].length);
    }
  }
  return expanded;
}
