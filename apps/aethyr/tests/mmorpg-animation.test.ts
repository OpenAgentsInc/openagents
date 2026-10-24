import assert from "node:assert/strict";
import test from "node:test";
import {
  MmorpgAnimationClock,
  MMORPG_ANIMATION_FADES,
} from "../packages/runtime/mmorpg-animation.ts";
import type { CharacterClip } from "../packages/renderer/character.ts";
import type { QuickMmorpgMotion } from "../packages/runtime/quick-mmorpg-controller.ts";
const durations = {
  idle: 2.5666666667,
  walk: 1.1333333333,
  run: 0.7333333333,
  attack: 1.5333333333,
  dance: 4.8,
  death: 2.2333333333,
};
const manifest = (npc = false) => ({
  clips: Object.entries(durations)
    .filter(([name]) => !npc || !["run", "dance"].includes(name))
    .map(([name, duration]) => ({
      name,
      duration,
      distance: 0,
      tracks: [],
      loop: !["attack", "dance", "death"].includes(name),
    })) as CharacterClip[],
});
const accepted = (state: QuickMmorpgMotion, time: number) => ({
  state,
  actionTime: time,
  animationTime: time,
});
const near = (actual: number, expected: number, tolerance = 1e-7) =>
  assert(Math.abs(actual - expected) < tolerance, `${actual} != ${expected}`);

test("accepted states alone choose actions; four-clip NPCs do not need invented gaits", () => {
  const clock = new MmorpgAnimationClock(manifest(true));
  assert.deepEqual(clock.sample(accepted("idle", 0.7), 1 / 60, 10), {
    animation: "idle",
    time: 0.7,
  });
  for (let i = 1; i <= 180; i++) {
    const pose = clock.sample(accepted("attack", i / 60), 1 / 60, 10 + i / 60);
    assert.equal(pose.animation, "attack");
  }
  const held = clock.sample(accepted("attack", 4), 1 / 60, 14);
  near(held.time, durations.attack);
  assert.equal(
    held.animation,
    "attack",
    "clip completion cannot choose idle or unlock gameplay",
  );
  assert.equal(held.blend, undefined);
  assert.throws(() => clock.sample(accepted("run", 0), 1 / 60, 15), /complete/);
  assert.equal(
    clock.sample(accepted("death", 0.016), 1 / 60, 16).animation,
    "death",
  );
});

test("every incoming action uses its source fade duration", () => {
  assert.deepEqual(MMORPG_ANIMATION_FADES, {
    idle: 0.25,
    walk: 0.1,
    run: 0.1,
    attack: 0.4,
    dance: 0.2,
    death: 0.2,
  });
  for (const next of Object.keys(durations) as QuickMmorpgMotion[]) {
    const clock = new MmorpgAnimationClock(manifest()),
      from = next === "idle" ? "walk" : "idle";
    clock.sample(accepted(from, 0.37), 0, 0);
    const pose = clock.sample(accepted(next, 1 / 60), 1 / 60, 1 / 60);
    assert.equal(pose.animation, next);
    assert.equal(pose.blend?.from, from);
    near(pose.blend!.weight, 1 / 60 / MMORPG_ANIMATION_FADES[next]);
  }
});

// Recorded from unmodified Three r124 AnimationMixer.crossFadeFrom(..., true),
// with synthetic clips and source player-state.js transitions; no art dependency.
const threeR124 = [
  {
    from: "idle",
    to: "walk",
    samples: [
      {
        frame: 1,
        time: 0.008910533785820008,
        fromTime: 0.39017973860104876,
        weight: 0.1666666716337204,
      },
      {
        frame: 7,
        time: 0.09339826802412668,
        fromTime: 0.5815196069081624,
        weight: 1,
      },
      {
        frame: 18,
        time: 0.27673160135746,
        fromTime: 0.5815196069081624,
        weight: 1,
      },
    ],
  },
  {
    from: "walk",
    to: "run",
    samples: [
      {
        frame: 1,
        time: 0.2511764708765741,
        fromTime: 0.3881818187236786,
        weight: 0.1666666716337204,
      },
      {
        frame: 7,
        time: 0.34137254989230587,
        fromTime: 0.527575754324595,
        weight: 1,
      },
      {
        frame: 18,
        time: 0.5247058832256393,
        fromTime: 0.527575754324595,
        weight: 1,
      },
    ],
  },
  {
    from: "run",
    to: "walk",
    samples: [
      {
        frame: 1,
        time: 0.5960606048054387,
        fromTime: 0.38568627456823984,
        weight: 0.1666666716337204,
      },
      {
        frame: 7,
        time: 0.7112121194150937,
        fromTime: 0.4601960800091425,
        weight: 1,
      },
      {
        frame: 18,
        time: 0.8945454527484276,
        fromTime: 0.4601960800091425,
        weight: 1,
      },
    ],
  },
  {
    from: "walk",
    to: "idle",
    samples: [
      {
        frame: 1,
        time: 0.03633987108866374,
        fromTime: 0.3860461763540904,
        weight: 0.06666667014360428,
      },
      {
        frame: 7,
        time: 0.2248692830403646,
        fromTime: 0.46929292897383373,
        weight: 0.46666666865348816,
      },
      {
        frame: 18,
        time: 0.4475490232308706,
        fromTime: 0.5529004337390264,
        weight: 1,
      },
    ],
  },
  {
    from: "idle",
    to: "attack",
    samples: [
      {
        frame: 1,
        time: 0.01023629109064738,
        fromTime: 0.3871346624692281,
        weight: 0.0416666679084301,
      },
      {
        frame: 7,
        time: 0.07752525011698404,
        fromTime: 0.4997705312569936,
        weight: 0.2916666567325592,
      },
      {
        frame: 18,
        time: 0.22702921628952025,
        fromTime: 0.7500271689891815,
        weight: 0.75,
      },
    ],
  },
  {
    from: "idle",
    to: "dance",
    samples: [
      {
        frame: 1,
        time: 0.02996031641960144,
        fromTime: 0.38602044800917307,
        weight: 0.0833333358168602,
      },
      {
        frame: 7,
        time: 0.18434343139330547,
        fromTime: 0.46857253233591717,
        weight: 0.5833333134651184,
      },
      {
        frame: 18,
        time: 0.37976190249125164,
        fromTime: 0.528506945768992,
        weight: 1,
      },
    ],
  },
  {
    from: "walk",
    to: "death",
    samples: [
      {
        frame: 1,
        time: 0.03149509827295939,
        fromTime: 0.38598258713881173,
        weight: 0.0833333358168602,
      },
      {
        frame: 7,
        time: 0.19215686122576395,
        fromTime: 0.4675124388933181,
        weight: 0.5833333134651184,
      },
      {
        frame: 18,
        time: 0.3889705856641133,
        fromTime: 0.5250995022058487,
        weight: 1,
      },
    ],
  },
] as {
  from: QuickMmorpgMotion;
  to: QuickMmorpgMotion;
  samples: { frame: number; time: number; fromTime: number; weight: number }[];
}[];
test("source phase transfer, incoming/outgoing warp, and fades match Three r124 golden frames", () => {
  for (const fixture of threeR124) {
    const clock = new MmorpgAnimationClock(manifest());
    clock.sample(accepted(fixture.from, 0.37), 0, 0);
    for (let frame = 1; frame <= 18; frame++) {
      const pose = clock.sample(
        accepted(fixture.to, frame / 60),
        1 / 60,
        frame / 60,
      );
      const expected = fixture.samples.find((s) => s.frame === frame);
      if (!expected) continue;
      near(pose.time, expected.time);
      near(pose.blend?.weight ?? 1, expected.weight);
      if (pose.blend) near(pose.blend.fromTime, expected.fromTime);
    }
  }
});

