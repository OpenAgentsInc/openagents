import * as Diff from "diff";
import * as FileSystem from "@effect/platform/FileSystem";
import * as Path from "@effect/platform/Path";
import { Effect } from "effect";
import * as S from "effect/Schema";
import type { Tool, ToolResult } from "./schema.js";
import { ToolExecutionError } from "./schema.js";

interface EditDetails {
  path: string;
  resolvedPath: string;
  diff: string;
  oldLength: number;
  newLength: number;
  delta: number;
  linesAdded: number;
  linesRemoved: number;
}

const pathField = S.String.pipe(
  S.minLength(1),
  S.annotations({ description: "Path to the file to edit (relative or absolute)" }),
);

const textField = S.String.pipe(
  S.minLength(1),
  S.annotations({ description: "Exact text to find and replace (must match exactly)" }),
);

const replacementField = S.String.pipe(
  S.annotations({ description: "New text to replace the old text with" }),
);

const EditParametersSchema = S.Struct({
  path: S.optional(pathField),
  file_path: S.optional(pathField),
  oldText: S.optional(textField),
  old_string: S.optional(textField),
  newText: S.optional(replacementField),
  new_string: S.optional(replacementField),
  replace_all: S.optional(S.Boolean),
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

const generateDiffString = (
  oldContent: string,
  newContent: string,
  contextLines = 4,
  diffChanges?: Diff.Change[],
) => {
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

  const changes = diffChanges ?? Diff.diffLines(oldContent, newContent);

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

const summarizeChanges = (changes: Array<Diff.Change>) =>
  changes.reduce(
    (acc, part) => {
      const rows = part.value.split("\n");
      if (rows[rows.length - 1] === "") rows.pop();
      if (part.added) {
        acc.added += rows.length;
      } else if (part.removed) {
        acc.removed += rows.length;
      }
      return acc;
    },
    { added: 0, removed: 0 },
  );

export const editTool: Tool<EditParameters, EditDetails, FileSystem.FileSystem | Path.Path> = {
  name: "edit",
  label: "edit",
  description:
    "Edit a file by replacing exact text. The old text must match exactly (including whitespace). Use replace_all to update every occurrence when the match is not unique.",
  schema: EditParametersSchema,
  execute: (params, options) =>
    Effect.gen(function* () {
      const fs = yield* FileSystem.FileSystem;
      const pathService = yield* Path.Path;
      const signal = options?.signal;

      yield* abortIf(signal);

      const inputPath = params.path ?? params.file_path;
      const oldText = params.oldText ?? params.old_string;
      const newText = params.newText ?? params.new_string;

      if (!inputPath) {
        return yield* Effect.fail(
          new ToolExecutionError("invalid_arguments", "Either path or file_path is required"),
        );
      }

      if (!oldText || !newText) {
        return yield* Effect.fail(
          new ToolExecutionError("invalid_arguments", "old_text and new_text are required"),
        );
      }

      const absolutePath = pathService.resolve(expandUserPath(inputPath, pathService));

      yield* fs.access(absolutePath, { readable: true, writable: true }).pipe(
        Effect.mapError(
          () => new ToolExecutionError("not_found", `File not found or not writable: ${inputPath}`),
        ),
      );

      yield* abortIf(signal);

      const content = yield* fs.readFileString(absolutePath).pipe(
        Effect.mapError(
          (error) => new ToolExecutionError("not_found", `Unable to read ${inputPath}: ${error.message}`),
        ),
      );

      yield* abortIf(signal);

      if (!content.includes(oldText)) {
        return yield* Effect.fail(
          new ToolExecutionError(
            "missing_old_text",
            `Could not find the exact text in ${inputPath}. The old text must match exactly including all whitespace and newlines.`,
          ),
        );
      }

      const occurrences = content.split(oldText).length - 1;

      if (!params.replace_all && occurrences !== 1) {
        return yield* Effect.fail(
          new ToolExecutionError(
            "not_unique",
            `Found ${occurrences} occurrences of the text in ${inputPath}. The text must be unique. Use replace_all to replace all occurrences.`,
          ),
        );
      }

      const newContent = params.replace_all
        ? content.split(oldText).join(newText)
        : (() => {
            const index = content.indexOf(oldText);
            return content.substring(0, index) + newText + content.substring(index + oldText.length);
          })();

      if (content === newContent) {
        return yield* Effect.fail(
          new ToolExecutionError(
            "unchanged",
            `No changes made to ${inputPath}. The replacement produced identical content.`,
          ),
        );
      }

      yield* fs.writeFileString(absolutePath, newContent).pipe(
        Effect.mapError(
          (error) => new ToolExecutionError("not_found", `Unable to write ${inputPath}: ${error.message}`),
        ),
      );

      yield* abortIf(signal);

      const diffChanges = Diff.diffLines(content, newContent);
      const changeSummary = summarizeChanges(diffChanges);
      const diffString = generateDiffString(content, newContent, 4, diffChanges);

      const result: ToolResult<EditDetails> = {
        content: [
          {
            type: "text",
            text: `Successfully replaced text in ${inputPath}. Changed ${oldText.length} characters to ${newText.length} characters.`,
          },
        ],
        details: {
          path: inputPath,
          resolvedPath: absolutePath,
          diff: diffString,
          oldLength: content.length,
          newLength: newContent.length,
          delta: newContent.length - content.length,
          linesAdded: changeSummary.added,
          linesRemoved: changeSummary.removed,
        },
      };

      return result;
    }),
};
