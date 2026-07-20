import { createHash } from "node:crypto";
import { existsSync, lstatSync, mkdirSync, readFileSync, realpathSync, rmSync, statSync, writeFileSync } from "node:fs";
import path from "node:path";
import { spawn, spawnSync } from "node:child_process";
import { Effect } from "effect";

import { workspaceGitEnvironment } from "../git-process-environment.ts";
import {
  IdeRepositoryGenerationSchema,
  IdeSourceControlConfigGenerationSchema,
  IdeSourceControlCredentialHelperGenerationSchema,
  IdeSourceControlFailureSchema,
  IdeSourceControlRecoveryRefSchema,
  IdeSourceControlRefGenerationSchema,
  IdeSourceControlRemoteGenerationSchema,
  IdeSourceControlSnapshotSchema,
  type IdeSourceControlCommand,
  type IdeSourceControlFailureCode,
  type IdeSourceControlSnapshot,
} from "./source-control-contract.ts";
import { IdeWorktreeRefSchema } from "./project-contract.ts";
import {
  IdeSourceControlServiceError,
  type IdeSourceControlAdapter,
  type IdeSourceControlAdapterResult,
} from "./source-control-service.ts";
import type {
  IdePortableMutationAuthority,
  IdePortableMutationPermit,
} from "./portable-mutation-authority.ts";

type WorktreeRef = typeof IdeWorktreeRefSchema.Type;
type GitResult =
  | Readonly<{ ok: true; stdout: string; stderr: string }>
  | Readonly<{ ok: false; code: number | null; stdout: string; stderr: string; timedOut: boolean }>;

const digest = (value: string): string => createHash("sha256").update(value).digest("hex");
const opaque = (prefix: string, value: string): string => `${prefix}.${digest(value)}`;
const secretPath = (value: string): boolean => /(^|\/)(?:\.env(?:\..*)?|id_(?:rsa|dsa|ecdsa|ed25519)|credentials?|secrets?)(?:$|[._-])/iu.test(value);

const runGit = (
  root: string,
  args: ReadonlyArray<string>,
  input?: string,
  timeoutMs = 30_000,
): GitResult => {
  const child = spawnSync("git", ["-C", root, ...args], {
    cwd: root,
    encoding: "utf8",
    input,
    timeout: timeoutMs,
    maxBuffer: 16_000_000,
    stdio: [input === undefined ? "ignore" : "pipe", "pipe", "pipe"],
    env: { ...workspaceGitEnvironment(), GIT_TERMINAL_PROMPT: "0" },
  });
  return child.status === 0 && child.error === undefined
    ? { ok: true, stdout: String(child.stdout ?? ""), stderr: String(child.stderr ?? "") }
    : {
        ok: false,
        code: child.status,
        stdout: String(child.stdout ?? ""),
        stderr: String(child.stderr ?? ""),
        timedOut: (child.error as NodeJS.ErrnoException | undefined)?.code === "ETIMEDOUT",
      };
};

const runSupervisedGit = (
  root: string,
  args: ReadonlyArray<string>,
  authorized: () => boolean,
  input?: string,
  timeoutMs = 30_000,
): Promise<GitResult> => new Promise((resolve) => {
  const detached = process.platform !== "win32";
  const child = spawn("git", ["-C", root, ...args], {
    cwd: root,
    detached,
    stdio: ["pipe", "pipe", "pipe"],
    env: { ...workspaceGitEnvironment(), GIT_TERMINAL_PROMPT: "0" },
  });
  let stdout = "";
  let stderr = "";
  let spawnError: NodeJS.ErrnoException | null = null;
  let timedOut = false;
  let terminationStarted = false;
  let forceKillTimer: ReturnType<typeof setTimeout> | null = null;

  const signalTree = (signal: NodeJS.Signals): void => {
    if (child.pid === undefined) return;
    if (process.platform === "win32") {
      const killer = spawn("taskkill", ["/pid", String(child.pid), "/t", "/f"], {
        stdio: "ignore",
        windowsHide: true,
      });
      killer.unref();
      return;
    }
    try {
      if (detached) process.kill(-child.pid, signal);
      else child.kill(signal);
    } catch {
      child.kill(signal);
    }
  };
  const terminate = (): void => {
    if (terminationStarted) return;
    terminationStarted = true;
    signalTree("SIGTERM");
    forceKillTimer = setTimeout(() => signalTree("SIGKILL"), 250);
    forceKillTimer.unref();
  };
  const authorityTimer = setInterval(() => {
    if (!authorized()) terminate();
  }, 10);
  authorityTimer.unref();
  const timeout = setTimeout(() => {
    timedOut = true;
    terminate();
  }, timeoutMs);
  timeout.unref();
  const append = (current: string, chunk: Buffer): string =>
    `${current}${chunk.toString("utf8")}`.slice(0, 16_000_000);
  child.stdout.on("data", (chunk: Buffer) => { stdout = append(stdout, chunk); });
  child.stderr.on("data", (chunk: Buffer) => { stderr = append(stderr, chunk); });
  child.on("error", (error: NodeJS.ErrnoException) => { spawnError = error; });
  child.on("close", (code) => {
    clearInterval(authorityTimer);
    clearTimeout(timeout);
    if (forceKillTimer !== null) clearTimeout(forceKillTimer);
    resolve(code === 0 && spawnError === null
      ? { ok: true, stdout, stderr }
      : { ok: false, code, stdout, stderr, timedOut });
  });
  if (input === undefined) child.stdin.end();
  else child.stdin.end(input, "utf8");
});

