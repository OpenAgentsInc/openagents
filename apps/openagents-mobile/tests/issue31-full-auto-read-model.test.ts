import { readFileSync } from "node:fs";
import { describe, expect, test } from "vite-plus/test";

import {
  ISSUE31_FULL_AUTO_READ_MODEL_SCHEMA,
  canRenderIssue31FullAutoControls,
  readIssue31FullAutoProjection,
} from "../src/workroom/issue31-full-auto-read-model.ts";

const FIXTURE_ROOT = "../../../packages/sarah/fixtures/issue31-workroom";

const fixture = (name: string): unknown =>
  JSON.parse(readFileSync(new URL(`${FIXTURE_ROOT}/${name}`, import.meta.url), "utf8"));

const canonical = (): unknown =>
  fixture("openagents.omega.issue31.fullauto.v1.canonical.json");

const HOST = {
  hostRef: "host.omega.device-alpha",
  snapshotRef: "snapshot.omega.issue31.000042",
} as const;

const ready = (payload: unknown = canonical()) => {
  const model = readIssue31FullAutoProjection(payload, HOST);
  if (model.state !== "ready") throw new Error(`expected ready, got ${model.state}`);
  return model;
};

describe(ISSUE31_FULL_AUTO_READ_MODEL_SCHEMA, () => {
  test("shows conversation-adjacent run, provider, and evidence rows from one payload", () => {
    const model = ready();
    expect(model.runs).toHaveLength(2);
    expect(model.accounts).toHaveLength(3);
    // The whole point of omega#47: one Workroom, not three product surfaces.
    expect(model.runs.every((run) => run.evidence !== undefined)).toBe(true);
    expect(model.accounts.every((account) => account.laneRef.length > 0)).toBe(true);
  });

  test("maps each account to its lane and to the runs actually on that lane", () => {
    const model = ready();
    const codex1 = model.accounts.find((a) => a.accountRef === "account.codex.1");
    const codex2 = model.accounts.find((a) => a.accountRef === "account.codex.2");
    const claude = model.accounts.find((a) => a.accountRef === "account.claude.1");

    expect(codex1?.laneRef).toBe("lane.codex-local");
    expect(codex1?.runRefs).toEqual(["run.full-auto.run-01"]);
    // Same provider, different account, different lane, no runs. A lane is not
    // an account, and an idle account must not borrow another's work.
    expect(codex2?.laneRef).toBe("lane.codex-local-2");
    expect(codex2?.runRefs).toEqual([]);
    expect(claude?.runRefs).toEqual(["run.full-auto.run-00"]);
  });

  test("renders controls only on a live run, each bound to its exact generation", () => {
    const model = ready();
    const live = model.runs.find((run) => run.runRef === "run.full-auto.run-01");
    const finished = model.runs.find((run) => run.runRef === "run.full-auto.run-00");

    expect(canRenderIssue31FullAutoControls(live!)).toBe(true);
    expect(live?.controls.map((control) => control.kind)).toEqual(["pause", "stop"]);
    expect(live?.controls.every((control) => control.runGeneration === 7)).toBe(true);
    expect(live?.controls.every((control) => control.idempotencyRef.length > 0)).toBe(true);

    // A finished run offers nothing to press: a control whose completion can
    // never arrive is worse than no control.
    expect(canRenderIssue31FullAutoControls(finished!)).toBe(false);
    expect(finished?.controls).toEqual([]);
    expect(finished?.terminalReasonRef).toBe("reason.full-auto.objective-met");
  });

  test("carries the exact host-measured unattended duration", () => {
    const model = ready();
    // The phone must not recompute this from its own clock.
    expect(model.runs.map((run) => run.unattendedMs)).toEqual([5400000, 1980000]);
  });

  test("follows one finished unit from objective through authority receipt", () => {
    const model = ready();
    const finished = model.runs.find((run) => run.runRef === "run.full-auto.run-00");
    if (finished?.evidence.state !== "complete") throw new Error("expected a complete chain");
    expect(finished.evidence.hops.map((hop) => hop.kind)).toEqual([
      "objective",
      "turn",
      "change",
      "project_generation",
      "test",
      "typed_outcome",
      "host_verification",
      "authority_decision",
      "receipt",
    ]);
    expect(finished.evidence.authorityAllowed).toBe(true);
  });

  test("renders a broken chain as unavailable, not as partial proof", () => {
    const model = ready();
    const live = model.runs.find((run) => run.runRef === "run.full-auto.run-01");
    if (live?.evidence.state !== "unavailable") throw new Error("expected an unavailable chain");
    expect(live.evidence.reasonClass).toBe("hop_missing");
    expect(live.evidence.brokenAt).toBe("host_verification");
    expect("hops" in live.evidence).toBe(false);
  });

  test("reports a handoff's exact host-owned outcome and keeps unbound ones separate", () => {
    const model = ready();
    const claude = model.accounts.find((a) => a.accountRef === "account.claude.1");
    expect(claude?.handoff?.state).toBe("completed");
    expect(claude?.handoff?.outcomeRef).toBe("outcome.provider.claude.01");
    expect(claude?.handoff?.isTerminal).toBe(true);

    // Neither the pending xai request nor the refused openai login ever bound
    // to an account, so neither can be attributed to one. A refused login in
    // particular must not appear against an unrelated working account.
    expect(model.unboundHandoffs.map((handoff) => handoff.provider)).toEqual(["xai", "openai"]);
    expect(model.unboundHandoffs[0]?.state).toBe("requested");
    expect(model.unboundHandoffs[0]?.outcomeRef).toBeNull();
    expect(model.unboundHandoffs[1]?.state).toBe("refused");
    expect(model.unboundHandoffs[1]?.reasonClass).toBe("provider_login_requires_host");
    expect(model.unboundHandoffs[1]?.outcomeRef).toBe("outcome.provider.codex.03");

    // The codex accounts stay clean: a refused handoff for the same provider
    // is not evidence about an account it never touched.
    expect(model.accounts.find((a) => a.accountRef === "account.codex.1")?.handoff).toBeNull();
    expect(model.accounts.find((a) => a.accountRef === "account.codex.2")?.handoff).toBeNull();
  });

  test("refuses a payload bound to a different host snapshot", () => {
    const model = readIssue31FullAutoProjection(canonical(), {
      hostRef: HOST.hostRef,
      snapshotRef: "snapshot.omega.issue31.000043",
    });
    expect(model.state).toBe("unavailable");
    if (model.state !== "unavailable") throw new Error("unreachable");
    expect(model.reason).toBe("snapshot_mismatch");
  });

  test("reports an absent host projection rather than an empty run list", () => {
    const model = readIssue31FullAutoProjection(canonical(), null);
    expect(model.state).toBe("unavailable");
    if (model.state !== "unavailable") throw new Error("unreachable");
    expect(model.reason).toBe("no_host_projection");
  });

  test.for([
    "credential-label",
    "private-path",
    "stale-generation",
    "lane-as-account",
    "partial-chain",
    "self-reported",
    "terminal-control",
    "handoff-no-outcome",
  ])("turns the %s violation into an explicit unreadable state", (suffix) => {
    const model = readIssue31FullAutoProjection(
      fixture(`openagents.omega.issue31.fullauto.v1.negative-${suffix}.json`),
      HOST,
    );
    expect(model.state).toBe("unavailable");
    if (model.state !== "unavailable") throw new Error("unreachable");
    // A host defect must not crash the phone, and must not be quietly
    // downgraded into a partial view either.
    expect(model.reason).toBe("host_projection_unreadable");
  });
});
