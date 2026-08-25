/**
 * The plugin engine seam.
 *
 * The tool layer in `coder-plugins.ts` owns the manifest, the digest pin,
 * and the marshalling; everything that actually touches a WASM runtime sits
 * behind {@link PluginEngine} so an engine with fuel metering, memory
 * ceilings, or WASI (wasmtime, an Extism-derived host) can replace the Node
 * default without the tool layer noticing. Two operations:
 *
 * - `inspect` — compile the artifact and report its import and export
 *   names, so the loader can prove by inspection that the module asks for
 *   exactly the capabilities its manifest declares, before anything runs.
 * - `invoke` — instantiate the verified bytes, feed one packet through the
 *   `packet-v0` entry under the declared limits, and settle with the output
 *   packet or a typed refusal. Never throws.
 *
 * The default engine keeps the demo's model: one `node:worker_threads`
 * worker per invocation, terminated at the timeout (a WASM call is
 * synchronous and cannot be preempted in-process) or on cancellation, so a
 * runaway guest costs its own worker and nothing else, and no state
 * survives between calls.
 *
 * Capability imports live host-side of this seam too. When the job carries
 * mounts, the worker exposes exactly two imports — `openagents.read_file`
 * and `openagents.list_dir` — and confines every path: relative to a
 * declared root only, `..` resolved and checked, symlinks refused, a
 * per-file size bound on reads, and a per-listing entry bound on listings.
 * The answer crosses back as a status-prefixed packet the PDK decodes:
 * `0x00` + bytes, or `0x01` + a `{"code", "reason"}` refusal.
 */

import { Worker } from "node:worker_threads";

/** Why the host would not do what was asked. Never thrown; always returned. */
export interface PluginRefusal {
  readonly code:
    | "manifest_unreadable"
    | "manifest_invalid"
    | "abi_unsupported"
    | "artifact_unreadable"
    | "digest_mismatch"
    | "capabilities_unsupported"
    | "mount_invalid"
    | "imports_undeclared"
    | "exports_missing"
    | "not_wasm"
    | "timeout"
    | "cancelled"
    | "trap"
    | "bad_packet"
    | "approval_unavailable"
    | "approval_refused";
  readonly reason: string;
}

export const isRefusal = (value: unknown): value is PluginRefusal =>
  typeof value === "object" &&
  value !== null &&
  typeof (value as PluginRefusal).code === "string" &&
  typeof (value as PluginRefusal).reason === "string";

export const refuse = (code: PluginRefusal["code"], reason: string): PluginRefusal => ({
  code,
  reason,
});

/** What a compiled module declares, before anything is instantiated. */
export interface ModuleShape {
  /** Import names as `module.name`, e.g. `openagents.read_file`. */
  readonly imports: ReadonlyArray<string>;
  readonly exports: ReadonlyArray<string>;
}

/** One packet through one instance, under limits. */
export interface EngineJob {
  readonly wasm: Uint8Array;
  /** The `packet-v0` entry export name (`handle_packet`). */
  readonly entry: string;
  /** The allocator export name (`packet_alloc`). */
  readonly alloc: string;
  /** The input packet. */
  readonly input: Uint8Array;
  /** Wall-clock bound; at expiry the instance is destroyed, not awaited. */
  readonly timeoutMs: number;
  /**
   * Resolved, realpath'd absolute directory roots the guest may read
   * through `openagents.read_file`. Empty means the import does not exist.
   */
  readonly mounts: ReadonlyArray<string>;
  /** Per-file byte bound for mounted reads. */
  readonly mountFileLimit: number;
  /** Per-listing entry bound for mounted directory listings. */
  readonly mountDirEntryLimit: number;
  /** Cancels the invocation the same way the timeout does. */
  readonly signal?: AbortSignal | undefined;
}

/**
 * A WASM engine the plugin host can run on. Implementations must enforce
 * the job's limits themselves — the tool layer never wraps an engine in a
 * timer, because an engine that cannot kill a runaway guest is not
 * enforcing anything.
 */
export interface PluginEngine {
  readonly name: string;
  inspect(wasm: Uint8Array): ModuleShape | PluginRefusal;
  invoke(job: EngineJob): Promise<Uint8Array | PluginRefusal>;
}

/**
 * The invocation worker, as source.
 *
 * A string rather than a file because the worker is part of this module's
 * contract, and a path into `dist/` breaks the moment tests run from
 * source. The worker instantiates the already-verified bytes, copies the
 * packet in through the guest's allocator, calls the entry, and posts the
 * output packet back. Anything the guest does wrong — a trap, an
 * out-of-range packet — comes back as a message, and anything it does
 * forever is ended by the host's timer terminating the whole worker.
 *
 * Mount confinement runs in here, synchronously, because a WASM import is
 * a synchronous call: lexical containment after resolving `..`, a symlink
 * refusal on the target, a realpath check against the realpath'd root so a
 * symlinked parent cannot smuggle the read out, and the size bound checked
 * before the bytes are touched.
 */
