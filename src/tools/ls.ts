import * as FileSystem from "@effect/platform/FileSystem";
import * as Path from "@effect/platform/Path";
import { Effect } from "effect";
import * as S from "effect/Schema";
import type { Tool } from "./schema.js";
import { ToolExecutionError } from "./schema.js";

interface LsDetails {
  root: string;
  resolvedRoot: string;
  recursive: boolean;
  includeHidden: boolean;
  maxResults?: number;
  entries: number;
  directoriesVisited: number;
  filesVisited: number;
  truncated: boolean;
  durationMs: number;
}

const LsParametersSchema = S.Struct({
  path: S.optional(S.String),
  file_path: S.optional(
    S.String.pipe(
      S.annotations({ description: "SDK-style alias for path" }),
    ),
  ),
  recursive: S.optional(S.Boolean),
  includeHidden: S.optional(S.Boolean),
  maxResults: S.optional(S.Number.pipe(S.int(), S.greaterThan(0))),
});

type LsParameters = S.Schema.Type<typeof LsParametersSchema>;

export const lsTool: Tool<
  LsParameters,
  LsDetails,
  FileSystem.FileSystem | Path.Path
> = {
  name: "ls",
  label: "ls",
  description:
    "List files and directories under a path. Supports optional recursion, hidden files toggle, and max results.",
  schema: LsParametersSchema,
  execute: (params) =>
    Effect.gen(function* () {
      const fs = yield* FileSystem.FileSystem;
      const pathService = yield* Path.Path;

      const inputPath = params.file_path ?? params.path ?? ".";
      const root = pathService.resolve(inputPath);
      const exists = yield* fs.exists(root).pipe(
        Effect.mapError(
          (e) => new ToolExecutionError("command_failed", `Failed to check path: ${e.message}`),
        ),
      );
      if (!exists) {
        return yield* Effect.fail(
          new ToolExecutionError("not_found", `Path not found: ${inputPath}`),
        );
      }

      const recursive = params.recursive ?? false;
      const includeHidden = params.includeHidden ?? false;
      const maxResults = params.maxResults;

      const startedAt = Date.now();
      let directoriesVisited = 0;
      let filesVisited = 0;
      const results: string[] = [];
      const stack = [root];

      while (stack.length > 0) {
        const current = stack.pop()!;
        const entries = yield* fs.readDirectory(current).pipe(
          Effect.mapError(
            (e) => new ToolExecutionError("command_failed", `Failed to read directory: ${e.message}`),
          ),
        );

        for (const entry of entries) {
          if (maxResults && results.length >= maxResults) break;

          const fullPath = pathService.join(current, entry);
          const name = pathService.basename(fullPath);
          if (!includeHidden && name.startsWith(".")) continue;

          const relative = pathService.relative(root, fullPath) || ".";
          const info = yield* fs.stat(fullPath).pipe(
            Effect.mapError(
              (e) => new ToolExecutionError("command_failed", `Failed to stat entry: ${e.message}`),
            ),
          );

          if (info.type === "Directory") {
            directoriesVisited++;
            results.push(`${relative}/`);
          } else {
            filesVisited++;
            results.push(relative);
          }

          if (recursive && info.type === "Directory") {
            stack.push(fullPath);
          }
        }

        if (maxResults && results.length >= maxResults) break;
      }

      return {
        content: [
          {
            type: "text" as const,
            text: results.length === 0 ? "No entries found." : results.sort().join("\n"),
          },
        ],
        details: {
          root: inputPath,
          resolvedRoot: root,
          recursive,
          includeHidden,
          ...(maxResults !== undefined ? { maxResults } : {}),
          entries: results.length,
          directoriesVisited,
          filesVisited,
          truncated: maxResults ? results.length >= maxResults : false,
          durationMs: Date.now() - startedAt,
        },
      };
    }),
};
