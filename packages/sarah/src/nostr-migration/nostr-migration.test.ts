import { describe, expect, it } from "vite-plus/test";

import {
  SARAH_NOSTR_MIGRATION_MANIFEST_SCHEMA,
  SARAH_NOSTR_RECORD_MODE_ENV,
  SARAH_NOSTR_SHADOW_PUBLISH_ENV,
  SarahNostrMigrationStageError,
  SarahNostrMigrationStageMachine,
  buildSarahNostrMigrationManifest,
  canTransitionSarahNostrMigrationStage,
  compareKhalaAndNostrDurableEvents,
  conversationTagFromThreadRef,
  extractSarahDigest,
  isSarahConversationTag,
  isSarahThreadRef,
  khalaRemainsRecordAuthority,
  nostrIsRecordAuthority,
  projectNostrDurableEventForDrift,
  resolveSarahConversationIdentity,
  resolveSarahNostrRecordMode,
  shouldPublishSarahNostrFromMode,
  threadRefFromConversationTag,
  validateSarahNostrMigrationRollback,
} from "./index.ts";

const digest = "ab".repeat(12);
const threadRef = `thread.sarah.${digest}`;
const conversation = `sarah.${digest}`;
const eventId = (n: number) => n.toString(16).padStart(64, "0");

describe("SARAH-NR-08 conversation mapping", () => {
  it("maps thread.sarah.<digest> ↔ sarah.<digest> both ways", () => {
    expect(isSarahThreadRef(threadRef)).toBe(true);
    expect(isSarahConversationTag(conversation)).toBe(true);
    expect(conversationTagFromThreadRef(threadRef)).toBe(conversation);
    expect(threadRefFromConversationTag(conversation)).toBe(threadRef);
    expect(extractSarahDigest(threadRef)).toBe(digest);
    expect(extractSarahDigest(conversation)).toBe(digest);
    expect(extractSarahDigest(digest)).toBe(digest);
  });

  it("rejects non-Sarah refs", () => {
    expect(isSarahThreadRef("thread.other.abc")).toBe(false);
    expect(conversationTagFromThreadRef("thread.other.abc")).toBeNull();
    expect(threadRefFromConversationTag("not-a-tag")).toBeNull();
    expect(extractSarahDigest("thread.sarah.short")).toBeNull();
  });

  it("resolveSarahConversationIdentity accepts either form", () => {
    expect(resolveSarahConversationIdentity(threadRef)).toEqual({
      threadRef,
      conversation,
      digest,
    });
    expect(resolveSarahConversationIdentity(conversation)).toEqual({
      threadRef,
      conversation,
      digest,
    });
    expect(() => resolveSarahConversationIdentity("bad")).toThrow(
      /expected thread\.sarah/,
    );
  });
});

describe("SARAH-NR-08 stage machine", () => {
  it("starts at shadow and advances shadow → cutover → retirement", () => {
    const machine = new SarahNostrMigrationStageMachine("shadow");
    expect(machine.getStage()).toBe("shadow");
    expect(machine.khalaIsRecordAuthority()).toBe(true);
    expect(machine.shouldWriteKhala()).toBe(true);
    expect(machine.shouldPublishToNostr()).toBe(true);

    expect(machine.cutover()).toBe("cutover");
    expect(machine.khalaIsRecordAuthority()).toBe(false);
    expect(machine.nostrIsRecordAuthority()).toBe(true);
    expect(machine.shouldWriteKhala()).toBe(true);

    expect(machine.retire()).toBe("retirement");
    expect(machine.shouldWriteKhala()).toBe(false);
    expect(machine.nostrIsRecordAuthority()).toBe(true);
    expect(machine.getHistory()).toEqual(["shadow", "cutover", "retirement"]);
  });

  it("is idempotent for same-stage transitions", () => {
    const machine = new SarahNostrMigrationStageMachine("shadow");
    expect(machine.transition("shadow")).toBe("shadow");
    expect(machine.getHistory()).toEqual(["shadow"]);
  });

  it("allows rollback cutover→shadow and retirement→cutover", () => {
    const machine = new SarahNostrMigrationStageMachine("shadow");
    machine.cutover();
    expect(machine.rollback()).toBe("shadow");
    machine.cutover();
    machine.retire();
    expect(machine.rollback()).toBe("cutover");
  });

  it("rejects illegal transitions", () => {
    expect(canTransitionSarahNostrMigrationStage("shadow", "retirement")).toBe(
      false,
    );
    const machine = new SarahNostrMigrationStageMachine("shadow");
    expect(() => machine.transition("retirement")).toThrow(
      SarahNostrMigrationStageError,
    );
  });

  it("rollback at shadow is a no-op", () => {
    const machine = new SarahNostrMigrationStageMachine("shadow");
    expect(machine.rollback()).toBe("shadow");
  });
});

