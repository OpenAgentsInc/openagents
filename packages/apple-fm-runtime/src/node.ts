import { createHash } from "node:crypto";
import { spawn, type ChildProcess } from "node:child_process";
import { readFileSync, statSync } from "node:fs";
import path from "node:path";

import {
  APPLE_FM_CANONICAL_HELPER_VERSION,
  APPLE_FM_DEFAULT_BASE_URL,
  APPLE_FM_HELPER_PROTOCOL_VERSION,
} from "./identity.js";
import { appleFmComplete, appleFmProbe, type AppleFmCompletionTurn, type AppleFmProbe } from "./client.js";
import type { AppleFmLaunchOutcome, AppleFmLauncher, AppleFmLauncherSession } from "./supervisor.js";

/**
 * `@openagentsinc/apple-fm-runtime/node` — Node host authority (AFS-02).
 *
 * Node host authority lives only in this subpath so a browser or mobile bundle
 * cannot import the helper host by accident (the AFS boundary check admits Node
 * imports only in this file). It owns helper discovery, signature and digest
 * verification, spawn, readiness polling, adoption, and shutdown — extracted
 * from `apps/openagents-desktop/src/apple-fm-native-helper.ts`, minus the
 * dependency on the nested Pylon runtime client. Readiness and completion use
 * the portable loopback client in the root export.
 *
 * Desktop keeps Electron IPC, `process.resourcesPath`, packaged-app staging,
 * signing, ASAR, and notarization; this module owns only the neutral helper
 * lifecycle and manifest generation from the single wire-version source.
 */

export const APPLE_FM_HELPER_BASENAME = "foundation-bridge" as const;
export const APPLE_FM_DEFAULT_PORT = 11435 as const;
export const AppleFmHelperRelativePath = path.join("native", process.arch, APPLE_FM_HELPER_BASENAME);
/** The bridge carries its OWN manifest so it never collides with the voice helper's. */
export const AppleFmHelperManifestRelativePath = path.join("native", process.arch, "foundation-bridge.manifest.json");

export interface AppleFmHelperManifest {
  readonly protocolVersion: typeof APPLE_FM_HELPER_PROTOCOL_VERSION;
  readonly helperVersion: string;
  readonly architecture: string;
  readonly sha256: string;
}

/**
 * Generate the native helper manifest from the SINGLE wire-version source. The
 * protocol version and helper version come from `identity.ts`, so the manifest,
 * the Swift `bridgeVersion`, and the Desktop staging pin cannot drift again.
 */
export const generateAppleFmHelperManifest = (input: {
  readonly sha256: string;
  readonly architecture: string;
  readonly helperVersion?: string;
}): AppleFmHelperManifest => ({
  protocolVersion: APPLE_FM_HELPER_PROTOCOL_VERSION,
  helperVersion: input.helperVersion ?? APPLE_FM_CANONICAL_HELPER_VERSION,
  architecture: input.architecture,
  sha256: input.sha256,
});

/** macOS Apple Silicon gate. Any other platform is `not_supported`. */
export const appleFmHelperSupported = (): boolean => process.platform === "darwin" && process.arch === "arm64";

export const resolveAppleFmHelperPath = (resourcesPath: string): string =>
  path.join(resourcesPath, AppleFmHelperRelativePath);

/**
 * Resolve + verify the packaged helper. Throws a typed reason on any failure so
 * the launcher can classify it (missing vs tampered): manifest shape,
 * architecture, executable bit, sha256, and a caller-supplied signature check.
 */
