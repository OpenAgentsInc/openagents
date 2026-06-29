// Khala driver tests (fakes, no network): the ReAct loop asks for one action,
// feeds observations back, ends on done/fail or the step cap, and applies a
// BOUNDED corrective re-prompt on an invalid reply before failing honestly.

import { describe, expect, test } from "bun:test";
import { KhalaActionParseError } from "./khala-action";
import type { ChatClient, ChatMessage } from "./khala-driver";
import { makeKhalaDriver } from "./khala-driver";

function scriptedChat(replies: ReadonlyArray<string>): ChatClient & { seen: ChatMessage[][] } {
  let i = 0;
  const seen: ChatMessage[][] = [];
  return {
    seen,
    complete: async (messages) => {
      seen.push([...messages]);
      return replies[i++] ?? '{"action":"fail","reason":"out of script"}';
    },
  };
}

describe("makeKhalaDriver", () => {
  test("returns actions then ends on done; records a pass verdict", async () => {
    const chat = scriptedChat([
      '{"action":"navigate","url":"/login"}',
      '{"action":"done","verdict":"pass","summary":"ok"}',
    ]);
    const driver = makeKhalaDriver({ goal: "g", chat, log: () => undefined });
    const a1 = await driver.nextAction();
    expect(a1).toEqual({ action: "navigate", url: "/login" });
    driver.recordObservation("navigated; url is /login");
    const a2 = await driver.nextAction();
    expect(a2).toBeNull();
    expect(driver.finalVerdict()).toBe("pass");
    const t = driver.transcript();
    expect(t.map((r) => r.action.action)).toEqual(["navigate", "done"]);
    expect(t[0]!.observation).toBe("navigated; url is /login");
  });

  test("feeds the observation back into the conversation", async () => {
    const chat = scriptedChat([
      '{"action":"readText"}',
      '{"action":"done","verdict":"pass"}',
    ]);
    const driver = makeKhalaDriver({ goal: "g", chat, log: () => undefined });
    await driver.nextAction();
    driver.recordObservation("page text: Hello");
    await driver.nextAction();
    const lastTurnMessages = chat.seen.at(-1)!;
    expect(lastTurnMessages.some((m) => m.role === "user" && m.content.includes("page text: Hello"))).toBe(true);
  });

  test("a bounded re-prompt recovers from one invalid reply", async () => {
    const chat = scriptedChat([
      "oops not json", // invalid
      '{"action":"done","verdict":"pass"}', // corrected
    ]);
    const driver = makeKhalaDriver({ goal: "g", chat, reparseAttempts: 1, log: () => undefined });
    const action = await driver.nextAction();
    expect(action).toBeNull(); // the corrected reply was a done
    expect(driver.finalVerdict()).toBe("pass");
  });

  test("fails honestly when every attempt is invalid (no fabricated action)", async () => {
    const chat = scriptedChat(["nope", "still nope"]);
    const driver = makeKhalaDriver({ goal: "g", chat, reparseAttempts: 1, log: () => undefined });
    await expect(driver.nextAction()).rejects.toBeInstanceOf(KhalaActionParseError);
  });

  test("ends with an incomplete verdict at the step cap", async () => {
    const chat: ChatClient = { complete: async () => '{"action":"navigate","url":"/x"}' };
    const driver = makeKhalaDriver({ goal: "g", chat, maxTurns: 2, log: () => undefined });
    expect(await driver.nextAction()).not.toBeNull();
    driver.recordObservation("ok");
    expect(await driver.nextAction()).not.toBeNull();
    driver.recordObservation("ok");
    expect(await driver.nextAction()).toBeNull(); // cap
    expect(driver.finalVerdict()).toBe("incomplete");
  });
});
