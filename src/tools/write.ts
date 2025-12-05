import * as FileSystem from "@effect/platform/FileSystem";
import * as Path from "@effect/platform/Path";
import { Effect } from "effect";
import * as S from "effect/Schema";
import type { Tool } from "./schema.js";
import { ToolExecutionError } from "./schema.js";

const pathField = S.String.pipe(
  S.minLength(1),
  S.annotations({ description: "Path to the file to write (relative or absolute)" }),
);

const WriteParametersSchema = S.Struct({
  path: S.optional(pathField),
  file_path: S.optional(pathField),
  content: S.String.pipe(S.annotations({ description: "Content to write to the file" })),
});

type WriteParameters = S.Schema.Type<typeof WriteParametersSchema>;

const expandUserPath = (path: string, pathService: Path.Path) => {
  const home = typeof Bun !== "undefined" ? Bun.env.HOME ?? Bun.env.USERPROFILE : undefined;

  if (home && (path === "~" || path.startsWith("~/"))) {
    const suffix = path === "~" ? "" : path.slice(2);
    return suffix ? pathService.join(home, suffix) : home;
  }

  return path;
};

export const writeTool: Tool<WriteParameters, undefined, FileSystem.FileSystem | Path.Path> = {
  name: "write",
  label: "write",
  description:
    "Write content to a file. Creates the file if it doesn't exist, overwrites if it does. Automatically creates parent directories.",
  schema: WriteParametersSchema,
  execute: (params) =>
    Effect.gen(function* () {
      const fs = yield* FileSystem.FileSystem;
      const pathService = yield* Path.Path;

      const inputPath = params.path ?? params.file_path;
      if (!inputPath) {
        return yield* Effect.fail(
          new ToolExecutionError("invalid_arguments", "Either path or file_path is required"),
        );
      }

      const absolutePath = pathService.resolve(expandUserPath(inputPath, pathService));
      const dir = pathService.dirname(absolutePath);

      yield* fs.makeDirectory(dir, { recursive: true }).pipe(
        Effect.mapError(
          (error) => new ToolExecutionError("command_failed", `Failed to create directory: ${error.message}`),
        ),
      );

      yield* fs.writeFileString(absolutePath, params.content).pipe(
        Effect.mapError(
          (error) => new ToolExecutionError("command_failed", `Failed to write file: ${error.message}`),
        ),
      );

      return {
        content: [
          {
            type: "text",
            text: `Successfully wrote ${params.content.length} bytes to ${inputPath}`,
          },
        ],
      };
    }),
};
