import { describe, expect, it } from "vite-plus/test";

import {
  XP_ALGORITHM_ID,
  XP_AWARD_KIND,
  XP_BADGE_AWARD_KIND,
  XP_BADGE_DEFINITION_KIND,
  XP_FORBIDDEN_COPY_TERMS,
  XP_LEVEL_THRESHOLDS,
  XP_NAMESPACE,
  XP_POINT_TABLE,
  XP_PROFILE_BADGES_KIND,
  XP_RANK_KIND,
  XP_SETTLEMENT_DEFERRED_ISSUE,
  XP_SETTLEMENT_DEFERRED_PACKET,
  assertXpCopyPublicSafe,
  buildWorkUnitAwardTemplate,
  buildXpAwardTemplate,
  buildXpBadgeAwardTemplate,
  buildXpBadgeDefinitionTemplate,
  buildXpProfileBadgesTemplate,
  buildXpRankTemplate,
  earnedBadgeIds,
  isValidXpAward,
  levelForPoints,
  parseXpAwardEvent,
  parseXpRankEvent,
  pointsForAwardKind,
  projectRank,
  rankAgreesWithAwards,
  resolveRankFromAwards,
  selectContributingAwards,
  sumAwardPoints,
  type XpAwardRecord,
} from "./index.ts";

const SCORER = "a".repeat(64);
const OTHER_SCORER = "b".repeat(64);
const EARNER = "c".repeat(64);
const STRANGER = "d".repeat(64);
const WORK_1 = "e".repeat(64);
const WORK_2 = "f".repeat(64);
const WORK_3 = "1".repeat(64);
const RECEIPT = "receipt.openagents.acceptance.v1:fixture";

const mkAward = (
  partial: Partial<XpAwardRecord> &
    Pick<XpAwardRecord, "awardKind" | "workEventId">,
): XpAwardRecord => ({
  awardKind: partial.awardKind,
  points: partial.points ?? pointsForAwardKind(partial.awardKind),
  earnerPubkey: partial.earnerPubkey ?? EARNER,
  workEventId: partial.workEventId,
  receiptRef: partial.receiptRef ?? RECEIPT,
  scorerPubkey: partial.scorerPubkey ?? SCORER,
  ...(partial.awardEventId ? { awardEventId: partial.awardEventId } : {}),
  ...(partial.tier !== undefined ? { tier: partial.tier } : {}),
  ...(partial.jobType ? { jobType: partial.jobType } : {}),
  ...(partial.createdAt !== undefined ? { createdAt: partial.createdAt } : {}),
});

describe("SARAH-CW-06 experience namespace and point table", () => {
  it("publishes the fixed point table from §35.4", () => {
    expect(XP_NAMESPACE).toBe("com.openagents.xp");
    expect(XP_ALGORITHM_ID).toBe("openagents.xp.v1");
    expect(XP_POINT_TABLE["accepted_work_unit.tier_1"]).toBe(10);
    expect(XP_POINT_TABLE["accepted_work_unit.tier_2"]).toBe(20);
    expect(XP_POINT_TABLE["accepted_work_unit.tier_3"]).toBe(40);
    expect(XP_POINT_TABLE.accepted_independent_verification).toBe(5);
    expect(XP_POINT_TABLE.reproduced_defect).toBe(8);
    expect(XP_POINT_TABLE.accepted_review).toBe(3);
    expect(XP_POINT_TABLE.first_job_type).toBe(5);
  });

  it("defers settlement and forbids earnings language", () => {
    expect(XP_SETTLEMENT_DEFERRED_ISSUE).toBe(9230);
    expect(XP_SETTLEMENT_DEFERRED_PACKET).toBe("SARAH-CW-07");
    expect(XP_FORBIDDEN_COPY_TERMS).toContain("earnings");
    expect(() => assertXpCopyPublicSafe("total earnings this week")).toThrow(
      /forbidden_xp_copy|earnings/,
    );
    expect(() => assertXpCopyPublicSafe("experience total")).not.toThrow();
  });

  it("uses fixed level thresholds", () => {
    expect(levelForPoints(0).level).toBe(0);
    expect(levelForPoints(9).level).toBe(0);
    expect(levelForPoints(10).level).toBe(1);
    expect(levelForPoints(50).level).toBe(2);
    expect(levelForPoints(150).level).toBe(3);
    expect(levelForPoints(400).level).toBe(4);
    expect(levelForPoints(1000).level).toBe(5);
    expect(XP_LEVEL_THRESHOLDS.map((r) => r.minPoints)).toEqual([
      0, 10, 50, 150, 400, 1000,
    ]);
  });
});

