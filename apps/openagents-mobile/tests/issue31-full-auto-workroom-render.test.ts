/**
 * The rendered half of omega#47.
 *
 * The read model already refuses to describe a projection dishonestly. These
 * tests watch the Workroom actually refuse to draw one: a broken evidence chain
 * must not put a single hop on screen, an accepted control must not read as
 * finished, and a refused provider login must not appear against a working
 * account. A fail-closed path nobody watched refuse is not proven to fail
 * closed.
 */
import { readFileSync } from "node:fs";
import { describe, expect, test } from "vite-plus/test";

import { defaultMobileAccessibilityProfile } from "../src/screens/khala-core.ts";
import { emptyIssue31WorkroomReadModel } from "../src/workroom/issue31-workroom-read-model.ts";
import { initialIssue31MobileNostrControlState } from "../src/workroom/issue31-mobile-nostr-runtime.ts";
import {
  readIssue31FullAutoProjection,
  type Issue31FullAutoReadModel,
} from "../src/workroom/issue31-full-auto-read-model.ts";
import type { Issue31OwnerCommandState } from "../src/workroom/issue31-owner-private-read-model.ts";
import { renderMobileIssue31WorkroomView } from "../src/screens/mobile-issue31-workroom-view.ts";
import { emptyIssue31CommunityReadModel } from "../src/workroom/issue31-community-read-model.ts";

const FIXTURE_ROOT = "../../../packages/sarah/fixtures/issue31-workroom";

const canonical = (): Record<string, unknown> =>
  JSON.parse(
    readFileSync(
      new URL(`${FIXTURE_ROOT}/openagents.omega.issue31.fullauto.v1.canonical.json`, import.meta.url),
      "utf8",
    ),
  ) as Record<string, unknown>;

const HOST = {
  hostRef: "host.omega.device-alpha",
  snapshotRef: "snapshot.omega.issue31.000042",
} as const;

const render = (
  fullAuto: Issue31FullAutoReadModel,
  commands: ReadonlyArray<Issue31OwnerCommandState> = [],
): string => {
  const workroom = emptyIssue31WorkroomReadModel();
  const view = renderMobileIssue31WorkroomView(
    { ...workroom, ownerPrivate: { ...workroom.ownerPrivate, commands } },
    "owner_private",
    initialIssue31MobileNostrControlState(),
    defaultMobileAccessibilityProfile,
    {
      draft: "",
      memoryQuery: "",
      reminderDraft: "",
      transcriptLimit: 20,
      notice: null,
    },
    fullAuto,
    emptyIssue31CommunityReadModel(),
    { draft: "", subject: "", appealDraft: "", notice: null },
  );
  return JSON.stringify(view);
};

const ready = (
  payload: unknown = canonical(),
): Extract<Issue31FullAutoReadModel, { state: "ready" }> => {
  const model = readIssue31FullAutoProjection(payload, HOST);
  if (model.state !== "ready") throw new Error(`expected ready, got ${model.state}`);
  return model;
};