describe("SARAH-NR-08 record mode flag", () => {
  it("defaults to khala and honors RECORD_MODE over SHADOW_PUBLISH", () => {
    expect(
      resolveSarahNostrRecordMode({
        [SARAH_NOSTR_RECORD_MODE_ENV]: undefined,
        [SARAH_NOSTR_SHADOW_PUBLISH_ENV]: undefined,
      }),
    ).toBe("khala");
    expect(
      resolveSarahNostrRecordMode({
        [SARAH_NOSTR_SHADOW_PUBLISH_ENV]: "1",
      }),
    ).toBe("shadow");
    expect(
      resolveSarahNostrRecordMode({
        [SARAH_NOSTR_RECORD_MODE_ENV]: "nostr",
        [SARAH_NOSTR_SHADOW_PUBLISH_ENV]: "1",
      }),
    ).toBe("nostr");
    expect(
      resolveSarahNostrRecordMode({
        [SARAH_NOSTR_RECORD_MODE_ENV]: "shadow",
      }),
    ).toBe("shadow");
  });

  it("maps mode to publish and authority helpers", () => {
    expect(shouldPublishSarahNostrFromMode("khala")).toBe(false);
    expect(shouldPublishSarahNostrFromMode("shadow")).toBe(true);
    expect(shouldPublishSarahNostrFromMode("nostr")).toBe(true);
    expect(khalaRemainsRecordAuthority("khala")).toBe(true);
    expect(khalaRemainsRecordAuthority("shadow")).toBe(true);
    expect(khalaRemainsRecordAuthority("nostr")).toBe(false);
    expect(nostrIsRecordAuthority("nostr")).toBe(true);
    expect(nostrIsRecordAuthority("shadow")).toBe(false);
  });

  it("falls back when RECORD_MODE is invalid", () => {
    expect(
      resolveSarahNostrRecordMode({
        [SARAH_NOSTR_RECORD_MODE_ENV]: "not-a-mode",
        [SARAH_NOSTR_SHADOW_PUBLISH_ENV]: "1",
      }),
    ).toBe("shadow");
    expect(
      resolveSarahNostrRecordMode({
        [SARAH_NOSTR_RECORD_MODE_ENV]: "not-a-mode",
      }),
    ).toBe("khala");
  });
});

describe("SARAH-NR-08 drift comparator", () => {
  it("reports ok when ladders agree", () => {
    const khala = [
      { kind: "turn.started" as const, seq: 1, turnRef: "turn.1" },
      { kind: "tool.call" as const, seq: 2, turnRef: "turn.1" },
      { kind: "turn.finished" as const, seq: 3, turnRef: "turn.1" },
    ];
    const nostr = [
      { entry: "turn.started" as const, seq: 1, turnRef: "turn.1", eventId: eventId(1) },
      { entry: "tool.call" as const, seq: 2, turnRef: "turn.1", eventId: eventId(2) },
      { entry: "turn.finished" as const, seq: 3, turnRef: "turn.1", eventId: eventId(3) },
    ];
    const report = compareKhalaAndNostrDurableEvents({ khala, nostr });
    expect(report.ok).toBe(true);
    expect(report.matched).toBe(3);
    expect(report.items).toEqual([]);
  });

  it("reports missing_on_nostr and entry_mismatch", () => {
    const report = compareKhalaAndNostrDurableEvents({
      khala: [
        { kind: "turn.started", seq: 1, turnRef: "turn.1" },
        { kind: "tool.call", seq: 2, turnRef: "turn.1" },
      ],
      nostr: [
        { entry: "turn.started", seq: 1, turnRef: "turn.1" },
        { entry: "tool.result", seq: 2, turnRef: "turn.1" },
      ],
    });
    expect(report.ok).toBe(false);
    expect(report.items.some((i) => i.kind === "entry_mismatch")).toBe(true);

    const missing = compareKhalaAndNostrDurableEvents({
      khala: [{ kind: "turn.started", seq: 1, turnRef: "turn.1" }],
      nostr: [],
    });
    expect(missing.items[0]?.kind).toBe("missing_on_nostr");
  });

  it("projects kind 44300 tags when seq is present", () => {
    const projected = projectNostrDurableEventForDrift({
      id: eventId(9),
      kind: 44300,
      tags: [
        ["entry", "turn.started"],
        ["turn", "turn.1"],
        ["seq", "1"],
      ],
    });
    expect(projected).toEqual({
      entry: "turn.started",
      seq: 1,
      turnRef: "turn.1",
      eventId: eventId(9),
    });
    expect(
      projectNostrDurableEventForDrift({
        kind: 24200,
        tags: [["entry", "turn.started"]],
      }),
    ).toBeNull();
  });
});

