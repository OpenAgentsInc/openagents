/** Bounded real controls, applied identically to recorded and unrecorded shots. */
export const NEUTRAL_BATTLE_INPUT = Object.freeze({
  forward: 0,
  turn: 0,
  sprint: false,
  attack: false,
});

/** Slow real look inputs; each update sends only the elapsed angular difference. */
export function battleRecordingLook(seconds, previousMs, elapsedMs, view) {
  if (
    !Number.isFinite(seconds) ||
    seconds < 2 ||
    seconds > 60 ||
    !Number.isFinite(previousMs) ||
    !Number.isFinite(elapsedMs) ||
    previousMs < 0 ||
    elapsedMs < previousMs ||
    !["first", "third"].includes(view)
  )
    throw new RangeError("Choose a bounded participant camera sweep.");
  const phase = (ms) => Math.min(1, ms / (seconds * 1000)) * Math.PI * 2;
  const a = phase(previousMs),
    b = phase(elapsedMs);
  const amplitude = view === "third" ? Math.PI / 6 : Math.PI / 12;
  const bounded = (value) => Math.max(-0.5, Math.min(0.5, value));
  return {
    lookX: bounded(amplitude * (Math.sin(b) - Math.sin(a))),
    lookY: bounded(0.0125 * (Math.cos(a) - Math.cos(b))),
  };
}

export function battleParticipationPlan(seconds) {
  if (!Number.isFinite(seconds) || seconds < 2 || seconds > 60)
    throw new RangeError("Choose a two to sixty second battle shot.");
  return [
    [0, {}],
    [0.05, { forward: 1 }],
    [0.2, { attack: true }],
    [0.45, { forward: 1, turn: 0.2, sprint: true }],
    [0.6, { turn: -0.3, attack: true }],
    [0.85, { attack: true }],
  ].map(([fraction, input]) => ({
    atMs: fraction * seconds * 1000,
    input: { ...NEUTRAL_BATTLE_INPUT, ...input },
  }));
}

/** Respawn jumps do not count as player-controlled travel. */
export function battleParticipantEvidence(samples, events = []) {
  const actors = samples.map((sample) => ({
    at: sample.at,
    actor: sample.battle?.participant,
  }));
  const valid =
    actors.length >= 2 &&
    actors.every(
      ({ actor }) =>
        actor?.active === true &&
        Array.isArray(actor.position) &&
        actor.position.length === 3 &&
        actor.position.every(Number.isFinite) &&
        Number.isSafeInteger(actor.attackSequence) &&
        actor.attackSequence >= 0 &&
        Number.isFinite(actor.health) &&
        Number.isFinite(actor.maxHealth) &&
        actor.maxHealth > 0 &&
        actor.health >= 0 &&
        actor.health <= actor.maxHealth,
    );
  let movementMetres = 0,
    damageTaken = 0,
    attackIncrements = 0;
  if (valid)
    for (let i = 1; i < actors.length; i++) {
      const a = actors[i - 1],
        b = actors[i];
      const respawn = events.some(
        (event) =>
          event.kind === "respawn-request" &&
          event.atMs >= a.at &&
          event.atMs <= b.at,
      );
      if (respawn) continue;
      if (a.actor.alive && b.actor.alive)
        movementMetres += Math.hypot(
          ...b.actor.position.map((n, axis) => n - a.actor.position[axis]),
        );
      damageTaken += Math.max(0, a.actor.health - b.actor.health);
      attackIncrements += Math.max(
        0,
        b.actor.attackSequence - a.actor.attackSequence,
      );
    }
  return {
    valid,
    movementMetres,
    damageTaken,
    observedAttackIncrements: attackIncrements,
    respawnRequests: events.filter((event) => event.kind === "respawn-request")
      .length,
    activeParticipation: valid && movementMetres >= 0.5 && attackIncrements > 0,
    scope:
      "Observed timed local-actor state changes; respawn displacement is excluded. Attack sequences prove actual simulation attacks, not guaranteed hits. Damage and kill attribution require GPU combat counters.",
  };
}
