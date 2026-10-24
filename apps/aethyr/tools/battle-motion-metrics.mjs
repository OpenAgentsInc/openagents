/** Analyze read-only, consecutive fixed-step crowd states in Node or the GPU harness. */
export function evaluateBattleMotion(frames, policy) {
  const {
    radii,
    speeds,
    step = 1 / 60,
    turnRate = 4,
    pushSpeed = 0.6,
    penetrationTolerance = 0.05,
    clearance = 0.1,
    targetRatio = 0.75,
    targetDistanceTolerance = 0.15,
  } = policy;
  const tolerance = 1e-4,
    failures = [],
    failedChecks = new Set();
  const checks = {
    validSamples: true,
    consecutiveTicks: true,
    boundedTurning: true,
    boundedTravel: true,
    coreClearance: true,
    animationContinuity: true,
    stableTargets: true,
  };
  const fail = (check, detail) => {
    checks[check] = false;
    failedChecks.add(check);
    if (failures.length < 32) failures.push({ check, ...detail });
  };
  const summary = {
    frames: frames?.length ?? 0,
    actors: 0,
    comparedLives: 0,
    respawns: 0,
    deaths: 0,
    movingIntervals: 0,
    clockIntervals: 0,
    gaitIntervals: 0,
    attackIntervals: 0,
    tightContactPairs: 0,
    targetRetainedIntervals: 0,
    validTargetSwitches: 0,
    maxYawDelta: 0,
    maxTravel: 0,
    maxCorePenetration: 0,
    maxOverlappingPairs: 0,
    maxClearanceShortfall: 0,
    contactPairs: 0,
  };
  const contactFrames = [];
  const valid = (a) =>
    a &&
    Number.isInteger(a.id) &&
    a.id >= 0 &&
    radii[a.kind] > 0 &&
    speeds[a.kind] > 0 &&
    Array.isArray(a.position) &&
    (a.team === 0 || a.team === 1) &&
    [a.health, a.yaw, a.actionTime, a.animationTime, ...a.position].every(
      Number.isFinite,
    ) &&
    a.position.length === 3 &&
    a.health >= 0 &&
    Number.isInteger(a.target) &&
    a.target >= 0 &&
    a.target <= 0xffffffff &&
    Number.isInteger(a.state) &&
    a.state >= 0 &&
    a.state <= 3;
  if (!Array.isArray(frames) || frames.length < 2 || frames.length > 512) {
    fail("validSamples", {
      reason: "Choose two to 512 consecutive readbacks.",
    });
    return { ok: false, checks, summary, failures, contactFrames };
  }
  let previous;
  for (const frame of frames) {
    if (
      !Number.isSafeInteger(frame.tick) ||
      !Array.isArray(frame.actors) ||
      frame.actors.length < 1 ||
      frame.actors.length > 256 ||
      frame.actors.some((a) => !valid(a)) ||
      new Set(frame.actors.map((a) => a.id)).size !== frame.actors.length
    ) {
      fail("validSamples", { tick: frame.tick });
      continue;
    }
    const now = new Map(frame.actors.map((a) => [a.id, a]));
    summary.actors = Math.max(summary.actors, now.size);
    let overlapPairs = 0,
      contacts = 0,
      tightContacts = 0,
      maxPenetration = 0,
      minGap = Infinity;
    for (let i = 0; i < frame.actors.length; i++) {
      const a = frame.actors[i];
      if (a.health <= 0) continue;
      for (let j = i + 1; j < frame.actors.length; j++) {
        const b = frame.actors[j];
        if (b.health <= 0) continue;
        const distance = Math.hypot(
          a.position[0] - b.position[0],
          a.position[2] - b.position[2],
        );
        const gap = distance - radii[a.kind] - radii[b.kind];
        minGap = Math.min(minGap, gap);
        if (gap < clearance + 0.5) contacts++;
        if (gap <= clearance + 0.025) tightContacts++;
        if (gap < 0) overlapPairs++;
        maxPenetration = Math.max(maxPenetration, -gap);
        summary.maxClearanceShortfall = Math.max(
          summary.maxClearanceShortfall,
          clearance - gap,
        );
        if (gap < -penetrationTolerance)
          fail("coreClearance", {
            tick: frame.tick,
            pair: [a.id, b.id],
            gap,
            distance,
            radiusSum: radii[a.kind] + radii[b.kind],
          });
      }
    }
    summary.contactPairs += contacts;
    summary.tightContactPairs += tightContacts;
    summary.maxOverlappingPairs = Math.max(
      summary.maxOverlappingPairs,
      overlapPairs,
    );
    summary.maxCorePenetration = Math.max(
      summary.maxCorePenetration,
      maxPenetration,
    );
    contactFrames.push({
      tick: frame.tick,
      contacts,
      tightContacts,
      overlappingPairs: overlapPairs,
      maxPenetration,
      minCoreGap: Number.isFinite(minGap) ? minGap : null,
    });
    if (previous) {
      const before = new Map(previous.actors.map((a) => [a.id, a]));
      if (
        frame.tick !== previous.tick + 1 ||
        now.size !== before.size ||
        [...now.keys()].some((id) => !before.has(id))
      ) {
        fail("consecutiveTicks", { previous: previous.tick, tick: frame.tick });
        previous = frame;
        continue;
      }
      for (const a of frame.actors) {
        const b = before.get(a.id);
        if (b.health <= 0 && a.health > 0) {
          summary.respawns++;
          continue;
        }
        if (b.health > 0 && a.health <= 0) {
          summary.deaths++;
          continue;
        }
        if (a.health > 0) {
          summary.comparedLives++;
          const yaw = Math.abs(
            Math.atan2(Math.sin(a.yaw - b.yaw), Math.cos(a.yaw - b.yaw)),
          );
          const travel = Math.hypot(
            a.position[0] - b.position[0],
            a.position[2] - b.position[2],
          );
          summary.maxYawDelta = Math.max(summary.maxYawDelta, yaw);
          summary.maxTravel = Math.max(summary.maxTravel, travel);
          if (travel > 1e-4) summary.movingIntervals++;
          if (yaw > turnRate * step + tolerance)
            fail("boundedTurning", { tick: frame.tick, id: a.id, yaw });
          if (travel > (speeds[a.kind] + pushSpeed) * step + tolerance)
            fail("boundedTravel", { tick: frame.tick, id: a.id, travel });
          if (
            a.animationTime < b.animationTime - tolerance ||
            a.animationTime > b.animationTime + 1.2 * step + tolerance
          )
            fail("animationContinuity", {
              tick: frame.tick,
              id: a.id,
              field: "gait",
              before: b.animationTime,
              after: a.animationTime,
            });
          const oldTarget = before.get(b.target);
          const oldTargetNow = now.get(b.target);
          if (
            oldTarget?.health > 0 &&
            oldTargetNow?.health > 0 &&
            oldTarget.team !== b.team
          ) {
            if (a.target === b.target) summary.targetRetainedIntervals++;
            else {
              const nextTarget = before.get(a.target);
              const distance = (other) =>
                Math.hypot(
                  b.position[0] - other.position[0],
                  b.position[2] - other.position[2],
                );
              const oldDistance = distance(oldTarget),
                newDistance = nextTarget ? distance(nextTarget) : Infinity;
              if (
                !nextTarget ||
                nextTarget.health <= 0 ||
                nextTarget.team === b.team ||
                newDistance >=
                  oldDistance * targetRatio + targetDistanceTolerance
              )
                fail("stableTargets", {
                  tick: frame.tick,
                  id: a.id,
                  from: b.target,
                  to: a.target,
                  oldDistance,
                  newDistance,
                });
              else summary.validTargetSwitches++;
            }
          }
        }
        if (a.state === b.state) {
          summary.clockIntervals++;
          if (a.state === 1) summary.gaitIntervals++;
          if (a.state === 2) summary.attackIntervals++;
          const change = a.actionTime - b.actionTime;
          const maxAdvance =
            a.state === 1 ? 1.2 * step : a.state === 0 ? 0 : step;
          if (change < -tolerance || change > maxAdvance + tolerance)
            fail("animationContinuity", {
              tick: frame.tick,
              id: a.id,
              field: "action",
              state: a.state,
              before: b.actionTime,
              after: a.actionTime,
            });
        }
      }
    }
    previous = frame;
  }
  return {
    ok: failedChecks.size === 0,
    checks,
    summary,
    failures,
    contactFrames,
    policy: {
      radii,
      speeds,
      step,
      turnRate,
      pushSpeed,
      penetrationTolerance,
      clearance,
      targetRatio,
      targetDistanceTolerance,
    },
    scope:
      "Living-body horizontal cores, actual actor clocks and target IDs. Corpses and respawn jumps are separate; this does not establish pixel silhouette or physical presentation.",
  };
}

