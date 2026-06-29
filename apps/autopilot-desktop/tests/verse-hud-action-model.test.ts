import { describe, expect, test } from "bun:test"

import {
  projectVerseChatDraft,
  projectVerseHotbarSlots,
  projectVerseModerationDisplay,
  verseChatTimestampLabel,
  verseContextActionsForTarget,
} from "../src/shared/verse-hud-action-model"

describe("Verse HUD action model", () => {
  test("dedupes and syncs hotbar slots from model actions", () => {
    const slots = projectVerseHotbarSlots({
      actions: [
        {
          actionId: "focus:pylon.alpha",
          kind: "focus",
          label: "Focus Alpha",
          slot: 2,
          targetRef: "pylon.alpha",
        },
        {
          actionId: "focus:pylon.alpha",
          kind: "focus",
          label: "Duplicate Alpha",
          slot: 3,
          targetRef: "pylon.alpha",
        },
        {
          actionId: "inspect:avatar.bravo",
          kind: "inspect",
          label: "Inspect Bravo",
        },
      ],
    })

    expect(slots[0]?.actionId).toBe("new-coder-session")
    expect(slots[1]?.actionId).toBe("focus:pylon.alpha")
    expect(slots[2]?.actionId).toBe("inspect:avatar.bravo")
    expect(slots.filter(slot => slot.actionId === "focus:pylon.alpha")).toHaveLength(1)
    expect(slots[9]?.key).toBe("0")
  })

  test("routes chat prefixes to local, run, global, and forum contexts", () => {
    expect(projectVerseChatDraft("hello")).toEqual({
      channel: "local",
      text: "hello",
      prefix: "/local",
    })
    expect(projectVerseChatDraft("/run seal window")).toMatchObject({
      channel: "run",
      text: "seal window",
    })
    expect(projectVerseChatDraft("/global pylon online")).toMatchObject({
      channel: "global",
      text: "pylon online",
    })
    expect(projectVerseChatDraft("/forum post reflection")).toMatchObject({
      channel: "forum",
      text: "post reflection",
    })
  })

  test("formats timestamps and composes backend moderation output", () => {
    expect(
      verseChatTimestampLabel(
        "2026-06-22T13:00:00.000Z",
        new Date("2026-06-22T14:00:00.000Z"),
      ),
    ).toBe("13:00")
    expect(
      verseChatTimestampLabel(
        "2026-06-21T13:00:00.000Z",
        new Date("2026-06-22T14:00:00.000Z"),
      ),
    ).toBe("06-21 13:00")
    expect(projectVerseModerationDisplay({
      text: "raw text",
      moderation: {
        state: "masked",
        replacementText: "[moderated by worker]",
        sourceRefs: ["moderation.world.1"],
      },
    })).toEqual({
      visible: true,
      text: "[moderated by worker]",
      tone: "masked",
      sourceRefs: ["moderation.world.1"],
    })
    expect(projectVerseModerationDisplay({
      text: "raw text",
      moderation: { state: "blocked" },
    }).visible).toBe(false)
  })

  test("builds pure context actions for pylon and avatar targets", () => {
    expect(verseContextActionsForTarget({
      kind: "pylon",
      ref: "pylon.alpha",
      label: "Alpha",
      online: false,
    })).toEqual([
      {
        actionId: "inspect:pylon.alpha",
        label: "Inspect Alpha",
        targetRef: "pylon.alpha",
        enabled: true,
      },
      {
        actionId: "focus:pylon.alpha",
        label: "Focus Alpha",
        targetRef: "pylon.alpha",
        enabled: true,
      },
      {
        actionId: "tip:pylon.alpha",
        label: "Tip Alpha",
        targetRef: "pylon.alpha",
        enabled: false,
      },
    ])
    expect(verseContextActionsForTarget({
      kind: "avatar",
      ref: "avatar.bravo",
      label: "Bravo",
    }).map(action => action.actionId)).toEqual([
      "inspect:avatar.bravo",
      "focus:avatar.bravo",
      "chat:avatar.bravo",
    ])
  })
})
