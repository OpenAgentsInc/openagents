import { describe, expect, test } from "vite-plus/test";
import { Schema } from "effect";
import { readFileSync } from "node:fs";

import { Ide13PackagedOwnerLocalJourneyTraceSchema } from "./ide-portable-packaged-owner-local-journey.ts";

describe("IDE-13 packaged owner-local journey contract", () => {
  test("keeps package participation distinct from the production owner-local target", () => {
    const source = readFileSync(
      new URL("./ide-portable-packaged-owner-local-journey.ts", import.meta.url),
      "utf8",
    );
    expect(source).toContain("packaged_shell_concurrent_owner_local_target");
    expect(source).toContain("one_physical_host_two_logical_owner_local_targets");
    expect(source).toContain("authenticatedSyncClaimed");
    expect(source).toContain("initiatedMoveClaimed");
  });

  test("rejects a trace that claims authenticated Sync", () => {
    const decode = Schema.decodeUnknownSync(Ide13PackagedOwnerLocalJourneyTraceSchema);
    expect(() =>
      decode({
        schemaVersion: "openagents.desktop.ide-portable-packaged-owner-local-composite-trace.v1",
        issue: "IDE-13",
        candidateCommitSha: "a".repeat(40),
        artifactTreeSha256: "b".repeat(64),
        events: [],
        privateMaterialIncluded: false,
        authenticatedSyncClaimed: true,
        packagedShellInitiatedMoveClaimed: false,
      }),
    ).toThrow();
  });
});