/** Rear melee ranks must advance while no opposing body is close enough to stop them. */
export function evaluateBattleConvoy(checkpoints, policy = {}) {
  const {
    centerX = 0,
    radius = 100,
    step = 1 / 60,
    rearFraction = 0.94,
    minimumEligible = 16,
    minimumPassingFraction = 0.5,
    minimumNetForward = 4,
    minimumLateForward = 0.5,
    contactDistance = 4,
  } = policy;
  const checks = {
    validSamples: true,
    fullFiveSeconds: true,
    rearCohort: true,
    lateForwardProgress: true,
  };
  const failures = [],
    actors = [];
  const fail = (check, reason) => {
    checks[check] = false;
    failures.push({ check, reason });
  };
  if (
    !Array.isArray(checkpoints) ||
    checkpoints.length !== 6 ||
    checkpoints.some(
      (frame) =>
        !Number.isInteger(frame.tick) ||
        !Array.isArray(frame.actors) ||
        frame.actors.length !== 10000 ||
        new Set(frame.actors.map((a) => a.id)).size !== 10000 ||
        frame.actors.some(
          (a) =>
            !Number.isInteger(a.id) ||
            a.id < 0 ||
            a.id >= 10000 ||
            !Array.isArray(a.position) ||
            a.position.length !== 3 ||
            ![...a.position, a.health].every(Number.isFinite) ||
            (a.team !== 0 && a.team !== 1),
        ),
    )
  ) {
    fail(
      "validSamples",
      "Six complete finite 10,000-actor checkpoints are required.",
    );
    return { ok: false, checks, failures, actors };
  }
  const elapsed = (checkpoints.at(-1).tick - checkpoints[0].tick) * step;
  const lateElapsed =
    (checkpoints.at(-1).tick - checkpoints.at(-2).tick) * step;
  if (
    Math.abs(elapsed - 5) > 1e-6 ||
    Math.abs(lateElapsed - 1) > 1e-6 ||
    checkpoints.some(
      (frame, i) => i && frame.tick - checkpoints[i - 1].tick !== 60,
    )
  )
    fail(
      "fullFiveSeconds",
      "Readbacks must cover five fixed-step seconds with one-second checkpoints.",
    );
  const indexed = checkpoints.map(
    (frame) => new Map(frame.actors.map((a) => [a.id, a])),
  );
  const cohort = checkpoints[0].actors.filter(
    (a) =>
      a.kind === "paladin" &&
      a.health > 0 &&
      a.id % 4 !== 0 &&
      (a.team === 0 ? centerX - a.position[0] : a.position[0] - centerX) >=
        radius * rearFraction,
  );
  for (const initial of cohort) {
    const poses = indexed.map((frame) => frame.get(initial.id));
    const alive = poses.every(
      (a) => a.health > 0 && a.kind === initial.kind && a.team === initial.team,
    );
    let nearestOpposingDistance = Infinity;
    checkpoints.forEach((frame, i) => {
      const a = poses[i];
      for (const enemy of frame.actors)
        if (enemy.health > 0 && enemy.team !== a.team)
          nearestOpposingDistance = Math.min(
            nearestOpposingDistance,
            Math.hypot(
              a.position[0] - enemy.position[0],
              a.position[2] - enemy.position[2],
            ),
          );
    });
    const direction = initial.team === 0 ? 1 : -1;
    const netForward =
      (poses.at(-1).position[0] - initial.position[0]) * direction;
    const lateForward =
      (poses.at(-1).position[0] - poses.at(-2).position[0]) * direction;
    const eligible = alive && nearestOpposingDistance > contactDistance;
    actors.push({
      id: initial.id,
      initialPosition: initial.position,
      finalPosition: poses.at(-1).position,
      alive,
      nearestOpposingDistance: Number.isFinite(nearestOpposingDistance)
        ? nearestOpposingDistance
        : null,
      eligible,
      exclusion: !alive
        ? "death-or-class-change"
        : !eligible
          ? "opposing-contact"
          : null,
      netForward,
      lateForward,
      progressed:
        eligible &&
        netForward >= minimumNetForward &&
        lateForward >= minimumLateForward,
    });
  }
  const eligible = actors.filter((a) => a.eligible),
    progressed = eligible.filter((a) => a.progressed);
  const passingFraction = eligible.length
    ? progressed.length / eligible.length
    : 0;
  if (eligible.length < minimumEligible)
    fail(
      "rearCohort",
      "Too few living rear melee actors remained away from opponents.",
    );
  if (passingFraction < minimumPassingFraction)
    fail(
      "lateForwardProgress",
      "Rear ranks stalled despite remaining away from opposing contact.",
    );
  return {
    ok: Object.values(checks).every(Boolean),
    checks,
    failures,
    actors,
    summary: {
      elapsed,
      lateElapsed,
      selected: cohort.length,
      eligible: eligible.length,
      progressed: progressed.length,
      passingFraction,
      minimumNet: eligible.length
        ? Math.min(...eligible.map((a) => a.netForward))
        : null,
      minimumLate: eligible.length
        ? Math.min(...eligible.map((a) => a.lateForward))
        : null,
    },
    policy: {
      centerX,
      radius,
      step,
      rearFraction,
      minimumEligible,
      minimumPassingFraction,
      minimumNetForward,
      minimumLateForward,
      contactDistance,
    },
    scope:
      "Public-seeded rear Paladins excluding actual ranged-role IDs and sampled opposing contact. Progress uses measured GPU displacement, including a separate final-second gate; stationary ranged actors and frontline contact are not required to move.",
  };
}