describe("NIP-32 award templates", () => {
  it("builds a kind-1985 award with namespace, work, receipt, and fixed points", () => {
    const { template, record } = buildXpAwardTemplate({
      awardKind: "accepted_work_unit.tier_2",
      earnerPubkey: EARNER,
      workEventId: WORK_1,
      receiptRef: RECEIPT,
      createdAt: 1_700_000_000,
    });

    expect(template.kind).toBe(XP_AWARD_KIND);
    expect(template.created_at).toBe(1_700_000_000);
    expect(template.tags).toContainEqual(["L", XP_NAMESPACE]);
    expect(template.tags).toContainEqual([
      "l",
      "accepted_work_unit.tier_2",
      XP_NAMESPACE,
    ]);
    expect(template.tags).toContainEqual(["p", EARNER]);
    expect(template.tags).toContainEqual(["e", WORK_1, "", "work"]);
    expect(template.tags).toContainEqual(["receipt", RECEIPT]);
    expect(template.tags).toContainEqual(["points", "20"]);
    expect(template.tags).toContainEqual(["algorithm", XP_ALGORITHM_ID]);
    expect(template.tags).toContainEqual(["tier", "2"]);
    expect(record.points).toBe(20);
    expect(record.tier).toBe(2);
  });

  it("builds work-unit awards from tier and requires jobType for first_job_type", () => {
    const { record } = buildWorkUnitAwardTemplate({
      tier: 3,
      earnerPubkey: EARNER,
      workEventId: WORK_1,
      receiptRef: RECEIPT,
    });
    expect(record.awardKind).toBe("accepted_work_unit.tier_3");
    expect(record.points).toBe(40);

    expect(() =>
      buildXpAwardTemplate({
        awardKind: "first_job_type",
        earnerPubkey: EARNER,
        workEventId: WORK_1,
        receiptRef: RECEIPT,
      }),
    ).toThrow(/jobType/);

    const first = buildXpAwardTemplate({
      awardKind: "first_job_type",
      earnerPubkey: EARNER,
      workEventId: WORK_1,
      receiptRef: RECEIPT,
      jobType: "labor.code_task",
    });
    expect(first.template.tags).toContainEqual(["job_type", "labor.code_task"]);
  });

  it("rejects award notes that imply payment", () => {
    expect(() =>
      buildXpAwardTemplate({
        awardKind: "accepted_review",
        earnerPubkey: EARNER,
        workEventId: WORK_1,
        receiptRef: RECEIPT,
        note: "payout unlocked for this unit",
      }),
    ).toThrow(/forbidden_xp_copy/);
  });

  it("requires work event and receipt; rejects missing receipt", () => {
    expect(() =>
      buildXpAwardTemplate({
        awardKind: "accepted_review",
        earnerPubkey: EARNER,
        workEventId: WORK_1,
        receiptRef: "",
      }),
    ).toThrow(/receipt/);
  });

  it("parses a scorer-signed award and rejects inflated points tags", () => {
    const { template } = buildXpAwardTemplate({
      awardKind: "reproduced_defect",
      earnerPubkey: EARNER,
      workEventId: WORK_1,
      receiptRef: RECEIPT,
      createdAt: 42,
    });

    const parsed = parseXpAwardEvent({
      id: WORK_2,
      pubkey: SCORER,
      kind: template.kind,
      created_at: template.created_at,
      tags: template.tags,
      content: template.content,
    });
    expect(parsed).not.toBeNull();
    expect(parsed!.points).toBe(8);
    expect(parsed!.scorerPubkey).toBe(SCORER);
    expect(isValidXpAward(parsed!, [SCORER])).toBe(true);
    expect(isValidXpAward(parsed!, [OTHER_SCORER])).toBe(false);

    const inflatedTags = template.tags.map((t) =>
      t[0] === "points" ? (["points", "999"] as const) : t,
    );
    expect(
      parseXpAwardEvent({
        pubkey: SCORER,
        kind: XP_AWARD_KIND,
        tags: inflatedTags,
      }),
    ).toBeNull();
  });

  it("ignores non-namespace labels and self-labels from non-scorers", () => {
    expect(
      parseXpAwardEvent({
        pubkey: SCORER,
        kind: XP_AWARD_KIND,
        tags: [
          ["L", "com.example.other"],
          ["l", "accepted_review", "com.example.other"],
          ["p", EARNER],
          ["e", WORK_1, "", "work"],
          ["receipt", RECEIPT],
        ],
      }),
    ).toBeNull();

    const { template } = buildXpAwardTemplate({
      awardKind: "accepted_review",
      earnerPubkey: EARNER,
      workEventId: WORK_1,
      receiptRef: RECEIPT,
    });
    const selfLabel = parseXpAwardEvent({
      pubkey: EARNER,
      kind: template.kind,
      tags: template.tags,
    });
    expect(selfLabel).not.toBeNull();
    // Parsed, but never valid for scoring unless earner is also a scorer key.
    expect(isValidXpAward(selfLabel!, [SCORER])).toBe(false);
  });
});

