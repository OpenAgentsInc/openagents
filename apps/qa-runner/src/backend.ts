// Isolation backend abstraction (modeled on executor's src/vm).
//
// A backend provisions an isolated environment for a run, exposes a way to
// acquire a browser inside it, and tears down cleanly. Two implementations:
//   - `localBackend`: runs on THIS host with real chromium. Implemented, works
//     now. "Isolation" here is a fresh browser context + a per-run artifact dir;
//     it is local-first per the OSS requirement.
//   - `cloudVmBackend`: an interface-only seam for OpenAgents Cloud firecracker
//     microVMs / sek8s confidential runners. OWNER-GATED: it throws "not armed"
//     unless an injected provisioner is supplied. cloud/oa-node/oa-workroomd
//     wires the real provisioner later (per-run microVM with ssh/push/tunnel,
//     like executor's VmHandle).

import {
  acquirePlaywrightBrowser,
  type AcquiredBrowser,
  type PlaywrightArtifacts,
} from "@openagentsinc/probe-runtime";
import type { Target } from "./target";

export interface BackendSession {
  /** Acquire a browser inside the provisioned environment. */
  readonly acquireBrowser: () => Promise<AcquiredBrowser & { artifacts: () => PlaywrightArtifacts }>;
  /** Tear the environment down. */
  readonly teardown: () => Promise<void>;
}

export interface Backend {
  readonly name: string;
  /** Provision a fresh isolated session for `target`, writing artifacts to `artifactDir`. */
  readonly provision: (input: {
    readonly target: Target;
    readonly artifactDir: string;
    readonly headed?: boolean;
  }) => Promise<BackendSession>;
}

export interface LocalBackendOptions {
  /** Override the playwright chromium (for tests). */
  readonly chromium?: unknown;
}

/**
 * The local backend: real chromium on this host. Each provision is a fresh
 * browser context + per-run artifact dir. Works now; this is the default.
 */
export function localBackend(options: LocalBackendOptions = {}): Backend {
  return {
    name: "local",
    provision: async ({ target, artifactDir, headed }) => {
      return {
        acquireBrowser: () =>
          acquirePlaywrightBrowser({
            baseUrl: target.baseUrl,
            artifactDir,
            ...(headed !== undefined ? { headed } : {}),
            ...(options.chromium !== undefined ? { chromium: options.chromium } : {}),
          }),
        teardown: async () => undefined,
      };
    },
  };
}

export class CloudVmBackendNotArmedError extends Error {
  constructor() {
    super(
      "cloudVmBackend is not armed: per-run OpenAgents Cloud microVM provisioning " +
        "(firecracker / sek8s, via oa-node / oa-workroomd) is owner-gated. Inject a " +
        "provisioner to enable it.",
    );
    this.name = "CloudVmBackendNotArmedError";
  }
}

/**
 * A provisioner for a real isolated VM. The cloud wiring (oa-node/oa-workroomd)
 * implements this; the shape mirrors executor's VmHandle (ssh/push/tunnel) but
 * is reduced here to what the runner needs: give it a session.
 *
 * `provision` is the only method the browser runner needs today. The richer
 * lifecycle below (`CloudVmProvisionerV2`) is the typed provision/exec/teardown
 * seam the cross-OS Cloud-VM work (#6186) grows into; it is additive and
 * optional so existing provisioners keep working.
 */
export interface CloudVmProvisioner {
  readonly provision: (input: {
    readonly target: Target;
    readonly artifactDir: string;
    readonly headed?: boolean;
  }) => Promise<BackendSession>;
}

/** The OS tier a Cloud microVM is requested on (executor's 3-OS breadth). */
export type CloudVmOs = "linux" | "macos" | "windows";

/** A handle to a provisioned microVM, mirroring executor's VmHandle (reduced). */
export interface CloudVmHandle {
  /** Opaque provider id (e.g. firecracker microVM id / sek8s pod name). */
  readonly id: string;
  /** The OS this VM is running. */
  readonly os: CloudVmOs;
  /**
   * Run a command inside the VM (ssh/exec). Returns combined output + exit
   * code. Used by terminal/native-desktop drivers running INSIDE the VM.
   */
  readonly exec: (
    command: string,
    args?: ReadonlyArray<string>,
  ) => Promise<{ readonly code: number; readonly output: string }>;
  /** Acquire a browser session inside the VM (for the browser runner). */
  readonly acquireBrowser: BackendSession["acquireBrowser"];
  /** Tear the VM down and release its resources. */
  readonly teardown: () => Promise<void>;
}

