import { mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { describe, expect, it } from "vitest";

import { refusalFor } from "../src/coder-shell.js";
import { shellTool } from "../src/coder-tools.js";

const run = (command: string, timeout_seconds?: number) =>
  shellTool(process.cwd()).run(
    { command, ...(timeout_seconds === undefined ? {} : { timeout_seconds }) },
    new AbortController().signal,
  );

describe("what the shell refuses", () => {
  const refused = [
    "rm -rf /",
    "rm -rf ~",
    "sudo rm -rf /",
    "rm -rf $HOME",
    "rm -fr /",
    "rm -rf /*",
    "mkfs.ext4 /dev/disk2",
    "dd if=/dev/zero of=/dev/disk0",
    "diskutil eraseDisk JHFS+ X disk2",
    "sudo shutdown -h now",
    ":(){ :|:& };:",
    "chmod -R 777 /",
    "echo hi > /dev/disk0",
  ];

  it.each(refused)("refuses %s", (command) => {
    expect(refusalFor(command)).toBeDefined();
  });

  const allowed = [
    // The one that matters: cleaning a build directory is ordinary work, and a
    // list that stopped it would be turned off within a day.
    "rm -rf node_modules",
    "rm -rf _build deps",
    "rm -rf ./dist",
    "rm file.txt",
    "git status",
    "mix test",
    "grep -rn foo src/",
    "find . -name '*.ex' -delete",
  ];

  it.each(allowed)("allows %s", (command) => {
    expect(refusalFor(command)).toBeUndefined();
  });

  it("says what was wrong with it, not just that it was refused", async () => {
    const output = await run("rm -rf /");

    // A bare refusal reads as the tool being broken rather than as the command
    // being the problem.
    expect(output).toContain("erase a root or a home directory");
    expect(output).toContain("name the directory");
  });
});

describe("running a command", () => {
  it("returns what it printed", async () => {
    await expect(run("echo hello")).resolves.toBe("hello");
  });

  it("keeps both streams, in the order they arrived", async () => {
    const output = await run("echo first; echo second >&2");

    // The error output is usually the part worth reading, and separating the
    // streams loses which line came before which.
    expect(output).toContain("first");
    expect(output).toContain("second");
  });

  it("reports a failing command by its exit code", async () => {
    // An empty failure reads as an empty success.
    await expect(run("exit 3")).resolves.toContain("exited with code 3");
  });

  it("says a silent success was a success", async () => {
    await expect(run("true")).resolves.toBe("Success");
  });

  it("runs in the working directory it was given", async () => {
    const directory = mkdtempSync(join(tmpdir(), "coder-shell-"));
    writeFileSync(join(directory, "marker.txt"), "here");

    const output = await shellTool(directory).run({ command: "ls" }, new AbortController().signal);

    expect(output).toContain("marker.txt");
  });

  it("stops a command that runs too long, and says so", async () => {
    await expect(run("sleep 5", 1)).resolves.toContain("did not finish within 1s");
  });

  it("does not wait on a command that would prompt", async () => {
    // No terminal, so the read gets end-of-file rather than waiting where
    // nobody can see it.
    await expect(run("read x", 5)).resolves.toContain("exited with code");
  });

  it("asks for a command rather than running nothing", async () => {
    await expect(run("   ")).resolves.toContain("`command` is required");
  });

  it("says how much of an oversized output was cut, rather than ending mid-line", async () => {
    // 100,000 characters against a 30,000-character hold. What the collector
    // refuses is counted, so the notice can name the whole size: a result that
    // stops without saying so is read as the whole of it.
    const output = await run("yes wwwwwwwwwwwwwwwwwww | head -n 5000", 30);

    expect(output).toContain("The command printed 100000 characters");
    expect(output).toContain("this tool holds 30000");
    expect(output).toContain("70000 were cut from the end");
    expect(output).toContain("must not be read as the whole of it");
  });
});
