import { constants as fsConstants } from "node:fs"
import {
  access,
  lstat,
  open,
  readdir,
  realpath,
  stat,
} from "node:fs/promises"
import { basename, isAbsolute, relative, resolve, sep } from "node:path"

import { Schema as S } from "effect"

import {
  KHALA_CODE_EDITOR_DEFAULT_MAX_FILE_BYTES,
  KhalaCodeEditorDirectoryReadResult,
  KhalaCodeEditorError,
  type KhalaCodeEditorErrorCode,
  type KhalaCodeEditorDirectoryReadRequest,
  type KhalaCodeEditorDirectoryReadResult as KhalaCodeEditorDirectoryReadResultValue,
  type KhalaCodeEditorFileReadRequest,
  type KhalaCodeEditorFileReadResult,
  KhalaCodeEditorFileReadResult as KhalaCodeEditorFileReadResultSchema,
  type KhalaCodeEditorNodeKind,
  type KhalaCodeEditorProvider,
  KhalaCodeEditorProviderListResult,
  type KhalaCodeEditorProviderListResult as KhalaCodeEditorProviderListResultValue,
  type KhalaCodeEditorTreeNode,
  type KhalaCodeEditorWorkspaceReadResult,
  KhalaCodeEditorWorkspaceReadResult as KhalaCodeEditorWorkspaceReadResultSchema,
} from "../shared/editor.js"

type MaybePromise<T> = T | Promise<T>

export type KhalaCodeEditorFileService = {
  readonly providerList: () => MaybePromise<KhalaCodeEditorProviderListResultValue>
  readonly workspaceRead: () => MaybePromise<KhalaCodeEditorWorkspaceReadResult>
  readonly directoryRead: (
    request?: KhalaCodeEditorDirectoryReadRequest,
  ) => MaybePromise<KhalaCodeEditorDirectoryReadResultValue>
  readonly fileRead: (
    request: KhalaCodeEditorFileReadRequest,
  ) => MaybePromise<KhalaCodeEditorFileReadResult>
}

export type KhalaCodeEditorFileServiceOptions = {
  readonly providerId?: string
  readonly workingDirectory: string
}

const LOCAL_WORKSPACE_PROVIDER_ID = "local-workspace"

const decode = <A>(
  schema: S.Schema<A>,
  value: unknown,
): A => S.decodeUnknownSync(schema as never, { onExcessProperty: "error" })(value)

const editorError = (
  code: KhalaCodeEditorErrorCode,
  message: string,
  input: {
    readonly path?: string
    readonly providerId?: string
  } = {},
) => decode(KhalaCodeEditorError, {
  code,
  message,
  ...(input.path === undefined ? {} : { path: input.path }),
  ...(input.providerId === undefined ? {} : { providerId: input.providerId }),
})

const editorErrorInput = (
  path: string | undefined,
  providerId?: string,
): {
  readonly path?: string
  readonly providerId?: string
} => ({
  ...(path === undefined ? {} : { path }),
  ...(providerId === undefined ? {} : { providerId }),
})

const unavailable = (
  message: string,
  providerId: string,
): KhalaCodeEditorProviderListResultValue => decode(KhalaCodeEditorProviderListResult, {
  error: editorError("provider_unavailable", message, { providerId }),
  ok: false,
})

const isInsideOrEqual = (root: string, candidate: string): boolean => {
  const delta = relative(root, candidate)
  return delta === "" || (!delta.startsWith("..") && !isAbsolute(delta))
}

const safeRealpath = async (path: string): Promise<string> => {
  try {
    return await realpath(path)
  } catch {
    return resolve(path)
  }
}

const normalizeRoot = async (workingDirectory: string): Promise<string> => {
  const root = await safeRealpath(workingDirectory)
  await access(root, fsConstants.R_OK)
  const rootStat = await stat(root)
  if (!rootStat.isDirectory()) {
    throw new Error("Khala editor workspace root is not a directory.")
  }
  return root
}

const fileName = (path: string): string => basename(path) || path

const provider = (
  providerId: string,
  rootPath: string,
): KhalaCodeEditorProvider => ({
  capabilities: {
    read: true,
    watch: false,
    write: false,
  },
  kind: "local_workspace",
  label: fileName(rootPath),
  providerId,
  rootPath,
  status: "available",
})

const normalizeProviderId = (
  requestProviderId: string | undefined,
  providerId: string,
): KhalaCodeEditorError | null =>
  requestProviderId === undefined || requestProviderId === providerId
    ? null
    : editorError("provider_unavailable", "Requested editor provider is not available.", {
      providerId: requestProviderId,
    })