test("first observations, rewound capture clocks, and explicit resets seed accepted clip times", () => {
  const clock = new MmorpgAnimationClock(manifest());
  near(
    clock.sample(accepted("walk", 1.5), 1 / 60, 10).time,
    1.5 % durations.walk,
  );
  clock.sample(accepted("run", 0.2), 1 / 60, 11);
  const rewound = clock.sample(accepted("dance", 2.3), 1 / 60, 2);
  assert.deepEqual(rewound, { animation: "dance", time: 2.3 });
  clock.reset();
  assert.deepEqual(clock.sample(accepted("death", 20), 1 / 60, 0), {
    animation: "death",
    time: durations.death,
  });
});

test("same-frame render passes do not double advance and slow frames retain the source cap", () => {
  const clock = new MmorpgAnimationClock(manifest());
  clock.sample(accepted("idle", 0), 0, 0);
  const first = clock.sample(accepted("walk", 1 / 60), 1 / 60, 1 / 60);
  assert.deepEqual(
    clock.sample(accepted("walk", 1 / 60), 1 / 60, 1 / 60),
    first,
  );
  const a = new MmorpgAnimationClock(manifest()),
    b = new MmorpgAnimationClock(manifest());
  a.sample(accepted("idle", 0), 0, 0);
  b.sample(accepted("idle", 0), 0, 0);
  assert.deepEqual(
    a.sample(accepted("idle", 0.1), 0.1, 0.1),
    b.sample(accepted("idle", 0.1), 1 / 30, 0.1),
  );
});

test("a later accepted one-shot can restart without rendering choosing another state", () => {
  const clock = new MmorpgAnimationClock(manifest());
  clock.sample(accepted("attack", 1.2), 0, 0);
  const restarted = clock.sample(accepted("attack", 1 / 60), 1 / 60, 1 / 60);
  assert.equal(restarted.animation, "attack");
  assert.equal(restarted.blend?.from, "attack");
  assert(restarted.time < 0.02);
  assert.equal(restarted.blend!.layers!.length, 1);
  assert.equal(restarted.blend!.layers![0].time, restarted.time);
  assert.equal(
    restarted.blend!.fromTime,
    restarted.time,
    "the original mixer reuses one attack action instead of inventing an outgoing duplicate",
  );
});

test("invalid clocks, actions, and clips are rejected before presentation state changes", () => {
  const clock = new MmorpgAnimationClock(manifest());
  clock.sample(accepted("idle", 0.2), 0, 0);
  for (const invalid of [
    () => clock.sample(accepted("walk", NaN), 1 / 60, 1),
    () => clock.sample(accepted("walk", -0.1), 1 / 60, 1),
    () => clock.sample(accepted("walk", 0), NaN, 1),
    () => clock.sample(accepted("walk", 0), 1 / 60, NaN),
  ])
    assert.throws(invalid);
  const good = clock.sample(accepted("idle", 0.3), 1 / 60, 1 / 60);
  near(good.time, 0.2 + 1 / 60);
  assert.equal(good.blend, undefined);
  assert.throws(
    () =>
      new MmorpgAnimationClock({
        clips: manifest().clips.filter((c) => c.name !== "attack"),
      }),
  );
  assert.throws(
    () =>
      new MmorpgAnimationClock({
        clips: [...manifest().clips, manifest().clips[0]],
      }),
  );
  assert.throws(
    () =>
      new MmorpgAnimationClock({
        clips: manifest().clips.map((c) => ({ ...c, duration: 0 })),
      }),
  );
});
