/**
 * The demo WASM plugin host for `openagents coder`.
 *
 * This is the one-off walking demo ahead of the plugin walking skeleton
 * (OpenAgentsInc/openagents#26): load a manifest, verify the artifact digest,
 * instantiate a pure-compute WASM module, call `handle_packet(bytes) -> bytes`
 * under a timeout, and surface the whole thing to the model as a session
 * tool. `docs/plugins/2026-08-24-coder-plugin-demo-shape.md` records what the
 * real skeleton keeps and what it replaces.
 *
 * The contract, in miniature:
 *
 * - **Manifest first.** Identity, artifact digest, typed input and output
 *   schemas, and capability declarations. Absence of a capability means
 *   denial; this demo host accepts only the empty declaration — no mounts,
 *   no hosts — because it implements no host imports at all.
 * - **Digest before load.** The artifact's SHA-256 is compared to the
 *   manifest's pin before the module is compiled. A mismatch is a refusal,
 *   not a warning.
 * - **Pure compute only.** The module's import list must be empty. A module
 *   that asks for imports is refused by inspection, before instantiation, so
 *   the sandbox is a property of what was loaded rather than a hope about
 *   what it does.
 * - **Timeout by termination.** A WASM call is synchronous and cannot be
 *   preempted in-process, so every invocation runs in a `worker_threads`
 *   worker the host terminates when the manifest's bound expires. A runaway
 *   guest costs its own worker and nothing else.
 * - **Typed refusals both ways.** The host refuses with `{code, reason}`;
 *   the guest returns `{"refusal": {...}}` inside its output packet. Both
 *   read as text to the model, which can act on a refusal and cannot act on
 *   a turn that died.
 */

