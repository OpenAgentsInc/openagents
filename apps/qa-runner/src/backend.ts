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
 */
export interface CloudVmProvisioner {
  readonly provision: (input: {
    readonly target: Target;
    readonly artifactDir: string;
    readonly headed?: boolean;
  }) => Promise<BackendSession>;
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
