// Container runtime seam for the containerized execution backend.
//
// The containerized backend (container-backend.ts) is the REAL, in-repo analogue
// of a cloud microVM: provision = start a container, exec = run a qa session
// inside it, teardown = stop it, and extract artifacts (result.json / video /
// trace) back out. The actual container engine is abstracted behind
// `ContainerRuntime` so that:
//   - the real path shells out to `docker` (`dockerContainerRuntime`);
//   - unit tests inject a deterministic fake (no Docker, no network) and still
//     prove the full provision -> exec -> copy-out -> teardown contract.
//
// Honesty rules:
//   - `dockerContainerRuntime().available()` reports the TRUE state of the host
//     (is `docker` on PATH and is the daemon reachable). The backend refuses to
//     run when Docker is absent; it never fakes a green.
//   - every method that shells out throws an explicit, typed error on failure;
//     a non-zero `docker` exit is surfaced, not swallowed.

import { spawn } from "node:child_process";

/** A typed, public-safe description of one container exec result. */
export interface ContainerExecResult {
  /** Process exit code inside the container (or of the engine command). */
  readonly code: number;
  /** Combined stdout+stderr, captured for the transcript. */
  readonly output: string;
}

/** Options for starting a container. */
export interface ContainerRunOptions {
  /** Image reference (e.g. an image bundling a headless browser). */
  readonly image: string;
  /** Human label applied as the container name (suffixed for uniqueness). */
  readonly name: string;
  /**
   * Command to run as the container entrypoint. A long-lived no-op keeps the
   * container alive so we can `exec` into it (provision/exec/teardown lifecycle),
   * mirroring how a microVM stays up between provision and teardown.
   */
  readonly command: ReadonlyArray<string>;
  /** Environment passed into the container (already public-safe; no secrets). */
  readonly env?: Readonly<Record<string, string>>;
  /** Extra engine flags (advanced; tests don't need these). */
  readonly extraArgs?: ReadonlyArray<string>;
}

/**
 * The container engine seam. A real implementation shells out to `docker`; the
 * fake in tests is fully deterministic. The shape is deliberately small — just
 * what the backend needs to provision/exec/copy-out/teardown.
 */
export interface ContainerRuntime {
  /** Engine name, surfaced in errors + the result backend label. */
  readonly name: string;
  /**
   * True only when the engine is actually usable on this host (binary on PATH
   * AND daemon reachable). Honest: a false here makes the backend refuse, never
   * fake. Async because reachability needs a probe.
   */
  readonly available: () => Promise<boolean>;
  /** Start a container; resolves with its id once running. */
  readonly run: (options: ContainerRunOptions) => Promise<{ readonly id: string }>;
  /** Exec a command inside a running container. */
  readonly exec: (
    id: string,
    command: ReadonlyArray<string>,
  ) => Promise<ContainerExecResult>;
  /**
   * Copy a path OUT of the container to a host path (artifact extraction).
   * Mirrors `docker cp <id>:<src> <dest>`.
   */
  readonly copyOut: (id: string, containerPath: string, hostPath: string) => Promise<void>;
  /** Stop + remove the container (teardown). Best-effort but reports failure. */
  readonly remove: (id: string) => Promise<void>;
}

export class ContainerRuntimeError extends Error {
  constructor(
    message: string,
    readonly detail?: { readonly code?: number; readonly output?: string },
  ) {
    super(message);
    this.name = "ContainerRuntimeError";
  }
}

/** Run an engine CLI command, capturing combined output. Never shell-interpolates. */
function execEngine(
  bin: string,
  args: ReadonlyArray<string>,
): Promise<ContainerExecResult> {
  return new Promise((resolve, reject) => {
    let child: ReturnType<typeof spawn>;
    try {
      child = spawn(bin, [...args], { stdio: ["ignore", "pipe", "pipe"] });
    } catch (error) {
      reject(
        new ContainerRuntimeError(
          `failed to spawn "${bin}": ${error instanceof Error ? error.message : String(error)}`,
        ),
      );
      return;
    }
    let output = "";
    child.stdout?.on("data", (c) => {
      output += c.toString();
    });
    child.stderr?.on("data", (c) => {
      output += c.toString();
    });
    child.on("error", (error) => {
      // ENOENT: the engine binary is not installed.
      reject(new ContainerRuntimeError(`"${bin}" not available: ${error.message}`));
    });
    child.on("close", (code) => {
      resolve({ code: code ?? -1, output });
    });
  });
}

export interface DockerContainerRuntimeOptions {
  /** Engine binary (default "docker"; "podman" also satisfies this CLI shape). */
  readonly bin?: string;
}

/**
 * The REAL container runtime: shells out to `docker` (or a compatible engine).
 * `available()` is honest — it returns false when the binary is missing or the
 * daemon is unreachable, so the backend can refuse instead of faking.
 */
export function dockerContainerRuntime(
  options: DockerContainerRuntimeOptions = {},
): ContainerRuntime {
  const bin = options.bin ?? "docker";
  return {
    name: bin,
    available: async () => {
      try {
        // `docker info` exits non-zero if the daemon is unreachable, and the
        // spawn rejects if the binary is missing — both => not available.
        const r = await execEngine(bin, ["info", "--format", "{{.ServerVersion}}"]);
        return r.code === 0;
      } catch {
        return false;
      }
    },
    run: async (opts) => {
      const args: string[] = ["run", "-d", "--name", opts.name];
      for (const [k, v] of Object.entries(opts.env ?? {})) {
        args.push("-e", `${k}=${v}`);
      }
      if (opts.extraArgs) args.push(...opts.extraArgs);
      args.push(opts.image, ...opts.command);
      const r = await execEngine(bin, args);
      if (r.code !== 0) {
        throw new ContainerRuntimeError(`${bin} run failed`, { code: r.code, output: r.output });
      }
      // `docker run -d` prints the full container id; the name is also usable.
      const id = r.output.trim().split("\n").pop()?.trim() || opts.name;
      return { id };
    },
    exec: async (id, command) => {
      const r = await execEngine(bin, ["exec", id, ...command]);
      return r;
    },
    copyOut: async (id, containerPath, hostPath) => {
      const r = await execEngine(bin, ["cp", `${id}:${containerPath}`, hostPath]);
      if (r.code !== 0) {
        throw new ContainerRuntimeError(`${bin} cp failed`, { code: r.code, output: r.output });
      }
    },
    remove: async (id) => {
      const r = await execEngine(bin, ["rm", "-f", id]);
      if (r.code !== 0) {
        throw new ContainerRuntimeError(`${bin} rm failed`, { code: r.code, output: r.output });
      }
    },
  };
}
