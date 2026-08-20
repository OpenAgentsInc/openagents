import * as NodeServices from "@effect/platform-node/NodeServices";
import { Effect, Layer } from "effect";
import { execFileSync } from "node:child_process";
import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { describe, expect, it } from "vitest";

import {
  GitRunner,
  gitCloneArgv,
  gitRunnerLayer,
  repositoryFromRemoteUrl,
} from "../src/git-runner.js";

describe("git clone argument construction", () => {
  it("keeps repository URLs and destination names as literal argv values", () => {
    const argv = gitCloneArgv({
      url: "http://localhost:4000/git/octavia/project.git",
      directory: "--upload-pack=malicious",
    });
    expect(argv).toEqual([
      "-c",
      "credential.helper=",
      "-c",
      "credential.http://localhost:4000.helper=!openagents --api-url http://localhost:4000 auth git-credential",
      "clone",
      "--",
      "http://localhost:4000/git/octavia/project.git",
      "--upload-pack=malicious",
    ]);
  });

  it("never constructs a shell command or credential-bearing URL", () => {
    const argv = gitCloneArgv({ url: "https://openagents.com/git/octavia/project.git" });
    expect(argv).toEqual([
      "-c",
      "credential.helper=",
      "-c",
      "credential.https://openagents.com.helper=!openagents --api-url https://openagents.com auth git-credential",
      "clone",
      "--",
      "https://openagents.com/git/octavia/project.git",
    ]);
    expect(argv.join(" ")).not.toContain("Authorization");
    expect(argv.join(" ")).not.toContain("token");
  });

  it("accepts only exact repository URLs from the selected API origin", async () => {
    await expect(
      Effect.runPromise(
        repositoryFromRemoteUrl(
          "https://openagents.com",
          "https://openagents.com/git/octavia/project.git",
        ),
      ),
    ).resolves.toBe("octavia/project");
    await expect(
      Effect.runPromise(
        repositoryFromRemoteUrl(
          "https://openagents.com",
          "https://token@openagents.com/git/octavia/project.git",
        ),
      ),
    ).rejects.toThrow("not an admitted OpenAgents repository URL");
    await expect(
      Effect.runPromise(
        repositoryFromRemoteUrl(
          "https://openagents.com",
          "https://example.com/git/octavia/project.git",
        ),
      ),
    ).rejects.toThrow("not an admitted OpenAgents repository URL");
  });

  it("attaches an absent remote, infers it, and refuses an unrelated collision", async () => {
    const directory = await mkdtemp(join(tmpdir(), "openagents-cli-git-"));
    try {
      execFileSync("git", ["init", "--quiet", directory]);
      const layer = gitRunnerLayer.pipe(Layer.provide(NodeServices.layer));
      const attach = (url: string) =>
        Effect.gen(function* () {
          const git = yield* GitRunner;
          return yield* git.attachRemote({
            origin: "http://localhost:4000",
            url,
            directory,
            remote: "origin",
          });
        }).pipe(Effect.provide(layer));

      await expect(
        Effect.runPromise(attach("http://localhost:4000/git/octavia/project.git")),
      ).resolves.toEqual({
        remote: "origin",
        nextPushArguments: ["push", "-u", "origin", "HEAD"],
      });
      await Effect.runPromise(
        Effect.gen(function* () {
          const git = yield* GitRunner;
          yield* git.configureCredentialHelper("http://localhost:4000", "local", directory);
        }).pipe(Effect.provide(layer)),
      );
      expect(
        execFileSync(
          "git",
          [
            "-C",
            directory,
            "config",
            "--local",
            "--get-all",
            "credential.http://localhost:4000.helper",
          ],
          { encoding: "utf8" },
        ).split("\n"),
      ).toEqual(["", "!openagents --api-url http://localhost:4000 auth git-credential", ""]);
      await expect(
        Effect.runPromise(
          Effect.gen(function* () {
            const git = yield* GitRunner;
            return yield* git.inferRepository("http://localhost:4000", directory);
          }).pipe(Effect.provide(layer)),
        ),
      ).resolves.toBe("octavia/project");
      await expect(
        Effect.runPromise(attach("http://localhost:4000/git/octavia/other.git")),
      ).rejects.toThrow("did not overwrite it");
      expect(
        execFileSync("git", ["-C", directory, "remote", "get-url", "origin"], {
          encoding: "utf8",
        }).trim(),
      ).toBe("http://localhost:4000/git/octavia/project.git");
    } finally {
      await rm(directory, { recursive: true, force: true });
    }
  });
});
