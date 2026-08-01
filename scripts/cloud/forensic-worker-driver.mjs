#!/usr/bin/env node

import { constants as fsConstants, accessSync } from "node:fs";
import { spawnSync } from "node:child_process";

const DRIVER_REF = "driver.openagents.forensic-worker.v1";
const BUBBLEWRAP = "/usr/bin/bwrap";
const WORKSPACE = "/workspace";

const refuse = (reason) => {
  process.stderr.write(`${JSON.stringify({ error: "forensic_worker_preflight_refused", reason })}\n`);
  process.exitCode = 1;
};

if (process.argv.length !== 3 || process.argv[2] !== "preflight") {
  refuse("unsupported_operation");
} else if (process.platform !== "linux") {
  refuse("linux_required");
} else {
  try {
    accessSync(BUBBLEWRAP, fsConstants.X_OK);
    accessSync(WORKSPACE, fsConstants.R_OK | fsConstants.X_OK);
    const probe = spawnSync(
      BUBBLEWRAP,
      [
        "--die-with-parent",
        "--unshare-net",
        "--unshare-pid",
        "--unshare-uts",
        "--unshare-ipc",
        "--ro-bind",
        "/",
        "/",
        "--bind",
        WORKSPACE,
        WORKSPACE,
        "--tmpfs",
        "/run",
        "--proc",
        "/proc",
        "--dev",
        "/dev",
        "--chdir",
        WORKSPACE,
        "/bin/true",
      ],
      {
        cwd: WORKSPACE,
        env: { PATH: "/usr/bin:/bin" },
        encoding: "utf8",
        timeout: 5_000,
        windowsHide: true,
      },
    );
    if (probe.error !== undefined || probe.status !== 0 || probe.signal !== null) {
      refuse("bubblewrap_probe_failed");
    } else {
      process.stdout.write(
        `${JSON.stringify({
          schema: "openagents.forensic_worker_preflight.v1",
          driverRef: DRIVER_REF,
          linux: true,
          bubblewrap: true,
          networkNamespace: "unshared",
          workspaceRoot: "workspace",
        })}\n`,
      );
    }
  } catch {
    refuse("runtime_dependency_missing");
  }
}
