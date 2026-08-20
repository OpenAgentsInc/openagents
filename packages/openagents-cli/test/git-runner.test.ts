import { describe, expect, it } from "vitest";

import { gitCloneArgv } from "../src/git-runner.js";

describe("git clone argument construction", () => {
  it("keeps repository URLs and destination names as literal argv values", () => {
    const argv = gitCloneArgv({
      url: "http://localhost:4000/git/octavia/project.git",
      directory: "--upload-pack=malicious",
    });
    expect(argv).toEqual([
      "clone",
      "--",
      "http://localhost:4000/git/octavia/project.git",
      "--upload-pack=malicious",
    ]);
  });

  it("never constructs a shell command or credential-bearing URL", () => {
    const argv = gitCloneArgv({ url: "https://openagents.com/git/octavia/project.git" });
    expect(argv).toEqual(["clone", "--", "https://openagents.com/git/octavia/project.git"]);
    expect(argv.join(" ")).not.toContain("Authorization");
    expect(argv.join(" ")).not.toContain("token");
  });
});