const INVOKE_WORKER = `
const { parentPort, workerData } = require("node:worker_threads");
const { lstatSync, readdirSync, readFileSync, realpathSync } = require("node:fs");
const { isAbsolute, join, resolve, sep } = require("node:path");
(async () => {
  const { wasm, input, entry, alloc, mounts, mountFileLimit, mountDirEntryLimit } = workerData;
  try {
    let guest = null;

    const refusalPacket = (code, reason) => {
      const body = new TextEncoder().encode(JSON.stringify({ code, reason }));
      const packet = new Uint8Array(body.length + 1);
      packet[0] = 1;
      packet.set(body, 1);
      return packet;
    };
    const okPacket = (bytes) => {
      const packet = new Uint8Array(bytes.length + 1);
      packet[0] = 0;
      packet.set(bytes, 1);
      return packet;
    };

    const readMounted = (path) => {
      if (isAbsolute(path)) {
        return refusalPacket("mount_denied", "absolute paths are refused; mounted paths are relative to a declared mount root");
      }
      for (const root of mounts) {
        const candidate = resolve(root, path);
        // Lexical confinement: resolve() has already applied "..", so a
        // candidate outside the root is an escape, not a file in it.
        if (candidate !== root && !candidate.startsWith(root + sep)) {
          return refusalPacket("mount_denied", "the path escapes the mount root");
        }
        let stat;
        try {
          stat = lstatSync(candidate);
        } catch {
          continue; // Not in this mount; try the next declared root.
        }
        if (stat.isSymbolicLink()) {
          return refusalPacket("mount_denied", "symlinks inside a mount are refused");
        }
        if (!stat.isFile()) {
          return refusalPacket("file_unreadable", "the path is not a regular file");
        }
        // A symlinked parent directory can still point outside; the real
        // path of the candidate must sit under the real path of the root.
        let real;
        try {
          real = realpathSync(candidate);
        } catch (cause) {
          return refusalPacket("file_unreadable", String((cause && cause.message) || cause));
        }
        if (real !== root && !real.startsWith(root + sep)) {
          return refusalPacket("mount_denied", "the path resolves outside the mount root");
        }
        if (stat.size > mountFileLimit) {
          return refusalPacket("file_too_large", "the file is " + String(stat.size) + " bytes; the per-file bound is " + String(mountFileLimit));
        }
        try {
          return okPacket(readFileSync(candidate));
        } catch (cause) {
          return refusalPacket("file_unreadable", String((cause && cause.message) || cause));
        }
      }
      return refusalPacket("mount_denied", "no declared mount contains the path");
    };

    // List one directory inside one declared mount, by mount index. The
    // index makes the target root explicit — a scanner over two mounts
    // (say ~/.claude and ~/.codex) must never have "which root answered?"
    // ambiguity for a listing. Same confinement as readMounted, plus an
    // entry bound instead of a byte bound.
    const listMounted = (mountIndex, path) => {
      if (!Number.isInteger(mountIndex) || mountIndex < 0 || mountIndex >= mounts.length) {
        return refusalPacket("mount_denied", "the mount index names no declared mount");
      }
      if (isAbsolute(path)) {
        return refusalPacket("mount_denied", "absolute paths are refused; mounted paths are relative to a declared mount root");
      }
      const root = mounts[mountIndex];
      const candidate = resolve(root, path);
      if (candidate !== root && !candidate.startsWith(root + sep)) {
        return refusalPacket("mount_denied", "the path escapes the mount root");
      }
      let stat;
      try {
        stat = lstatSync(candidate);
      } catch {
        return refusalPacket("file_unreadable", "the mount has no such directory");
      }
      if (stat.isSymbolicLink()) {
        return refusalPacket("mount_denied", "symlinks inside a mount are refused");
      }
      if (!stat.isDirectory()) {
        return refusalPacket("file_unreadable", "the path is not a directory");
      }
      let real;
      try {
        real = realpathSync(candidate);
      } catch (cause) {
        return refusalPacket("file_unreadable", String((cause && cause.message) || cause));
      }
      if (real !== root && !real.startsWith(root + sep)) {
        return refusalPacket("mount_denied", "the path resolves outside the mount root");
      }
      let names;
      try {
        names = readdirSync(candidate);
      } catch (cause) {
        return refusalPacket("file_unreadable", String((cause && cause.message) || cause));
      }
      names.sort();
      const truncated = names.length > mountDirEntryLimit;
      const entries = [];
      for (const name of names.slice(0, mountDirEntryLimit)) {
        let kind = "other";
        let size = 0;
        let mtimeMs = 0;
        try {
          const entryStat = lstatSync(join(candidate, name));
          kind = entryStat.isSymbolicLink()
            ? "symlink"
            : entryStat.isFile()
              ? "file"
              : entryStat.isDirectory()
                ? "dir"
                : "other";
          size = entryStat.size;
          mtimeMs = Math.floor(entryStat.mtimeMs);
        } catch {
          // A racing unlink between readdir and lstat: report the name as
          // "other" so the guest can skip it, rather than failing the listing.
        }
        entries.push({ name, kind, size, mtime_ms: mtimeMs });
      }
      return okPacket(new TextEncoder().encode(JSON.stringify({ entries, truncated })));
    };

    // Write an answer packet into guest memory through the guest's own
    // allocator and pack its location the way handle_packet does.
    const answerGuest = (packet) => {
      const ptr = guest[alloc](packet.length);
      new Uint8Array(guest.memory.buffer).set(packet, ptr);
      return (BigInt(ptr) << 32n) | BigInt(packet.length);
    };

    // The capability imports exist only when the manifest declared mounts;
    // the loader has already refused any module that asks for more.
    const guestPath = (pathPtr, pathLen) => {
      const memory = new Uint8Array(guest.memory.buffer);
      return new TextDecoder().decode(memory.slice(pathPtr, pathPtr + pathLen));
    };
    const imports =
      mounts.length > 0
        ? {
            openagents: {
              read_file: (pathPtr, pathLen) => {
                let packet;
                try {
                  packet = readMounted(guestPath(pathPtr, pathLen));
                } catch (cause) {
                  packet = refusalPacket("file_unreadable", String((cause && cause.message) || cause));
                }
                return answerGuest(packet);
              },
              list_dir: (mountIndex, pathPtr, pathLen) => {
                let packet;
                try {
                  packet = listMounted(mountIndex, guestPath(pathPtr, pathLen));
                } catch (cause) {
                  packet = refusalPacket("file_unreadable", String((cause && cause.message) || cause));
                }
                return answerGuest(packet);
              },
            },
          }
        : {};

    const { instance } = await WebAssembly.instantiate(wasm, imports);
    guest = instance.exports;
    const ptr = guest[alloc](input.length);
    new Uint8Array(guest.memory.buffer).set(input, ptr);
    const packed = guest[entry](ptr, input.length);
    const outPtr = Number(BigInt(packed) >> 32n);
    const outLen = Number(BigInt(packed) & 0xffffffffn);
    // Re-read the buffer: the call may have grown memory, detaching the old view.
    const view = new Uint8Array(guest.memory.buffer);
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
 * The default engine: plain `WebAssembly` in a worker per invocation.
 *
 * One worker per call costs a few milliseconds of instantiation and buys
 * the two properties the contract cares about: the timeout is enforceable
 * against a guest that never returns, and no state survives from one call
 * to the next, so every invocation runs on memory the previous one cannot
 * have corrupted. The manifest's `memory_max_mib` is declared but not
 * enforced here — rustc exports memory rather than importing it, so a
 * bounded host memory cannot be injected; a ceiling-enforcing engine slots
 * in through this same interface.
 */
export const nodeWorkerEngine: PluginEngine = {
  name: "node-worker",

  inspect(wasm) {
    let module: WebAssembly.Module;
    try {
      module = new WebAssembly.Module(wasm as Uint8Array<ArrayBuffer>);
    } catch (cause) {
      return refuse("not_wasm", cause instanceof Error ? cause.message : String(cause));
    }
    return {
      imports: WebAssembly.Module.imports(module).map((entry) => `${entry.module}.${entry.name}`),
      exports: WebAssembly.Module.exports(module).map((entry) => entry.name),
    };
  },

  invoke(job) {
    return new Promise((settle) => {
      const worker = new Worker(INVOKE_WORKER, {
        eval: true,
        workerData: {
          wasm: job.wasm,
          input: job.input,
          entry: job.entry,
          alloc: job.alloc,
          mounts: [...job.mounts],
          mountFileLimit: job.mountFileLimit,
          mountDirEntryLimit: job.mountDirEntryLimit,
        },
      });

      let done = false;
      const finish = (outcome: Uint8Array | PluginRefusal) => {
        if (done) return;
        done = true;
        clearTimeout(timer);
        job.signal?.removeEventListener("abort", onAbort);
        void worker.terminate();
        settle(outcome);
      };

      const timer = setTimeout(() => {
        finish(
          refuse(
            "timeout",
            `the plugin did not answer within ${String(job.timeoutMs)}ms, the bound its manifest declares, ` +
              "and its worker was terminated",
          ),
        );
      }, job.timeoutMs);

      const onAbort = () => {
        finish(refuse("cancelled", "the invocation was cancelled and its worker terminated"));
      };
      if (job.signal !== undefined) {
        if (job.signal.aborted) {
          onAbort();
          return;
        }
        job.signal.addEventListener("abort", onAbort, { once: true });
      }

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
  },
};

/** The engine the host uses unless an invocation names another. */
export const defaultEngine: PluginEngine = nodeWorkerEngine;