describe("rank algorithm pure functions", () => {
  it("sums only scorer awards for the earner and ignores strangers", () => {
    const awards: XpAwardRecord[] = [
      mkAward({
        awardKind: "accepted_work_unit.tier_1",
        workEventId: WORK_1,
        tier: 1,
        createdAt: 1,
      }),
      mkAward({
        awardKind: "accepted_independent_verification",
        workEventId: WORK_2,
        createdAt: 2,
      }),
      mkAward({
        awardKind: "accepted_work_unit.tier_3",
        workEventId: WORK_3,
        earnerPubkey: STRANGER,
        tier: 3,
        createdAt: 3,
      }),
      mkAward({
        awardKind: "accepted_review",
        workEventId: "2".repeat(64),
        scorerPubkey: STRANGER,
        createdAt: 4,
      }),
    ];

    const total = sumAwardPoints(awards, {
      earnerPubkey: EARNER,
      scorerPubkeys: [SCORER],
    });
    expect(total).toBe(10 + 5);

    const projection = projectRank(awards, {
      earnerPubkey: EARNER,
      scorerPubkeys: [SCORER],
    });
    expect(projection.totalPoints).toBe(15);
    expect(projection.level).toBe(1);
    expect(projection.levelId).toBe("level_1");
    expect(projection.awardCount).toBe(2);
    expect(projection.algorithmId).toBe(XP_ALGORITHM_ID);
  });

  it("counts first_job_type once per job type and dedupes work+kind", () => {
    const awards: XpAwardRecord[] = [
      mkAward({
        awardKind: "first_job_type",
        workEventId: WORK_1,
        jobType: "labor.code_task",
        createdAt: 1,
      }),
      mkAward({
        awardKind: "first_job_type",
        workEventId: WORK_2,
        jobType: "labor.code_task",
        createdAt: 2,
      }),
      mkAward({
        awardKind: "first_job_type",
        workEventId: WORK_3,
        jobType: "labor.review",
        createdAt: 3,
      }),
      mkAward({
        awardKind: "accepted_work_unit.tier_1",
        workEventId: WORK_1,
        tier: 1,
        createdAt: 1,
      }),
      // replay of same work+kind
      mkAward({
        awardKind: "accepted_work_unit.tier_1",
        workEventId: WORK_1,
        tier: 1,
        createdAt: 9,
      }),
    ];

    const contributing = selectContributingAwards(awards, {
      earnerPubkey: EARNER,
      scorerPubkeys: [SCORER],
    });
    expect(contributing).toHaveLength(3);
    expect(sumAwardPoints(awards, { earnerPubkey: EARNER, scorerPubkeys: [SCORER] })).toBe(
      5 + 5 + 10,
    );
  });

  it("builds a NIP-85 rank template and treats awards as authority on disagreement", () => {
    const awards = [
      mkAward({
        awardKind: "accepted_work_unit.tier_2",
        workEventId: WORK_1,
        tier: 2,
        awardEventId: WORK_2,
        createdAt: 1,
      }),
    ];
    const projection = projectRank(awards, {
      earnerPubkey: EARNER,
      scorerPubkeys: [SCORER],
    });
    expect(projection.totalPoints).toBe(20);

    const template = buildXpRankTemplate({
      projection,
      createdAt: 99,
    });
    expect(template.kind).toBe(XP_RANK_KIND);
    expect(template.tags).toContainEqual(["d", EARNER]);
    expect(template.tags).toContainEqual(["rank", "20"]);
    expect(template.tags).toContainEqual(["level", "1"]);
    expect(template.tags).toContainEqual(["algorithm", XP_ALGORITHM_ID]);

    const parsed = parseXpRankEvent(template);
    expect(parsed).not.toBeNull();
    expect(rankAgreesWithAwards(parsed!, awards, [SCORER])).toBe(true);

    const inflated = {
      ...projection,
      totalPoints: 9999,
      level: 5,
      levelId: "level_5",
    };
    expect(rankAgreesWithAwards(inflated, awards, [SCORER])).toBe(false);

    const resolved = resolveRankFromAwards(awards, {
      earnerPubkey: EARNER,
      scorerPubkeys: [SCORER],
      publishedRank: inflated,
    });
    expect(resolved.usedAwards).toBe(true);
    expect(resolved.publishedDisagreed).toBe(true);
    expect(resolved.projection.totalPoints).toBe(20);
  });

  it("never multiplies experience into settlement fields", () => {
    const projection = projectRank(
      [
        mkAward({
          awardKind: "accepted_work_unit.tier_3",
          workEventId: WORK_1,
          tier: 3,
        }),
      ],
      { earnerPubkey: EARNER, scorerPubkeys: [SCORER] },
    );
    const json = JSON.stringify(projection);
    for (const term of ["earnings", "payout", "payment", "sats", "msats"]) {
      expect(json.toLowerCase()).not.toContain(term);
    }
    expect(XP_SETTLEMENT_DEFERRED_ISSUE).toBe(9230);
  });
});