const resolveWorkspacePath = async (
  rootPath: string,
  requestPath: string | undefined,
): Promise<string | KhalaCodeEditorError> => {
  const trimmed = requestPath?.trim()
  const candidate = trimmed === undefined || trimmed.length === 0
    ? rootPath
    : resolve(isAbsolute(trimmed) ? trimmed : resolve(rootPath, trimmed))
  if (!isInsideOrEqual(rootPath, candidate)) {
    return editorError(
      "outside_workspace",
      "Path is outside the editor workspace.",
      editorErrorInput(requestPath),
    )
  }
  try {
    const resolved = await realpath(candidate)
    if (!isInsideOrEqual(rootPath, resolved)) {
      return editorError(
        "outside_workspace",
        "Path resolves outside the editor workspace.",
        editorErrorInput(requestPath),
      )
    }
  } catch (error) {
    const code = (error as { readonly code?: unknown }).code
    if (code === "ENOENT") {
      return editorError("not_found", "Path was not found.", editorErrorInput(requestPath))
    }
    return editorError(
      "unknown",
      error instanceof Error ? error.message : String(error),
      editorErrorInput(requestPath),
    )
  }
  return candidate
}

const nodeFromStat = (
  input: {
    readonly depth: number
    readonly kind: KhalaCodeEditorNodeKind
    readonly path: string
    readonly parentPath: string | null
    readonly providerId: string
    readonly rootPath: string
    readonly sizeBytes: number | null
    readonly mtimeMs: number | null
    readonly symlink: boolean
  },
): KhalaCodeEditorTreeNode => ({
  childrenLoaded: false,
  depth: input.depth,
  kind: input.kind,
  mtime: input.mtimeMs === null ? null : Math.trunc(input.mtimeMs),
  name: fileName(input.path),
  parentPath: input.parentPath,
  path: input.path,
  providerId: input.providerId,
  readonly: false,
  rootPath: input.rootPath,
  sizeBytes: input.sizeBytes,
  symlink: input.symlink,
})

const nodeKind = (stats: Awaited<ReturnType<typeof lstat>>): KhalaCodeEditorNodeKind =>
  stats.isSymbolicLink()
    ? "symlink"
    : stats.isDirectory()
      ? "directory"
      : "file"

const nodeForPath = async (
  path: string,
  input: {
    readonly depth: number
    readonly parentPath: string | null
    readonly providerId: string
    readonly rootPath: string
  },
): Promise<KhalaCodeEditorTreeNode> => {
  const stats = await lstat(path)
  const kind = nodeKind(stats)
  return nodeFromStat({
    ...input,
    kind,
    path,
    sizeBytes: kind === "file" ? stats.size : null,
    mtimeMs: stats.mtimeMs,
    symlink: stats.isSymbolicLink(),
  })
}

const depthFromRoot = (rootPath: string, path: string): number => {
  const delta = relative(rootPath, path)
  return delta === "" ? 0 : delta.split(sep).filter(Boolean).length
}

const compareNodes = (
  left: KhalaCodeEditorTreeNode,
  right: KhalaCodeEditorTreeNode,
): number => {
  if (left.kind === "directory" && right.kind !== "directory") return -1
  if (left.kind !== "directory" && right.kind === "directory") return 1
  return left.name.localeCompare(right.name, undefined, { sensitivity: "base" })
}

const isTextBuffer = (buffer: Buffer): boolean => {
  if (buffer.includes(0)) return false
  const inspected = buffer.subarray(0, Math.min(buffer.length, 8192))
  if (inspected.length === 0) return true
  let suspicious = 0
  for (const byte of inspected) {
    const allowedControl =
      byte === 7 ||
      byte === 8 ||
      byte === 9 ||
      byte === 10 ||
      byte === 12 ||
      byte === 13 ||
      byte === 27
    if (byte < 32 && !allowedControl) suspicious += 1
  }
  return suspicious / inspected.length < 0.1
}

