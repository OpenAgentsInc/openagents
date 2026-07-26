import {
  ForgeCollaborationFailure,
  ForgeCollaborationResult,
  type ForgeCollaborationRequest,
} from "@/features/forge/collaboration-read";
import { ForgeCollaborationPage } from "@/features/forge/collaboration-view";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, test } from "vitest";

import { forgeCollaborationProjection } from "./collaboration-fixture";

const loaded = (projection = forgeCollaborationProjection()) =>
  ForgeCollaborationResult.cases.loaded.make({ projection });
const changeRequest: ForgeCollaborationRequest = { owner: "OpenAgentsInc", repo: "omega", view: "change", changeRef: "change.forge.1" };

describe("Forge collaboration view", () => {
  test("renders source-backed change, review, comments, checks, and merge receipt state", () => {
    const html = renderToStaticMarkup(<ForgeCollaborationPage request={changeRequest} result={loaded()} />);
    expect(html).toContain("Add the Forge collaboration surface");
    expect(html).toContain("standard 1617");
    expect(html).toContain("kind 1111");
    expect(html).toContain("Conversation");
    expect(html).toContain("Typecheck");
    expect(html).toContain("Merge outcome");
    expect(html).not.toContain("GitHub fallback");
  });

  test("does not make unresolved or disagreeing proposal actionable", () => {
    const html = renderToStaticMarkup(<ForgeCollaborationPage request={changeRequest} result={loaded(forgeCollaborationProjection({ change: { ...forgeCollaborationProjection().change!, proposalResolution: "disagreement" } }))} />);
    expect(html).toContain("Sources disagree about this proposal. It is not actionable.");
  });

  test("renders every attention item without collapsing disagreement", () => {
    const html = renderToStaticMarkup(<ForgeCollaborationPage request={{ owner: "openagents", repo: "attention", view: "attention" }} result={loaded()} />);
    expect(html).toContain("Review requested");
    expect(html).toContain("Check failed");
    expect(html).toContain("Proposal disagreement");
    expect(html).toContain("Decision required from actor.binding.reviewer");
  });

  test("fails closed and never claims a GitHub fallback", () => {
    const result = ForgeCollaborationResult.cases.failed.make({ failure: ForgeCollaborationFailure.cases.authentication_required.make({ detail: "Invitation required." }) });
    const html = renderToStaticMarkup(<ForgeCollaborationPage request={changeRequest} result={result} />);
    expect(html).toContain("Invitation required");
    expect(html).toContain("This page never uses a GitHub fallback.");
  });
});
