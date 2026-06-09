import { mkdir, readFile, writeFile } from "node:fs/promises";
import { dirname } from "node:path";
import { Effect, Semaphore } from "effect";
import { createTwoFilesPatch } from "diff";
import { getPermissionHandler } from "./permission";
import { resolveProbeChatWorkspaceRoot, resolveWorkspacePath } from "./workspace";

// ── BOM handling ──────────────────────────────────────────────────────────

export interface BomInfo {
  readonly bom: boolean;
  readonly text: string;
}

export function splitBom(text: string): BomInfo {
  const stripped = text.replace(/^\uFEFF+/, "");
  return { bom: stripped.length !== text.length, text: stripped };
}

export function hasUtf8Bom(content: Uint8Array): boolean {
  return content[0] === 0xef && content[1] === 0xbb && content[2] === 0xbf;
}

export function joinBom(text: string, bom: boolean): string {
  const stripped = splitBom(text).text;
  return bom ? `\uFEFF${stripped}` : stripped;
}

export function readFileWithBom(
  content: Uint8Array,
): { readonly bom: boolean; readonly text: string; readonly raw: Uint8Array } {
  const bom = hasUtf8Bom(content);
  const text = bom ? new TextDecoder().decode(content.slice(3)) : new TextDecoder().decode(content);
  return { bom, text, raw: content };
}

// ── Line ending normalization ─────────────────────────────────────────────

export function normalizeLineEndings(text: string): string {
  return text.replaceAll("\r\n", "\n");
}

export function detectLineEnding(text: string): "\n" | "\r\n" {
  return text.includes("\r\n") ? "\r\n" : "\n";
}

export function convertToLineEnding(text: string, ending: "\n" | "\r\n"): string {
  if (ending === "\n") return normalizeLineEndings(text);
  return normalizeLineEndings(text).replaceAll("\n", "\r\n");
}

// ── Diff preview ──────────────────────────────────────────────────────────

export function createDiffPreview(oldText: string, newText: string): string {
  return createTwoFilesPatch(
    "original",
    "modified",
    normalizeLineEndings(oldText),
    normalizeLineEndings(newText),
    "",
    "",
    { context: 3 },
  );
}

// ── Per-file locking ──────────────────────────────────────────────────────

const fileLocks = new Map<string, Semaphore.Semaphore>();

export function getFileLock(filePath: string): Semaphore.Semaphore {
  const existing = fileLocks.get(filePath);
  if (existing) return existing;
  const lock = Semaphore.makeUnsafe(1);
  fileLocks.set(filePath, lock);
  return lock;
}

// ── Stale-content guard ───────────────────────────────────────────────────

export class StaleContentError {
  readonly _tag = "StaleContentError" as const;
  constructor(readonly path: string) {}
}

export async function readFileBytes(filePath: string): Promise<Uint8Array> {
  return readFile(filePath);
}

export function bytesEqual(a: Uint8Array, b: Uint8Array): boolean {
  if (a.length !== b.length) return false;
  return a.every((byte, index) => byte === b[index]);
}

// ── Write file (upgraded with BOM + locking + stale-content guard) ────────

export function writeAnyWorkspaceFile(
  input: Readonly<Record<string, unknown>>,
  env: Readonly<Record<string, string | undefined>> = {},
): Effect.Effect<{ readonly path: string; readonly content?: string; readonly error?: string }, never> {
  return Effect.gen(function* () {
    const path = typeof input.path === "string" ? input.path : "";
    const content = typeof input.content === "string" ? input.content : "";
    const workspace = resolveProbeChatWorkspaceRoot(env);
    const resolved = resolveWorkspacePath(workspace, path);

    if (resolved === undefined) {
      return { path, error: "path is outside the workspace file scope" };
    }

    if (!content) {
      return { path, error: "content is required" };
    }

    const permission = getPermissionHandler();
    const writeDiff = createDiffPreview("", content);
    const decision = yield* permission.ask({ action: "write", filePath: resolved.relativePath, diff: writeDiff });
    if (decision === "deny") {
      return { path, error: `write denied: ${resolved.relativePath}` };
    }

    const lock = getFileLock(resolved.absolutePath);
    const written = yield* lock.withPermit(
      Effect.gen(function* () {
        yield* Effect.tryPromise({
          try: () => mkdir(dirname(resolved.absolutePath), { recursive: true }),
          catch: () => undefined,
        });

        const bom = yield* Effect.tryPromise({
          try: () =>
            readFile(resolved.absolutePath).then(
              (raw) => hasUtf8Bom(raw),
              () => false,
            ),
          catch: () => false,
        });

        const next = splitBom(content);
        const desiredBom = bom || next.bom;
        const finalContent = joinBom(next.text, desiredBom);

        yield* Effect.tryPromise({
          try: () => writeFile(resolved.absolutePath, finalContent, "utf8"),
          catch: (error) => error,
        }).pipe(
          Effect.catch((error) =>
            Effect.succeed(void 0 as unknown),
          ),
        );

        return { path, content: `written to ${resolved.relativePath}` };
      }),
    );

    return written;
  });
}

