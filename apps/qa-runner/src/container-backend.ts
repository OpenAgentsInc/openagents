// Containerized execution backend — the REAL, in-repo analogue of a cloud VM.
//
// Issue #6186 asks for a real cross-environment/isolated execution backend wired
// through the existing `cloudVmBackend` / provisioner seam (backend.ts). The
// production firecracker/sek8s microVM provisioner lives in `cloud` and is
// owner-gated; this is the locally-runnable real backend that exercises the SAME
// provision -> exec -> teardown -> artifact-extraction lifecycle against a
// container engine (Docker), so the seam is proven end-to-end without faking.
//
// Lifecycle (mirrors executor's VmHandle, reduced to what the runner needs):
//   provision  -> start a container from an image that bundles a headless
//                 browser; the container stays up (a long-lived no-op entrypoint)
//                 so we can exec into it.
//   exec       -> run a qa session INSIDE the container. The session writes its
//                 result.json + artifacts (video / trace / screenshots) to a
//                 known workdir inside the container.
//   extract    -> `docker cp` the artifact workdir back OUT to the host run dir,
//                 so the result + video + trace are dereferenceable with no
//                 access to the container.
//   teardown   -> stop + remove the container.
//
// OWNER-GATED / ARMED-BY-ENV (default OFF):
//   The backend is INERT unless explicitly armed (`QA_CONTAINER_BACKEND=1`, or
//   `armed: true`). This matches the owner-gated posture of the cloud seam: a
//   real isolated execution backend does not turn itself on. When un-armed it
//   throws `ContainerBackendNotArmedError`.
//
// HONEST ABOUT DOCKER:
//   When armed but the container engine is not actually available on the host
//   (binary missing / daemon down), provisioning throws
//   `ContainerEngineUnavailableError`. It NEVER silently falls back to local and
//   NEVER fakes a green.
//
// DETERMINISTIC IN CI:
//   The container engine is injected (`ContainerRuntime`). Unit tests pass a
//   fake runtime that records the lifecycle and writes a synthetic artifact set,
//   proving provision/exec/extract/teardown + the armed/unarmed/Docker-absent
//   branches with NO Docker and NO network.

import { mkdirSync } from "node:fs";

import type { CloudVmHandle, CloudVmOs } from "./backend";
import {
  dockerContainerRuntime,
  type ContainerExecResult,
  type ContainerRuntime,
} from "./container-runtime";
import type { Target } from "./target";

/** Default image expected to bundle a headless browser + the session entrypoint. */
export const DEFAULT_CONTAINER_IMAGE = "mcr.microsoft.com/playwright:v1.61.0-noble";

/** Path INSIDE the container where a session writes its artifacts. */
export const CONTAINER_ARTIFACT_DIR = "/qa/artifacts";

export class ContainerBackendNotArmedError extends Error {
  constructor() {
    super(
      "containerBackend is not armed: the containerized execution backend " +
        "(real isolated runs in a container) is owner-gated and OFF by default. " +
        "Arm it explicitly with QA_CONTAINER_BACKEND=1 (or { armed: true }).",
    );
    this.name = "ContainerBackendNotArmedError";
  }
}

export class ContainerEngineUnavailableError extends Error {
  constructor(engine: string) {
    super(
      `containerBackend is armed but the container engine "${engine}" is not ` +
        "available on this host (binary missing or daemon unreachable). It will " +
        "NOT fall back to local or fake a result. Start Docker, or run un-armed.",
    );
    this.name = "ContainerEngineUnavailableError";
  }
}

export interface ContainerBackendOptions {
  /**
   * Arm the backend. Defaults to reading `QA_CONTAINER_BACKEND` from `env`
   * ("1"/"true" => armed). Owner-gated: OFF unless explicitly set.
   */
  readonly armed?: boolean;
  /** Env source for the arming check (default `process.env`). */
  readonly env?: Readonly<Record<string, string | undefined>>;
  /** Injectable container engine (tests inject a fake; default real Docker). */
  readonly runtime?: ContainerRuntime;
  /** Image bundling a headless browser + session entrypoint. */
  readonly image?: string;
  /** Injectable id/clock for deterministic container names in tests. */
  readonly now?: () => number;
}

