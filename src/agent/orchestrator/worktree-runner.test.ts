import { describe, expect, test } from "bun:test";
import { defaultProjectConfig } from "../../tasks/index.js";
import type { ProjectConfig } from "../../tasks/schema.js";
import { getInstallSettings } from "./worktree-runner.js";

describe("worktree-runner install settings", () => {
  const baseConfig = defaultProjectConfig("test");

  test("applies defaults when parallelExecution is absent", () => {
    const settings = getInstallSettings(baseConfig);
    expect(settings.args).toEqual(["--frozen-lockfile"]);
    expect(settings.timeoutMs).toBe(15 * 60 * 1000);
    expect(settings.skipInstall).toBe(false);
  });

  test("uses custom args and timeout, including skip-install flag", () => {
    const config: ProjectConfig = {
      ...baseConfig,
      parallelExecution: {
        ...baseConfig.parallelExecution,
        installArgs: ["--skip-install", "--offline"],
        installTimeoutMs: 5_000,
      },
    };

    const settings = getInstallSettings(config);
    expect(settings.args).toEqual(["--offline"]);
    expect(settings.timeoutMs).toBe(5_000);
    expect(settings.skipInstall).toBe(true);
  });
});