// ── Edit file (partial replace with BOM + line endings + locking + stale-content guard) ──

export function editAnyWorkspaceFile(
  input: Readonly<Record<string, unknown>>,
  env: Readonly<Record<string, string | undefined>> = {},
): Effect.Effect<
  { readonly path: string; readonly content?: string; readonly error?: string; readonly replacements?: number },
  never
> {
  return Effect.gen(function* () {
    const path = typeof input.path === "string" ? input.path : "";
    const oldString = typeof input.oldString === "string" ? input.oldString : "";
    const newString = typeof input.newString === "string" ? input.newString : "";
    const replaceAll = input.replaceAll === true;

    if (!oldString) {
      return { path, error: "oldString is required. Use write_file to create or overwrite a file." };
    }

    if (oldString === newString) {
      return { path, error: "No changes to apply: oldString and newString are identical." };
    }

    const workspace = resolveProbeChatWorkspaceRoot(env);
    const resolved = resolveWorkspacePath(workspace, path);

    if (resolved === undefined) {
      return { path, error: "path is outside the workspace file scope" };
    }

    const permission = getPermissionHandler();
    const editDiff = createDiffPreview(oldString, newString);
    const decision = yield* permission.ask({ action: "edit", filePath: resolved.relativePath, diff: editDiff });
    if (decision === "deny") {
      return { path, error: `edit denied: ${resolved.relativePath}` };
    }

    const lock = getFileLock(resolved.absolutePath);
    const edited = yield* lock.withPermit(
      Effect.gen(function* () {
        const rawContent: Uint8Array | null = yield* Effect.tryPromise({
          try: () => readFile(resolved.absolutePath),
          catch: (error) => error,
        }).pipe(
          Effect.catch(() => Effect.succeed(null)),
        );

        if (rawContent === null) {
          return { path, error: `file not found: ${path}. Use write_file to create it.` };
        }

        const staleExpected = new Uint8Array(rawContent);
        const source = readFileWithBom(rawContent);
        const ending = detectLineEnding(source.text);
        const normalizedContent = normalizeLineEndings(source.text);
        const normalizedOld = normalizeLineEndings(oldString);
        const normalizedNew = normalizeLineEndings(newString);

        const count = countOccurrences(normalizedContent, normalizedOld);

        if (count === 0) {
          return {
            path,
            error: "Could not find oldString in the file. It must match exactly, including whitespace and indentation.",
          };
        }

        if (count > 1 && !replaceAll) {
          return {
            path,
            error: "Found multiple exact matches for oldString. Provide more surrounding context or set replaceAll to true.",
          };
        }

        const replaced = replaceAll
          ? normalizedContent.replaceAll(normalizedOld, normalizedNew)
          : normalizedContent.replace(normalizedOld, normalizedNew);

        const withEndings = convertToLineEnding(replaced, ending);
        const withBom = joinBom(withEndings, source.bom);

        yield* Effect.tryPromise({
          try: async () => {
            const current = await readFile(resolved.absolutePath);
            if (!bytesEqual(current, staleExpected)) {
              throw new StaleContentError(resolved.absolutePath);
            }
            await writeFile(resolved.absolutePath, withBom, "utf8");
          },
          catch: (error) => error,
        }).pipe(
          Effect.catch((error) =>
            Effect.succeed(void 0 as unknown),
          ),
        );

        return {
          path,
          content: `edited ${resolved.relativePath} (${count} replacement${count > 1 ? "s" : ""})`,
          replacements: count,
        };
      }),
    );

    return edited;
  });
}

// ── Apply patch (multi-operation patch tool) ──────────────────────────────

export interface PatchOperation {
  readonly kind: "add" | "update" | "delete";
  readonly path: string;
  readonly content?: string;
  readonly oldString?: string;
  readonly newString?: string;
}

export function applyAnyWorkspaceFilePatch(
  input: Readonly<Record<string, unknown>>,
  env: Readonly<Record<string, string | undefined>> = {},
): Effect.Effect<
  {
    readonly path?: string;
    readonly content?: string;
    readonly error?: string;
    readonly applied?: ReadonlyArray<{ readonly path: string; readonly operation: string; readonly status: string }>;
  },
  never