export const createKhalaCodeEditorFileService = (
  options: KhalaCodeEditorFileServiceOptions,
): KhalaCodeEditorFileService => {
  const providerId = options.providerId ?? LOCAL_WORKSPACE_PROVIDER_ID
  let rootPathPromise: Promise<string> | undefined
  const rootPath = () => {
    rootPathPromise ??= normalizeRoot(options.workingDirectory)
    return rootPathPromise
  }

  const resolveRequest = async (
    requestProviderId: string | undefined,
    requestPath: string | undefined,
  ): Promise<{
    readonly error: KhalaCodeEditorError
  } | {
    readonly rootPath: string
    readonly path: string
  }> => {
    const providerError = normalizeProviderId(requestProviderId, providerId)
    if (providerError !== null) return { error: providerError }
    const root = await rootPath()
    const resolved = await resolveWorkspacePath(root, requestPath)
    return typeof resolved === "string"
      ? { path: resolved, rootPath: root }
      : { error: { ...resolved, providerId } }
  }

  return {
    async providerList() {
      try {
        const root = await rootPath()
        return decode(KhalaCodeEditorProviderListResult, {
          ok: true,
          providers: [provider(providerId, root)],
        })
      } catch (error) {
        return unavailable(error instanceof Error ? error.message : String(error), providerId)
      }
    },
    async workspaceRead() {
      try {
        const root = await rootPath()
        return decode(KhalaCodeEditorWorkspaceReadResultSchema, {
          ok: true,
          roots: [{
            label: fileName(root),
            path: root,
            providerId,
            readonly: false,
          }],
        })
      } catch (error) {
        return decode(KhalaCodeEditorWorkspaceReadResultSchema, {
          error: editorError("provider_unavailable", error instanceof Error ? error.message : String(error), {
            providerId,
          }),
          ok: false,
        })
      }
    },
    async directoryRead(request = {}) {
      try {
        const resolved = await resolveRequest(request.providerId, request.path)
        if ("error" in resolved) {
          return decode(KhalaCodeEditorDirectoryReadResult, {
            error: resolved.error,
            ok: false,
          })
        }
        const stats = await lstat(resolved.path)
        if (!stats.isDirectory()) {
          return decode(KhalaCodeEditorDirectoryReadResult, {
            error: editorError(
              "not_directory",
              "Path is not a directory.",
              editorErrorInput(request.path, providerId),
            ),
            ok: false,
          })
        }
        const depth = depthFromRoot(resolved.rootPath, resolved.path)
        const entries = await readdir(resolved.path, { withFileTypes: true })
        const children = await Promise.all(entries.map(async entry =>
          nodeForPath(resolve(resolved.path, entry.name), {
            depth: depth + 1,
            parentPath: resolved.path,
            providerId,
            rootPath: resolved.rootPath,
          }).catch(() => null)
        ))
        const node = await nodeForPath(resolved.path, {
          depth,
          parentPath: resolved.path === resolved.rootPath
            ? null
            : resolve(resolved.path, ".."),
          providerId,
          rootPath: resolved.rootPath,
        })
        return decode(KhalaCodeEditorDirectoryReadResult, {
          entries: children
            .filter((child): child is KhalaCodeEditorTreeNode => child !== null)
            .sort(compareNodes),
          node: { ...node, childrenLoaded: true },
          ok: true,
          providerId,
          rootPath: resolved.rootPath,
          truncated: false,
        })
      } catch (error) {
        return decode(KhalaCodeEditorDirectoryReadResult, {
          error: editorError(
            "provider_unavailable",
            error instanceof Error ? error.message : String(error),
            editorErrorInput(request.path, providerId),
          ),
          ok: false,
        })
      }
    },
    async fileRead(request) {
      try {
        const resolved = await resolveRequest(request.providerId, request.path)
        if ("error" in resolved) {
          return decode(KhalaCodeEditorFileReadResultSchema, {
            error: resolved.error,
            ok: false,
          })
        }
        const stats = await lstat(resolved.path)
        if (!stats.isFile()) {
          return decode(KhalaCodeEditorFileReadResultSchema, {
            error: editorError(
              "not_file",
              "Path is not a file.",
              editorErrorInput(request.path, providerId),
            ),
            ok: false,
          })
        }
        const maxBytes = request.maxBytes === undefined
          ? KHALA_CODE_EDITOR_DEFAULT_MAX_FILE_BYTES
          : Math.max(0, Math.trunc(request.maxBytes))
        if (stats.size > maxBytes) {
          return decode(KhalaCodeEditorFileReadResultSchema, {
            error: editorError(
              "file_too_large",
              "File is larger than the editor read cap.",
              editorErrorInput(request.path, providerId),
            ),
            ok: false,
          })
        }
        const handle = await open(resolved.path, "r")
        try {
          const buffer = Buffer.alloc(stats.size)
          await handle.read(buffer, 0, stats.size, 0)
          if (!isTextBuffer(buffer)) {
            return decode(KhalaCodeEditorFileReadResultSchema, {
              error: editorError(
                "binary_file",
                "Binary files are not rendered by the editor.",
                editorErrorInput(request.path, providerId),
              ),
              ok: false,
            })
          }
          return decode(KhalaCodeEditorFileReadResultSchema, {
            content: buffer.toString("utf8"),
            encoding: "utf8",
            mtime: Math.trunc(stats.mtimeMs),
            ok: true,
            path: resolved.path,
            providerId,
            rootPath: resolved.rootPath,
            sizeBytes: stats.size,
          })
        } finally {
          await handle.close()
        }
      } catch (error) {
        return decode(KhalaCodeEditorFileReadResultSchema, {
          error: editorError(
            "provider_unavailable",
            error instanceof Error ? error.message : String(error),
            editorErrorInput(request.path, providerId),
          ),
          ok: false,
        })
      }
    },
  }
}
