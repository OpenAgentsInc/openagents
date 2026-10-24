import assert from "node:assert/strict";
import test from "node:test";
import { GameplayVisualSync } from "../packages/runtime/gameplay-sync.ts";
import {
  validateGameplayVisualState,
  type GameplayVisualState,
} from "../packages/renderer/gameplay-effects.ts";

const state = () => ({
  version: 0,
  objectCount: 1000,
  destroyed: [] as number[],
  debris: [] as GameplayVisualState["debris"][number][],
  rubble: [] as GameplayVisualState["debris"][number][],
  effects: [] as GameplayVisualState["effects"][number][],
});
function renderer() {
  return {
    calls: [] as { mode: string; state: GameplayVisualState }[],
    setGameplayState(mode: "day" | "night", value: GameplayVisualState) {
      validateGameplayVisualState(value);
      this.calls.push({ mode, state: structuredClone(value) });
    },
  };
}
const effect = (age = 0) => ({
  id: 1,
  kind: "impact" as const,
  origin: [0, 1, 0] as [number, number, number],
  target: [0, 1, 0] as [number, number, number],
  age,
  life: 2,
  radius: 1,
});
const debris = (age = 0) => ({
  id: 2,
  nodeId: 0,
  sourceId: 0,
  chunk: 0,
  sourceMin: [0, 0, 0] as [number, number, number],
  sourceMax: [1, 1, 1] as [number, number, number],
  position: [0, 1 + age, 0] as [number, number, number],
  rotation: [0, 0, 0] as [number, number, number],
  halfExtents: [0.5, 0.5, 0.5] as [number, number, number],
  velocity: [0, 1, 0] as [number, number, number],
  age,
  life: 2,
});

test("idle snapshots apply once despite fresh arrays and advancing simulation metadata", () => {
  const sync = new GameplayVisualSync(),
    target = renderer();
  for (let tick = 0; tick < 1000; tick++) {
    const snapshot = { ...state(), tick };
    assert.equal(sync.apply(target, "day", snapshot), tick === 0);
  }
  assert.equal(target.calls.length, 1);
  assert.equal(sync.apply(target, "day", { ...state(), destroyed: [4] }), true);
  assert.equal(sync.apply(target, "day", { ...state(), destroyed: [5] }), true);
  assert.equal(
    sync.apply(target, "day", { ...state(), destroyed: [5] }),
    false,
  );
});

test("every effect, debris, and rubble snapshot advances even without a kernel version change", () => {
  for (const field of ["effects", "debris", "rubble", "castle"] as const) {
    const sync = new GameplayVisualSync(),
      target = renderer();
    sync.apply(target, "day", state());
    for (const age of [0, 0.1, 0.2]) {
      const value = state();
      if (field === "effects") value.effects.push(effect(age));
      else if (field === "castle") {
        const { rubble: _rubble, ...legacy } = value;
        assert.equal(
          sync.apply(target, "day", {
            ...legacy,
            castle: { rubble: [debris(age)] },
          }),
          true,
        );
        continue;
      } else value[field].push(debris(age));
      assert.equal(sync.apply(target, "day", value), true);
    }
    assert.equal(
      sync.apply(target, "day", state()),
      true,
      "Expiration must clear the last active visuals.",
    );
    assert.equal(sync.apply(target, "day", state()), false);
    assert.equal(target.calls.length, 5);
  }
});

test("renderer, mode, initialization, version, and object inventory changes force application", () => {
  const sync = new GameplayVisualSync(),
    first = renderer(),
    second = renderer();
  sync.apply(first, "day", state());
  assert.equal(sync.apply(second, "day", state()), true);
  assert.equal(sync.apply(second, "night", state()), true);
  assert.equal(sync.apply(second, "night", { ...state(), version: 1 }), true);
  assert.equal(
    sync.apply(second, "night", { ...state(), version: 1, objectCount: 2 }),
    true,
  );
  sync.reset();
  assert.equal(sync.apply(second, "night", state()), true);
  assert.equal(
    sync.apply(second, "night", { ...state(), objectCount: 0 }),
    false,
  );
  assert.equal(sync.apply(second, "night", state()), true);
});

test("failed candidates are not cached and cached IDs cannot be mutated through a snapshot", () => {
  const sync = new GameplayVisualSync(),
    target = renderer();
  const value = { ...state(), destroyed: [1] };
  sync.apply(target, "day", value);
  value.destroyed[0] = 2;
  assert.equal(sync.apply(target, "day", value), true);
  const invalid = { ...state(), version: 1, destroyed: [1, 1] };
  assert.throws(() => sync.apply(target, "day", invalid));
  assert.throws(() => sync.apply(target, "day", invalid));
  assert.equal(sync.apply(target, "day", value), false);
  assert.equal(sync.apply(target, "day", { ...state(), version: 1 }), true);
});
