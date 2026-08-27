import { describe, expect, test } from "vite-plus/test";

import { QaSwarmRegressionCandidate } from "./index";
import { Schema as S } from "effect";

describe("QA Swarm shared projection", () => {
  test("distinguishes validated, proposed, and reviewed-merge-only landed regressions", () => {
    const decode = S.decodeUnknownSync(QaSwarmRegressionCandidate);

    expect(
      decode({
        _tag: "validated",
        candidateRef: "candidate.example",
        discoveryRef: "discovery.example",
        label: "Example",
        rerunReceiptRef: "receipt.rerun.example",
        testHref: "generated/example.e2e.test.ts",
      })._tag,
    ).toBe("validated");
    expect(
      decode({
        _tag: "proposed",
        candidateRef: "candidate.example",
        commitProposalRef: "commit-proposal:example",
        discoveryRef: "discovery.example",
        issueRef: "github.issue:example",
        label: "Example",
        pullRequestRef: "github.pr:example",
        rerunReceiptRef: "receipt.rerun.example",
        testHref: "generated/example.e2e.test.ts",
      })._tag,
    ).toBe("proposed");
    expect(() =>
      decode({
        _tag: "proposed",
        candidateRef: "candidate.example",
        commitProposalRef: "commit-proposal:example",
        discoveryRef: "discovery.example",
        label: "Example",
        pullRequestRef: "github.pr:example",
        rerunReceiptRef: "receipt.rerun.example",
        testHref: "generated/example.e2e.test.ts",
      }),
    ).toThrow();
    expect(() =>
      decode({
        _tag: "landed",
        candidateRef: "candidate.example",
        discoveryRef: "discovery.example",
        label: "Example",
        mergedCommitRef: "git.commit:example",
        pullRequestRef: "github.pr:example",
        rerunReceiptRef: "receipt.rerun.example",
        testHref: "generated/example.e2e.test.ts",
      }),
    ).toThrow();
  });
});
