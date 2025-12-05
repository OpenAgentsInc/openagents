import * as FileSystem from "@effect/platform/FileSystem";
import * as Path from "@effect/platform/Path";
import { Effect } from "effect";
import * as S from "effect/Schema";
import type { Tool } from "./schema.js";
import { ToolExecutionError } from "./schema.js";

const MAX_LINES = 2000;
const MAX_LINE_LENGTH = 2000;

const IMAGE_MIME_TYPES: Record<string, string> = {
  ".jpg": "image/jpeg",
  ".jpeg": "image/jpeg",
  ".png": "image/png",
  ".gif": "image/gif",
  ".webp": "image/webp",
};

const pathField = S.String.pipe(
  S.minLength(1),
  S.annotations({ description: "Path to the file to read (relative or absolute)" }),
);

const baseReadFields = {
  offset: S.optional(
    S.Number.pipe(
      S.int(),
      S.greaterThanOrEqualTo(1),
      S.annotations({ description: "Line number to start reading from (1-indexed)" }),
    ),
  ),
  limit: S.optional(
    S.Number.pipe(
      S.int(),
      S.greaterThanOrEqualTo(1),
      S.annotations({ description: "Maximum number of lines to read" }),
    ),
  ),
};

const ReadParametersSchema = S.Struct({
  file_path: S.optional(pathField),
  // Backward-compat alias kept for internal callers
  path: S.optional(pathField),
  ...baseReadFields,
});

type ReadParameters = S.Schema.Type<typeof ReadParametersSchema>;

const expandUserPath = (path: string, pathService: Path.Path) => {
  const home = typeof Bun !== "undefined" ? Bun.env.HOME ?? Bun.env.USERPROFILE : undefined;

  if (home && (path === "~" || path.startsWith("~/"))) {
    const suffix = path === "~" ? "" : path.slice(2);
    return suffix ? pathService.join(home, suffix) : home;
  }

  return path;
};

const isImage = (filePath: string): string | undefined => {
  const ext = filePath.slice(filePath.lastIndexOf(".")).toLowerCase();
  return IMAGE_MIME_TYPES[ext];
};

export const readTool: Tool<
  ReadParameters,
  undefined,
  FileSystem.FileSystem | Path.Path
> = {
  name: "read",
  label: "read",
  description:
    "Read the contents of a file. Supports text files and images (jpg, png, gif, webp). Prefer `file_path` (SDK naming); `path` remains as a backward-compatible alias. For text, defaults to the first 2000 lines; use offset/limit to paginate.",
  schema: ReadParametersSchema,
  execute: (params, _options) =>
    Effect.gen(function* () {
      const fs = yield* FileSystem.FileSystem;
      const pathService = yield* Path.Path;

      const inputPath = params.file_path ?? params.path;
      if (!inputPath) {
        return yield* Effect.fail(
          new ToolExecutionError("invalid_arguments", "Either file_path or path is required"),
        );
      }

      const absolutePath = pathService.resolve(expandUserPath(inputPath, pathService));
      const mimeType = isImage(absolutePath);

      const exists = yield* fs.exists(absolutePath).pipe(
        Effect.mapError((e) => new ToolExecutionError("command_failed", String(e))),
      );
      if (!exists) {
        return yield* Effect.fail(new ToolExecutionError("not_found", `File not found: ${inputPath}`));
      }

      if (mimeType) {
        const data = yield* fs.readFile(absolutePath).pipe(
          Effect.mapError((e) => new ToolExecutionError("command_failed", `Failed to read file: ${e.message}`)),
        );
        const base64 = Buffer.from(data).toString("base64");

        return {
          content: [
            { type: "text" as const, text: `Read image file [${mimeType}]` },
            { type: "image" as const, data: base64, mimeType },
          ],
        };
      }

      const textContent = yield* fs.readFileString(absolutePath).pipe(
        Effect.mapError(
          (e) => new ToolExecutionError("command_failed", `Failed to read file: ${e.message}`),
        ),
      );
      const lines = textContent.split("\n");

      const startLine = params.offset ? params.offset - 1 : 0;
      const limit = params.limit ?? MAX_LINES;
      const endLine = Math.min(startLine + limit, lines.length);

      if (startLine >= lines.length) {
        return yield* Effect.fail(
          new ToolExecutionError(
            "invalid_arguments",
            `Offset ${params.offset} is beyond end of file (${lines.length} lines total)`,
          ),
        );
      }

      let hadTruncatedLines = false;
      const selectedLines = lines.slice(startLine, endLine).map((line) => {
        if (line.length > MAX_LINE_LENGTH) {
          hadTruncatedLines = true;
          return line.slice(0, MAX_LINE_LENGTH);
        }
        return line;
      });

      let outputText = selectedLines.join("\n");
      const notices: string[] = [];

      if (hadTruncatedLines) {
        notices.push(`Some lines were truncated to ${MAX_LINE_LENGTH} characters for display`);
      }

      if (endLine < lines.length) {
        const remaining = lines.length - endLine;
        notices.push(`${remaining} more lines not shown. Use offset=${endLine + 1} to continue reading`);
      }

      if (notices.length > 0) {
        outputText += `\n\n... (${notices.join(". ")})`;
      }

      return {
        content: [{ type: "text" as const, text: outputText }],
      };
    }),
};
