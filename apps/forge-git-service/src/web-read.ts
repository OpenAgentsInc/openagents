import { spawn } from "node:child_process";
import { join, posix } from "node:path";

import { Context, Effect, Layer } from "effect";

import { ForgeGitConfiguration, type ForgeGitConfigurationShape } from "./config.js";
import { ForgeGitWebReadError } from "./model.js";
import {
  ForgeWebReadFile,
  ForgeWebReadProjection,
  ForgeWebReadSchemaVersion,
  type ForgeWebReadPolicyDecision,
  type ForgeWebReadProjection as ForgeWebReadProjectionType,
  type ForgeWebReadRequest,
} from "./web-read-model.js";

const textDecoder = new TextDecoder();
const textEncoder = new TextEncoder();
const maximumCommandBytes = 16 * 1024 * 1024;
const maximumCommits = 30;

export interface ForgeWebReadShape {
  readonly readAsset: (input: {
    readonly commitId: string;
    readonly maxImageBytes: number;
    readonly objectId: string;
    readonly owner: string;
    readonly path: string;
    readonly repo: string;
  }) => Effect.Effect<
    Readonly<{
      bytes: Uint8Array;
      contentType: "image/png" | "image/jpeg" | "image/gif" | "image/webp" | "image/svg+xml";
    }>,
    ForgeGitWebReadError
  >;
  readonly read: (input: {
    readonly policy: ForgeWebReadPolicyDecision;
    readonly request: ForgeWebReadRequest;
  }) => Effect.Effect<ForgeWebReadProjectionType, ForgeGitWebReadError>;
}

export class ForgeWebRead extends Context.Service<ForgeWebRead, ForgeWebReadShape>()(
  "@openagentsinc/forge-git-service/WebRead",
) {}

const webReadError = (
  operation: string,
  code: string,
  status: number,
  cause?: unknown,
): ForgeGitWebReadError =>
  new ForgeGitWebReadError({
    ...(cause === undefined ? {} : { cause }),
    code,
    operation,
    status,
  });

const collect = async (
  stream: NodeJS.ReadableStream,
  maximumBytes: number,
): Promise<Uint8Array> => {
  const chunks: Array<Uint8Array> = [];
  let total = 0;
  for await (const chunk of stream) {
    const bytes =
      typeof chunk === "string" ? textEncoder.encode(chunk) : new Uint8Array(chunk as Buffer);
    total += bytes.byteLength;
    if (total > maximumBytes) throw new Error("git output exceeded its bound");
    chunks.push(bytes);
  }
  const output = new Uint8Array(total);
  let offset = 0;
  for (const chunk of chunks) {
    output.set(chunk, offset);
    offset += chunk.byteLength;
  }
  return output;
};

const runGit = async (
  configuration: ForgeGitConfigurationShape,
  gitDirectory: string,
  args: ReadonlyArray<string>,
  maximumBytes = maximumCommandBytes,
): Promise<Uint8Array> => {
  const child = spawn(configuration.gitBinary, ["--git-dir", gitDirectory, ...args], {
    env: {
      GIT_CONFIG_NOSYSTEM: "1",
      HOME: process.env["HOME"],
      PATH: process.env["PATH"],
    },
    stdio: ["ignore", "pipe", "pipe"],
  });
  const stdout = collect(child.stdout, maximumBytes);
  const stderr = collect(child.stderr, 16 * 1024);
  const exitCode = new Promise<number | null>((resolve, reject) => {
    child.once("error", reject);
    child.once("close", resolve);
  });
  const [code, output, diagnostic] = await Promise.all([exitCode, stdout, stderr]);
  if (code !== 0) {
    throw new Error(textDecoder.decode(diagnostic) || `git exited with status ${String(code)}`);
  }
  return output;
};

const text = (bytes: Uint8Array): string => textDecoder.decode(bytes).trimEnd();

const containsControlCharacter = (value: string): boolean =>
  [...value].some((character) => {
    const codePoint = character.codePointAt(0) ?? 0;
    return codePoint <= 31 || codePoint === 127;
  });