const runGh = (root: string): GitResult => {
  const child = spawnSync("gh", ["pr", "view", "--json", "number,url,state,headRefName,baseRefName,headRefOid,commits,reviews,statusCheckRollup,mergeable,mergedAt,updatedAt"], {
    cwd: root,
    encoding: "utf8",
    timeout: 30_000,
    maxBuffer: 8_000_000,
    stdio: ["ignore", "pipe", "pipe"],
    env: { ...workspaceGitEnvironment(), GH_PROMPT_DISABLED: "1", GH_NO_UPDATE_NOTIFIER: "1" },
  });
  return child.status === 0 && child.error === undefined
    ? { ok: true, stdout: String(child.stdout ?? ""), stderr: String(child.stderr ?? "") }
    : { ok: false, code: child.status, stdout: String(child.stdout ?? ""), stderr: String(child.stderr ?? ""), timedOut: (child.error as NodeJS.ErrnoException | undefined)?.code === "ETIMEDOUT" };
};

const safeRelative = (root: string, value: string): string | null => {
  if (value === "" || path.isAbsolute(value) || /[\0\r\n]/u.test(value)) return null;
  const normalized = path.posix.normalize(value.replaceAll("\\", "/"));
  const resolved = path.resolve(root, normalized);
  return normalized === "." || normalized.startsWith("../") || path.relative(root, resolved).startsWith("..")
    ? null
    : normalized;
};

const failure = (
  code: IdeSourceControlFailureCode,
  message: string,
  current: IdeSourceControlSnapshot | null,
  operationRef: IdeSourceControlCommand extends infer _ ? string | null : never = null,
  retryable = false,
) => new IdeSourceControlServiceError({
  failure: IdeSourceControlFailureSchema.make({
    schemaVersion: "openagents.desktop.ide-source-control.v1",
    operationRef: operationRef as never,
    code,
    message,
    currentVersion: current?.version ?? null,
    conflictPaths: current?.paths.filter((entry) =>
      entry.indexState === "conflicted" || entry.worktreeState === "conflicted").map((entry) => entry.path) ?? [],
    recoveryRef: null,
    retryable,
  }),
});

const classify = (result: Exclude<GitResult, { ok: true }>): IdeSourceControlFailureCode => {
  const text = `${result.stdout}\n${result.stderr}`.toLowerCase();
  if (result.timedOut) return "cancelled";
  if (text.includes("index.lock") || text.includes("another git process")) return "index_locked";
  if (text.includes("non-fast-forward") || text.includes("fetch first")) return "non_fast_forward";
  if (text.includes("hook") && (text.includes("failed") || text.includes("declined"))) return "hook_failed";
  if (text.includes("sign") || text.includes("gpg")) return "signing_failed";
  if (text.includes("authentication") || text.includes("could not read username")) return "credential_unavailable";
  if (text.includes("conflict")) return "conflict_state";
  if (text.includes("local changes") || text.includes("would be overwritten")) return "dirty_state";
  if (text.includes("remote rejected") || text.includes("protected branch")) return "remote_rejected";
  return "operation_failed";
};

const assertGit = (
  result: GitResult,
  message: string,
  current: IdeSourceControlSnapshot | null,
  operationRef: string | null,
): string => {
  if (result.ok) return result.stdout;
  throw failure(classify(result), message, current, operationRef, result.timedOut);
};

const pathState = (value: string) => {
  switch (value) {
    case "A": return "added" as const;
    case "M": return "modified" as const;
    case "D": return "deleted" as const;
    case "R": return "renamed" as const;
    case "C": return "copied" as const;
    case "T": return "type_changed" as const;
    case "U": return "conflicted" as const;
    default: return "unmodified" as const;
  }
};

export interface IdeSourceControlGitAdapterOptions {
  readonly root: string;
  readonly seed: IdeSourceControlSnapshot;
  readonly now?: () => string;
  readonly worktreePath?: (worktreeRef: WorktreeRef) => string;
  readonly recoveryRoot?: string;
  readonly mutationAuthority?: IdePortableMutationAuthority;
  readonly mutationPermit?: () => IdePortableMutationPermit | undefined;
  readonly beforeMutationSpawn?: () => void;
  readonly afterMutationProcess?: () => void;
}