describe("issue 31 Workroom Full Auto rendering", () => {
  test("shows conversation, run, provider, and evidence in one Workroom", () => {
    const serialized = render(ready());
    // The conversation is already here; these three join it rather than living
    // behind a separate product surface.
    expect(serialized).toContain("Conversation");
    expect(serialized).toContain("Full Auto");
    expect(serialized).toContain("Provider accounts");
    expect(serialized).toContain("Evidence complete");
  });

  test("states the exact host-measured unattended duration, not a device clock", () => {
    const serialized = render(ready());
    // 5_400_000 ms exactly as the host reported it.
    expect(serialized).toContain("01:30:00 unattended");
  });

  test("a viewer can follow one finished unit from objective to authority receipt", () => {
    const serialized = render(ready());
    for (const hop of [
      "objective",
      "turn",
      "change",
      "project_generation",
      "test",
      "typed_outcome",
      "host_verification",
      "authority_decision",
      "receipt",
    ]) {
      expect(serialized).toContain(hop);
    }
  });

  test("a broken chain renders as unavailable and draws no hop at all", () => {
    const payload = canonical();
    const evidence = payload["evidence"] as Array<Record<string, unknown>>;
    // Only the broken chain remains, so any hop text on screen came from it.
    payload["evidence"] = evidence.filter((chain) => chain["completeness"] === "unavailable");
    const serialized = render(ready(payload));
    expect(serialized).toContain("Evidence unavailable");
    expect(serialized).not.toContain("Evidence complete");
    expect(serialized).not.toContain("authority_decision");
    expect(serialized).not.toContain("host_verification ·");
  });

  test.for([
    ["hop_missing", "a step of the chain is missing"],
    ["hop_mismatched", "two records disagree about this run"],
    ["hop_private", "a step cannot be shown on this device"],
    ["self_reported", "the run reported its own success"],
  ])("names %s in words the owner can act on", ([reasonClass, copy]) => {
    const payload = canonical();
    payload["evidence"] = [
      { completeness: "unavailable", runRef: "run.full-auto.run-01", reasonClass },
    ];
    const serialized = render(ready(payload));
    expect(serialized).toContain(copy);
    expect(serialized).not.toContain("Evidence complete");
  });

  test("an accepted control never reads as finished", () => {
    const model = ready();
    const live = model.runs.find((run) => run.runRef === "run.full-auto.run-01");
    const control = live?.controls[0];
    if (control === undefined) throw new Error("expected a live control");
    const serialized = render(model, [
      {
        state: "accepted",
        intentEventId: "event.intent.0001",
        actionRef: control.actionRef,
        idempotencyRef: control.idempotencyRef,
        handlingRef: "handling.host.0001",
        sourceEventId: "event.projection.0009",
      },
    ]);
    expect(serialized).toContain("not finished");
    expect(serialized).not.toContain("Completed by your Omega host");
  });

  test("a control completes on screen only from an Omega-owned terminal result", () => {
    const model = ready();
    const live = model.runs.find((run) => run.runRef === "run.full-auto.run-01");
    const control = live?.controls[0];
    if (control === undefined) throw new Error("expected a live control");
    const serialized = render(model, [
      {
        state: "terminal",
        intentEventId: "event.intent.0001",
        actionRef: control.actionRef,
        idempotencyRef: control.idempotencyRef,
        handlingRef: "handling.host.0001",
        sourceEventId: "event.projection.0009",
      },
    ]);
    expect(serialized).toContain("Completed by your Omega host · event.projection.0009");
  });

  test("a finished run offers nothing to press", () => {
    const payload = canonical();
    const runs = payload["runs"] as Array<Record<string, unknown>>;
    payload["runs"] = runs.filter((run) => run["runRef"] === "run.full-auto.run-00");
    payload["evidence"] = (payload["evidence"] as Array<Record<string, unknown>>).filter(
      (chain) => chain["runRef"] === "run.full-auto.run-00",
    );
    const serialized = render(ready(payload));
    expect(serialized).toContain("This run has finished. No control can change it.");
  });

  test("states the account-to-lane relation instead of implying it", () => {
    const serialized = render(ready());
    expect(serialized).toContain("Account account.codex.1 serves lane lane.codex-local");
    // The idle second codex account borrows nothing from the first.
    expect(serialized).toContain("Account account.codex.2 serves lane lane.codex-local-2");
    expect(serialized).toContain("no runs on that lane");
  });

  test("a refused provider login stays unattributed and the host keeps the credential", () => {
    const serialized = render(ready());
    expect(serialized).toContain("not bound to an account");
    expect(serialized).toContain("provider_login_requires_host");
    expect(serialized).toContain("Your Omega host holds every provider login and token");
  });

  test("an absent projection is stated, never drawn as an empty run list", () => {
    const serialized = render(readIssue31FullAutoProjection(canonical(), null));
    expect(serialized).toContain("not paired to an Omega host");
    // No account list, no run list, no connect action — the section states its
    // absence and stops. (The capability card of the same name is a separate
    // row and legitimately still describes what this source would show.)
    expect(serialized).not.toContain("issue31-fa-accounts-title");
    expect(serialized).not.toContain("issue31-fa-runs-empty");
    expect(serialized).not.toContain("issue31-fa-connect-provider");
  });

  test("a payload from a different snapshot is withheld rather than shown as current", () => {
    const stale = readIssue31FullAutoProjection(canonical(), {
      hostRef: HOST.hostRef,
      snapshotRef: "snapshot.omega.issue31.000043",
    });
    const serialized = render(stale);
    expect(serialized).toContain("different host snapshot");
    expect(serialized).not.toContain("Evidence complete");
  });

  test("no credential, token, or private path can appear anywhere on the surface", () => {
    const serialized = render(ready());
    for (const forbidden of ["Bearer", "sk-", "auth.json", "/Users/", "access_token"]) {
      expect(serialized).not.toContain(forbidden);
    }
  });
});