import { createHash } from "node:crypto";
import { readFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { Worker } from "node:worker_threads";

import type { CoderTool } from "./coder-tools.js";

/** Why the host would not do what was asked. Never thrown; always returned. */
export interface PluginRefusal {
  readonly code:
    | "manifest_unreadable"
    | "manifest_invalid"
    | "artifact_unreadable"
    | "digest_mismatch"
    | "capabilities_unsupported"
    | "imports_declared"
    | "exports_missing"
    | "not_wasm"
    | "timeout"
    | "trap"
    | "bad_packet";
  readonly reason: string;
}

export const isRefusal = (value: unknown): value is PluginRefusal =>
  typeof value === "object" &&
  value !== null &&
  typeof (value as PluginRefusal).code === "string" &&
  typeof (value as PluginRefusal).reason === "string";

/** The manifest fields this host reads. The file may carry more. */
export interface PluginManifest {
  readonly name: string;
  readonly version: string;
  readonly description: string;
  readonly artifact: { readonly path: string; readonly digest: string };
  readonly abi: { readonly entry: string; readonly alloc: string };
  readonly interface: {
    readonly input: Record<string, unknown>;
    readonly output: Record<string, unknown>;
  };
  readonly capabilities: {
    readonly mounts: ReadonlyArray<unknown>;
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
}

/** Ceiling on the manifest's own timeout, so a manifest cannot ask for an hour. */
const TIMEOUT_CEILING_MS = 30_000;

/** How much plugin output the model is shown. */
const PLUGIN_OUTPUT_LIMIT = 16_000;

const refuse = (code: PluginRefusal["code"], reason: string): PluginRefusal => ({ code, reason });

/**
 * Load a plugin from its manifest: parse, validate, verify the digest, and
 * prove by inspection that the module is pure compute with the declared ABI.
 *
 * Everything that can be checked before the first invocation is checked here,
 * so `/plugin load` either says exactly what is wrong or hands back a plugin
 * whose next failure can only be about the packet.
 */
export function loadPluginFromManifest(manifestPath: string): LoadedPlugin | PluginRefusal {
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

  // This host implements no imports, so the only capability set it can
  // enforce is the empty one. Declared-but-denied, never declared-and-ignored.
  if (manifest.capabilities.mounts.length > 0 || manifest.capabilities.hosts.length > 0) {
    return refuse(
      "capabilities_unsupported",
      "this host runs pure computation only: the manifest declares mounts or hosts, " +
        "and there are no host imports to grant them through",
    );
  }

  let wasm: Uint8Array<ArrayBuffer>;
  const artifactPath = resolve(dirname(manifestPath), manifest.artifact.path);
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

  let module: WebAssembly.Module;
  try {
    module = new WebAssembly.Module(wasm);
  } catch (cause) {
    return refuse("not_wasm", cause instanceof Error ? cause.message : String(cause));
  }

  const imports = WebAssembly.Module.imports(module);
  if (imports.length > 0) {
    const named = imports.map((entry) => `${entry.module}.${entry.name}`).join(", ");
    return refuse(
      "imports_declared",
      `the module asks for host imports (${named}); this host instantiates with none`,
    );
  }

  const exports = new Set(WebAssembly.Module.exports(module).map((entry) => entry.name));
  for (const name of [manifest.abi.entry, manifest.abi.alloc, "memory"]) {
    if (!exports.has(name)) {
      return refuse("exports_missing", `the module does not export \`${name}\``);
    }
  }

  return { manifest, wasm, digest };
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
    typeof abi["entry"] !== "string" ||
    typeof abi["alloc"] !== "string"
  ) {
    return bad("`abi` (`entry` and `alloc` export names)");
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

  return {
    name,
    version,
    description,
    artifact: { path: artifact["path"], digest: artifact["digest"] },
    abi: { entry: abi["entry"], alloc: abi["alloc"] },
    interface: {
      input: iface["input"] as Record<string, unknown>,
      output: iface["output"] as Record<string, unknown>,
    },
    capabilities: {
      mounts: capabilities["mounts"],
      hosts: capabilities["hosts"],
      timeout_ms: Math.min(capabilities["timeout_ms"], TIMEOUT_CEILING_MS),
    },
  };
}

/**
 * The invocation worker, as source.
 *
 * A string rather than a file because the worker is part of this module's
 * contract, and a path into `dist/` breaks the moment tests run from source.
 * The worker instantiates the already-verified bytes with an empty import
 * object, copies the packet in through the guest's allocator, calls the entry,
 * and posts the output packet back. Anything the guest does wrong — a trap, an
 * out-of-range packet — comes back as a message, and anything it does forever
 * is ended by the host's timer terminating the whole worker.
 */
const INVOKE_WORKER = `
const { parentPort, workerData } = require("node:worker_threads");
(async () => {
  const { wasm, input, entry, alloc } = workerData;
  try {
    const { instance } = await WebAssembly.instantiate(wasm, {});
    const call = instance.exports[entry];
    const reserve = instance.exports[alloc];
    const memory = instance.exports.memory;
    const ptr = reserve(input.length);
    new Uint8Array(memory.buffer).set(input, ptr);
    const packed = call(ptr, input.length);
    const outPtr = Number(BigInt(packed) >> 32n);
    const outLen = Number(BigInt(packed) & 0xffffffffn);
    // Re-read the buffer: the call may have grown memory, detaching the old view.
    const view = new Uint8Array(memory.buffer);
    if (outPtr + outLen > view.length) {
      parentPort.postMessage({ trap: "the output packet points outside guest memory" });
      return;
    }
    parentPort.postMessage({ output: view.slice(outPtr, outPtr + outLen) });
  } catch (cause) {
    parentPort.postMessage({ trap: cause instanceof Error ? cause.message : String(cause) });
  }
})();
`;

/**
 * Call the plugin once: packet bytes in, packet bytes out, or a refusal.
 *
 * One worker per invocation. That costs a few milliseconds of instantiation
 * and buys the two properties the contract cares about: the timeout is
 * enforceable against a guest that never returns, and no state survives from
 * one call to the next, so every invocation runs on memory the previous one
 * cannot have corrupted.
 */
export function invokePlugin(
  plugin: LoadedPlugin,
  input: Uint8Array,
  options?: { readonly timeoutMs?: number | undefined },
): Promise<Uint8Array | PluginRefusal> {
  const timeoutMs = options?.timeoutMs ?? plugin.manifest.capabilities.timeout_ms;

  return new Promise((settle) => {
    const worker = new Worker(INVOKE_WORKER, {
      eval: true,
      workerData: {
        wasm: plugin.wasm,
        input,
        entry: plugin.manifest.abi.entry,
        alloc: plugin.manifest.abi.alloc,
      },
    });

    let done = false;
    const finish = (outcome: Uint8Array | PluginRefusal) => {
      if (done) return;
      done = true;
      clearTimeout(timer);
      void worker.terminate();
      settle(outcome);
    };

    const timer = setTimeout(() => {
      finish(
        refuse(
          "timeout",
          `the plugin did not answer within ${String(timeoutMs)}ms, the bound its manifest declares, ` +
            "and its worker was terminated",
        ),
      );
    }, timeoutMs);

    worker.on("message", (message: { output?: Uint8Array; trap?: string }) => {
      if (message.output !== undefined) finish(new Uint8Array(message.output));
      else finish(refuse("trap", message.trap ?? "the plugin trapped without a message"));
    });
    worker.on("error", (cause) => {
      finish(refuse("trap", cause.message));
    });
    worker.on("exit", (code) => {
      if (!done && code !== 0)
        finish(refuse("trap", `the plugin worker exited with code ${String(code)}`));
    });
  });
}

/**
 * The tool a loaded plugin materializes for the session.
 *
 * The manifest is the whole declaration: its name is the tool name, its
 * description is what the model reads, its input schema is the parameters.
 * `run` is the marshalling layer — arguments to a JSON packet, packet to the
 * plugin, output packet back as text — and every host refusal is a sentence
 * the model can act on rather than an exception the turn dies of.
 */
export function pluginTool(plugin: LoadedPlugin): CoderTool {
  const { manifest } = plugin;
  return {
    name: manifest.name,
    description:
      `${manifest.description}\n\n` +
      `Experimental WASM plugin \`${manifest.name}\` v${manifest.version}, loaded for this ` +
      `session only (${plugin.digest.slice(0, 19)}…). It runs sandboxed pure computation: no ` +
      "file, network, or environment access. The result is a JSON object with either `ok` or " +
      "`refusal`.",
    parameters: manifest.interface.input,
    run: async (args) => {
      const packet = new TextEncoder().encode(JSON.stringify(args));
      const outcome = await invokePlugin(plugin, packet);
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

/** What `/plugin load` reports, for a notice or a plain line. */
export function describeLoad(outcome: LoadedPlugin | PluginRefusal): string {
  if (isRefusal(outcome)) {
    return `Plugin not loaded (${outcome.code}): ${outcome.reason}`;
  }
  const { manifest } = outcome;
  return (
    `Loaded plugin \`${manifest.name}\` v${manifest.version} — digest verified ` +
    `(${outcome.digest.slice(0, 19)}…, ${String(outcome.wasm.length)} bytes, pure compute, ` +
    `${String(manifest.capabilities.timeout_ms)}ms bound). The \`${manifest.name}\` tool is ` +
    "declared to the model for this session. Experimental."
  );
}