export const verifyAppleFmHelper = (input: {
  readonly resourcesPath: string;
  readonly manifest: AppleFmHelperManifest;
  readonly verifySignature: (absolutePath: string) => boolean;
}): string => {
  const absolutePath = resolveAppleFmHelperPath(input.resourcesPath);
  if (input.manifest.protocolVersion !== 1 || input.manifest.architecture !== process.arch) {
    throw new Error("apple_fm_helper_manifest_mismatch");
  }
  const stats = statSync(absolutePath);
  if (!stats.isFile() || (stats.mode & 0o111) === 0) throw new Error("apple_fm_helper_not_executable");
  const digest = createHash("sha256").update(readFileSync(absolutePath)).digest("hex");
  if (digest !== input.manifest.sha256) throw new Error("apple_fm_helper_digest_mismatch");
  if (!input.verifySignature(absolutePath)) throw new Error("apple_fm_helper_signature_invalid");
  return absolutePath;
};

/**
 * Spawn the bridge on an explicit loopback port with a hardened environment.
 * No shell, no detach, no ambient PATH.
 */
export const spawnAppleFmHelper = (absolutePath: string, port: number): ChildProcess =>
  spawn(absolutePath, ["--port", String(port)], {
    cwd: path.dirname(absolutePath),
    env: { LANG: "C", LC_ALL: "C", HOME: "/var/empty", PATH: "" },
    stdio: ["ignore", "pipe", "pipe"],
    detached: false,
    windowsHide: true,
  });

/** Probe live readiness through the portable loopback client. Never throws. */
export const appleFmClientProbe = (baseUrl: string, fetchImpl: typeof fetch = fetch): Promise<AppleFmProbe> =>
  appleFmProbe(baseUrl, fetchImpl);

/** Run one bounded read-only completion through the portable loopback client. */
export const appleFmClientComplete = (
  baseUrl: string,
  prompt: string,
  fetchImpl: typeof fetch = fetch,
): Promise<AppleFmCompletionTurn> => appleFmComplete(baseUrl, prompt, fetchImpl);

// ---------------------------------------------------------------------------
// Packaged launcher — adopt an existing healthy bridge, else verify + spawn.
// ---------------------------------------------------------------------------

/** The minimal child-process surface the launcher needs (for test injection). */
export interface AppleFmChildProcess {
  readonly once: (event: "exit", listener: (code: number | null, signal: NodeJS.Signals | null) => void) => unknown;
  readonly kill: (signal?: NodeJS.Signals) => unknown;
}

export interface PackagedAppleFmLauncherOptions {
  readonly resourcesPath: string;
  readonly verifySignature: (absolutePath: string) => boolean;
  readonly loadManifest?: () => AppleFmHelperManifest;
  readonly port?: number;
  readonly baseUrl?: string;
  readonly fetchImpl?: typeof fetch;
  readonly supported?: () => boolean;
  readonly spawnHelper?: (absolutePath: string, port: number) => AppleFmChildProcess;
  readonly probe?: (baseUrl: string, fetchImpl: typeof fetch) => Promise<AppleFmProbe>;
  readonly complete?: (baseUrl: string, prompt: string, fetchImpl: typeof fetch) => Promise<AppleFmCompletionTurn>;
  readonly readinessTimeoutMs?: number;
  readonly pollIntervalMs?: number;
  readonly sleep?: (ms: number) => Promise<void>;
}

const defaultSleep = (ms: number): Promise<void> => new Promise((resolve) => setTimeout(resolve, ms));

