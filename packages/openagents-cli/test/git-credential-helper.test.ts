import { describe, expect, it } from "vitest";

import {
  credentialHelperCommand,
  parseGitCredentialRequest,
} from "../src/git-credential-helper.js";

describe("Git credential helper", () => {
  it("parses only admitted protocol fields without retaining credential input", () => {
    expect(
      parseGitCredentialRequest(
        "protocol=https\nhost=openagents.com\npath=git/octavia/project.git\nusername=ignored\npassword=ignored\n",
      ),
    ).toEqual({
      protocol: "https",
      host: "openagents.com",
      path: "git/octavia/project.git",
    });
  });

  it("constructs an origin-scoped helper without a credential", () => {
    const command = credentialHelperCommand("https://openagents.com");
    expect(command).toBe("!openagents --api-url https://openagents.com auth git-credential");
    expect(command).not.toContain("oa_pat_");
  });
});
