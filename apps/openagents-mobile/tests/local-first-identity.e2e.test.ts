import { describe, expect, test } from "bun:test";
import { validateBehaviorContractRegistry } from "@openagentsinc/behavior-contracts";
import { openAgentsMobileUxContractRegistry } from "../src/contracts/ux-contracts";
import { renderContentView, initialHomeState } from "../src/screens/home-core";
describe("openagents_mobile.seam.identity.local_first_account_link.v1", () => {
  test("registers the enforced mobile local-first contract", () => {
    expect(
      validateBehaviorContractRegistry(openAgentsMobileUxContractRegistry).ok,
    ).toBe(true);
  });
  test("renders usable local identity and optional account upgrade without a login gate", () => {
    const view = JSON.stringify(
      renderContentView({
        ...initialHomeState,
        surfaceMode: "openagents",
        syncPhase: "local_ready",
      }),
    );
    expect(view).toContain("Local device ready");
    expect(view).toContain("Link OpenAgents account");
    expect(view).toContain("work without an account");
  });
});
