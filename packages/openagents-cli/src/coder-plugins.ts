/**
 * The WASM plugin host for `openagents coder`.
 *
 * The walking skeleton for OpenAgentsInc/openagents#26: load a manifest,
 * verify the artifact digest, prove by inspection that the module asks for
 * exactly the capabilities its manifest declares, and invoke
 * `handle_packet(bytes) -> bytes` through the engine seam under the
 * declared limits. `docs/plugins/2026-08-24-coder-plugin-demo-shape.md`
 * records the demo this grew from.
 *
 * The contract:
 *
 * - **Manifest first.** Identity, artifact digest pin, the `packet-v0` ABI
 *   declaration, typed input and output schemas, and capability
 *   declarations. Absence of a capability means denial.
 * - **Digest before load.** The artifact's SHA-256 is compared to the
 *   manifest's pin before the module is compiled. A mismatch is a refusal,
 *   not a warning.
 * - **Imports must be declared.** A module's import list must be covered by
 *   the capabilities its manifest declares: nothing for pure compute, and
 *   exactly `openagents.read_file` when the manifest declares read-only
 *   mounts. Anything else is refused by inspection, before instantiation,
 *   so the sandbox is a property of what was loaded rather than a hope
 *   about what it does.
 * - **Mounts are read-only and confined.** A declared mount resolves to a
 *   real directory at load; at invocation the engine's `read_file` import
 *   canonicalizes every path, refuses absolute paths, `..` escapes, and
 *   symlinks, and bounds the bytes per file.
 * - **Limits are the engine's job.** Timeout by termination, cancellation
 *   the same way. See {@link PluginEngine} in `coder-plugin-engine.ts`.
 * - **Typed refusals both ways.** The host refuses with `{code, reason}`;
 *   the guest returns `{"refusal": {...}}` inside its output packet. Both
 *   read as text to the model, which can act on a refusal and cannot act
 *   on a turn that died.
 */

import { createHash } from "node:crypto";
import { readFileSync, realpathSync, statSync } from "node:fs";
import { dirname, resolve } from "node:path";

import {
  defaultEngine,
  isRefusal,
  refuse,
  type PluginEngine,
  type PluginRefusal,
} from "./coder-plugin-engine.js";
import type { CoderTool } from "./coder-tools.js";

export { isRefusal, type PluginEngine, type PluginRefusal } from "./coder-plugin-engine.js";

/** The one packet ABI this host speaks. The manifest must declare it. */
export const SUPPORTED_ABI = "packet-v0";

/** A read-only directory grant, as the manifest declares it. */
export interface PluginMount {
  /** Directory path, resolved relative to the manifest's directory. */
  readonly path: string;
  /** Only `true` is accepted; a writable mount is refused, not downgraded. */
  readonly readonly: true;
}

/** The manifest fields this host reads. The file may carry more. */
export interface PluginManifest {
  readonly name: string;
  readonly version: string;
  readonly description: string;
  readonly artifact: { readonly path: string; readonly digest: string };
  readonly abi: { readonly kind: string; readonly entry: string; readonly alloc: string };
  readonly interface: {
    readonly input: Record<string, unknown>;
    readonly output: Record<string, unknown>;
  };
  readonly capabilities: {
    readonly mounts: ReadonlyArray<PluginMount>;
    readonly hosts: ReadonlyArray<unknown>;
    readonly timeout_ms: number;
  };
}

/** A plugin that passed every check and is ready to invoke. */
export interface LoadedPlugin {
  readonly manifest: PluginManifest;
  /** The artifact bytes, held so an invocation cannot race a file rewrite. */
  readonly wasm: Uint8Array;
  /** The verified digest, `sha256:<hex>`, for receipts and notices. */
  readonly digest: string;
  /** Declared mounts, resolved to realpath'd absolute directory roots. */
  readonly mounts: ReadonlyArray<string>;
}

/** Ceiling on the manifest's own timeout, so a manifest cannot ask for an hour. */
const TIMEOUT_CEILING_MS = 30_000;

/** Per-file byte bound for reads through a mount. */
export const MOUNT_FILE_LIMIT = 1_048_576;

/** How much plugin output the model is shown. */
const PLUGIN_OUTPUT_LIMIT = 16_000;

