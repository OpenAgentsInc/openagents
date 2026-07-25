import { Schema as S } from "effect";
import { describe, expect, test } from "vite-plus/test";

import {
  SARAH_AUTHORITY_REVISION,
  SarahPrincipalApiResponseSchema,
  sanitizeSarahConversationResponse,
} from "./mobile-principal.ts";

describe("Sarah mobile principal contract", () => {
  test("decodes the bounded principal response without the Node-capable root barrel", () => {
    const response = S.decodeUnknownSync(SarahPrincipalApiResponseSchema)({
      ok: true,
      routeRef: "route.mobile.sarah.principal.v1",
      principal: {
        schema: "openagents.sarah.principal.v1",
        principalRef: "principal.sarah",
        displayName: "Sarah",
        role: "Owner orchestrator",
        threadRef: "thread.owner.mobile",
        authorityProfileRef: "openagents.sarah-owner-orchestrator",
        authorityRevision: SARAH_AUTHORITY_REVISION,
        rootAuthorityProfileRef: "openagents.owner-delegated-autonomy",
        rootAuthorityRevision: 8,
        memory: "durable_cited",
        capabilities: [],
      },
    });
    expect(response.principal.threadRef).toBe("thread.owner.mobile");
  });

  test("keeps private provenance outside owner conversation text", () => {
    expect(
      sanitizeSarahConversationResponse("Ready. [source.private.owner_1]\n\n\nNext step."),
    ).toBe("Ready.\n\nNext step.");
  });
});