export const createPackagedAppleFmLauncher = (options: PackagedAppleFmLauncherOptions): AppleFmLauncher => {
  const port = options.port ?? APPLE_FM_DEFAULT_PORT;
  const baseUrl = options.baseUrl ?? APPLE_FM_DEFAULT_BASE_URL;
  const fetchImpl = options.fetchImpl ?? fetch;
  const supported = options.supported ?? appleFmHelperSupported;
  const probe = options.probe ?? appleFmClientProbe;
  const complete = options.complete ?? appleFmClientComplete;
  const spawnHelper = options.spawnHelper ?? spawnAppleFmHelper;
  const loadManifest =
    options.loadManifest ??
    ((): AppleFmHelperManifest => {
      const parsed: unknown = JSON.parse(
        readFileSync(path.join(options.resourcesPath, AppleFmHelperManifestRelativePath), "utf8"),
      );
      const record = typeof parsed === "object" && parsed !== null ? (parsed as Record<string, unknown>) : {};
      return {
        protocolVersion: APPLE_FM_HELPER_PROTOCOL_VERSION,
        helperVersion: typeof record.helperVersion === "string" ? record.helperVersion : APPLE_FM_CANONICAL_HELPER_VERSION,
        architecture: typeof record.architecture === "string" ? record.architecture : process.arch,
        sha256: typeof record.sha256 === "string" ? record.sha256 : "",
      };
    });
  const readinessTimeoutMs = options.readinessTimeoutMs ?? 20_000;
  const pollIntervalMs = options.pollIntervalMs ?? 250;
  const sleep = options.sleep ?? defaultSleep;

  const sessionFor = (mode: "launched" | "adopted", child: AppleFmChildProcess | null): AppleFmLauncherSession => ({
    mode,
    probe: () => probe(baseUrl, fetchImpl),
    complete: (prompt) => complete(baseUrl, prompt, fetchImpl),
    // Never stop an adopted operator bridge; only kill a child we launched.
    stop: () => {
      if (mode === "launched" && child !== null) {
        try {
          child.kill("SIGTERM");
        } catch {
          /* the child may already be gone; killing is best-effort */
        }
      }
    },
  });

  return {
    supported,
    launch: async ({ onCrash }): Promise<AppleFmLaunchOutcome> => {
      // 1. ADOPT: a bridge already healthy at the configured base URL wins.
      const adoptProbe = await probe(baseUrl, fetchImpl);
      if (adoptProbe.ready) return { kind: "session", session: sessionFor("adopted", null) };

      // 2. Resolve + verify the packaged helper.
      let absolutePath: string;
      try {
        const manifest = loadManifest();
        absolutePath = verifyAppleFmHelper({
          resourcesPath: options.resourcesPath,
          manifest,
          verifySignature: options.verifySignature,
        });
      } catch (error) {
        const reason = error instanceof Error ? error.message : "apple_fm_helper_unavailable";
        if (
          reason === "apple_fm_helper_digest_mismatch" ||
          reason === "apple_fm_helper_signature_invalid" ||
          reason === "apple_fm_helper_manifest_mismatch" ||
          reason === "apple_fm_helper_not_executable"
        ) {
          return { kind: "failed", blockerRef: `blocker.apple_fm.${reason}`, failureClass: reason };
        }
        return { kind: "helper_missing", blockerRef: "blocker.apple_fm.helper_missing" };
      }

      // 3. Spawn and poll /health until ready or the typed timeout.
      let child: AppleFmChildProcess;
      try {
        child = spawnHelper(absolutePath, port);
      } catch {
        return { kind: "failed", blockerRef: "blocker.apple_fm.spawn_failed", failureClass: "spawn_failed" };
      }
      let crashed = false;
      let adopted = false;
      child.once("exit", () => {
        crashed = true;
        if (adopted) onCrash("helper_crashed");
      });

      const deadline = Date.now() + readinessTimeoutMs;
      while (Date.now() < deadline) {
        if (crashed) {
          return { kind: "failed", blockerRef: "blocker.apple_fm.helper_crashed", failureClass: "helper_crashed" };
        }
        const readyProbe = await probe(baseUrl, fetchImpl);
        if (readyProbe.ready) {
          adopted = true;
          return { kind: "session", session: sessionFor("launched", child) };
        }
        await sleep(pollIntervalMs);
      }
      try {
        child.kill("SIGTERM");
      } catch {
        /* best-effort */
      }
      return { kind: "failed", blockerRef: "blocker.apple_fm.readiness_timeout", failureClass: "readiness_timeout" };
    },
  };
};

export { appleFmProbe, appleFmComplete };
export type { AppleFmProbe, AppleFmCompletionTurn };