describe("NIP-58 badge specs", () => {
  it("publishes definition templates without payment language", () => {
    const def = buildXpBadgeDefinitionTemplate({
      badgeId: "first-accepted-unit",
      createdAt: 10,
    });
    expect(def.kind).toBe(XP_BADGE_DEFINITION_KIND);
    expect(def.tags).toContainEqual(["d", "first-accepted-unit"]);
    expect(def.tags).toContainEqual(["L", XP_NAMESPACE]);
    expect(def.content.toLowerCase()).not.toContain("earnings");
    expect(def.content).toContain("Experience only");
  });

  it("derives earned badges from the award stream and levels", () => {
    const awards: XpAwardRecord[] = [
      mkAward({
        awardKind: "accepted_work_unit.tier_1",
        workEventId: WORK_1,
        tier: 1,
        createdAt: 1,
      }),
      mkAward({
        awardKind: "accepted_independent_verification",
        workEventId: WORK_2,
        createdAt: 2,
      }),
      mkAward({
        awardKind: "accepted_review",
        workEventId: WORK_3,
        createdAt: 3,
      }),
    ];
    // 10 + 5 + 3 = 18 → level 1
    const badges = earnedBadgeIds(awards, {
      earnerPubkey: EARNER,
      scorerPubkeys: [SCORER],
    });
    expect(badges).toContain("first-accepted-unit");
    expect(badges).toContain("first-verification");
    expect(badges).toContain("first-review");
    expect(badges).toContain("level-1");
    expect(badges).not.toContain("level-2");
    expect(badges).not.toContain("first-defect-repro");
  });

  it("builds badge award and profile badge list templates", () => {
    const awardTpl = buildXpBadgeAwardTemplate({
      badgeId: "level-1",
      issuerPubkey: SCORER,
      earnerPubkey: EARNER,
      createdAt: 5,
    });
    expect(awardTpl.kind).toBe(XP_BADGE_AWARD_KIND);
    expect(awardTpl.tags).toContainEqual([
      "a",
      `${XP_BADGE_DEFINITION_KIND}:${SCORER}:level-1`,
    ]);
    expect(awardTpl.tags).toContainEqual(["p", EARNER]);

    const profile = buildXpProfileBadgesTemplate({
      earnerPubkey: EARNER,
      issuerPubkey: SCORER,
      badgeIds: ["level-1", "first-accepted-unit"],
      awardEventIds: [WORK_1, WORK_2],
      createdAt: 6,
    });
    expect(profile.kind).toBe(XP_PROFILE_BADGES_KIND);
    expect(profile.tags).toContainEqual(["d", "profile_badges"]);
    expect(profile.tags.some((t) => t[0] === "a" && t[1]?.includes("level-1"))).toBe(
      true,
    );
  });

  it("reaches higher level badges when points cross thresholds", () => {
    // 25 × tier_2 (20) = 500 points → level 4
    const awards: XpAwardRecord[] = Array.from({ length: 25 }, (_, i) =>
      mkAward({
        awardKind: "accepted_work_unit.tier_2",
        workEventId: i.toString(16).padStart(64, "0"),
        tier: 2,
        createdAt: i,
      }),
    );
    const projection = projectRank(awards, {
      earnerPubkey: EARNER,
      scorerPubkeys: [SCORER],
    });
    expect(projection.totalPoints).toBe(500);
    expect(projection.level).toBe(4);

    const badges = earnedBadgeIds(awards, {
      earnerPubkey: EARNER,
      scorerPubkeys: [SCORER],
    });
    expect(badges).toEqual(
      expect.arrayContaining([
        "first-accepted-unit",
        "level-1",
        "level-2",
        "level-3",
        "level-4",
      ]),
    );
    expect(badges).not.toContain("level-5");
  });
});