/** True when the env arms the container backend. */
export function isContainerBackendArmed(
  env: Readonly<Record<string, string | undefined>> = process.env,
): boolean {
  const v = env.QA_CONTAINER_BACKEND;
  return v === "1" || v === "true";
}

/** Map our OS tier vocabulary to the engine's platform notion (informational). */
function osForRuntime(os: CloudVmOs): string {
  // Container engines run linux guests; macos/windows tiers require the
  // production cloud provisioner (firecracker/sek8s) and are out of scope for
  // the local container backend. We accept the request but record linux.
  return os === "linux" ? "linux" : `${os} (requested; container backend runs linux)`;
}

export interface ContainerSessionInput {
  readonly target: Target;
  /** Host directory artifacts are extracted INTO. */
  readonly artifactDir: string;
  /** OS tier requested (the local container backend runs linux). */
  readonly os?: CloudVmOs;
  /**
   * The command to run INSIDE the container to produce the session + artifacts.
   * It MUST write its outputs under {@link CONTAINER_ARTIFACT_DIR}. Injectable so
   * a caller can run the qa-runner's own session entrypoint, a smoke command, or
   * a custom driver; defaults to a self-contained probe that proves the
   * exec+extract path (writes a minimal result.json + a snapshot file).
   */
  readonly sessionCommand?: ReadonlyArray<string>;
  /** Public-safe env passed into the container (no secrets). */
  readonly env?: Readonly<Record<string, string>>;
}

export interface ContainerSessionOutcome {
  /** The opaque container id used for the run. */
  readonly containerId: string;
  /** The OS tier (informational). */
  readonly os: string;
  /** Combined exec output (transcript). */
  readonly exec: ContainerExecResult;
  /** Host directory the artifacts were extracted into. */
  readonly extractedTo: string;
}

/**
 * A self-contained default session command: writes a minimal, public-safe
 * result.json + a snapshot marker under the in-container artifact dir using only
 * a POSIX shell. This proves the exec->extract path against ANY image with a
 * shell (so the real-Docker proof does not require a heavyweight browser run),
 * while a real caller can inject a browser-driving command instead.
 */
export function defaultProbeSessionCommand(target: Target): ReadonlyArray<string> {
  // The result is assembled INSIDE the container so timestamps reflect the real
  // in-container run. `$TS` is an ISO-8601 instant from `date` (POSIX). The
  // public-safe QaRunResult schema requires startedAt/endedAt/durationMs, so the
  // probe records them honestly (a single short exec => duration 0).
  const head =
    `{"schemaVersion":"openagents.qa_runner.result.v1","status":"pass",` +
    `"target":{"name":${JSON.stringify(target.name)},"baseUrl":${JSON.stringify(target.baseUrl)}},` +
    `"brain":"container-probe","backend":"container",`;
  const tail =
    `"durationMs":0,` +
    `"steps":[{"index":0,"kind":"exec","label":"in-container probe","status":"ok"}],` +
    `"artifacts":{"screenshots":["snapshot.txt"]}}`;
  // Single sh -c so it works as a `docker exec` command on a minimal image.
  const script =
    `mkdir -p ${CONTAINER_ARTIFACT_DIR} && ` +
    `TS=$(date -u +%Y-%m-%dT%H:%M:%SZ) && ` +
    `printf '%s"startedAt":"%s","endedAt":"%s",%s' ` +
    `'${head}' "$TS" "$TS" '${tail}' > ${CONTAINER_ARTIFACT_DIR}/result.json && ` +
    `printf 'in-container snapshot for %s\\n' '${target.name}' > ${CONTAINER_ARTIFACT_DIR}/snapshot.txt`;
  return ["sh", "-c", script];
}

/**
 * Run a full session inside a freshly provisioned container and extract its
 * artifacts to the host. This is the real provision -> exec -> extract ->
 * teardown lifecycle. Owner-gated (armed) + honest about Docker availability.
 *
 * Resolves with the container id, the exec transcript, and the host directory
 * the artifacts were copied into. Teardown always runs (even on exec failure),
 * so a container is never leaked.
 */
