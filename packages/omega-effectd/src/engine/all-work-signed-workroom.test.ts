import { describe, expect, it } from "vite-plus/test";

import { configuredSignedWorkroomRelayUrls } from "./all-work-signed-workroom.ts";

describe("All Work signed Workroom relay policy", () => {
  it("uses an audience-scoped closed allowlist and keeps the legacy key workroom-only", () => {
    const environment = {
      OPENAGENTS_OMEGA_SIGNED_WORKROOM_RELAYS: "wss://workroom.example",
      OPENAGENTS_OMEGA_SIGNED_WORKROOM_RELAYS_OWNER_ONLY: "wss://owner.example",
      OPENAGENTS_OMEGA_SIGNED_WORKROOM_RELAYS_PUBLIC: "wss://public.example",
    };
    expect(configuredSignedWorkroomRelayUrls("workroom", environment)).toEqual([
      "wss://workroom.example",
    ]);
    expect(configuredSignedWorkroomRelayUrls("owner_only", environment)).toEqual([
      "wss://owner.example",
    ]);
    expect(configuredSignedWorkroomRelayUrls("public", environment)).toEqual([
      "wss://public.example",
    ]);
    expect(configuredSignedWorkroomRelayUrls("private", environment)).toEqual([]);
  });
});