/**
 * Load a plugin from its manifest: parse, validate, verify the digest, and
 * prove by inspection that the module's imports are covered by its declared
 * capabilities.
 *
 * Everything that can be checked before the first invocation is checked
 * here, so `/plugin load` either says exactly what is wrong or hands back a
 * plugin whose next failure can only be about the packet.
 */
export function loadPluginFromManifest(
  manifestPath: string,
  engine: PluginEngine = defaultEngine,
): LoadedPlugin | PluginRefusal {
  let raw: string;
  try {
    raw = readFileSync(manifestPath, "utf8");
  } catch (cause) {
    return refuse("manifest_unreadable", cause instanceof Error ? cause.message : String(cause));
  }

  let parsed: unknown;
  try {
    parsed = JSON.parse(raw);
  } catch {
    return refuse("manifest_invalid", `${manifestPath} is not JSON`);
  }

  const manifest = validateManifest(parsed);
  if (isRefusal(manifest)) return manifest;

  // The only host capability that exists is the read-only mount. Anything
  // else is declared-but-denied, never declared-and-ignored.
  if (manifest.capabilities.hosts.length > 0) {
    return refuse(
      "capabilities_unsupported",
      "the manifest declares network hosts, and this host has no network capability to grant",
    );
  }

  const manifestDir = dirname(manifestPath);
  const mounts: string[] = [];
  for (const mount of manifest.capabilities.mounts) {
    const declared = resolve(manifestDir, mount.path);
    let root: string;
    try {
      root = realpathSync(declared);
      if (!statSync(root).isDirectory()) {
        return refuse("mount_invalid", `mount \`${mount.path}\` is not a directory`);
      }
    } catch {
      return refuse("mount_invalid", `mount \`${mount.path}\` does not resolve to a readable directory`);
    }
    mounts.push(root);
  }

  let wasm: Uint8Array<ArrayBuffer>;
  const artifactPath = resolve(manifestDir, manifest.artifact.path);
  try {
    // Copied out of the Buffer pool so the bytes sit on their own
    // ArrayBuffer, which both the compiler and the worker transfer want.
    wasm = Uint8Array.from(readFileSync(artifactPath));
  } catch (cause) {
    return refuse("artifact_unreadable", cause instanceof Error ? cause.message : String(cause));
  }

  const digest = `sha256:${createHash("sha256").update(wasm).digest("hex")}`;
  if (digest !== manifest.artifact.digest) {
    return refuse(
      "digest_mismatch",
      `the manifest pins ${manifest.artifact.digest} but ${manifest.artifact.path} is ${digest}; ` +
        "the artifact is not the one the manifest describes, so it does not load",
    );
  }

  const shape = engine.inspect(wasm);
  if (isRefusal(shape)) return shape;

  // Every import must be granted by a declared capability. Mounts grant
  // exactly one: the read_file capability import.
  const granted = new Set(mounts.length > 0 ? ["openagents.read_file"] : []);
  const undeclared = shape.imports.filter((name) => !granted.has(name));
  if (undeclared.length > 0) {
    const grantHint =
      mounts.length > 0
        ? "the declared mounts grant only `openagents.read_file`"
        : "the manifest declares no capabilities, so the module may import nothing";
    return refuse(
      "imports_undeclared",
      `the module asks for host imports its manifest does not declare (${undeclared.join(", ")}); ${grantHint}`,
    );
  }

  const exports = new Set(shape.exports);
  for (const name of [manifest.abi.entry, manifest.abi.alloc, "memory"]) {
    if (!exports.has(name)) {
      return refuse("exports_missing", `the module does not export \`${name}\``);
    }
  }

  return { manifest, wasm, digest, mounts };
}