const safeRepositoryPath = (value: string): boolean =>
  !value.includes("\\") &&
  !containsControlCharacter(value) &&
  value.split("/").every((segment) => segment !== "." && segment !== "..");

const safeRevision = (value: string): boolean =>
  value !== "" && !value.startsWith("-") && !containsControlCharacter(value);

const parseInteger = (value: string | undefined): number => {
  const parsed = Number(value);
  return Number.isFinite(parsed) ? parsed : 0;
};

const imageMimeType = (
  path: string,
): "image/png" | "image/jpeg" | "image/gif" | "image/webp" | "image/svg+xml" | undefined => {
  const lower = path.toLowerCase();
  if (lower.endsWith(".png")) return "image/png";
  if (lower.endsWith(".jpg") || lower.endsWith(".jpeg")) return "image/jpeg";
  if (lower.endsWith(".gif")) return "image/gif";
  if (lower.endsWith(".webp")) return "image/webp";
  if (lower.endsWith(".svg")) return "image/svg+xml";
  return undefined;
};

const languageFor = (path: string): string | undefined => {
  const extension = posix.extname(path).slice(1).toLowerCase();
  const aliases: Readonly<Record<string, string>> = {
    cjs: "javascript",
    css: "css",
    html: "html",
    js: "javascript",
    json: "json",
    jsx: "jsx",
    md: "markdown",
    mjs: "javascript",
    py: "python",
    rs: "rust",
    sh: "shell",
    ts: "typescript",
    tsx: "tsx",
    yaml: "yaml",
    yml: "yaml",
  };
  return aliases[extension];
};

const sameOriginAssetUrl = (input: {
  readonly commitId: string;
  readonly maxImageBytes: number;
  readonly objectId: string;
  readonly owner: string;
  readonly path: string;
  readonly repo: string;
}): string => {
  const encodedPath = input.path.split("/").map(encodeURIComponent).join("/");
  const search = new URLSearchParams({
    commit: input.commitId,
    max_image_bytes: String(input.maxImageBytes),
    object: input.objectId,
  });
  return `/internal/v1/repositories/${encodeURIComponent(input.owner)}/${encodeURIComponent(input.repo)}/web-read-asset/${encodedPath}?${search.toString()}`;
};

const markdownImageReferences = (markdown: string): ReadonlyArray<string> => {
  const references: Array<string> = [];
  const expression = /!\[[^\]]*\]\(\s*(?:<([^>]+)>|([^\s)]+))(?:\s+["'][^"']*["'])?\s*\)/gu;
  for (const match of markdown.matchAll(expression)) {
    const value = match[1] ?? match[2];
    if (value !== undefined && value !== "") references.push(value);
  }
  return [...new Set(references)];
};

const limitUtf8 = (
  value: string,
  maximumBytes: number,
): Readonly<{ text: string; truncated: boolean }> => {
  const bytes = textEncoder.encode(value);
  if (bytes.byteLength <= maximumBytes) return { text: value, truncated: false };
  return {
    text: textDecoder.decode(bytes.slice(0, maximumBytes)),
    truncated: true,
  };
};

