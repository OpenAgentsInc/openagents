import * as FileSystem from "@effect/platform/FileSystem";
import * as Path from "@effect/platform/Path";
import { Effect } from "effect";
import * as S from "effect/Schema";
import type { Tool } from "./schema.js";
import { ToolExecutionError } from "./schema.js";

const FindParametersSchema = S.Struct({
  path: S.optional(S.String),
  pattern: S.optional(S.String), // substring match on filename
  maxResults: S.optional(S.Number.pipe(S.int(), S.greaterThan(0))),
  includeHidden: S.optional(S.Boolean),
});

type FindParameters = S.Schema.Type<typeof FindParametersSchema>;

const matches = (name: string, pattern?: string, includeHidden?: boolean) => {
  if (!includeHidden && name.startsWith(".")) return false;
  if (!pattern) return true;
  return name.toLowerCase().includes(pattern.toLowerCase());
};

export const findTool: Tool<
  FindParameters,
  undefined,
  FileSystem.FileSystem | Path.Path
> = {
  name: "find",
  label: "find",
  description:
    "Find files and directories under a path. Supports substring pattern matching, max results, and hidden file toggle.",
  schema: FindParametersSchema,
  execute: (params) =>
    Effect.gen(function* () {
      const fs = yield* FileSystem.FileSystem;
      const pathService = yield* Path.Path;

      const root = pathService.resolve(params.path ?? ".");
      const exists = yield* fs.exists(root).pipe(
        Effect.mapError(
          (e) => new ToolExecutionError("command_failed", `Failed to check path: ${e.message}`),
        ),
      );
      if (!exists) {
        return yield* Effect.fail(
          new ToolExecutionError("not_found", `Path not found: ${params.path ?? "."}`),
        );
      }

      const collected: string[] = [];
      const stack = [root];

      while (stack.length > 0) {
        const current = stack.pop()!;
        const entries = yield* fs.readDirectory(current).pipe(
          Effect.mapError(
            (e) => new ToolExecutionError("command_failed", `Failed during search: ${e.message}`),
          ),
        );

        for (const entry of entries) {
          if (params.maxResults && collected.length >= params.maxResults) {
            break;
          }

          const fullPath = pathService.join(current, entry);
          const relative = pathService.relative(root, fullPath) || ".";
          const name = pathService.basename(fullPath);

          const stat = yield* fs.stat(fullPath).pipe(
            Effect.mapError(
              (e) => new ToolExecutionError("command_failed", `Failed during search: ${e.message}`),
            ),
          );

          if (stat.type === "Directory") {
            stack.push(fullPath);
          }

          if (matches(name, params.pattern, params.includeHidden)) {
            collected.push(relative);
          }
        }

        if (params.maxResults && collected.length >= params.maxResults) {
          break;
        }
      }

      if (collected.length === 0) {
        return { content: [{ type: "text" as const, text: "No matches found." }] };
      }

      return {
        content: [{ type: "text" as const, text: collected.join("\n") }],
      };
    }),
};