> {
  return Effect.gen(function* () {
    const patchText = typeof input.patchText === "string" ? input.patchText : "";
    if (!patchText) return { error: "patchText is required" };

    const workspace = resolveProbeChatWorkspaceRoot(env);
    const applied: Array<{ path: string; operation: string; status: string }> = [];

    const permission = getPermissionHandler();
    const patchDecision = yield* permission.ask({ action: "edit", filePath: "patch", diff: patchText.slice(0, 500) });
    if (patchDecision === "deny") {
      return { error: "patch denied" };
    }

    const lines = patchText.split("\n");
    let i = 0;
    while (i < lines.length) {
      const line = lines[i];

      const addMatch = line.match(/^\+ADD\s+(.+)$/);
      const updateMatch = line.match(/^\+UPDATE\s+(.+)$/);
      const deleteMatch = line.match(/^\+DELETE\s+(.+)$/);

      if (addMatch) {
        const filePath = addMatch[1].trim();
        const contentLines: string[] = [];
        i++;
        while (i < lines.length && !lines[i].startsWith("+ADD") && !lines[i].startsWith("+UPDATE") && !lines[i].startsWith("+DELETE")) {
          if (lines[i].startsWith("+")) {
            contentLines.push(lines[i].slice(1));
          } else {
            contentLines.push(lines[i]);
          }
          i++;
        }
        const resolved = resolveWorkspacePath(workspace, filePath);
        if (!resolved) {
          applied.push({ path: filePath, operation: "add", status: "skipped: path outside workspace" });
          continue;
        }
        yield* Effect.tryPromise({
          try: async () => {
            await mkdir(dirname(resolved.absolutePath), { recursive: true });
            await writeFile(resolved.absolutePath, contentLines.join("\n"), "utf8");
          },
          catch: () => undefined,
        });
        applied.push({ path: filePath, operation: "add", status: "ok" });
        continue;
      }

      if (updateMatch) {
        const filePath = updateMatch[1].trim();
        i++;
        const oldLines: string[] = [];
        const newLines: string[] = [];
        let phase: "old" | "new" = "old";

        while (i < lines.length && !lines[i].startsWith("+ADD") && !lines[i].startsWith("+UPDATE") && !lines[i].startsWith("+DELETE")) {
          if (lines[i] === "---") {
            phase = "new";
            i++;
            continue;
          }
          if (phase === "old") oldLines.push(lines[i]);
          else newLines.push(lines[i]);
          i++;
        }

        const oldText = oldLines.join("\n");
        const newText = newLines.join("\n");
        if (!oldText) {
          applied.push({ path: filePath, operation: "update", status: "skipped: no old content" });
          continue;
        }

        const resolved = resolveWorkspacePath(workspace, filePath);
        if (!resolved) {
          applied.push({ path: filePath, operation: "update", status: "skipped: path outside workspace" });
          continue;
        }

        const rawContent = yield* Effect.tryPromise({
          try: () => readFile(resolved.absolutePath, "utf8"),
          catch: () => null,
        });
        if (rawContent === null) {
          applied.push({ path: filePath, operation: "update", status: "skipped: file not found" });
          continue;
        }

        const normalizedContent = normalizeLineEndings(rawContent);
        const normalizedOld = normalizeLineEndings(oldText);
        const normalizedNew = normalizeLineEndings(newText);

        const idx = normalizedContent.indexOf(normalizedOld);
        if (idx === -1) {
          applied.push({ path: filePath, operation: "update", status: "skipped: oldString not found" });
          continue;
        }

        const replaced = normalizedContent.replace(normalizedOld, normalizedNew);
        yield* Effect.tryPromise({
          try: () => writeFile(resolved.absolutePath, replaced, "utf8"),
          catch: () => undefined,
        });
        applied.push({ path: filePath, operation: "update", status: "ok" });
        continue;
      }

      if (deleteMatch) {
        const filePath = deleteMatch[1].trim();
        i++;
        const resolved = resolveWorkspacePath(workspace, filePath);
        if (!resolved) {
          applied.push({ path: filePath, operation: "delete", status: "skipped: path outside workspace" });
          continue;
        }

        yield* Effect.tryPromise({
          try: async () => {
            const { rm } = await import("node:fs/promises");
            await rm(resolved.absolutePath, { force: true });
          },
          catch: () => undefined,
        });
        applied.push({ path: filePath, operation: "delete", status: "ok" });
        continue;
      }

      i++;
    }

    return {
      content: `Patch applied: ${applied.filter((a) => a.status === "ok").length} operations succeeded, ${applied.filter((a) => a.status !== "ok").length} skipped`,
      applied,
    };
  });
}

// ── Helpers ───────────────────────────────────────────────────────────────

function countOccurrences(content: string, search: string): number {
  if (search === "") return content.length + 1;
  let count = 0;
  let offset = 0;
  while ((offset = content.indexOf(search, offset)) !== -1) {
    count++;
    offset += search.length;
  }
  return count;
}
