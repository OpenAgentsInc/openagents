/** Measure coordinated displacement independently of GPU order labels. */
export function squadMotion(
  initial,
  frames,
  members,
  direction,
  minimumProjection = 0.5,
  minimumFraction = 0.75,
) {
  const finite = (value) => typeof value === "number" && Number.isFinite(value);
  if (
    !Array.isArray(initial) ||
    !Array.isArray(frames) ||
    !frames.length ||
    !Array.isArray(members) ||
    members.length < 2 ||
    new Set(members).size !== members.length ||
    !members.every(Number.isSafeInteger) ||
    !Array.isArray(direction) ||
    direction.length !== 2 ||
    !direction.every(finite) ||
    Math.hypot(...direction) < 1e-6 ||
    !finite(minimumProjection) ||
    minimumProjection <= 0 ||
    !finite(minimumFraction) ||
    minimumFraction <= 0 ||
    minimumFraction > 1
  )
    throw Error(
      "Choose distinct members, real frames, and a finite movement direction.",
    );
  const unit = direction.map((value) => value / Math.hypot(...direction)),
    start = new Map(initial.map((a) => [a.id, a])),
    last = new Map(frames.at(-1).actors.map((a) => [a.id, a]));
  if (
    !frames.every(
      (frame, index) =>
        Number.isSafeInteger(frame.tick) &&
        (!index || frame.tick > frames[index - 1].tick),
    )
  )
    throw Error("Squad samples must advance in tick order.");
  const actors = members.map((id) => {
    const a = start.get(id),
      b = last.get(id);
    if (!a || !b || !a.position?.every(finite) || !b.position?.every(finite))
      throw Error(
        "A selected squad member has no finite starting or final position.",
      );
    const dx = b.position[0] - a.position[0],
      dz = b.position[2] - a.position[2];
    return {
      id,
      projection: dx * unit[0] + dz * unit[1],
      distance: Math.hypot(dx, dz),
      alive: b.health > 0,
    };
  });
  const passed = actors.filter(
    (a) => a.alive && a.projection >= minimumProjection,
  ).length;
  return {
    ok: passed / members.length >= minimumFraction,
    passed,
    members: members.length,
    fraction: passed / members.length,
    minimumProjection,
    minimumFraction,
    meanProjection:
      actors.reduce((sum, a) => sum + a.projection, 0) / actors.length,
    actors,
  };
}
export function squadClearance(frames, radii) {
  let minimum = Infinity,
    pair = null;
  for (const frame of frames)
    for (let i = 0; i < frame.actors.length; i++)
      for (let j = i + 1; j < frame.actors.length; j++) {
        const a = frame.actors[i],
          b = frame.actors[j];
        if (a.health <= 0 || b.health <= 0) continue;
        if (
          ![...a.position, ...b.position, radii[a.kind], radii[b.kind]].every(
            Number.isFinite,
          )
        )
          throw Error("A body sample has invalid coordinates or radius.");
        const gap =
          Math.hypot(
            a.position[0] - b.position[0],
            a.position[2] - b.position[2],
          ) -
          radii[a.kind] -
          radii[b.kind];
        if (gap < minimum) {
          minimum = gap;
          pair = { tick: frame.tick, left: a.id, right: b.id };
        }
      }
  return { minimumCoreGap: Number.isFinite(minimum) ? minimum : null, pair };
}
