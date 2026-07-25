/**
 * The rendered half of omega#46 exit 4.
 *
 * The read model can compute coverage perfectly and the phone can still draw a
 * short list that looks whole — that was the defect. These tests watch the
 * Workroom actually say it: three coverage states that render differently, a
 * withheld count that names its cause and reason on screen, a lower bound that
 * is never drawn as an exact number, and the rows that did arrive still drawn
 * beside the count that did not.
 *
 * This is source-level rendering over the Effect Native view, not a screenshot.
 * A physical device rendering stays on omega#49.
 */
import { describe, expect, test } from "vite-plus/test";

import { defaultMobileAccessibilityProfile } from "../src/screens/khala-core.ts";
import { renderMobileIssue31WorkroomView } from "../src/screens/mobile-issue31-workroom-view.ts";
import { emptyIssue31CommunityReadModel } from "../src/workroom/issue31-community-read-model.ts";
import { readIssue31FullAutoProjection } from "../src/workroom/issue31-full-auto-read-model.ts";
import {
  emptyIssue31OwnerPrivateReadModel,
  type Issue31OwnerCoverage,
  type Issue31OwnerWithheldRow,
} from "../src/workroom/issue31-owner-private-read-model.ts";
import { initialIssue31MobileNostrControlState } from "../src/workroom/issue31-mobile-nostr-runtime.ts";
import { emptyIssue31WorkroomReadModel } from "../src/workroom/issue31-workroom-read-model.ts";

const engram = {
  sourceEventId: "e".repeat(64),
  sourceCreatedAt: 1_784_937_650,
  dTag: "f".repeat(64),
  body: { slug: "mem/release_evidence", value: "The release candidate is notarized." },
} as const;

const render = (
  coverage: Issue31OwnerCoverage,
  withheld: ReadonlyArray<Issue31OwnerWithheldRow> = [],
  // Local memory only draws search results, so a query is how a row reaches the
  // screen at all. The coverage line is drawn either way.
  memoryQuery = "",
): string => {
  const workroom = emptyIssue31WorkroomReadModel();
  const view = renderMobileIssue31WorkroomView(
    {
      ...workroom,
      ownerPrivate: {
        ...emptyIssue31OwnerPrivateReadModel(),
        status: coverage === "partial" ? "gap" : "ready",
        memory: [engram],
        coverage,
        withheld,
      },
    },
    "owner_private",
    initialIssue31MobileNostrControlState(),
    defaultMobileAccessibilityProfile,
    {
      draft: "",
      memoryQuery,
      reminderDraft: "",
      transcriptLimit: 20,
      notice: null,
    },
    readIssue31FullAutoProjection(null, {
      hostRef: "host.omega.device-alpha",
      snapshotRef: "snapshot.omega.issue31.000042",
    }),
    emptyIssue31CommunityReadModel(),
    { draft: "", subject: "", appealDraft: "", notice: null },
  );
  return JSON.stringify(view);
};

const quarantined: Issue31OwnerWithheldRow = {
  cause: "quarantined",
  count: 3,
  exact: true,
  reasonRef: "reason.omega.invalid_projection_source",
  observedBy: "host",
  deepLink: "openagents://omega/workroom?room=owner_private&withheld=quarantined",
};

const scanBound: Issue31OwnerWithheldRow = {
  cause: "scan_bound",
  count: 1,
  exact: false,
  reasonRef: "reason.omega.projection_scan_bound",
  observedBy: "host",
  deepLink: "openagents://omega/workroom?room=owner_private&withheld=scan_bound",
};

describe("Issue 31 owner-private coverage rendering", () => {
  test("complete, unknown, and partial are three different renderings", () => {
    const complete = render("complete");
    const unknown = render("unknown");
    const partial = render("partial", [quarantined]);
    expect(complete).not.toBe(unknown);
    expect(complete).not.toBe(partial);
    expect(unknown).not.toBe(partial);
    // Silence is the one that has to be visibly different from completeness.
    expect(complete).toContain("Every source Sarah can use reached this device.");
    expect(unknown).toContain("This host has not stated how complete this view is.");
    expect(unknown).not.toContain("Every source Sarah can use reached this device.");
  });

  test("a withheld count names its number, its cause, and its reason on screen", () => {
    const partial = render("partial", [quarantined]);
    expect(partial).toContain("reason.omega.invalid_projection_source");
    expect(partial).toContain("quarantined");
    expect(partial).toContain("3");
    // Fail visible, not fail closed: the engram that did arrive is still drawn
    // beside the count of the ones that did not.
    const searched = render("partial", [quarantined], "release_evidence");
    expect(searched).toContain("mem/release_evidence");
    expect(searched).toContain("Withheld from this device");
  });

  test("a lower bound is never drawn as an exact number", () => {
    const partial = render("partial", [scanBound]);
    // Both places the number appears, asserted separately: a summary line that
    // says "1" while the row says "at least 1" is still a phone stating a
    // precision the host does not have.
    expect(partial).toContain(
      "Withheld from this device: at least 1 · scan_bound · reason.omega.projection_scan_bound",
    );
    expect(partial).toContain("at least 1 withheld · scan_bound");
    const exact = render("partial", [quarantined]);
    expect(exact).toContain(
      "Withheld from this device: 3 · quarantined · reason.omega.invalid_projection_source",
    );
    expect(exact).not.toContain("at least");
  });

  test("each withheld cause gets its own row the owner can open", () => {
    const partial = render("partial", [quarantined, scanBound]);
    expect(partial).toContain("openagents://omega/workroom?room=owner_private&withheld=quarantined");
    expect(partial).toContain("openagents://omega/workroom?room=owner_private&withheld=scan_bound");
    expect(partial).toContain("observed by host");
  });
});