/**
 * The typed cross-OS provisioner the cloud wiring implements later. Distinct
 * from the v1 `provision`-only seam so it can be added without breaking the
 * existing browser-only contract: `provisionVm` returns a full VM handle
 * (provision / exec / teardown) on a requested OS tier.
 *
 * OWNER-GATED: the real implementation lives in `cloud` (oa-node/oa-workroomd
 * over firecracker / sek8s). There is intentionally NO live implementation here.
 */
export interface CloudVmProvisionerV2 extends CloudVmProvisioner {
  readonly provisionVm: (input: {
    readonly target: Target;
    readonly artifactDir: string;
    readonly os: CloudVmOs;
    readonly headed?: boolean;
  }) => Promise<CloudVmHandle>;
}

/**
 * An INERT stub provisioner: every method errors honestly with
 * `CloudVmBackendNotArmedError`. This is the owner-gated default — wiring this
 * in does NOT silently fall back to local; it makes the un-armed state explicit
 * and testable. The real provisioner from `cloud` replaces it.
 */
export function inertCloudVmProvisioner(): CloudVmProvisionerV2 {
  const fail = (): never => {
    throw new CloudVmBackendNotArmedError();
  };
  return {
    provision: async () => fail(),
    provisionVm: async () => fail(),
  };
}

export interface CloudVmBackendOptions {
  /**
   * The real provisioner (OpenAgents Cloud microVM). When absent, the backend
   * is INERT and throws `CloudVmBackendNotArmedError` on provision — there is no
   * fake green: an un-armed cloud backend cannot silently fall back to local.
   */
  readonly provisioner?: CloudVmProvisioner;
}

/**
 * The cloud-VM backend seam. Interface-only + owner-gated: without an injected
 * provisioner it throws on provision. This is the "use VMs and infra from
 * OpenAgents Cloud" surface the example flow targets; the real firecracker /
 * sek8s provisioner is wired later in `cloud`.
 */
export function cloudVmBackend(options: CloudVmBackendOptions = {}): Backend {
  return {
    name: "cloud-vm",
    provision: async (input) => {
      if (!options.provisioner) throw new CloudVmBackendNotArmedError();
      return options.provisioner.provision(input);
    },
  };
}

// ── Native-desktop driver — SPEC ONLY (stretch, #6186) ───────────────────────
//
// droid-control uses trycua/cua's `cua-driver` (accessibility tree + screenshots
// over a native desktop). This is the SPEC for that seam so the contract is
// settled before any real native driver lands. There is intentionally NO real
// implementation here: `nativeDesktopDriver()` returns a stub that errors. A
// real macOS/Windows driver (likely a trycua/cua adapter running INSIDE a
// `CloudVmHandle`) implements `NativeDesktopDriver` later.

export type NativeDesktopOs = "macos" | "windows";

/**
 * A native-desktop driver: read the accessibility tree + screenshots, and
 * synthesize input — the surface a native UI test drives against. Mirrors the
 * computer-use browser/terminal seams so the runner stays driver-agnostic.
 */
export interface NativeDesktopDriver {
  readonly os: NativeDesktopOs;
  /** Snapshot the accessibility tree as a serializable, public-safe structure. */
  readonly accessibilityTree: () => Promise<unknown>;
  /** Capture a screenshot to `path` (PNG). Returns the written path. */
  readonly screenshot: (path: string) => Promise<string>;
  /** Click an accessibility node by a stable selector/role+name intent. */
  readonly click: (selector: string) => Promise<void>;
  /** Type text into the focused element. */
  readonly type: (text: string) => Promise<void>;
  /** Release the driver/session. */
  readonly teardown: () => Promise<void>;
}

export class NativeDesktopDriverNotImplementedError extends Error {
  constructor() {
    super(
      "nativeDesktopDriver is spec-only (#6186 stretch): the native-desktop " +
        "accessibility+screenshot driver (trycua/cua-style, macOS/Windows) is " +
        "not implemented. It will run inside a CloudVmHandle and is wired later.",
    );
    this.name = "NativeDesktopDriverNotImplementedError";
  }
}

/**
 * SPEC-ONLY stub. Throws `NativeDesktopDriverNotImplementedError`. Present so
 * the seam + error are typed and testable; do NOT mistake this for a working
 * native driver.
 */
export function nativeDesktopDriver(_os: NativeDesktopOs): NativeDesktopDriver {
  throw new NativeDesktopDriverNotImplementedError();
}
