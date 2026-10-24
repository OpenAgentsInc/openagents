import test from "node:test";
import assert from "node:assert/strict";
import {
  DRUID_ABILITIES,
  isDruidAbility,
  validDruidStatus,
  type DruidStatus,
} from "../packages/runtime/druid-abilities.ts";
import {
  INITIAL_STATUS,
  PREVIEW_PROTOCOL,
  isPreviewEvent,
} from "../packages/editor-services/protocol.ts";

const session = (): DruidStatus => ({
  enabled: true,
  selected: "meteor_shower",
  destroyed: 2,
  debris: 16,
  effects: 3,
  casts: 1,
  impacts: 1,
  tick: 270,
});
test("the play action catalog distinguishes the beam from the sky strike", () => {
  assert.deepEqual(
    DRUID_ABILITIES.map((a) => a.id),
    ["meteor_shower", "lightning_bolt", "thunderbolt", "destroy_castle"],
  );
  for (const a of DRUID_ABILITIES) assert.ok(isDruidAbility(a.id));
  for (const invalid of ["", "delete_all", "Meteor Shower", {}, null])
    assert.equal(isDruidAbility(invalid), false);
});
test("the castle ability admits only consistent bounded progress and a distant automatic target", () => {
  const castle = {
    total: 6846,
    remaining: 6746,
    destroyed: 100,
    active: true,
    complete: false,
    cancelled: false,
    preparing: false,
    waves: 4,
    impacts: 12,
    elapsed: 1.6,
    rubble: 32,
  };
  const value = {
    ...session(),
    selected: "destroy_castle",
    castle,
    target: { point: [0, 100, -400], normal: [0, 1, 0], distance: 420 },
  };
  assert.ok(validDruidStatus(value));
  assert.ok(
    validDruidStatus({ ...value, castle: { ...castle, rubble: 1024 } }),
  );
  assert.equal(
    validDruidStatus({ ...value, selected: "lightning_bolt" }),
    false,
  );
  for (const patch of [
    { total: 65537 },
    { remaining: 6846 },
    { rubble: 1025 },
    { complete: true },
    { preparing: "yes" },
    { elapsed: NaN },
  ])
    assert.equal(
      validDruidStatus({ ...value, castle: { ...castle, ...patch } }),
      false,
    );
});
test("bounded gameplay summaries reject malformed physics and targeting values", () => {
  assert.ok(validDruidStatus(session()));
  assert.ok(validDruidStatus({ ...session(), rubble: 1024 }));
  for (const patch of [
    { destroyed: 65537 },
    { debris: 257 },
    { rubble: 1025 },
    { rubble: -1 },
    { preparing: "now" },
    { effects: 257 },
    { tick: -1 },
    { casts: Infinity },
    { selected: "unknown" },
    { casting: { ability: "thunderbolt", remaining: NaN } },
    { target: { point: [0, 1, 2], normal: [0, 1, 0], distance: 101 } },
  ])
    assert.equal(validDruidStatus({ ...session(), ...patch }), false);
});
test("gameplay status uses the checked worker session without changing project save state", () => {
  const status = {
    ...INITIAL_STATUS,
    phase: "ready",
    gameplay: session(),
    revision: 4,
    saved: true,
  };
  const event = {
    protocol: PREVIEW_PROTOCOL,
    sessionId: "world-a",
    type: "status",
    status,
  };
  assert.ok(isPreviewEvent(event, "world-a"));
  assert.equal(isPreviewEvent(event, "world-b"), false);
  assert.equal(
    isPreviewEvent(
      {
        ...event,
        status: { ...status, gameplay: { ...session(), destroyed: -1 } },
      },
      "world-a",
    ),
    false,
  );
  assert.equal(status.revision, 4);
  assert.equal(status.saved, true);
});
