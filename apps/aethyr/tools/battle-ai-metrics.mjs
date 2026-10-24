/** Independent numeric oracles for the bounded GPU AI fixtures. */
export function aiHash(value) {
  let x = value >>> 0;
  x = Math.imul(x ^ (x >>> 16), 0x7feb352d) >>> 0;
  x = Math.imul(x ^ (x >>> 15), 0x846ca68b) >>> 0;
  return (x ^ (x >>> 16)) >>> 0;
}
export function rangedBand(id, seed, radius) {
  const preferred = Math.min(
    48 + 8 * ((aiHash(id ^ seed ^ 0x6c8e9cf5) & 0xffffff) / 16777216),
    Math.max(4.5, radius * 0.6),
  );
  return {
    preferred,
    inner: preferred - Math.min(8, preferred * 0.2),
    outer: preferred + Math.min(4, preferred * 0.12),
    pressure: Math.min(8, preferred * 0.25),
  };
}
export function ledPoint(actor, origin, speed, maximumFlight) {
  const distance = Math.hypot(
    actor.position[0] - origin[0],
    actor.position[2] - origin[2],
  );
  const flight = Math.min(maximumFlight, Math.max(0.3, distance / speed));
  const lead = actor.velocity.map((v) => v * flight);
  const length = Math.hypot(...lead),
    factor = Math.min(1, 6 / Math.max(length, 0.0001));
  return {
    flight,
    displacement: lead.map((v) => v * factor),
    point: [
      actor.position[0] + lead[0] * factor,
      0.6,
      actor.position[2] + lead[1] * factor,
    ],
  };
}
export function siegeChoice(actors, siege, id, tick, radius) {
  const count = actors.length,
    offset = aiHash(id ^ tick) % count;
  const cell = (a) =>
    a.position
      .filter((_, i) => i !== 1)
      .map((v) => Math.max(0, Math.min(511, Math.floor((v + 1024) / 4))));
  const candidates = [];
  for (let sample = 0; sample < 24; sample++) {
    const candidate =
      (offset +
        Math.floor(
          Math.fround(Math.fround(sample) * Math.fround(count)) / 24,
        )) %
      count;
    const actor = actors[candidate];
    if (actor.health <= 0 || actor.team === siege.team) continue;
    const center = cell(actor);
    let score = 0;
    for (const other of actors) {
      const q = cell(other);
      if (
        other.health <= 0 ||
        Math.abs(q[0] - center[0]) > 2 ||
        Math.abs(q[1] - center[1]) > 2 ||
        Math.hypot(
          other.position[0] - actor.position[0],
          other.position[2] - actor.position[2],
        ) >= 8
      )
        continue;
      score += other.team === siege.team ? -0.5 : other.state === 2 ? 3 : 1;
    }
    candidates.push({
      id: candidate,
      score,
      distance: Math.hypot(
        actor.position[0] - siege.position[0],
        actor.position[2] - siege.position[2],
      ),
    });
  }
  candidates.sort(
    (a, b) => b.score - a.score || a.distance - b.distance || a.id - b.id,
  );
  const best = candidates[0];
  if (!best) return { id: null, candidates };
  const lead = ledPoint(actors[best.id], siege.position, 33, 3);
  return {
    ...best,
    candidates,
    ...lead,
    point: lead.point.map((v, i) =>
      i === 1 ? v : Math.max(-radius, Math.min(radius, v)),
    ),
  };
}