export async function runContainerSession(
  input: ContainerSessionInput,
  options: ContainerBackendOptions = {},
): Promise<ContainerSessionOutcome> {
  const env = options.env ?? process.env;
  const armed = options.armed ?? isContainerBackendArmed(env);
  if (!armed) throw new ContainerBackendNotArmedError();

  const runtime = options.runtime ?? dockerContainerRuntime();
  if (!(await runtime.available())) {
    throw new ContainerEngineUnavailableError(runtime.name);
  }

  const image = options.image ?? DEFAULT_CONTAINER_IMAGE;
  const now = options.now ?? Date.now;
  const containerName = `qa-runner-${now()}`;
  const os = input.os ?? "linux";

  mkdirSync(input.artifactDir, { recursive: true });

  // Provision: start a long-lived container so we can exec into it. `sleep`-style
  // keep-alive mirrors a microVM staying up between provision and teardown.
  const { id } = await runtime.run({
    image,
    name: containerName,
    command: ["sh", "-c", "sleep 3600"],
    ...(input.env ? { env: input.env } : {}),
  });

  try {
    // Exec: run the session inside the container; it writes artifacts to the
    // in-container artifact dir.
    const sessionCommand = input.sessionCommand ?? defaultProbeSessionCommand(input.target);
    const exec = await runtime.exec(id, sessionCommand);

    // Extract: copy the in-container artifact dir back out to the host run dir.
    // `docker cp <id>:/qa/artifacts/.` copies the directory CONTENTS into the
    // host dir (the trailing /. matches docker's content-copy semantics).
    await runtime.copyOut(id, `${CONTAINER_ARTIFACT_DIR}/.`, input.artifactDir);

    return {
      containerId: id,
      os: osForRuntime(os),
      exec,
      extractedTo: input.artifactDir,
    };
  } finally {
    // Teardown: never leak a container, even if exec/extract threw.
    await runtime.remove(id).catch(() => undefined);
  }
}

/**
 * A `CloudVmHandle`-shaped provisioner over the container engine: provision
 * starts the container, `exec` runs commands inside it, `teardown` removes it.
 * This satisfies the typed cross-OS handle contract from backend.ts using a real
 * (local) isolated environment — the bridge between the owner-gated cloud seam
 * and a runnable backend. Owner-gated + honest about Docker the same way as
 * {@link runContainerSession}.
 *
 * `acquireBrowser` is intentionally NOT wired here: acquiring a Playwright
 * browser process living INSIDE the container over the handle is the cloud
 * provisioner's job (ssh/tunnel into the VM). For the local container backend,
 * the supported path is exec-a-session + extract artifacts
 * ({@link runContainerSession}); calling `acquireBrowser` throws an explicit
 * error rather than faking an in-container browser.
 */
export async function provisionContainerVm(
  input: { readonly target: Target; readonly artifactDir: string; readonly os: CloudVmOs },
  options: ContainerBackendOptions = {},
): Promise<CloudVmHandle> {
  const env = options.env ?? process.env;
  const armed = options.armed ?? isContainerBackendArmed(env);
  if (!armed) throw new ContainerBackendNotArmedError();

  const runtime = options.runtime ?? dockerContainerRuntime();
  if (!(await runtime.available())) {
    throw new ContainerEngineUnavailableError(runtime.name);
  }

  const image = options.image ?? DEFAULT_CONTAINER_IMAGE;
  const now = options.now ?? Date.now;
  const containerName = `qa-runner-vm-${now()}`;
  mkdirSync(input.artifactDir, { recursive: true });

  const { id } = await runtime.run({
    image,
    name: containerName,
    command: ["sh", "-c", "sleep 3600"],
  });

  return {
    id,
    os: input.os,
    exec: async (command, args = []) => {
      const r = await runtime.exec(id, [command, ...args]);
      return { code: r.code, output: r.output };
    },
    acquireBrowser: () => {
      throw new Error(
        "container-backend acquireBrowser is not supported over the handle: " +
          "the local container backend runs sessions via exec + artifact " +
          "extraction (runContainerSession). In-container browser acquisition " +
          "over a tunnel is the cloud provisioner's job.",
      );
    },
    teardown: async () => {
      await runtime.remove(id);
    },
  };
}