const makeWebRead = (configuration: ForgeGitConfigurationShape): ForgeWebReadShape => {
  const readAsset = Effect.fn("ForgeWebRead.readAsset")(function* (input: {
    readonly commitId: string;
    readonly maxImageBytes: number;
    readonly objectId: string;
    readonly owner: string;
    readonly path: string;
    readonly repo: string;
  }) {
    const contentType = imageMimeType(input.path);
    if (
      contentType === undefined ||
      !safeRepositoryPath(input.path) ||
      !/^[0-9a-f]{40}(?:[0-9a-f]{24})?$/u.test(input.commitId) ||
      !/^[0-9a-f]{40}(?:[0-9a-f]{24})?$/u.test(input.objectId)
    ) {
      return yield* webReadError(
        "ForgeWebRead.readAsset.validate",
        "forge_web_read_asset_invalid",
        400,
      );
    }
    const gitDirectory = join(configuration.repositoryRoot, input.owner, `${input.repo}.git`);
    const git = (args: ReadonlyArray<string>, maximumBytes?: number) =>
      runGit(configuration, gitDirectory, args, maximumBytes);
    return yield* Effect.tryPromise({
      try: async () => {
        const containingRefs = text(
          await git([
            "for-each-ref",
            "--contains",
            input.commitId,
            "--format=%(refname)",
            "refs/heads",
            "refs/tags",
          ]),
        );
        if (containingRefs === "") {
          throw webReadError(
            "ForgeWebRead.readAsset.reachability",
            "forge_web_read_asset_not_found",
            404,
          );
        }
        const pathObjectId = text(
          await git([
            "rev-parse",
            "--verify",
            `${input.commitId}:${input.path}`,
          ]),
        );
        if (pathObjectId !== input.objectId) {
          throw webReadError(
            "ForgeWebRead.readAsset.identity",
            "forge_web_read_asset_not_found",
            404,
          );
        }
        const objectType = text(await git(["cat-file", "-t", input.objectId]));
        if (objectType !== "blob") {
          throw webReadError("ForgeWebRead.readAsset.type", "forge_web_read_asset_not_found", 404);
        }
        const size = parseInteger(text(await git(["cat-file", "-s", input.objectId])));
        if (size > input.maxImageBytes) {
          throw webReadError("ForgeWebRead.readAsset.size", "forge_web_read_asset_too_large", 413);
        }
        const bytes = await git(["cat-file", "blob", input.objectId], input.maxImageBytes);
        return { bytes, contentType };
      },
      catch: (cause) =>
        cause instanceof ForgeGitWebReadError
          ? cause
          : webReadError("ForgeWebRead.readAsset", "forge_web_read_asset_not_found", 404, cause),
    });
  });

  const read = Effect.fn("ForgeWebRead.read")(function* (input: {
    readonly policy: ForgeWebReadPolicyDecision;
    readonly request: ForgeWebReadRequest;
  }) {
    const { request } = input;
    if (
      (request.path !== undefined && !safeRepositoryPath(request.path)) ||
      (request.ref !== undefined && !safeRevision(request.ref)) ||
      (request.commit !== undefined && !safeRevision(request.commit)) ||
      (request.base !== undefined && !safeRevision(request.base))
    ) {
      return yield* webReadError("ForgeWebRead.validate", "forge_web_read_request_invalid", 400);
    }

    const gitDirectory = join(configuration.repositoryRoot, request.owner, `${request.repo}.git`);
    const git = (args: ReadonlyArray<string>, maximumBytes?: number) =>
      runGit(configuration, gitDirectory, args, maximumBytes);
    const execute = <T>(
      operation: string,
      effect: () => Promise<T>,
      code = "forge_web_read_failed",
      status = 500,
    ) =>
      Effect.tryPromise({
        try: effect,
        catch: (cause) => webReadError(operation, code, status, cause),
      });

    const requestedRevision = request.ref ?? "HEAD";
    const selectedObjectId = yield* execute(
      "ForgeWebRead.resolveRef",
      async () => text(await git(["rev-parse", "--verify", `${requestedRevision}^{commit}`])),
      "forge_web_read_revision_not_found",
      404,
    );
    const defaultBranch = yield* execute("ForgeWebRead.defaultBranch", async () => {
      try {
        return text(await git(["symbolic-ref", "--short", "HEAD"]));
      } catch {
        return "main";
      }
    });
    const selectedRef = request.ref ?? defaultBranch;

    const refs = yield* execute("ForgeWebRead.refs", async () => {
      const output = text(
        await git([
          "for-each-ref",
          "--format=%(objectname)%00%(refname)%00%(refname:short)",
          "refs/heads",
          "refs/tags",
        ]),
      );
      if (output === "") return [];
      return output.split("\n").map((line) => {
        const [objectId, fullName, name] = line.split("\0", 3);
        if (objectId === undefined || fullName === undefined || name === undefined) {
          throw new Error("invalid ref record");
        }
        const kind = fullName.startsWith("refs/tags/") ? ("tag" as const) : ("branch" as const);
        return {
          isDefault: kind === "branch" && name === defaultBranch,
          kind,
          name,
          objectId,
        };
      });
    });

    const commitSummary = async (objectId: string) => {
      const fields = text(
        await git(["show", "-s", "--format=%H%x00%h%x00%s%x00%an%x00%aI%x00%P", objectId]),
      ).split("\0");
      const [fullId, shortId, subject, authorName, authoredAt, parents = ""] = fields;
      if (
        fullId === undefined ||
        shortId === undefined ||
        subject === undefined ||
        authorName === undefined ||
        authoredAt === undefined
      ) {
        throw new Error("invalid commit record");
      }
      const stat = text(await git(["show", "--shortstat", "--format=", objectId]));
      const changedFiles = parseInteger(/(\d+) files? changed/u.exec(stat)?.[1]);
      const additions = parseInteger(/(\d+) insertions?\(\+\)/u.exec(stat)?.[1]);
      const deletions = parseInteger(/(\d+) deletions?\(-\)/u.exec(stat)?.[1]);
      return {
        additions,
        authoredAt,
        authorName,
        changedFiles,
        deletions,
        objectId: fullId,
        parentIds: parents === "" ? [] : parents.split(" "),
        shortId,
        subject,
      };
    };

    const commitIds = yield* execute("ForgeWebRead.commitIds", async () => {
      const output = text(
        await git([
          "log",
          `--max-count=${String(maximumCommits)}`,
          "--format=%H",
          selectedObjectId,
        ]),
      );
      return output === "" ? [] : output.split("\n");
    });
    const commits = yield* execute("ForgeWebRead.commits", () =>
      Promise.all(commitIds.map(commitSummary)),
    );

    const selectedPath = request.path ?? "";
    const selectedPathType =
      selectedPath === ""
        ? "tree"
        : yield* execute(
            "ForgeWebRead.pathType",
            async () => text(await git(["cat-file", "-t", `${selectedObjectId}:${selectedPath}`])),
            "forge_web_read_path_not_found",
            404,
          );
    const treeDirectory =
      selectedPathType === "tree"
        ? selectedPath
        : posix.dirname(selectedPath) === "."
          ? ""
          : posix.dirname(selectedPath);
    const treePath =
      treeDirectory === "" ? selectedObjectId : `${selectedObjectId}:${treeDirectory}`;
    const tree = yield* execute(
      "ForgeWebRead.tree",
      async () => {
        const output = await git(["ls-tree", "-l", "-z", treePath]);
        if (output.byteLength === 0) return [];
        return textDecoder
          .decode(output)
          .split("\0")
          .filter((line) => line !== "")
          .map((line) => {
            const separator = line.indexOf("\t");
            if (separator === -1) throw new Error("invalid tree record");
            const metadata = line.slice(0, separator).split(/\s+/u);
            const [mode, type, objectId, sizeText] = metadata;
            const name = line.slice(separator + 1);
            if (
              mode === undefined ||
              type === undefined ||
              objectId === undefined ||
              sizeText === undefined ||
              name === ""
            ) {
              throw new Error("invalid tree record");
            }
            const kind =
              mode === "160000"
                ? ("submodule" as const)
                : mode === "120000"
                  ? ("symlink" as const)
                  : type === "tree"
                    ? ("directory" as const)
                    : ("file" as const);
            return {
              kind,
              name,
              objectId,
              path: treeDirectory === "" ? name : `${treeDirectory}/${name}`,
              size: sizeText === "-" ? 0 : parseInteger(sizeText),
            };
          });
      },
      "forge_web_read_path_not_found",
      404,
    );

    const readFile = async (
      filePath: string,
      maximumTextBytes: number,
      maximumImageBytes: number,
    ) => {
      const objectId = text(
        await git(["rev-parse", "--verify", `${selectedObjectId}:${filePath}`]),
      );
      const byteSize = parseInteger(text(await git(["cat-file", "-s", objectId])));
      const mimeType = imageMimeType(filePath);
      if (mimeType !== undefined) {
        return byteSize <= maximumImageBytes
          ? ForgeWebReadFile.cases.image.make({
              byteSize,
              mimeType,
              objectId,
              path: filePath,
              sourceUrl: sameOriginAssetUrl({
                commitId: selectedObjectId,
                maxImageBytes: maximumImageBytes,
                objectId,
                owner: request.owner,
                path: filePath,
                repo: request.repo,
              }),
            })
          : ForgeWebReadFile.cases.refusal.make({
              byteSize,
              objectId,
              path: filePath,
              reason: "too_large",
            });
      }
      if (byteSize > maximumTextBytes) {
        return ForgeWebReadFile.cases.refusal.make({
          byteSize,
          objectId,
          path: filePath,
          reason: "too_large",
        });
      }
      const contentBytes = await git(["cat-file", "blob", objectId], maximumTextBytes + 1);
      if (contentBytes.includes(0)) {
        return ForgeWebReadFile.cases.refusal.make({
          byteSize,
          objectId,
          path: filePath,
          reason: "binary",
        });
      }
      const content = textDecoder.decode(contentBytes);
      if (/^README(?:\.[^.]+)?$/iu.test(posix.basename(filePath)) || /\.mdx?$/iu.test(filePath)) {
        const assets = (
          await Promise.all(
            markdownImageReferences(content).map(async (reference) => {
              if (
                reference.startsWith("/") ||
                reference.startsWith("#") ||
                /^[a-z][a-z0-9+.-]*:/iu.test(reference)
              ) {
                return undefined;
              }
              let decodedReference: string;
              try {
                decodedReference = decodeURIComponent(reference.split(/[?#]/u, 1)[0] ?? "");
              } catch {
                return undefined;
              }
              const resolvedPath = posix.normalize(
                posix.join(posix.dirname(filePath), decodedReference),
              );
              const assetMimeType = imageMimeType(resolvedPath);
              if (
                resolvedPath === "" ||
                resolvedPath.startsWith("../") ||
                resolvedPath.startsWith("/") ||
                !safeRepositoryPath(resolvedPath) ||
                assetMimeType === undefined
              ) {
                return undefined;
              }
              try {
                const assetObjectId = text(
                  await git(["rev-parse", "--verify", `${selectedObjectId}:${resolvedPath}`]),
                );
                const assetByteSize = parseInteger(
                  text(await git(["cat-file", "-s", assetObjectId])),
                );
                if (assetByteSize > maximumImageBytes) return undefined;
                return {
                  mimeType: assetMimeType,
                  path: resolvedPath,
                  sourceUrl: sameOriginAssetUrl({
                    commitId: selectedObjectId,
                    maxImageBytes: maximumImageBytes,
                    objectId: assetObjectId,
                    owner: request.owner,
                    path: resolvedPath,
                    repo: request.repo,
                  }),
                };
              } catch {
                return undefined;
              }
            }),
          )
        ).filter((asset) => asset !== undefined);
        return ForgeWebReadFile.cases.markdown.make({
          assets,
          byteSize,
          content,
          objectId,
          path: filePath,
        });
      }
      const language = languageFor(filePath);
      return ForgeWebReadFile.cases.text.make({
        byteSize,
        content,
        ...(language === undefined ? {} : { language }),
        objectId,
        path: filePath,
      });
    };

    const file =
      selectedPath === "" || selectedPathType === "tree"
        ? null
        : yield* execute(
            "ForgeWebRead.file",
            () => readFile(selectedPath, request.maxTextBytes, request.maxImageBytes),
            "forge_web_read_path_not_found",
            404,
          ).pipe(
            Effect.catchTag("ForgeGitWebReadError", (error) =>
              error.status === 404 ? Effect.succeed(null) : Effect.fail(error),
            ),
          );

    const readme = yield* execute("ForgeWebRead.readme", async () => {
      const candidates = await Promise.all(
        ["README.md", "README.mdx", "README", "readme.md"].map(async (candidate) => {
          try {
            const candidateFile = await readFile(
              candidate,
              request.maxTextBytes,
              request.maxImageBytes,
            );
            return ForgeWebReadFile.guards.markdown(candidateFile) ? candidateFile : null;
          } catch {
            return null;
          }
        }),
      );
      return candidates.find((candidate) => candidate !== null) ?? null;
    });

    const detailObjectId =
      request.commit === undefined
        ? selectedObjectId
        : yield* execute(
            "ForgeWebRead.resolveCommit",
            async () => text(await git(["rev-parse", "--verify", `${request.commit}^{commit}`])),
            "forge_web_read_revision_not_found",
            404,
          );
    const commit =
      request.view === "commit" || request.view === "diff"
        ? yield* execute("ForgeWebRead.commit", async () => ({
            ...(await commitSummary(detailObjectId)),
            body: text(await git(["show", "-s", "--format=%B", detailObjectId])),
          }))
        : null;

    const diff =
      request.view === "diff"
        ? yield* execute("ForgeWebRead.diff", async () => {
            const baseObjectId =
              request.base === undefined
                ? (commit?.parentIds[0] ?? detailObjectId)
                : text(await git(["rev-parse", "--verify", `${request.base}^{commit}`]));
            const unified = text(
              await git(
                ["diff", "--no-ext-diff", "--unified=3", baseObjectId, detailObjectId],
                Math.min(request.maxDiffBytes + 1, maximumCommandBytes),
              ),
            );
            const bounded = limitUtf8(unified, request.maxDiffBytes);
            return {
              baseObjectId,
              headObjectId: detailObjectId,
              truncated: bounded.truncated,
              unified: bounded.text,
            };
          })
        : null;

    const description = yield* execute("ForgeWebRead.description", async () => {
      try {
        return text(await git(["config", "--get", "openagents.description"]));
      } catch {
        return "";
      }
    });
    const nip34Coordinate = yield* execute(
      "ForgeWebRead.nip34Coordinate",
      async () => {
        const coordinate = text(await git(["config", "--get", "openagents.nip34Coordinate"]));
        if (!/^30617:[0-9a-f]{64}:.+$/u.test(coordinate)) {
          throw new Error("repository NIP-34 coordinate is invalid");
        }
        return coordinate;
      },
      "forge_web_read_metadata_unavailable",
      503,
    );
    const servedAt = new Date().toISOString();
    return ForgeWebReadProjection.make({
      access: input.policy.access,
      commit,
      commits,
      diff,
      file,
      readme,
      refs,
      repository: {
        authorityMode: "openagents_git_authoritative",
        canonicalCloneUrl: `https://openagents.com/git/${encodeURIComponent(request.owner)}/${encodeURIComponent(request.repo)}.git`,
        defaultBranch,
        description,
        maintainers:
          input.policy.access.mode === "public_web_read" ? [] : input.policy.repository.maintainers,
        name: request.repo,
        nip34Coordinate,
        owner: request.owner,
        projectionFreshness: servedAt,
        publicWebRead: input.policy.repository.publicWebRead,
        repositoryRef: `${request.owner}/${request.repo}`,
      },
      schema: ForgeWebReadSchemaVersion,
      selectedPath,
      selectedRef,
      servedAt,
      tree,
    });
  });
  return ForgeWebRead.of({ read, readAsset });
};

export const makeForgeWebReadLayer = (
  configuration: ForgeGitConfigurationShape,
): Layer.Layer<ForgeWebRead> => Layer.succeed(ForgeWebRead, makeWebRead(configuration));

export const layerForgeWebRead = Layer.effect(
  ForgeWebRead,
  Effect.gen(function* () {
    const configuration = yield* ForgeGitConfiguration;
    return makeWebRead(configuration);
  }),
);
