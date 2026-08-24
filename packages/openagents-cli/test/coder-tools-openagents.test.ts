import { describe, expect, it } from "vitest";

import { openagentsTool } from "../src/coder-tools.js";

const run = (args: ReadonlyArray<string>) =>
  openagentsTool().run({ args: [...args] }, new AbortController().signal);

describe("the openagents tool", () => {
  it("carries the catalog of what it is for, and points at --help", () => {
    const { description, parameters } = openagentsTool();

    expect(description).toContain("--help");
    expect(description).toContain("--json");
    expect(parameters["required"]).toEqual(["args"]);
  });

  it("runs the CLI this session is part of", async () => {
    // Resolved beside this module rather than from PATH, so what answers is
    // this build and not whichever copy happens to be installed.
    await expect(run(["--version"])).resolves.toContain("openagents v");
  });

  it("asks for arguments rather than running something arbitrary", async () => {
    await expect(run([])).resolves.toContain("`args` is required");
  });

  it("reports a failing command by its exit code, not as an empty answer", async () => {
    const output = await run(["issue", "view", "999999", "-R", "OpenAgentsInc/openagents"]);

    // An empty failure reads as an empty success.
    expect(output).toContain("exited with code");
  });

  it("refuses to start another coder session, and says what to use instead", async () => {
    const output = await run(["coder"]);

    expect(output).toContain("needs a terminal");
    expect(output).toContain("`delegate` tool");
  });

  it("refuses a login that would wait, and names the headless form", async () => {
    await expect(run(["auth", "login"])).resolves.toContain("auth login --headless");
  });

  it("allows the headless login, which is the one that does not wait", async () => {
    // The refusal is about waiting for a terminal, not about the command.
    const output = await run(["auth", "login", "--headless", "--help"]);

    expect(output).not.toContain("needs a terminal");
  });

  it("refuses a command that serves until it is stopped", async () => {
    await expect(run(["computer", "up"])).resolves.toContain("needs a terminal");
  });
});
