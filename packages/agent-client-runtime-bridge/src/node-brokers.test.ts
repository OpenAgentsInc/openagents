import { mkdtemp, readdir, rm, symlink, writeFile, readFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";

import { afterEach, describe, expect, it } from "vite-plus/test";

import type { AcpAuthorityLease } from "./authority.js";
import {
  NodeBrokerFault,
  createNodeFilesystemBroker,
  createNodeTerminalBroker,
} from "./node-brokers.js";

const roots: string[] = [];

const temporary = async (name: string): Promise<string> => {
  const value = await mkdtemp(join(tmpdir(), `openagents-${name}-`));
  roots.push(value);
  return value;
};

afterEach(async () => {
  await Promise.all(roots.splice(0).map((root) => rm(root, { recursive: true, force: true })));
});

const lease: AcpAuthorityLease = {
  requestRef: "request.node_broker.1",
  sessionId: "session.node_broker.1",
  connectionRef: "connection.node_broker.1",
  generation: 4,
  scopeRef: "scope.workspace.1",
};

describe("createNodeFilesystemBroker", () => {
  it("normalizes relative/absolute paths, reads UTF-8 line windows, and emits refs only", async () => {
    const root = await temporary("fs-read");
    const target = join(root, "fixture.txt");
    await writeFile(target, "one\ntwo\nthree\nfour", "utf8");
    const broker = await createNodeFilesystemBroker({
      workspaceRoot: root,
      maxReadBytes: 100,
      nextEvidenceRef: () => "evidence.fs.read.1",
    });

    await expect(
      broker.readTextFile!(
        { sessionId: lease.sessionId, path: "fixture.txt", line: 2, limit: 2 },
        lease,
      ),
    ).resolves.toEqual({
      value: { content: "two\nthree" },
      evidenceRefs: ["evidence.fs.read.1"],
    });
    await expect(
      broker.readTextFile!(
        { sessionId: lease.sessionId, path: resolve(target), line: 1, limit: 1 },
        lease,
      ),
    ).resolves.toMatchObject({ value: { content: "one" } });
  });

  it("blocks lexical traversal plus existing-target and parent symlink escapes", async () => {
    const root = await temporary("fs-containment");
    const outside = await temporary("fs-outside");
    await writeFile(join(outside, "secret.txt"), "secret", "utf8");
    await symlink(join(outside, "secret.txt"), join(root, "escaped-file"));
    await symlink(outside, join(root, "escaped-directory"));
    const broker = await createNodeFilesystemBroker({ workspaceRoot: root });

    await expect(
      broker.readTextFile!({ sessionId: lease.sessionId, path: "../secret.txt" }, lease),
    ).rejects.toEqual(new NodeBrokerFault("outside_workspace"));
    await expect(
      broker.readTextFile!({ sessionId: lease.sessionId, path: "escaped-file" }, lease),
    ).rejects.toEqual(new NodeBrokerFault("outside_workspace"));
    await expect(
      broker.writeTextFile!(
        { sessionId: lease.sessionId, path: "escaped-directory/new.txt", content: "blocked" },
        lease,
      ),
    ).rejects.toEqual(new NodeBrokerFault("outside_workspace"));
    await expect(readFile(join(outside, "new.txt"), "utf8")).rejects.toThrow();
  });

  it("enforces byte caps, fatal UTF-8 reads, aborts, and atomic write cleanup", async () => {
    const root = await temporary("fs-bounds");
    await writeFile(join(root, "large.txt"), "12345", "utf8");
    await writeFile(join(root, "invalid.txt"), Buffer.from([0xff, 0xfe]));
    const broker = await createNodeFilesystemBroker({
      workspaceRoot: root,
      maxReadBytes: 4,
      maxWriteBytes: 8,
    });
    await expect(
      broker.readTextFile!({ sessionId: lease.sessionId, path: "large.txt" }, lease),
    ).rejects.toEqual(new NodeBrokerFault("byte_limit"));
    const utf8Broker = await createNodeFilesystemBroker({ workspaceRoot: root, maxReadBytes: 20 });
    await expect(
      utf8Broker.readTextFile!({ sessionId: lease.sessionId, path: "invalid.txt" }, lease),
    ).rejects.toEqual(new NodeBrokerFault("invalid_utf8"));
    await expect(
      broker.writeTextFile!(
        { sessionId: lease.sessionId, path: "written.txt", content: "too-many-bytes" },
        lease,
      ),
    ).rejects.toEqual(new NodeBrokerFault("byte_limit"));

    const controller = new AbortController();
    controller.abort();
    await expect(
      utf8Broker.readTextFile!(
        { sessionId: lease.sessionId, path: "large.txt" },
        { ...lease, signal: controller.signal },
      ),
    ).rejects.toEqual(new NodeBrokerFault("aborted"));

    await expect(
      broker.writeTextFile!(
        { sessionId: lease.sessionId, path: "written.txt", content: "new text" },
        lease,
      ),
    ).resolves.toMatchObject({ value: {} });
    expect(await readFile(join(root, "written.txt"), "utf8")).toBe("new text");
    expect((await readdir(root)).some((name) => name.startsWith(".openagents-acp-"))).toBe(false);
  });
});

describe("createNodeTerminalBroker", () => {
  it("pins trusted PATH provenance and redacts secrets by default", async () => {
    const root = await temporary("terminal-default-redaction");
    const broker = await createNodeTerminalBroker({
      workspaceRoot: root,
      allowedEnvNames: ["PATH"],
      baseEnv: { PATH: "/usr/bin" },
      allow: () => true,
      nextTerminalId: () => "terminal.default-redaction",
    });
    await broker.create(
      {
        sessionId: lease.sessionId,
        command: process.execPath,
        args: ["-e", "process.stdout.write(`${process.env.PATH}:sk-secretvalue123456`)"],
        env: [{ name: "PATH", value: root }],
      },
      lease,
    );
    await broker.waitForExit(
      { sessionId: lease.sessionId, terminalId: "terminal.default-redaction" },
      lease,
    );
    const output = await broker.output(
      { sessionId: lease.sessionId, terminalId: "terminal.default-redaction" },
      lease,
    );
    expect(output.value.output).toBe("/usr/bin:[redacted]");
    await broker.release(
      { sessionId: lease.sessionId, terminalId: "terminal.default-redaction" },
      lease,
    );
    await broker.dispose();
  });

  it("applies command policy, contains cwd, filters env, redacts output, and enforces ownership", async () => {
    const root = await temporary("terminal");
    const outside = await temporary("terminal-outside");
    await symlink(outside, join(root, "escape"));
    const policyInputs: unknown[] = [];
    const broker = await createNodeTerminalBroker({
      workspaceRoot: root,
      allowedEnvNames: ["SAFE"],
      baseEnv: { SAFE: "base", SECRET: "base-secret" },
      allow: (input) => {
        policyInputs.push(input);
        return input.executable === process.execPath;
      },
      redactOutput: (output) => output.replaceAll("raw-secret", "[REDACTED]"),
      nextTerminalId: () => "terminal.owned.1",
      nextEvidenceRef: (operation) => `evidence.terminal.${operation}`,
    });
    const created = await broker.create(
      {
        sessionId: lease.sessionId,
        command: process.execPath,
        args: [
          "-e",
          "process.stdout.write(`${process.env.SAFE}:${String(process.env.SECRET)}:raw-secret`)",
        ],
        cwd: root,
        env: [
          { name: "SAFE", value: "request" },
          { name: "SECRET", value: "request-secret" },
        ],
      },
      lease,
    );
    expect(created).toEqual({
      value: { terminalId: "terminal.owned.1" },
      evidenceRefs: ["evidence.terminal.create"],
    });
    expect(policyInputs).toMatchObject([{ envNames: ["SAFE"], scopeRef: lease.scopeRef }]);
    await broker.waitForExit({ sessionId: lease.sessionId, terminalId: "terminal.owned.1" }, lease);
    const output = await broker.output(
      { sessionId: lease.sessionId, terminalId: "terminal.owned.1" },
      lease,
    );
    expect(output.value.output).toBe("request:undefined:[REDACTED]");
    expect(JSON.stringify(output.evidenceRefs)).not.toContain("raw-secret");
    await expect(
      broker.output(
        { sessionId: lease.sessionId, terminalId: "terminal.owned.1" },
        { ...lease, generation: 3 },
      ),
    ).rejects.toEqual(new NodeBrokerFault("terminal_not_owned"));
    await broker.release({ sessionId: lease.sessionId, terminalId: "terminal.owned.1" }, lease);
    expect(broker.ownedTerminalCount()).toBe(0);

    await expect(
      broker.create(
        {
          sessionId: lease.sessionId,
          command: process.execPath,
          args: ["-e", ""],
          cwd: join(root, "escape"),
        },
        lease,
      ),
    ).rejects.toEqual(new NodeBrokerFault("outside_workspace"));
    await broker.dispose();
  });

  it("retains bounded valid UTF-8 tails and refuses release while running", async () => {
    const root = await temporary("terminal-bounds");
    const broker = await createNodeTerminalBroker({
      workspaceRoot: root,
      allow: () => true,
      maxOutputBytes: 9,
      nextTerminalId: () => "terminal.bounds.1",
    });
    await broker.create(
      {
        sessionId: lease.sessionId,
        command: process.execPath,
        args: ["-e", "setTimeout(() => process.stdout.write('prefix-🙂🙂'), 30)"],
      },
      lease,
    );
    await expect(
      broker.release({ sessionId: lease.sessionId, terminalId: "terminal.bounds.1" }, lease),
    ).rejects.toEqual(new NodeBrokerFault("terminal_running"));
    await broker.waitForExit(
      { sessionId: lease.sessionId, terminalId: "terminal.bounds.1" },
      lease,
    );
    const output = await broker.output(
      { sessionId: lease.sessionId, terminalId: "terminal.bounds.1" },
      lease,
    );
    expect(Buffer.byteLength(output.value.output, "utf8")).toBeLessThanOrEqual(9);
    expect(output.value.output).not.toContain("�");
    expect(output.value.truncated).toBe(true);
    await broker.dispose();
  });

  it("kills an owned child on authority abort and disposes remaining processes", async () => {
    const root = await temporary("terminal-abort");
    let id = 0;
    const broker = await createNodeTerminalBroker({
      workspaceRoot: root,
      allow: () => true,
      nextTerminalId: () => `terminal.abort.${++id}`,
    });
    const controller = new AbortController();
    const created = await broker.create(
      {
        sessionId: lease.sessionId,
        command: process.execPath,
        args: ["-e", "setInterval(() => {}, 1000)"],
      },
      { ...lease, signal: controller.signal },
    );
    controller.abort();
    const status = await broker.waitForExit(
      { sessionId: lease.sessionId, terminalId: created.value.terminalId },
      lease,
    );
    expect(status.value.signal).toBe("SIGKILL");

    await broker.create(
      {
        sessionId: lease.sessionId,
        command: process.execPath,
        args: ["-e", "setInterval(() => {}, 1000)"],
      },
      lease,
    );
    await broker.dispose();
    expect(broker.ownedTerminalCount()).toBe(0);
    await expect(broker.health()).resolves.toBe("unhealthy");
  });
});