describe("SARAH-NR-08 export and rollback", () => {
  it("builds a public-safe idempotent manifest", () => {
    const ids = [eventId(1), eventId(2)];
    const a = buildSarahNostrMigrationManifest({
      stage: "cutover",
      threadRefOrConversation: threadRef,
      eventIds: ids,
      exportedAt: "2026-07-24T20:00:00.000Z",
      rollbackWindowClosesAt: "2026-08-07T20:00:00.000Z",
    });
    const b = buildSarahNostrMigrationManifest({
      stage: "cutover",
      threadRefOrConversation: conversation,
      eventIds: ids,
      exportedAt: "2026-07-24T20:00:00.000Z",
      rollbackWindowClosesAt: "2026-08-07T20:00:00.000Z",
    });
    expect(a.schema).toBe(SARAH_NOSTR_MIGRATION_MANIFEST_SCHEMA);
    expect(a.digestChain).toBe(b.digestChain);
    expect(a.conversation).toBe(conversation);
    expect(a.threadRef).toBe(threadRef);
    expect(a.eventCount).toBe(2);
    expect(JSON.stringify(a)).not.toContain("privateKey");
    expect(JSON.stringify(a)).not.toContain("nsec");
  });

  it("validates legal rollback and rejects window closed / bad target", () => {
    const manifest = buildSarahNostrMigrationManifest({
      stage: "cutover",
      threadRefOrConversation: conversation,
      eventIds: [eventId(1)],
      exportedAt: "2026-07-24T20:00:00.000Z",
      rollbackWindowClosesAt: "2026-08-07T20:00:00.000Z",
    });

    const ok = validateSarahNostrMigrationRollback({
      manifest,
      targetStage: "shadow",
      nowIso: "2026-07-25T00:00:00.000Z",
    });
    expect(ok.ok).toBe(true);

    const badTarget = validateSarahNostrMigrationRollback({
      manifest,
      targetStage: "retirement",
      nowIso: "2026-07-25T00:00:00.000Z",
    });
    expect(badTarget.ok).toBe(false);
    if (!badTarget.ok) {
      expect(badTarget.reason).toMatch(/illegal_rollback_target/);
    }

    const closed = validateSarahNostrMigrationRollback({
      manifest,
      targetStage: "shadow",
      nowIso: "2026-08-08T00:00:00.000Z",
    });
    expect(closed.ok).toBe(false);
    if (!closed.ok) {
      expect(closed.reason).toBe("rollback_window_closed");
    }

    const retirement = buildSarahNostrMigrationManifest({
      stage: "retirement",
      threadRefOrConversation: conversation,
      eventIds: [eventId(1)],
      exportedAt: "2026-07-24T20:00:00.000Z",
    });
    const back = validateSarahNostrMigrationRollback({
      manifest: retirement,
      targetStage: "cutover",
    });
    expect(back.ok).toBe(true);
  });

  it("detects digest chain tampering", () => {
    const manifest = buildSarahNostrMigrationManifest({
      stage: "cutover",
      threadRefOrConversation: conversation,
      eventIds: [eventId(1)],
      exportedAt: "2026-07-24T20:00:00.000Z",
    });
    const tampered = {
      ...manifest,
      eventIds: [eventId(2)],
    };
    const result = validateSarahNostrMigrationRollback({
      manifest: tampered,
      targetStage: "shadow",
    });
    expect(result.ok).toBe(false);
    if (!result.ok) {
      expect(result.reason).toBe("digest_chain_mismatch");
    }
  });
});
