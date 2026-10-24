/** Checks real projectile observations against the six-meteor spell timeline. */
export function evaluateBattleMeteors(frames, options) {
  const failures = [];
  const need = (value, message) => {
    if (!value) failures.push(message);
  };
  const first = frames[0];
  if (!first)
    return {
      ok: false,
      failures: ["No meteor frames were observed."],
      summary: {},
    };
  const scheduled = first.ordnance.samples.filter(
    (p) => p.kind === 3 && p.ownerEpoch === options.epoch,
  );
  need(
    scheduled.length === 6 && new Set(scheduled.map((p) => p.id)).size === 6,
    "A cast must reserve six distinct real projectiles.",
  );
  const damage = scheduled[0]?.damage;
  need(
    Number.isInteger(damage) &&
      damage >= 40 &&
      damage <= 240 &&
      scheduled.every((p) => p.damage === damage),
    "All six meteors must share one 40d6 roll.",
  );
  const records = new Map(
    scheduled.map((p) => [
      p.id,
      { target: p.target, launched: null, impacted: null, previousAge: p.age },
    ]),
  );
  for (const p of scheduled) {
    need(
      p.age < 0 && p.flight === 1 && p.radius === 4,
      "Scheduled meteors must retain their windup, one-second fall, and four-metre blast.",
    );
    need(
      Math.hypot(p.target[0] - options.aim[0], p.target[2] - options.aim[2]) <=
        6.001,
      "Meteor destinations escaped the frozen six-metre footprint.",
    );
    need(
      Math.hypot(
        p.position[0] - p.target[0] + 12,
        p.position[1] - p.target[1] - 80,
        p.position[2] - p.target[2] + 8,
      ) < 0.01,
      "A scheduled meteor does not start at its sky position.",
    );
  }
  let previousTick = -1;
  for (const frame of frames) {
    need(
      frame.tick > previousTick,
      "Meteor observations must advance actual simulation ticks.",
    );
    previousTick = frame.tick;
    need(
      frame.probe.gpu.meteorCasts === options.castsBefore + 1,
      "An inhibited or held sequence started another cast.",
    );
    if (frame.tick < options.castTick + 210)
      need(
        frame.probe.gpu.participantDamage === options.damageBefore &&
          frame.probe.gpu.meteorImpacts === options.impactsBefore,
        "Damage or impact occurred before the windup and fall completed.",
      );
    for (const p of frame.ordnance.samples) {
      const record = records.get(p.id);
      if (
        !record ||
        p.ownerEpoch !== options.epoch ||
        (p.kind !== 3 && p.kind !== 4)
      )
        continue;
      need(
        p.damage === damage,
        "A meteor changed its cast's shared damage roll.",
      );
      need(
        p.target.every((v, i) => Math.abs(v - record.target[i]) < 0.0001),
        "Changing aim moved an already committed meteor destination.",
      );
      if (p.kind === 3) {
        need(
          p.age + 0.0001 >= record.previousAge,
          "A falling meteor clock moved backwards.",
        );
        record.previousAge = p.age;
        if (p.age >= 0 && record.launched === null)
          record.launched = frame.tick;
      } else if (record.impacted === null) record.impacted = frame.tick;
    }
    need(
      frame.probe.gpu.meteorImpacts - options.impactsBefore ===
        [...records.values()].filter((r) => r.impacted !== null).length,
      "Impact counters disagree with real projectile-to-impact transitions.",
    );
  }
  const timeline = [...records.entries()]
    .map(([id, r]) => ({ id, ...r }))
    .sort((a, b) => a.launched - b.launched);
  timeline.forEach((record, index) => {
    const launch = options.castTick + 150 + Math.round(index * 0.13 * 60);
    need(
      record.launched !== null && Math.abs(record.launched - launch) <= 1,
      "A meteor did not launch after its 2.5-second cast and stagger.",
    );
    need(
      record.impacted !== null &&
        record.launched !== null &&
        Math.abs(record.impacted - record.launched - 60) <= 1,
      "A meteor did not fall for one second before impacting.",
    );
  });
  const last = frames.at(-1).probe.gpu;
  need(
    last.meteorImpacts - options.impactsBefore === 6,
    "The six scheduled meteors did not make exactly six impacts.",
  );
  need(
    last.participantDamage > options.damageBefore &&
      last.meteorHits > options.hitsBefore,
    "The shower did not damage real opposing actors.",
  );
  if (options.requireKill)
    need(
      last.participantKills > options.killsBefore,
      "No actual meteor kill received local credit.",
    );
  return {
    ok: failures.length === 0,
    failures: [...new Set(failures)],
    summary: {
      damageRoll: damage,
      projectiles: timeline,
      impacts: last.meteorImpacts - options.impactsBefore,
      damage: last.participantDamage - options.damageBefore,
      kills: last.participantKills - options.killsBefore,
      hits: last.meteorHits - options.hitsBefore,
    },
  };
}
