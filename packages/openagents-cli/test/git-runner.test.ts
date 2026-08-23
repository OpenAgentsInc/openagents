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
  orderedRemoteNames,
  parseGitRemotes,
  repositoryFromRemoteUrl,
} from "../src/git-runner.js";

describe("git clone argument construction", () => {
  it("keeps repository URLs and destination names as literal argv values", () => {
    const argv = gitCloneArgv({
      url: "http://localhost:4000/octavia/project.git",
      directory: "--upload-pack=malicious",
    });
    expect(argv).toEqual([
      "-c",
      "credential.helper=",
      "-c",
      "credential.http://localhost:4000.helper=!openagents --api-url http://localhost:4000 auth git-credential",
      "clone",
      "--",
      "http://localhost:4000/octavia/project.git",
      "--upload-pack=malicious",
    ]);
  });

  it("never constructs a shell command or credential-bearing URL", () => {
    const argv = gitCloneArgv({ url: "https://openagents.com/octavia/project.git" });
    expect(argv).toEqual([
      "-c",
      "credential.helper=",
      "-c",
      "credential.https://openagents.com.helper=!openagents --api-url https://openagents.com auth git-credential",
      "clone",
      "--",
      "https://openagents.com/octavia/project.git",
    ]);
    expect(argv.join(" ")).not.toContain("Authorization");
    expect(argv.join(" ")).not.toContain("token");
  });

  it("accepts only exact repository URLs from the selected API origin", async () => {
    await expect(
      Effect.runPromise(
        repositoryFromRemoteUrl(
          "https://openagents.com",
          "https://openagents.com/octavia/project.git",
        ),
      ),
    ).resolves.toBe("octavia/project");
    await expect(
      Effect.runPromise(
        repositoryFromRemoteUrl(
          "https://openagents.com",
          "https://token@openagents.com/octavia/project.git",
        ),
      ),
    ).rejects.toThrow("not an admitted OpenAgents repository URL");
    await expect(
      Effect.runPromise(
        repositoryFromRemoteUrl(
          "https://openagents.com",
          "https://example.com/octavia/project.git",
        ),
      ),
    ).rejects.toThrow("not an admitted OpenAgents repository URL");
    await expect(
      Effect.runPromise(
        repositoryFromRemoteUrl(
          "https://openagents.com",
          "https://openagents.com/git/octavia/project.git",
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
        Effect.runPromise(attach("http://localhost:4000/octavia/project.git")),
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
        Effect.runPromise(attach("http://localhost:4000/octavia/other.git")),
      ).rejects.toThrow("did not overwrite it");
      expect(
        execFileSync("git", ["-C", directory, "remote", "get-url", "origin"], {
          encoding: "utf8",
        }).trim(),
      ).toBe("http://localhost:4000/octavia/project.git");
    } finally {
      await rm(directory, { recursive: true, force: true });
    }
  });
});

const FORGE = "http://localhost:4000";

/** A throwaway checkout carrying exactly the remotes a case is about. */
const withRemotes = async (
  remotes: ReadonlyArray<readonly [string, string]>,
  assert: (directory: string) => Promise<void>,
) => {
  const directory = await mkdtemp(join(tmpdir(), "openagents-cli-remotes-"));
  try {
    execFileSync("git", ["init", "--quiet", directory]);
    for (const [name, url] of remotes) {
      execFileSync("git", ["-C", directory, "remote", "add", name, url]);
    }
    await assert(directory);
  } finally {
    await rm(directory, { recursive: true, force: true });
  }
};

const infer = (directory: string) =>
  Effect.runPromise(
    Effect.gen(function* () {
      const git = yield* GitRunner;
      return yield* git.inferRepository(FORGE, directory);
    }).pipe(Effect.provide(gitRunnerLayer.pipe(Layer.provide(NodeServices.layer)))),
  );

describe("repository inference from Git remotes", () => {
  it("reads one URL per remote and prefers the fetch URL", () => {
    expect(
      parseGitRemotes(
        [
          "openagents\thttp://localhost:4000/octavia/project.git (fetch)",
          "openagents\thttp://localhost:4000/octavia/project.git (push)",
          "mirror\thttps://github.com/octavia/project.git (fetch)",
          "mirror\tno-push (push)",
        ].join("\n"),
      ),
    ).toEqual([
      { name: "openagents", url: "http://localhost:4000/octavia/project.git" },
      { name: "mirror", url: "https://github.com/octavia/project.git" },
    ]);
  });

  it("tries origin, then the documented forge name, then the rest as listed", () => {
    expect(orderedRemoteNames(["upstream", "openagents", "origin"])).toEqual([
      "origin",
      "openagents",
      "upstream",
    ]);
    expect(orderedRemoteNames(["zulu", "alpha"])).toEqual(["zulu", "alpha"]);
  });

  it("infers from a forge remote named openagents, the name this project mandates", async () => {
    await withRemotes([["openagents", `${FORGE}/octavia/project.git`]], async (directory) => {
      await expect(infer(directory)).resolves.toBe("octavia/project");
    });
  });

  it("still infers where the forge remote is named origin", async () => {
    await withRemotes([["origin", `${FORGE}/octavia/project.git`]], async (directory) => {
      await expect(infer(directory)).resolves.toBe("octavia/project");
    });
  });

  it("ignores a GitHub mirror and takes the forge remote beside it", async () => {
    await withRemotes(
      [
        ["origin", "https://github.com/octavia/project.git"],
        ["openagents", `${FORGE}/octavia/forge-name.git`],
      ],
      async (directory) => {
        await expect(infer(directory)).resolves.toBe("octavia/forge-name");
      },
    );
  });

  it("prefers origin when both remotes are on the forge, so existing setups do not move", async () => {
    await withRemotes(
      [
        ["openagents", `${FORGE}/octavia/second.git`],
        ["origin", `${FORGE}/octavia/first.git`],
      ],
      async (directory) => {
        await expect(infer(directory)).resolves.toBe("octavia/first");
      },
    );
  });

  it("refuses a checkout whose only remote is a GitHub mirror, and says why", async () => {
    await withRemotes([["origin", "https://github.com/octavia/project.git"]], async (directory) => {
      await expect(infer(directory)).rejects.toThrow(/origin points at https:\/\/github\.com/u);
      await expect(infer(directory)).rejects.toThrow(/name does not decide this/u);
    });
  });

  it("names a remote on the forge whose path is not a repository", async () => {
    await withRemotes([["openagents", `${FORGE}/octavia/deep/project.git`]], async (directory) => {
      await expect(infer(directory)).rejects.toThrow(
        /openagents is not an OWNER\/REPO\.git path on that origin/u,
      );
    });
  });

  it("refuses a checkout with no remotes at all", async () => {
    await withRemotes([], async (directory) => {
      await expect(infer(directory)).rejects.toThrow(/has no Git remotes/u);
    });
  });
});
