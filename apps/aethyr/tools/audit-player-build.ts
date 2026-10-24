import { brotliCompressSync, constants, gzipSync } from "node:zlib";
import { dirname, posix } from "node:path";

export interface PlayerBuildRecord {
  fileName: string;
  type: "chunk" | "asset";
  modules?: readonly string[];
  imports?: readonly string[];
  dynamicImports?: readonly string[];
  /** HTML dependencies, worker entrypoints, and other emitted resource references. */
  referencedFiles?: readonly string[];
  isEntry?: boolean;
}
export interface PlayerBuildSize {
  files: number;
  rawBytes: number;
  gzipBytes: number;
  brotliBytes: number;
}
export interface PlayerBuildAuditOptions {
  roots: readonly string[];
  records: readonly PlayerBuildRecord[];
  /** The exact served build, including cooked content, not only files visited by this check. */
  files: ReadonlyMap<string, Uint8Array>;
  /** Additional forbidden source patterns; the standard editor exclusions always apply. */
  forbiddenModules?: readonly RegExp[];
}
export interface PlayerBuildAudit {
  schema: "aethyrforge.player-build-audit.v1";
  ok: boolean;
  errors: string[];
  roots: string[];
  reachable: string[];
  unreachable: string[];
  modules: string[];
  requests: number;
  totals: Record<"js" | "wasm" | "css" | "assets" | "all", PlayerBuildSize>;
  /** Includes unreachable files because a served file is still part of delivery. */
  delivered: PlayerBuildSize;
}
const defaultForbidden = [
  /(?:^|\/)node_modules\/(?:\.pnpm\/[^/]+\/node_modules\/)?(?:react(?:-dom)?|scheduler|@effect\/atom(?:-react)?)\//,
  /(?:^|\/)apps\/editor\//,
  /(?:^|\/)packages\/editor-services\//,
  /(?:^|\/)packages\/renderer\/(?:mesh-inspection|mesh-view|material-editor)\.[cm]?[jt]sx?$/,
  /(?:^|\/)(?:agent-(?:client|contract|oracle)|project-authoring|authoring-permit|commands)\.[cm]?[jt]sx?$/,
  /(?:^|\/)tools\/assets\//,
];
const maxFiles = 32_768;
function fileName(value: unknown): value is string {
  return (
    typeof value === "string" &&
    value.length > 0 &&
    value.length <= 1024 &&
    !/[\\\x00-\x20%?#:]/.test(value) &&
    !value.startsWith("/") &&
    value
      .split("/")
      .every((part) => part !== "" && part !== "." && part !== "..")
  );
}
function resolveReference(
  from: string,
  reference: string,
  files: ReadonlyMap<string, Uint8Array>,
): string | undefined {
  if (
    typeof reference !== "string" ||
    /[\\\x00-\x20%?#:]/.test(reference) ||
    reference.startsWith("//")
  )
    return;
  const direct = reference.startsWith("/") ? reference.slice(1) : reference;
  if (fileName(direct) && files.has(direct)) return direct;
  const relative = posix.normalize(posix.join(dirname(from), reference));
  return fileName(relative) && files.has(relative) ? relative : undefined;
}
const size = (): PlayerBuildSize => ({
  files: 0,
  rawBytes: 0,
  gzipBytes: 0,
  brotliBytes: 0,
});
const add = (target: PlayerBuildSize, source: PlayerBuildSize) => {
  for (const key of ["files", "rawBytes", "gzipBytes", "brotliBytes"] as const)
    target[key] += source[key];
};
function measure(bytes: Uint8Array): PlayerBuildSize {
  return {
    files: 1,
    rawBytes: bytes.byteLength,
    gzipBytes: gzipSync(bytes, { level: 9, mtime: 0 } as Parameters<
      typeof gzipSync
    >[1]).byteLength,
    brotliBytes: brotliCompressSync(bytes, {
      params: { [constants.BROTLI_PARAM_QUALITY]: 6 },
    }).byteLength,
  };
}
const category = (name: string): "js" | "wasm" | "css" | "assets" =>
  /\.[cm]?js$/.test(name)
    ? "js"
    : name.endsWith(".wasm")
      ? "wasm"
      : name.endsWith(".css")
        ? "css"
        : "assets";

/** Audit emitted bytes and source ownership, including lazy imports and worker edges. */
export function auditPlayerBuild(
  options: PlayerBuildAuditOptions,
): PlayerBuildAudit {
  const errors: string[] = [];
  const roots = [...options.roots];
  const result: PlayerBuildAudit = {
    schema: "aethyrforge.player-build-audit.v1",
    ok: false,
    errors,
    roots,
    reachable: [],
    unreachable: [],
    modules: [],
    requests: 0,
    totals: {
      js: size(),
      wasm: size(),
      css: size(),
      assets: size(),
      all: size(),
    },
    delivered: size(),
  };
  if (
    !roots.length ||
    roots.length > 32 ||
    options.records.length > maxFiles ||
    options.files.size > maxFiles
  ) {
    errors.push(
      "Choose bounded emitted records, files, and at least one player root.",
    );
    return result;
  }
  const byName = new Map<string, PlayerBuildRecord>();
  for (const item of options.records) {
    if (!fileName(item.fileName) || !["chunk", "asset"].includes(item.type)) {
      errors.push(`Invalid emitted record: ${String(item.fileName)}.`);
      continue;
    }
    if (byName.has(item.fileName))
      errors.push(`Duplicate emitted record: ${item.fileName}.`);
    else byName.set(item.fileName, item);
  }
  const measured = new Map<string, PlayerBuildSize>();
  for (const [name, bytes] of options.files) {
    if (!fileName(name) || !(bytes instanceof Uint8Array)) {
      errors.push(`Invalid delivered file: ${name}.`);
      continue;
    }
    if (name.endsWith(".map"))
      errors.push(`Source maps must not be delivered: ${name}.`);
    const counts = measure(bytes);
    measured.set(name, counts);
    add(result.delivered, counts);
  }
  const visited = new Set<string>(),
    sourceModules = new Set<string>();
  const pending = [...roots];
  while (pending.length) {
    const name = pending.pop()!;
    if (!fileName(name)) {
      errors.push(`Invalid player root or output reference: ${name}.`);
      continue;
    }
    if (visited.has(name)) continue;
    visited.add(name);
    if (!options.files.has(name)) {
      errors.push(`Missing delivered file: ${name}.`);
      continue;
    }
    const item = byName.get(name);
    if (!item) {
      errors.push(`Missing emitted dependency record: ${name}.`);
      continue;
    }
    if (
      category(name) === "js" &&
      (!Array.isArray(item.modules) || item.modules.length === 0)
    )
      errors.push(`Missing JavaScript module provenance: ${name}.`);
    if (
      item.modules !== undefined &&
      (!Array.isArray(item.modules) ||
        item.modules.length > maxFiles ||
        item.modules.some(
          (value) => typeof value !== "string" || value.length > 4096,
        ))
    ) {
      errors.push(`Invalid source modules for ${name}.`);
    } else {
      for (const source of item.modules ?? []) {
        const normalized = source
          .replaceAll("\\", "/")
          .replace(/^\0+/, "")
          .split("?")[0];
        sourceModules.add(normalized);
        if (
          [...defaultForbidden, ...(options.forbiddenModules ?? [])].some(
            (pattern) => {
              pattern.lastIndex = 0;
              return pattern.test(normalized);
            },
          )
        )
          errors.push(
            `Forbidden player source module: ${normalized} (${name}).`,
          );
      }
    }
    for (const key of [
      "imports",
      "dynamicImports",
      "referencedFiles",
    ] as const) {
      const references = item[key] ?? [];
      if (!Array.isArray(references) || references.length > maxFiles) {
        errors.push(`Invalid ${key} for ${name}.`);
        continue;
      }
      for (const reference of references) {
        const resolved = resolveReference(name, reference, options.files);
        if (!resolved)
          errors.push(
            `Missing or external player dependency: ${name} → ${String(reference)}.`,
          );
        else pending.push(resolved);
      }
    }
  }
  result.reachable = [...visited]
    .filter((name) => options.files.has(name))
    .sort();
  result.unreachable = [...options.files.keys()]
    .filter((name) => !visited.has(name))
    .sort();
  result.modules = [...sourceModules].sort();
  result.requests = result.reachable.length;
  for (const name of result.reachable) {
    const counts = measured.get(name);
    if (counts) {
      add(result.totals[category(name)], counts);
      add(result.totals.all, counts);
    }
  }
  if (result.unreachable.length)
    errors.push(
      `Unreachable files remain in the delivered player: ${result.unreachable.join(", ")}.`,
    );
  for (const name of byName.keys())
    if (!options.files.has(name))
      errors.push(`Emitted record has no delivered bytes: ${name}.`);
  result.ok = errors.length === 0;
  return result;
}