function validateManifest(value: unknown): PluginManifest | PluginRefusal {
  const bad = (what: string): PluginRefusal =>
    refuse("manifest_invalid", `the manifest is missing or mistypes ${what}`);

  if (typeof value !== "object" || value === null) return bad("the top-level object");
  const record = value as Record<string, unknown>;

  const name = record["name"];
  if (typeof name !== "string" || !/^[a-z][a-z0-9_]{0,63}$/.test(name)) {
    return bad("`name` (lowercase identifier, it becomes the tool name)");
  }
  const version = record["version"];
  if (typeof version !== "string" || version.length === 0) return bad("`version`");
  const description = record["description"];
  if (typeof description !== "string" || description.length === 0) return bad("`description`");

  const artifact = record["artifact"] as Record<string, unknown> | undefined;
  if (
    typeof artifact !== "object" ||
    artifact === null ||
    typeof artifact["path"] !== "string" ||
    typeof artifact["digest"] !== "string" ||
    !artifact["digest"].startsWith("sha256:")
  ) {
    return bad("`artifact` (`path` and a `sha256:` `digest`)");
  }

  const abi = record["abi"] as Record<string, unknown> | undefined;
  if (
    typeof abi !== "object" ||
    abi === null ||
    typeof abi["kind"] !== "string" ||
    typeof abi["entry"] !== "string" ||
    typeof abi["alloc"] !== "string"
  ) {
    return bad("`abi` (`kind`, `entry`, and `alloc`)");
  }
  if (abi["kind"] !== SUPPORTED_ABI) {
    return refuse(
      "abi_unsupported",
      `the manifest declares abi \`${abi["kind"]}\` and this host speaks \`${SUPPORTED_ABI}\` only`,
    );
  }

  const iface = record["interface"] as Record<string, unknown> | undefined;
  if (
    typeof iface !== "object" ||
    iface === null ||
    typeof iface["input"] !== "object" ||
    iface["input"] === null ||
    typeof iface["output"] !== "object" ||
    iface["output"] === null
  ) {
    return bad("`interface` (`input` and `output` JSON schemas)");
  }

  const capabilities = record["capabilities"] as Record<string, unknown> | undefined;
  if (
    typeof capabilities !== "object" ||
    capabilities === null ||
    !Array.isArray(capabilities["mounts"]) ||
    !Array.isArray(capabilities["hosts"]) ||
    typeof capabilities["timeout_ms"] !== "number" ||
    capabilities["timeout_ms"] <= 0
  ) {
    return bad("`capabilities` (`mounts`, `hosts`, positive `timeout_ms`)");
  }

  const mounts: PluginMount[] = [];
  for (const entry of capabilities["mounts"]) {
    const mount = entry as Record<string, unknown> | null;
    if (
      typeof mount !== "object" ||
      mount === null ||
      typeof mount["path"] !== "string" ||
      mount["path"].length === 0
    ) {
      return bad("`capabilities.mounts[]` (each mount needs a `path`)");
    }
    if (mount["readonly"] !== true) {
      // Writable mounts are a capability this host does not have. Refusing
      // here keeps "declared means enforced" honest.
      return refuse(
        "capabilities_unsupported",
        `mount \`${mount["path"]}\` is not marked \`"readonly": true\`; only read-only mounts exist`,
      );
    }
    mounts.push({ path: mount["path"], readonly: true });
  }

  return {
    name,
    version,
    description,
    artifact: { path: artifact["path"], digest: artifact["digest"] },
    abi: { kind: abi["kind"], entry: abi["entry"], alloc: abi["alloc"] },
    interface: {
      input: iface["input"] as Record<string, unknown>,
      output: iface["output"] as Record<string, unknown>,
    },
    capabilities: {
      mounts,
      hosts: capabilities["hosts"],
      timeout_ms: Math.min(capabilities["timeout_ms"], TIMEOUT_CEILING_MS),
    },
  };
}

/**
 * Call the plugin once: packet bytes in, packet bytes out, or a refusal.
 * The engine owns instantiation, the capability imports, and the limits.
 */
export function invokePlugin(
  plugin: LoadedPlugin,
  input: Uint8Array,
  options?: {
    readonly timeoutMs?: number | undefined;
    readonly signal?: AbortSignal | undefined;
    readonly engine?: PluginEngine | undefined;
  },
): Promise<Uint8Array | PluginRefusal> {
  const engine = options?.engine ?? defaultEngine;
  return engine.invoke({
    wasm: plugin.wasm,
    entry: plugin.manifest.abi.entry,
    alloc: plugin.manifest.abi.alloc,
    input,
    timeoutMs: options?.timeoutMs ?? plugin.manifest.capabilities.timeout_ms,
    mounts: plugin.mounts,
    mountFileLimit: MOUNT_FILE_LIMIT,
    signal: options?.signal,
  });
}

