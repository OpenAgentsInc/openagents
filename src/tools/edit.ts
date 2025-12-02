import * as Diff from "diff";
import * as FileSystem from "@effect/platform/FileSystem";
import * as Path from "@effect/platform/Path";
import { Effect } from "effect";
import * as S from "effect/Schema";
import type { Tool, ToolResult } from "./schema.js";
import { ToolExecutionError } from "./schema.js";

const EditParametersSchema = S.Struct({
  path: S.String.pipe(
    S.minLength(1),
    S.annotations({ description: "Path to the file to edit (relative or absolute)" }),
  ),
  oldText: S.String.pipe(
    S.minLength(1),
    S.annotations({ description: "Exact text to find and replace (must match exactly)" }),
  ),
  newText: S.String.pipe(
    S.annotations({ description: "New text to replace the old text with" }),
  ),
});

type EditParameters = S.Schema.Type<typeof EditParametersSchema>;

const abortIf = (signal?: AbortSignal) =>
  signal?.aborted
    ? Effect.fail(new ToolExecutionError("aborted", "Operation aborted"))
    : Effect.void;

const expandUserPath = (path: string, pathService: Path.Path) => {
  const home = Bun.env.HOME ?? Bun.env.USERPROFILE;

  if (home && (path === "~" || path.startsWith("~/"))) {
    const suffix = path === "~" ? "" : path.slice(2);
    return suffix ? pathService.join(home, suffix) : home;
  }

  return path;
};

const generateDiffString = (oldContent: string, newContent: string, contextLines = 4) => {
  const oldLines = oldContent.split("\n");
  const newLines = newContent.split("\n");
  const maxLineNum = Math.max(oldLines.length, newLines.length);
  const lineNumWidth = String(maxLineNum).length;

  const output: string[] = [];

  let oldLineNum = 1;
  let newLineNum = 1;
  let lastWasChange = false;

  const pushLine = (prefix: string, lineNum: number, line: string) => {
    output.push(`${prefix}${String(lineNum).padStart(lineNumWidth, " ")} ${line}`);
  };

  const changes = Diff.diffLines(oldContent, newContent);

  for (let i = 0; i < changes.length; i++) {
    const part = changes[i]!;
    const raw = part.value.split("\n");

    if (raw[raw.length - 1] === "") {
      raw.pop();
    }

    if (part.added || part.removed) {
      for (const line of raw) {
        if (part.added) {
          pushLine("+", newLineNum, line);
          newLineNum++;
        } else {
          pushLine("-", oldLineNum, line);
          oldLineNum++;
        }
      }
      lastWasChange = true;
    } else {
      const nextPartIsChange = i < changes.length - 1 && (changes[i + 1]!.added || changes[i + 1]!.removed);

      if (lastWasChange || nextPartIsChange) {
        let linesToShow = raw;
        let skipStart = 0;
        let skipEnd = 0;

        if (!lastWasChange) {
          skipStart = Math.max(0, raw.length - contextLines);
          linesToShow = raw.slice(skipStart);
        }

        if (!nextPartIsChange && linesToShow.length > contextLines) {
          skipEnd = linesToShow.length - contextLines;
          linesToShow = linesToShow.slice(0, contextLines);
        }

        if (skipStart > 0) {
          output.push(` ${"".padStart(lineNumWidth, " ")} ...`);
          oldLineNum += skipStart;
          newLineNum += skipStart;
        }

        for (const line of linesToShow) {
          pushLine(" ", oldLineNum, line);
          oldLineNum++;
          newLineNum++;
        }

        if (skipEnd > 0) {
          output.push(` ${"".padStart(lineNumWidth, " ")} ...`);
          oldLineNum += skipEnd;
          newLineNum += skipEnd;
        }
      } else {
        oldLineNum += raw.length;
        newLineNum += raw.length;
      }

      lastWasChange = false;
    }
  }

  return output.join("\n");
};

export const editTool: Tool<EditParameters, { diff: string }, FileSystem.FileSystem | Path.Path> = {
  name: "edit",
  label: "edit",
  description:
    "Edit a file by replacing exact text. The oldText must match exactly (including whitespace). Use this for precise, surgical edits.",
  schema: EditParametersSchema,
  execute: (params, options) =>
    Effect.gen(function* (_) {
      const fs = yield* _(FileSystem.FileSystem);
      const pathService = yield* _(Path.Path);
      const signal = options?.signal;

      yield* _(abortIf(signal));

      const absolutePath = pathService.resolve(expandUserPath(params.path, pathService));

      yield* _(
        fs.access(absolutePath, { readable: true, writable: true }).pipe(
          Effect.mapError(
            () => new ToolExecutionError("not_found", `File not found or not writable: ${params.path}`),
          ),
        ),
      );

      yield* _(abortIf(signal));

      const content = yield* _(
        fs.readFileString(absolutePath).pipe(
          Effect.mapError(
            (error) => new ToolExecutionError("not_found", `Unable to read ${params.path}: ${error.message}`),
          ),
        ),
      );

      yield* _(abortIf(signal));

      if (!content.includes(params.oldText)) {
        return yield* _(
          Effect.fail(
            new ToolExecutionError(
              "missing_old_text",
              `Could not find the exact text in ${params.path}. The old text must match exactly including all whitespace and newlines.`,
            ),
          ),
        );
      }

      const occurrences = content.split(params.oldText).length - 1;

      if (occurrences !== 1) {
        return yield* _(
          Effect.fail(
            new ToolExecutionError(
              "not_unique",
              `Found ${occurrences} occurrences of the text in ${params.path}. The text must be unique. Please provide more context to make it unique.`,
            ),
          ),
        );
      }

      const index = content.indexOf(params.oldText);
      const newContent =
        content.substring(0, index) + params.newText + content.substring(index + params.oldText.length);

      if (content === newContent) {
        return yield* _(
          Effect.fail(
            new ToolExecutionError(
              "unchanged",
              `No changes made to ${params.path}. The replacement produced identical content.`,
            ),
          ),
        );
      }

      yield* _(
        fs.writeFileString(absolutePath, newContent).pipe(
          Effect.mapError(
            (error) => new ToolExecutionError("not_found", `Unable to write ${params.path}: ${error.message}`),
          ),
        ),
      );

      yield* _(abortIf(signal));

      const result: ToolResult<{ diff: string }> = {
        content: [
          {
            type: "text",
            text: `Successfully replaced text in ${params.path}. Changed ${params.oldText.length} characters to ${params.newText.length} characters.`,
          },
        ],
        details: { diff: generateDiffString(content, newContent) },
      };

      return result;
    }),
};
