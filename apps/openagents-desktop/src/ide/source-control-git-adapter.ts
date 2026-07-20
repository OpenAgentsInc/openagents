import { createHash } from "node:crypto";
import { existsSync, realpathSync, statSync } from "node:fs";
import path from "node:path";
import { spawnSync } from "node:child_process";
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

type WorktreeRef = typeof IdeWorktreeRefSchema.Type;
type GitResult =
  | Readonly<{ ok: true; stdout: string; stderr: string }>
  | Readonly<{ ok: false; code: number | null; stdout: string; stderr: string; timedOut: boolean }>;

const digest = (value: string): string => createHash("sha256").update(value).digest("hex");
const opaque = (prefix: string, value: string): string => `${prefix}.${digest(value)}`;

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
}

export const makeIdeSourceControlGitAdapter = (
  options: IdeSourceControlGitAdapterOptions,
): IdeSourceControlAdapter => {
  const root = realpathSync(options.root);
  const now = options.now ?? (() => new Date().toISOString());
  const recoveries = new Map<string, string>();
  let generation = Number(options.seed.version.repositoryGeneration);
  let stopped = false;

  const snapshot = (): IdeSourceControlSnapshot => {
    if (stopped) return IdeSourceControlSnapshotSchema.make({ ...options.seed, stopped: true });
    const raw = assertGit(
      runGit(root, ["status", "--porcelain=v2", "--branch", "-z", "--untracked-files=all"]),
      "Git status is unavailable.",
      null,
      null,
    );
    const headResult = runGit(root, ["rev-parse", "--verify", "HEAD"]);
    const headOid = headResult.ok && headResult.stdout.trim() !== "" ? headResult.stdout.trim() as never : null;
    const indexResult = runGit(root, ["write-tree"]);
    const indexOid = indexResult.ok ? indexResult.stdout.trim() : digest(`index\0${raw}`);
    const diff = runGit(root, ["diff", "--binary", "--no-ext-diff", "--no-textconv"]);
    const worktreeOid = digest(`${raw}\0${diff.ok ? diff.stdout : "unavailable"}`);
    let branch: string | null = null;
    let upstream: string | null = null;
    let ahead = 0;
    let behind = 0;
    const paths: Array<IdeSourceControlSnapshot["paths"][number]> = [];
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
        });
      } else if (record.startsWith("? ")) {
        const relative = record.slice(2);
        paths.push({
          path: relative, priorPath: null, indexState: "unmodified", worktreeState: "untracked",
          baseOid: null, indexOid: null,
          worktreeOid: existsSync(path.join(root, relative)) ? digest(`${relative}\0${statSync(path.join(root, relative)).size}`) : null,
          modeBefore: null, modeAfter: null, conflict: null, secretWithheld: false,
          ignored: false, binary: false, truncated: false,
        });
      }
    }
    const worktreeRaw = assertGit(runGit(root, ["worktree", "list", "--porcelain", "-z"]), "Worktrees are unavailable.", null, null);
    const blocks = worktreeRaw.split("\0\0").filter(Boolean);
    const worktrees = blocks.map((block, index) => {
      const lines = block.split("\0");
      const worktreePath = lines.find((line) => line.startsWith("worktree "))?.slice(9) ?? root;
      const branchRef = lines.find((line) => line.startsWith("branch "))?.slice(7) ?? null;
      return {
        worktreeRef: index === 0 ? options.seed.binding.worktreeRef : `ide.worktree.${digest(worktreePath)}` as WorktreeRef,
        branch: branchRef?.replace(/^refs\/heads\//u, "") ?? null,
        headOid: (lines.find((line) => line.startsWith("HEAD "))?.slice(5) ?? null) as never,
        detached: lines.includes("detached"), locked: lines.some((line) => line.startsWith("locked")),
        prunable: lines.some((line) => line.startsWith("prunable")), ownerRef: null, activeSessionRef: null,
        removalPreviewRef: index === 0 ? null : opaque("ide.scm-worktree-preview", `${worktreePath}\0${headOid ?? "unborn"}`),
      };
    });
    generation += 1;
    const statusRef = opaque("ide.scm-status", `${headOid ?? "unborn"}\0${indexOid}\0${worktreeOid}\0${raw}`);
    const delivery = [
      { phase: "changed" as const, proven: paths.length > 0, evidenceRefs: [statusRef], observedAt: now(), freshness: "current" as const },
      { phase: "committed" as const, proven: headOid !== null, evidenceRefs: headOid === null ? [] : [headOid], observedAt: now(), freshness: "current" as const },
      { phase: "pushed" as const, proven: ahead === 0 && upstream !== null && headOid !== null, evidenceRefs: upstream === null ? [] : [upstream], observedAt: now(), freshness: "current" as const },
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
      branch, upstream, detached: branch === null, ahead, behind, operation: { _tag: "Idle" },
      paths, worktrees, delivery, observedAt: now(), stopped: false,
    });
  };

  const execute = (
    command: Exclude<IdeSourceControlCommand, { readonly _tag: "Refresh" }>,
    current: IdeSourceControlSnapshot,
  ): IdeSourceControlAdapterResult => {
    const op = command.operationRef;
    const paths = "selection" in command && command.selection._tag === "Paths"
      ? command.selection.paths.map((value) => safeRelative(root, value))
      : [];
    if (paths.some((value) => value === null)) throw failure("policy_refused", "A path leaves the worktree.", current, op);
    let recoveryRef: IdeSourceControlAdapterResult["recoveryRef"] = null;
    const run = (args: ReadonlyArray<string>, message: string, input?: string) =>
      assertGit(runGit(root, args, input), message, current, op);
    switch (command._tag) {
      case "Stage": command.selection._tag === "Patch" ? run(["apply", "--cached", "--unidiff-zero", "-"], "The selected patch could not be staged.", command.selection.patch) : run(["add", "--", ...(paths as string[])], "The paths could not be staged."); break;
      case "Unstage": command.selection._tag === "Patch" ? run(["apply", "--cached", "--reverse", "--unidiff-zero", "-"], "The selected patch could not be unstaged.", command.selection.patch) : run(["restore", "--staged", "--", ...(paths as string[])], "The paths could not be unstaged."); break;
      case "Discard": {
        const patch = command.selection._tag === "Patch" ? command.selection.patch : run(["diff", "--binary", "--", ...(paths as string[])], "The recovery patch could not be created.");
        recoveryRef = IdeSourceControlRecoveryRefSchema.make(`ide.scm-recovery.${digest(`${op}\0${patch}`)}`);
        recoveries.set(recoveryRef, patch);
        command.selection._tag === "Patch" ? run(["apply", "--reverse", "--unidiff-zero", "-"], "The selected patch could not be discarded.", patch) : run(["restore", "--worktree", "--", ...(paths as string[])], "The paths could not be discarded.");
        break;
      }
      case "Recover": { const patch = recoveries.get(command.recoveryRef); if (patch === undefined) throw failure("recovery_unavailable", "The recovery record is unavailable.", current, op); run(["apply", "-"], "The recovery patch could not be applied.", patch); recoveries.delete(command.recoveryRef); break; }
      case "Commit": run(["commit", ...(command.amend ? ["--amend"] : []), ...(command.sign ? ["-S"] : []), ...(command.runHooks ? [] : ["--no-verify"]), "-m", command.message], "The commit could not be created."); break;
      case "BranchCreate": run([command.checkout ? "switch" : "branch", ...(command.checkout ? ["-c"] : []), command.name], "The branch could not be created."); break;
      case "TagCreate": run(["tag", ...(command.sign ? ["-s", "-m", command.name] : []), command.name, command.targetOid], "The tag could not be created."); break;
      case "Switch": run(["switch", ...(command.detach ? ["--detach"] : []), command.refName], "The ref could not be switched."); break;
      case "Merge": run(["merge", ...(command.noFastForward ? ["--no-ff"] : []), command.refName], "The merge did not complete."); break;
      case "Rebase": run(["rebase", ...(command.onto === null ? [] : ["--onto", command.onto]), command.upstream], "The rebase did not complete."); break;
      case "CherryPick": run(["cherry-pick", ...command.commitOids], "The cherry-pick did not complete."); break;
      case "Revert": run(["revert", "--no-edit", ...command.commitOids], "The revert did not complete."); break;
      case "Continue": run([command.operation === "cherry_pick" ? "cherry-pick" : command.operation, "--continue"], "The operation could not continue."); break;
      case "Abort": run([command.operation === "cherry_pick" ? "cherry-pick" : command.operation, "--abort"], "The operation could not abort."); break;
      case "Fetch": run(["fetch", ...(command.prune ? ["--prune"] : []), command.remote], "Fetch did not complete."); break;
      case "Pull": run(["pull", command.strategy === "ff_only" ? "--ff-only" : command.strategy === "rebase" ? "--rebase" : "--no-rebase", command.remote, command.branch], "Pull did not complete."); break;
      case "Push": run(["push", ...(command.forcePolicy === "force_with_lease" ? [`--force-with-lease=${command.refspec}:${command.expectedRemoteOid ?? ""}`] : []), command.remote, command.refspec], "Push did not complete."); break;
      case "WorktreeCreate": { const target = options.worktreePath?.(command.worktreeRef); if (target === undefined) throw failure("policy_refused", "No admitted worktree placement exists.", current, op); run(["worktree", "add", target, command.branch], "The worktree could not be created."); break; }
      case "WorktreeRemove": { const entry = current.worktrees.find((item) => item.worktreeRef === command.worktreeRef); const target = options.worktreePath?.(command.worktreeRef); if (entry?.removalPreviewRef !== command.previewRef || target === undefined || !command.recoverable) throw failure("policy_refused", "The worktree removal preview is stale or not recoverable.", current, op); run(["worktree", "remove", target], "The worktree could not be removed."); break; }
      case "WorktreeRepair": run(["worktree", "repair"], "Worktree metadata could not be repaired."); break;
      case "History": run(["log", `--max-count=${command.limit}`, "--format=%H%x00%P%x00%an%x00%aI%x00%s", command.commitish], "History is unavailable."); break;
      case "Blame": run(["blame", "--line-porcelain", command.commitOid, "--", command.path], "Blame is unavailable."); break;
      case "ProviderRefresh": break;
    }
    const next = snapshot();
    return { snapshot: next, changedPaths: next.paths.map((entry) => entry.path), conflictPaths: next.paths.filter((entry) => entry.indexState === "conflicted").map((entry) => entry.path), omittedFacts: [], recoveryRef };
  };

  return {
    refresh: () => Effect.try({ try: snapshot, catch: (cause) => cause instanceof IdeSourceControlServiceError ? cause : failure("repository_unavailable", "The repository could not be refreshed.", null) }),
    execute: (command, current) => Effect.try({ try: () => execute(command, current), catch: (cause) => cause instanceof IdeSourceControlServiceError ? cause : failure("operation_failed", "The Git operation failed.", current, command.operationRef) }),
    stop: () => Effect.sync(() => { stopped = true; recoveries.clear(); }),
  };
};
