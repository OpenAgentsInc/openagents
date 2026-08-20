import { spawn } from "node:child_process";
import { once } from "node:events";
import { chmod, mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { createServer as createHttpServer } from "node:http";
import { createServer, type Socket } from "node:net";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";

import { afterEach, describe, expect, it } from "vitest";

const children = new Set<ReturnType<typeof spawn>>();

afterEach(() => {
  for (const child of children) child.kill("SIGKILL");
  children.clear();
});

describe.each(["SIGINT", "SIGTERM"] as const)("%s handling", (signal) => {
  it("cancels an in-flight API request and exits with status 130", async () => {
    const sockets = new Set<Socket>();
    let receivedRequest: (() => void) | undefined;
    const requestReceived = new Promise<void>((resolveRequest) => {
      receivedRequest = resolveRequest;
    });
    const server = createServer((socket) => {
      sockets.add(socket);
      socket.once("close", () => sockets.delete(socket));
      socket.once("data", () => receivedRequest?.());
    });
    server.listen(0, "127.0.0.1");
    await once(server, "listening");
    const address = server.address();
    if (address === null || typeof address === "string")
      throw new Error("missing test server port");

    const child = spawn(
      process.execPath,
      [
        "--import",
        "tsx",
        resolve(import.meta.dirname, "../src/main.ts"),
        "--api-url",
        `http://127.0.0.1:${address.port}`,
        "repo",
        "list",
      ],
      {
        cwd: resolve(import.meta.dirname, "../../.."),
        env: {
          ...process.env,
          OPENAGENTS_TOKEN: "oa_pat_signal-fixture",
          NO_COLOR: "1",
        },
        stdio: ["ignore", "pipe", "pipe"],
      },
    );
    children.add(child);

    await Promise.race([
      requestReceived,
      new Promise<never>((_resolve, reject) =>
        setTimeout(() => reject(new Error("CLI did not start its API request")), 10_000),
      ),
    ]);
    child.kill(signal);
    const [code, exitSignal] = (await once(child, "exit")) as [number | null, string | null];
    children.delete(child);

    await new Promise((resolveWait) => setTimeout(resolveWait, 25));
    expect(code).toBe(130);
    expect(exitSignal).toBeNull();
    expect(sockets.size).toBe(0);

    const closed = once(server, "close");
    server.close();
    await closed;
  }, 15_000);
});

it("terminates an in-flight Git child process before exiting with status 130", async () => {
  const directory = await mkdtemp(join(tmpdir(), "openagents-cli-signal-"));
  const marker = join(directory, "git-marker");
  const fakeGit = join(directory, "git");
  await writeFile(
    fakeGit,
    "#!/bin/sh\nprintf 'started' > \"$FAKE_GIT_MARKER\"\ntrap 'printf terminated > \"$FAKE_GIT_MARKER\"; exit 0' TERM INT\nwhile :; do /bin/sleep 1; done\n",
  );
  await chmod(fakeGit, 0o700);

  const server = createHttpServer((_request, response) => {
    const address = server.address();
    if (address === null || typeof address === "string")
      throw new Error("missing test server port");
    response.writeHead(200, { "content-type": "application/json" });
    response.end(
      JSON.stringify({
        id: "repository-1",
        name: "project",
        full_name: "octavia/project",
        owner: { id: 10, login: "octavia", type: "User" },
        private: true,
        visibility: "private",
        description: null,
        default_branch: "main",
        lifecycle_state: "ready",
        provision_error_code: null,
        clone_url: `http://127.0.0.1:${address.port}/git/octavia/project.git`,
        html_url: `http://127.0.0.1:${address.port}/octavia/project`,
        permissions: { admin: true, push: true, pull: true },
        created_at: "2026-08-20T00:00:00Z",
        updated_at: "2026-08-20T00:00:00Z",
      }),
    );
  });
  server.listen(0, "127.0.0.1");
  await once(server, "listening");
  const address = server.address();
  if (address === null || typeof address === "string") throw new Error("missing test server port");

  const child = spawn(
    process.execPath,
    [
      "--import",
      "tsx",
      resolve(import.meta.dirname, "../src/main.ts"),
      "--api-url",
      `http://127.0.0.1:${address.port}`,
      "repo",
      "clone",
      "octavia/project",
      join(directory, "clone"),
    ],
    {
      cwd: resolve(import.meta.dirname, "../../.."),
      env: {
        ...process.env,
        OPENAGENTS_TOKEN: "oa_pat_signal-fixture",
        FAKE_GIT_MARKER: marker,
        PATH: `${directory}:${process.env.PATH ?? ""}`,
      },
      stdio: ["ignore", "pipe", "pipe"],
    },
  );
  children.add(child);

  await waitForFile(marker, "started");
  child.kill("SIGINT");
  const [code, exitSignal] = (await once(child, "exit")) as [number | null, string | null];
  children.delete(child);

  expect(code).toBe(130);
  expect(exitSignal).toBeNull();
  expect(await readFile(marker, "utf8")).toBe("terminated");

  const closed = once(server, "close");
  server.close();
  await closed;
  await rm(directory, { recursive: true, force: true });
}, 15_000);

const waitForFile = async (path: string, contents: string): Promise<void> => {
  const deadline = Date.now() + 10_000;
  const check = async (): Promise<void> => {
    try {
      if ((await readFile(path, "utf8")) === contents) return;
    } catch {
      // The child has not created the marker yet.
    }
    if (Date.now() >= deadline) throw new Error("Git child process did not start");
    await new Promise((resolveWait) => setTimeout(resolveWait, 10));
    return check();
  };
  return check();
};
