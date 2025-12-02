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

const ReadParametersSchema = S.Struct({
  path: S.String.pipe(
    S.minLength(1),
    S.annotations({ description: "Path to the file to read (relative or absolute)" }),
  ),
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
    "Read the contents of a file. Supports text files and images (jpg, png, gif, webp). For text, defaults to the first 2000 lines; use offset/limit to paginate.",
  schema: ReadParametersSchema,
  execute: (params, _options) =>
    Effect.gen(function* (_) {
      const fs = yield* _(FileSystem.FileSystem);
      const pathService = yield* _(Path.Path);

      const absolutePath = pathService.resolve(expandUserPath(params.path, pathService));
      const mimeType = isImage(absolutePath);

      const exists = yield* _(fs.exists(absolutePath));
      if (!exists) {
        return yield* _(Effect.fail(new ToolExecutionError("not_found", `File not found: ${params.path}`)));
      }

      if (mimeType) {
        const data = yield* _(fs.readFile(absolutePath));
        const base64 = Buffer.from(data).toString("base64");

        return {
          content: [
            { type: "text" as const, text: `Read image file [${mimeType}]` },
            { type: "image" as const, data: base64, mimeType },
          ],
        };
      }

      const textContent = yield* _(fs.readFileString(absolutePath));
      const lines = textContent.split("\n");

      const startLine = params.offset ? params.offset - 1 : 0;
      const limit = params.limit ?? MAX_LINES;
      const endLine = Math.min(startLine + limit, lines.length);

      if (startLine >= lines.length) {
        return yield* _(
          Effect.fail(
            new ToolExecutionError(
              "invalid_arguments",
              `Offset ${params.offset} is beyond end of file (${lines.length} lines total)`,
            ),
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
