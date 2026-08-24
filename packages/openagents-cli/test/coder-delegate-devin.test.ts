import { chmodSync, mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { describe, expect, it } from "vitest";

import { DevinHarness, type DelegateEvent } from "../src/coder-delegate.js";

/** A stand-in for the binary, so the tests cost nothing and never call out. */
const fake = (script: string): string => {
  const directory = mkdtempSync(join(tmpdir(), "devin-harness-"));
  const path = join(directory, "devin-stub");
  writeFileSync(path, `#!/bin/sh\n${script}\n`);
  chmodSync(path, 0o755);
  return path;
};

const collect = async (
  harness: DevinHarness,
  cwd = process.cwd(),
): Promise<ReadonlyArray<DelegateEvent>> => {
  const events: DelegateEvent[] = [];
  for await (const event of harness.run(
    { prompt: "do the thing", cwd, transcriptPath: join(tmpdir(), "unused.jsonl") },
    new AbortController().signal,
  )) {
    events.push(event);
  }
  return events;
};

describe("running children on the Devin CLI", () => {
  it("names itself in the fleet, so a Devin child is not mistaken for an opencode one", () => {
    // Its credentials and its billing are not this session's, which is why the
    // agent is named rather than left implicit.
    expect(new DevinHarness().agent).toBe("devin");
  });

  it("reports the child's answer as text", async () => {
    const events = await collect(new DevinHarness({ command: fake('echo "PONG"') }));

    expect(events).toEqual([{ type: "text", value: "PONG" }]);
  });

  it("passes the prompt, the unattended mode, and the trust flag", async () => {
    // Print mode cannot show the trust prompt, so without the flag a child in a
    // directory nobody has opened Devin in exits before doing anything.
    const events = await collect(new DevinHarness({ command: fake('echo "$@"') }));

    const said = (events[0] as { value: string }).value;
    expect(said).toContain("-p do the thing");
    expect(said).toContain("--permission-mode dangerous");
    expect(said).toContain("--respect-workspace-trust false");
  });

  it("takes a different permission mode when one is asked for", async () => {
    const harness = new DevinHarness({ command: fake('echo "$@"'), permissionMode: "auto" });

    const said = ((await collect(harness))[0] as { value: string }).value;
    expect(said).toContain("--permission-mode auto");
    // The mode is what the fleet shows beside the agent, because print mode
    // does not report which model answered.
    expect(harness.model).toBe("auto");
  });

  it("reports a failing child as an error, with what it said", async () => {
    const events = await collect(
      new DevinHarness({ command: fake('echo "went wrong" >&2; exit 2') }),
    );

    expect(events).toEqual([{ type: "error", message: "went wrong" }]);
  });

  it("says so when a failing child said nothing at all", async () => {
    const events = await collect(new DevinHarness({ command: fake("exit 3") }));

    expect((events[0] as { message: string }).message).toContain("exited with code 3");
  });

  it("throws when the binary is not on PATH, rather than reporting an empty child", async () => {
    // Refused once for the fleet, not once per child.
    await expect(collect(new DevinHarness({ command: "devin-does-not-exist" }))).rejects.toThrow(
      "not on PATH",
    );
  });

  it("stops when the fleet is stopped", async () => {
    const harness = new DevinHarness({ command: fake("sleep 30") });
    const controller = new AbortController();
    const events: DelegateEvent[] = [];

    const running = (async () => {
      for await (const event of harness.run(
        { prompt: "x", cwd: process.cwd(), transcriptPath: "/tmp/x" },
        controller.signal,
      )) {
        events.push(event);
      }
    })();

    controller.abort();
    await running;

    // Killed rather than left running: a child holds a process, and a console
    // that exits while children keep spending leaves nothing to stop them with.
    expect(events.every((event) => event.type !== "text")).toBe(true);
  });
});