/** One sentence describing what the plugin can reach, for the model. */
const reachDescription = (plugin: LoadedPlugin): string =>
  plugin.mounts.length > 0
    ? `It runs sandboxed with read-only access to ${String(plugin.mounts.length)} mounted ` +
      "director" +
      (plugin.mounts.length === 1 ? "y" : "ies") +
      "; no writes, no network, no environment access."
    : "It runs sandboxed pure computation: no file, network, or environment access.";

/**
 * The tool a loaded plugin materializes for the session.
 *
 * The manifest is the whole declaration: its name is the tool name, its
 * description is what the model reads, its input schema is the parameters.
 * `run` is the marshalling layer — arguments to a JSON packet, packet to the
 * plugin, output packet back as text — and every host refusal is a sentence
 * the model can act on rather than an exception the turn dies of.
 */
export function pluginTool(plugin: LoadedPlugin, engine?: PluginEngine): CoderTool {
  const { manifest } = plugin;
  return {
    name: manifest.name,
    description:
      `${manifest.description}\n\n` +
      `Experimental WASM plugin \`${manifest.name}\` v${manifest.version}, loaded for this ` +
      `session only (${plugin.digest.slice(0, 19)}…). ${reachDescription(plugin)} The result ` +
      "is a JSON object with either `ok` or `refusal`.",
    parameters: manifest.interface.input,
    run: async (args, signal) => {
      const packet = new TextEncoder().encode(JSON.stringify(args));
      const outcome = await invokePlugin(plugin, packet, { signal, engine });
      if (isRefusal(outcome)) {
        return `The plugin refused (${outcome.code}): ${outcome.reason}`;
      }
      let text: string;
      try {
        text = new TextDecoder("utf-8", { fatal: true }).decode(outcome);
      } catch {
        return `The plugin refused (bad_packet): the output packet is not UTF-8 (${String(outcome.length)} bytes)`;
      }
      return text.length <= PLUGIN_OUTPUT_LIMIT
        ? text
        : `${text.slice(0, PLUGIN_OUTPUT_LIMIT)}\n…[truncated]`;
    },
  };
}

/**
 * The loaded plugin's identity, for provenance records.
 *
 * Everything a receipt or a trajectory needs to say which exact artifact ran:
 * the digest here is the full `sha256:<hex>` the host verified, never the
 * truncated form the prose notices show.
 */
export interface PluginIdentity {
  readonly name: string;
  readonly version: string;
  /** The verified digest, full `sha256:<hex>`. */
  readonly artifactDigest: string;
  /** The artifact's size in bytes. */
  readonly bytes: number;
  readonly abi: { readonly entry: string; readonly alloc: string };
  readonly timeoutMs: number;
  readonly capabilities: {
    readonly mounts: ReadonlyArray<unknown>;
    readonly hosts: ReadonlyArray<unknown>;
  };
  /** The tool the plugin materializes, which is the manifest's name. */
  readonly toolName: string;
}

/** Read a loaded plugin's identity, as a provenance record understands it. */
export function pluginIdentity(plugin: LoadedPlugin): PluginIdentity {
  const { manifest } = plugin;
  return {
    name: manifest.name,
    version: manifest.version,
    artifactDigest: plugin.digest,
    bytes: plugin.wasm.length,
    abi: { entry: manifest.abi.entry, alloc: manifest.abi.alloc },
    timeoutMs: manifest.capabilities.timeout_ms,
    capabilities: {
      mounts: manifest.capabilities.mounts,
      hosts: manifest.capabilities.hosts,
    },
    toolName: manifest.name,
  };
}

/** What `/plugin load` reports, for a notice or a plain line. */
export function describeLoad(outcome: LoadedPlugin | PluginRefusal): string {
  if (isRefusal(outcome)) {
    return `Plugin not loaded (${outcome.code}): ${outcome.reason}`;
  }
  const { manifest } = outcome;
  const reach =
    outcome.mounts.length > 0
      ? `${String(outcome.mounts.length)} read-only mount${outcome.mounts.length === 1 ? "" : "s"}`
      : "pure compute";
  return (
    `Loaded plugin \`${manifest.name}\` v${manifest.version} — digest verified ` +
    `(${outcome.digest.slice(0, 19)}…, ${String(outcome.wasm.length)} bytes, ${reach}, ` +
    `${String(manifest.capabilities.timeout_ms)}ms bound). The \`${manifest.name}\` tool is ` +
    "declared to the model for this session. Experimental."
  );
}
