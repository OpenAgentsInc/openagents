import { describe, expect, test } from "vite-plus/test";

import { createGrokAcpEventProjector } from "./event-projector.ts";
import {
  createBoundedAcpNativeEvidenceStore,
  type AcpProjectionEvent,
} from "@openagentsinc/agent-client-runtime-bridge";

describe("createGrokAcpEventProjector", () => {
  test("streams message_start, deltas, then done", async () => {
    const p = createGrokAcpEventProjector({
      threadId: "t1",
      turnId: "turn1",
      messageId: "m1",
    });

    const e1 = await p.onUpdate({
      sessionUpdate: "agent_message_chunk",
      content: { type: "text", text: "hel" },
    });
    expect(e1.map((e) => e.type)).toEqual(["message_start", "message_delta"]);

    const e2 = await p.onUpdate({
      sessionUpdate: "agent_message_chunk",
      content: { type: "text", text: "lo" },
    });
    expect(e2.map((e) => e.type)).toEqual(["message_delta"]);
    expect(p.text()).toBe("hello");

    const fin = await p.finish();
    expect(fin).toEqual([{ type: "message_done", turnId: "turn1", messageId: "m1" }]);
  });

  test("retains invalid future updates and emits canonical protocol drift", async () => {
    const canonical: AcpProjectionEvent[] = [];
    const store = createBoundedAcpNativeEvidenceStore({ maxEntries: 10, maxBytes: 10_000 });
    const p = createGrokAcpEventProjector({
      threadId: "t1",
      turnId: "turn1",
      grokSessionId: "s1",
      nativeEvidenceStore: store,
      onCanonicalEvent: (event) => canonical.push(event),
    });
    await p.onUpdate({ sessionUpdate: "future_update", secret: "native-only" }, "s1");
    expect(store.size()).toBe(1);
    expect(canonical.some((event) => event.kind === "raw.sidecar_ref")).toBe(true);
    expect(canonical.some((event) => event.kind === "degraded")).toBe(true);
    expect(JSON.stringify(canonical)).not.toContain("native-only");
  });
});