export const makeIdeSourceControlGitAdapter = (
  options: IdeSourceControlGitAdapterOptions,
): IdeSourceControlAdapter => {
  const root = realpathSync(options.root);
  const now = options.now ?? (() => new Date().toISOString());
  const recoveries = new Map<string, string>();
  let generation = Number(options.seed.version.repositoryGeneration);
  let stopped = false;
  let verifiedPush: Readonly<{ headOid: string; upstream: string }> | null = null;
  let verifiedCommit: Readonly<{ headOid: string; receiptRef: string }> | null = null;
  let providerDelivery: Readonly<{ headOid: string; number: string; state: string; reviews: string; checks: string; mergedAt: string }> | null = null;
  const managedWorktrees = new Map<string, Readonly<{ worktreeRef: WorktreeRef; ownerRef: string; creationHead: string | null }>>();
  const recoveryRoot = options.recoveryRoot === undefined ? null : path.resolve(options.recoveryRoot);
  if (recoveryRoot !== null) mkdirSync(recoveryRoot, { recursive: true, mode: 0o700 });
  const recoveryFile = (recoveryRef: string): string | null => recoveryRoot === null
    ? null
    : path.join(recoveryRoot, `${digest(recoveryRef)}.patch`);

  const snapshot = (): IdeSourceControlSnapshot => {
    if (stopped) return IdeSourceControlSnapshotSchema.make({ ...options.seed, stopped: true });
    const raw = assertGit(
      runGit(root, ["status", "--porcelain=v2", "--branch", "-z", "--untracked-files=all", "--ignored=matching"]),
      "Git status is unavailable.",
      null,
      null,
    );
    const headResult = runGit(root, ["rev-parse", "--verify", "HEAD"]);
    const headOid = headResult.ok && headResult.stdout.trim() !== "" ? headResult.stdout.trim() as never : null;
    const indexResult = runGit(root, ["write-tree"]);
    const indexOid = indexResult.ok ? indexResult.stdout.trim() : digest(`index\0${raw}`);
    const diff = runGit(root, ["diff", "--binary", "--no-ext-diff", "--no-textconv"]);
    const untracked = runGit(root, ["ls-files", "--others", "--exclude-standard", "-z"]);
    const untrackedFacts = untracked.ok
      ? untracked.stdout.split("\0").filter(Boolean).map((relative) => {
          const hashed = runGit(root, ["hash-object", "--no-filters", "--", relative]);
          return `${relative}\0${hashed.ok ? hashed.stdout.trim() : "unavailable"}`;
        }).join("\0")
      : "unavailable";
    const worktreeOid = digest(`${raw}\0${diff.ok ? diff.stdout : "unavailable"}\0${untrackedFacts}`);
    let branch: string | null = null;
    let upstream: string | null = null;
    let ahead = 0;
    let behind = 0;
    let paths: Array<IdeSourceControlSnapshot["paths"][number]> = [];
    const fields = raw.split("\0");
    for (let index = 0; index < fields.length; index++) {
      const record = fields[index]!;
      if (record.startsWith("# branch.head ")) branch = record.slice(14) === "(detached)" ? null : record.slice(14);
      else if (record.startsWith("# branch.upstream ")) upstream = record.slice(18);
      else if (record.startsWith("# branch.ab ")) {
        const match = /\+(\d+) -(\d+)/u.exec(record);
        ahead = Number(match?.[1] ?? 0); behind = Number(match?.[2] ?? 0);
      } else if (record.startsWith("1 ") || record.startsWith("2 ")) {
        const parts = record.split(" ");
        const rename = record.startsWith("2 ");
        const relative = parts.slice(rename ? 9 : 8).join(" ");
        const priorPath = rename ? fields[++index] ?? null : null;
        const xy = parts[1] ?? "..";
        paths.push({
          path: relative,
          priorPath,
          indexState: pathState(xy[0] ?? "."),
          worktreeState: pathState(xy[1] ?? "."),
          baseOid: (parts[6] ?? null) as never,
          indexOid: (parts[7] ?? null) as never,
          worktreeOid: parts[7] ?? null,
          modeBefore: parts[3] ?? null,
          modeAfter: parts[5] ?? null,
          conflict: null,
          secretWithheld: false,
          ignored: false,
          binary: false,
          truncated: false,
          stagedDiffRef: null,
          unstagedDiffRef: null,
        });
      } else if (record.startsWith("u ")) {
        const parts = record.split(" ");
        paths.push({
          path: parts.slice(10).join(" "), priorPath: null,
          indexState: "conflicted", worktreeState: "conflicted",
          baseOid: (parts[7] ?? null) as never, indexOid: (parts[8] ?? null) as never,
          worktreeOid: parts[9] ?? null, modeBefore: parts[3] ?? null, modeAfter: parts[6] ?? null,
          conflict: { baseOid: (parts[7] ?? null) as never, oursOid: (parts[8] ?? null) as never, theirsOid: (parts[9] ?? null) as never },
          secretWithheld: false, ignored: false, binary: false, truncated: false,
          stagedDiffRef: null, unstagedDiffRef: null,
        });
      } else if (record.startsWith("? ")) {
        const relative = record.slice(2);
        paths.push({
          path: relative, priorPath: null, indexState: "unmodified", worktreeState: "untracked",
          baseOid: null, indexOid: null,
          worktreeOid: existsSync(path.join(root, relative)) ? digest(`${relative}\0${statSync(path.join(root, relative)).size}`) : null,
          modeBefore: null, modeAfter: null, conflict: null, secretWithheld: false,
          ignored: false, binary: false, truncated: false,
          stagedDiffRef: null, unstagedDiffRef: null,
        });
      } else if (record.startsWith("! ")) {
        const relative = record.slice(2);
        paths.push({
          path: relative, priorPath: null, indexState: "ignored", worktreeState: "ignored",
          baseOid: null, indexOid: null, worktreeOid: null, modeBefore: null, modeAfter: null,
          conflict: null, secretWithheld: true, ignored: true, binary: false, truncated: false,
          stagedDiffRef: null, unstagedDiffRef: null,
        });
      }
    }
    paths = paths.map((entry) => {
      const absolute = path.join(root, entry.path);
      const file = existsSync(absolute) ? lstatSync(absolute) : null;
      const inspectable = file !== null && file.isFile() && file.size <= 8_000_000;
      const prefix = inspectable ? readFileSync(absolute).subarray(0, 8_192) : null;
      const binary = prefix?.includes(0) ?? false;
      const lfs = prefix?.toString("utf8").startsWith("version https://git-lfs.github.com/spec/v1\n") ?? false;
      const submodule = entry.modeAfter === "160000" || entry.modeBefore === "160000";
      return {
      ...entry,
      indexState: submodule ? "submodule" as const : lfs ? "lfs_pointer" as const : entry.indexState,
      worktreeState: submodule ? "submodule" as const : lfs ? "lfs_pointer" as const : entry.worktreeState,
      secretWithheld: entry.secretWithheld || secretPath(entry.path),
      binary,
      truncated: entry.truncated || (file?.isFile() === true && file.size > 8_000_000),
      stagedDiffRef: entry.indexState !== "unmodified" && entry.indexState !== "ignored"
        ? opaque("ide.scm-diff", `${headOid ?? "unborn"}\0${indexOid}\0${entry.path}\0staged`) as never
        : null,
      unstagedDiffRef: entry.worktreeState !== "unmodified" && entry.worktreeState !== "ignored"
        ? opaque("ide.scm-diff", `${indexOid}\0${worktreeOid}\0${entry.path}\0unstaged`) as never
        : null,
      };
    });
    const worktreeRaw = assertGit(runGit(root, ["worktree", "list", "--porcelain", "-z"]), "Worktrees are unavailable.", null, null);
    const blocks: string[][] = [];
    for (const field of worktreeRaw.split("\0").filter(Boolean)) {
      if (field.startsWith("worktree ")) blocks.push([field]);
      else blocks.at(-1)?.push(field);
    }
    const worktrees = blocks.map((lines, index) => {
      const worktreePath = lines.find((line) => line.startsWith("worktree "))?.slice(9) ?? root;
      const branchRef = lines.find((line) => line.startsWith("branch "))?.slice(7) ?? null;
      const worktreeHead = lines.find((line) => line.startsWith("HEAD "))?.slice(5) ?? null;
      const managed = managedWorktrees.get(worktreePath);
      const status = runGit(worktreePath, ["status", "--porcelain=v2", "-z", "--untracked-files=all"]);
      const dirty = !status.ok || status.stdout !== "";
      const upstreamProbe = runGit(worktreePath, ["rev-parse", "--abbrev-ref", "--symbolic-full-name", "@{upstream}"]);
      const aheadProbe = upstreamProbe.ok ? runGit(worktreePath, ["rev-list", "--count", "@{upstream}..HEAD"]) : null;
      const changed = managed !== undefined && (dirty || worktreeHead !== managed.creationHead);
      const unpushed = changed && (!upstreamProbe.ok || !aheadProbe?.ok || Number(aheadProbe.stdout.trim()) > 0);
      return {
        worktreeRef: index === 0 ? options.seed.binding.worktreeRef : managed?.worktreeRef ?? `ide.worktree.${digest(worktreePath)}` as WorktreeRef,
        branch: branchRef?.replace(/^refs\/heads\//u, "") ?? null,
        headOid: worktreeHead as never,
        detached: lines.includes("detached"), locked: lines.some((line) => line.startsWith("locked")),
        prunable: lines.some((line) => line.startsWith("prunable")), ownerRef: managed?.ownerRef ?? null, activeSessionRef: null,
        removalPreviewRef: index === 0 ? null : opaque("ide.scm-worktree-preview", `${worktreePath}\0${headOid ?? "unborn"}`),
        managed: managed !== undefined, dirty, changed, unpushed,
      };
    });
    generation += 1;
    const statusRef = opaque("ide.scm-status", `${headOid ?? "unborn"}\0${indexOid}\0${worktreeOid}\0${raw}\0${untrackedFacts}`);
    const gitDirectory = assertGit(runGit(root, ["rev-parse", "--git-dir"]), "Git metadata is unavailable.", null, null).trim();
    const metadataRoot = path.resolve(root, gitDirectory);
    const operation = existsSync(path.join(metadataRoot, "MERGE_HEAD")) ? { _tag: "Merge" as const, headName: null }
      : existsSync(path.join(metadataRoot, "rebase-merge")) || existsSync(path.join(metadataRoot, "rebase-apply")) ? { _tag: "Rebase" as const, onto: null, currentStep: 0 }
      : existsSync(path.join(metadataRoot, "CHERRY_PICK_HEAD")) ? { _tag: "CherryPick" as const, commitOid: null }
      : existsSync(path.join(metadataRoot, "REVERT_HEAD")) ? { _tag: "Revert" as const, commitOid: null }
      : { _tag: "Idle" as const };
    const currentCommit = verifiedCommit !== null && verifiedCommit.headOid === headOid ? verifiedCommit : null;
    const currentPush = verifiedPush !== null && verifiedPush.headOid === headOid && verifiedPush.upstream === upstream ? verifiedPush : null;
    const currentProvider = providerDelivery !== null && providerDelivery.headOid === headOid ? providerDelivery : null;
    const checks = currentProvider?.checks ?? "";
    const reviews = currentProvider?.reviews ?? "";
    const delivery = [
      { phase: "changed" as const, proven: paths.length > 0, evidenceRefs: [statusRef], observedAt: now(), freshness: "current" as const },
      { phase: "reviewed" as const, proven: reviews.includes("APPROVED"), evidenceRefs: currentProvider === null ? [] : [reviews], observedAt: now(), freshness: currentProvider === null ? "unknown" as const : "current" as const },
      { phase: "committed" as const, proven: currentCommit !== null, evidenceRefs: currentCommit === null ? [] : [currentCommit.headOid, currentCommit.receiptRef], observedAt: now(), freshness: currentCommit === null ? "unknown" as const : "current" as const },
      { phase: "pushed" as const, proven: currentPush !== null, evidenceRefs: currentPush === null ? [] : [currentPush.headOid, currentPush.upstream], observedAt: now(), freshness: currentPush === null ? "unknown" as const : "current" as const },
      { phase: "pull_request_open" as const, proven: currentProvider?.state.toUpperCase() === "OPEN", evidenceRefs: currentProvider === null ? [] : [currentProvider.number], observedAt: now(), freshness: currentProvider === null ? "unknown" as const : "current" as const },
      { phase: "checks_pending" as const, proven: currentProvider !== null && checks !== "" && !checks.includes("SUCCESS"), evidenceRefs: currentProvider === null ? [] : [checks], observedAt: now(), freshness: currentProvider === null ? "unknown" as const : "current" as const },
      { phase: "checks_failed" as const, proven: currentProvider !== null && /FAILURE|ERROR|CANCELLED/u.test(checks), evidenceRefs: currentProvider === null ? [] : [checks], observedAt: now(), freshness: currentProvider === null ? "unknown" as const : "current" as const },
      { phase: "review_requested" as const, proven: currentProvider !== null && reviews !== "[]", evidenceRefs: currentProvider === null ? [] : [reviews], observedAt: now(), freshness: currentProvider === null ? "unknown" as const : "current" as const },
      { phase: "review_approved" as const, proven: currentProvider !== null && reviews.includes("APPROVED"), evidenceRefs: currentProvider === null ? [] : [reviews], observedAt: now(), freshness: currentProvider === null ? "unknown" as const : "current" as const },
      { phase: "merged" as const, proven: currentProvider !== null && currentProvider.mergedAt !== "", evidenceRefs: currentProvider === null || currentProvider.mergedAt === "" ? [] : [currentProvider.mergedAt], observedAt: now(), freshness: currentProvider === null ? "unknown" as const : "current" as const },
      { phase: "owner_accepted" as const, proven: false, evidenceRefs: [], observedAt: now(), freshness: "unknown" as const },
      { phase: "released" as const, proven: false, evidenceRefs: [], observedAt: now(), freshness: "unknown" as const },
    ];
    return IdeSourceControlSnapshotSchema.make({
      ...options.seed,
      version: {
        repositoryGeneration: IdeRepositoryGenerationSchema.make(generation), statusRef, headOid,
        indexOid, worktreeOid,
        refGeneration: IdeSourceControlRefGenerationSchema.make(generation),
        configGeneration: IdeSourceControlConfigGenerationSchema.make(1),
        remoteGeneration: IdeSourceControlRemoteGenerationSchema.make(generation),
        credentialHelperGeneration: IdeSourceControlCredentialHelperGenerationSchema.make(1),
      },
      branch, upstream, detached: branch === null, ahead, behind, operation,
      paths, worktrees, delivery, observedAt: now(), stopped: false,
    });
  };

  const execute = async (
    command: Exclude<IdeSourceControlCommand, { readonly _tag: "Refresh" }>,
    current: IdeSourceControlSnapshot,
  ): Promise<IdeSourceControlAdapterResult> => {
    const op = command.operationRef;
    const readOnly = command._tag === "History" || command._tag === "Blame" || command._tag === "ProviderRefresh";
    if (!readOnly) {
      const live = snapshot();
      const exact = live.version.statusRef === current.version.statusRef &&
        live.version.headOid === current.version.headOid &&
        live.version.indexOid === current.version.indexOid &&
        live.version.worktreeOid === current.version.worktreeOid;
      if (!exact) throw failure("stale_version", "Repository state changed after the operation preview.", live, op, true);
    }
    const paths = "selection" in command && command.selection._tag === "Paths"
      ? command.selection.paths.map((value) => safeRelative(root, value))
      : [];
    if (paths.some((value) => value === null)) throw failure("policy_refused", "A path leaves the worktree.", current, op);
    if ((command._tag === "Stage" || command._tag === "Discard") && "selection" in command) {
      const selected = command.selection._tag === "Paths" ? command.selection.paths : [command.selection.path];
      if (selected.some((selectedPath) => current.paths.some((entry) => entry.path === selectedPath && (entry.secretWithheld || entry.ignored)))) {
        throw failure("policy_refused", "Ignored or secret-shaped content requires a separate admitted disclosure workflow.", current, op);
      }
    }
    const safeArgument = (value: string, label: string): string => {
      if (value.startsWith("-") || /[\0\r\n]/u.test(value)) {
        throw failure("policy_refused", `${label} has an option-shaped or invalid value.`, current, op);
      }
      return value;
    };
    const validatePatch = (
      selection: Extract<IdeSourceControlCommand, { readonly _tag: "Stage" }>["selection"] & { readonly _tag: "Patch" },
      source: "staged" | "unstaged",
    ): string => {
      const selectedPath = safeRelative(root, selection.path);
      if (selectedPath === null || (selection.selectedHunks.length === 0 && selection.selectedLines.length === 0)) {
        throw failure("policy_refused", "The partial selection is empty or leaves the worktree.", current, op);
      }
      const headers = [...selection.patch.matchAll(/^(?:diff --git a\/(.+) b\/(.+)|--- (?:a\/)?(.+)|\+\+\+ (?:b\/)?(.+))$/gmu)]
        .flatMap((match) => [match[1], match[2], match[3], match[4]])
        .filter((value): value is string => value !== undefined && value !== "/dev/null");
      if (headers.length === 0 || headers.some((value) => safeRelative(root, value) !== selectedPath)) {
        throw failure("policy_refused", "The partial patch does not match its selected path.", current, op);
      }
      const entry = current.paths.find((candidate) => candidate.path === selectedPath);
      const expectedDiffRef = source === "staged" ? entry?.stagedDiffRef : entry?.unstagedDiffRef;
      if (expectedDiffRef === null || expectedDiffRef === undefined || selection.diffRef !== expectedDiffRef) {
        throw failure("stale_version", "The partial patch does not match the canonical diff version.", current, op, true);
      }
      const canonical = source === "staged"
        ? runGit(root, ["diff", "--cached", "--binary", "--", selectedPath])
        : runGit(root, ["diff", "--binary", "--", selectedPath]);
      const changedLines = selection.patch.split("\n").filter((line) =>
        (line.startsWith("+") || line.startsWith("-")) &&
        !line.startsWith("+++") && !line.startsWith("---"));
      if (!canonical.ok || changedLines.some((line) => !canonical.stdout.includes(line))) {
        throw failure("partial_application", "The selected lines are not in the canonical diff.", current, op, true);
      }
      return selection.patch;
    };
    let recoveryRef: IdeSourceControlAdapterResult["recoveryRef"] = null;
    let observation: IdeSourceControlAdapterResult["observation"] = null;
    const requireCurrentPermit = (phase: "before" | "after"): void => {
      const currentPermit = options.mutationPermit?.();
      if (options.mutationAuthority !== undefined &&
        (currentPermit === undefined || !options.mutationAuthority.reauthorize(currentPermit))) {
        throw failure(
          "policy_refused",
          phase === "before"
            ? "Portable source-control authority changed before the Git side effect."
            : "Portable source-control authority changed during the Git side effect.",
          current,
          op,
          true,
        );
      }
    };
    const guardedGit = async (args: ReadonlyArray<string>, input?: string): Promise<GitResult> => {
      if (!readOnly) {
        requireCurrentPermit("before");
        options.beforeMutationSpawn?.();
        requireCurrentPermit("before");
      }
      const result = await runSupervisedGit(
        root,
        args,
        () => {
          const currentPermit = options.mutationPermit?.();
          return readOnly || options.mutationAuthority === undefined ||
            (currentPermit !== undefined && options.mutationAuthority.reauthorize(currentPermit));
        },
        input,
      );
      if (!readOnly) {
        options.afterMutationProcess?.();
        requireCurrentPermit("after");
      }
      return result;
    };
    const run = async (args: ReadonlyArray<string>, message: string, input?: string): Promise<string> =>
      assertGit(await guardedGit(args, input), message, current, op);
    if (!readOnly) requireCurrentPermit("before");
    switch (command._tag) {
      case "Stage": command.selection._tag === "Patch" ? await run(["apply", "--cached", "--unidiff-zero", "-"], "The selected patch could not be staged.", validatePatch(command.selection, "unstaged")) : await run(["add", "--", ...(paths as string[])], "The paths could not be staged."); break;
      case "Unstage": command.selection._tag === "Patch" ? await run(["apply", "--cached", "--reverse", "--unidiff-zero", "-"], "The selected patch could not be unstaged.", validatePatch(command.selection, "staged")) : await run(["restore", "--staged", "--", ...(paths as string[])], "The paths could not be unstaged."); break;
      case "Discard": {
        const patch = command.selection._tag === "Patch" ? validatePatch(command.selection, "unstaged") : await run(["diff", "--binary", "--", ...(paths as string[])], "The recovery patch could not be created.");
        recoveryRef = IdeSourceControlRecoveryRefSchema.make(`ide.scm-recovery.${digest(`${op}\0${patch}`)}`);
        recoveries.set(recoveryRef, patch);
        const persistedRecovery = recoveryFile(recoveryRef);
        if (persistedRecovery !== null) writeFileSync(persistedRecovery, patch, { encoding: "utf8", mode: 0o600, flag: "wx" });
        command.selection._tag === "Patch" ? await run(["apply", "--reverse", "--unidiff-zero", "-"], "The selected patch could not be discarded.", patch) : await run(["restore", "--worktree", "--", ...(paths as string[])], "The paths could not be discarded.");
        break;
      }
      case "Recover": {
        const persistedRecovery = recoveryFile(command.recoveryRef);
        const patch = recoveries.get(command.recoveryRef) ?? (persistedRecovery !== null && existsSync(persistedRecovery) ? readFileSync(persistedRecovery, "utf8") : undefined);
        if (patch === undefined) throw failure("recovery_unavailable", "The recovery record is unavailable.", current, op);
        await run(["apply", "-"], "The recovery patch could not be applied.", patch);
        recoveries.delete(command.recoveryRef);
        if (persistedRecovery !== null) rmSync(persistedRecovery, { force: true });
        break;
      }
      case "Commit": {
        const result = await guardedGit(["commit", ...(command.amend ? ["--amend"] : []), ...(command.sign ? ["-S"] : []), ...(command.runHooks ? [] : ["--no-verify"]), "-m", command.message]);
        if (!result.ok && command.sign) throw failure("signing_failed", "The signed commit could not be created.", current, op);
        const hooksRoot = assertGit(runGit(root, ["rev-parse", "--git-path", "hooks"]), "Git hook state is unavailable.", current, op).trim();
        const preCommit = path.resolve(root, hooksRoot, "pre-commit");
        if (!result.ok && command.runHooks && existsSync(preCommit)) throw failure("hook_failed", "A disclosed commit hook refused the commit.", current, op);
        assertGit(result, "The commit could not be created.", current, op);
        const committedHead = assertGit(runGit(root, ["rev-parse", "--verify", "HEAD"]), "The committed ref could not be verified.", current, op).trim();
        verifiedCommit = { headOid: committedHead, receiptRef: op };
        break;
      }
      case "BranchCreate": await run([command.checkout ? "switch" : "branch", ...(command.checkout ? ["-c"] : []), safeArgument(command.name, "Branch name")], "The branch could not be created."); break;
      case "TagCreate": await run(["tag", ...(command.sign ? ["-s", "-m", command.name] : []), safeArgument(command.name, "Tag name"), command.targetOid], "The tag could not be created."); break;
      case "Switch": await run(["switch", ...(command.detach ? ["--detach"] : []), safeArgument(command.refName, "Ref name")], "The ref could not be switched."); break;
      case "Merge": await run(["merge", ...(command.noFastForward ? ["--no-ff"] : []), safeArgument(command.refName, "Ref name")], "The merge did not complete."); break;
      case "Rebase": await run(["rebase", ...(command.onto === null ? [] : ["--onto", safeArgument(command.onto, "Onto ref")]), safeArgument(command.upstream, "Upstream ref")], "The rebase did not complete."); break;
      case "CherryPick": await run(["cherry-pick", ...command.commitOids], "The cherry-pick did not complete."); break;
      case "Revert": await run(["revert", "--no-edit", ...command.commitOids], "The revert did not complete."); break;
      case "Continue": await run([command.operation === "cherry_pick" ? "cherry-pick" : command.operation, "--continue"], "The operation could not continue."); break;
      case "Abort": await run([command.operation === "cherry_pick" ? "cherry-pick" : command.operation, "--abort"], "The operation could not abort."); break;
      case "Fetch": await run(["fetch", ...(command.prune ? ["--prune"] : []), safeArgument(command.remote, "Remote")], "Fetch did not complete."); break;
      case "Pull": await run(["pull", command.strategy === "ff_only" ? "--ff-only" : command.strategy === "rebase" ? "--rebase" : "--no-rebase", safeArgument(command.remote, "Remote"), safeArgument(command.branch, "Branch")], "Pull did not complete."); break;
      case "Push": {
        const remote = safeArgument(command.remote, "Remote");
        const refspec = safeArgument(command.refspec, "Refspec");
        await run(["push", ...(command.forcePolicy === "force_with_lease" ? [`--force-with-lease=${refspec}:${command.expectedRemoteOid ?? ""}`] : []), remote, refspec], "Push did not complete.");
        const destination = refspec.includes(":") ? refspec.slice(refspec.lastIndexOf(":") + 1) : refspec;
        const head = assertGit(runGit(root, ["rev-parse", "--verify", "HEAD"]), "The pushed commit could not be verified.", current, op).trim();
        const remoteRefs = await run(["ls-remote", remote, destination], "The pushed remote ref could not be verified.");
        const remoteOid = remoteRefs.trim().split(/\s+/u)[0] ?? "";
        if (remoteOid !== head) throw failure("remote_rejected", "The remote ref does not match the pushed commit.", current, op, true);
        verifiedPush = { headOid: head, upstream: `${remote}/${destination.replace(/^refs\/heads\//u, "")}` };
        break;
      }
      case "WorktreeCreate": {
        const target = options.worktreePath?.(command.worktreeRef);
        if (target === undefined || command.ownerRef === null || existsSync(target)) throw failure("policy_refused", "No unoccupied admitted worktree placement exists.", current, op);
        const creationHead = (await run(["rev-parse", "--verify", command.branch], "The worktree branch is unavailable.")).trim();
        await run(["worktree", "add", target, command.branch], "The worktree could not be created.");
        managedWorktrees.set(realpathSync(target), { worktreeRef: command.worktreeRef, ownerRef: command.ownerRef, creationHead });
        break;
      }
      case "WorktreeRemove": {
        const entry = current.worktrees.find((item) => item.worktreeRef === command.worktreeRef);
        const target = options.worktreePath?.(command.worktreeRef);
        if (entry?.removalPreviewRef !== command.previewRef || target === undefined || !command.recoverable || !entry.managed || entry.ownerRef === null || entry.activeSessionRef !== null || entry.dirty || entry.unpushed) {
          throw failure("policy_refused", "The worktree is occupied, changed without a pushed ref, dirty, unmanaged, or has a stale preview.", current, op);
        }
        const canonicalTarget = realpathSync(target);
        await run(["worktree", "remove", target], "The worktree could not be removed.");
        managedWorktrees.delete(canonicalTarget);
        break;
      }
      case "WorktreeRepair": await run(["worktree", "repair"], "Worktree metadata could not be repaired."); break;
      case "History": {
        const output = await run(["log", `--max-count=${command.limit}`, "--format=%H%x1f%P%x1f%an%x1f%aI%x1f%s%x1e", command.commitish], "History is unavailable.");
        observation = {
          _tag: "History", commitish: command.commitish, truncated: false,
          entries: output.split("\x1e").map((record) => record.trim()).filter(Boolean).map((record) => {
            const [commitOid = "", parents = "", author = "", authoredAt = "", summary = ""] = record.split("\x1f");
            return { commitOid: commitOid as never, parentOids: parents === "" ? [] : parents.split(" ") as never, author, authoredAt, summary };
          }),
        };
        break;
      }
      case "Blame": {
        const output = await run(["blame", "--line-porcelain", command.commitOid, "--", command.path], "Blame is unavailable.");
        const lines: Array<{ sourceOid: never; originalLine: number; finalLine: number; author: string; summary: string }> = [];
        let pending: { sourceOid: never; originalLine: number; finalLine: number; author: string; summary: string } | null = null;
        for (const line of output.split("\n")) {
          const header = /^([0-9a-f]{40,64}) (\d+) (\d+)(?: \d+)?$/u.exec(line);
          if (header !== null) {
            if (pending !== null) lines.push(pending);
            pending = { sourceOid: header[1] as never, originalLine: Number(header[2]), finalLine: Number(header[3]), author: "", summary: "" };
          } else if (pending !== null && line.startsWith("author ")) pending.author = line.slice(7);
          else if (pending !== null && line.startsWith("summary ")) pending.summary = line.slice(8);
        }
        if (pending !== null) lines.push(pending);
        observation = { _tag: "Blame", path: command.path, commitOid: command.commitOid, lines, truncated: false };
        break;
      }
      case "ProviderRefresh": {
        const result = runGh(root);
        if (!result.ok) {
          const text = `${result.stdout}\n${result.stderr}`.toLowerCase();
          throw failure(text.includes("auth") || text.includes("login") ? "credential_unavailable" : "provider_unavailable", "Pull-request state is unavailable from the configured provider.", current, op, true);
        }
        let row: Record<string, unknown>;
        try { row = JSON.parse(result.stdout) as Record<string, unknown>; }
        catch { throw failure("provider_unavailable", "The pull-request provider returned an invalid response.", current, op, true); }
        const textFact = (key: string): string => {
          const value = row[key];
          return typeof value === "string" || typeof value === "number" ? String(value).slice(0, 2_000) : "";
        };
        const listFact = (key: string): string => JSON.stringify(Array.isArray(row[key]) ? row[key] : []).slice(0, 2_000);
        observation = {
          _tag: "Provider", providerRef: command.providerRef, freshness: "current",
          facts: [
            "number", "url", "state", "headRefName", "baseRefName", "headRefOid", "mergeable", "mergedAt", "updatedAt",
          ].map((key) => ({ key, value: textFact(key) })).concat([
            { key: "commits", value: listFact("commits") },
            { key: "reviews", value: listFact("reviews") },
            { key: "checks", value: listFact("statusCheckRollup") },
          ]),
        };
        providerDelivery = {
          headOid: textFact("headRefOid"), number: textFact("number"), state: textFact("state"),
          reviews: listFact("reviews"), checks: listFact("statusCheckRollup"), mergedAt: textFact("mergedAt"),
        };
        break;
      }
    }
    if (!readOnly) requireCurrentPermit("after");
    const next = snapshot();
    return { snapshot: next, changedPaths: next.paths.map((entry) => entry.path), conflictPaths: next.paths.filter((entry) => entry.indexState === "conflicted").map((entry) => entry.path), omittedFacts: [], recoveryRef, observation };
  };

  return {
    refresh: () => Effect.try({ try: snapshot, catch: (cause) => cause instanceof IdeSourceControlServiceError ? cause : failure("repository_unavailable", "The repository could not be refreshed.", null) }),
    execute: (command, current) => Effect.tryPromise({ try: () => execute(command, current), catch: (cause) => cause instanceof IdeSourceControlServiceError ? cause : failure("operation_failed", "The Git operation failed.", current, command.operationRef) }),
    stop: () => Effect.sync(() => { stopped = true; recoveries.clear(); }),
  };
};
