/** Experimental mean-rate assessments remain separate from strict pacing gates. */
import { battleParticipantEvidence } from "./battle-shot-plan.mjs";
export const BATTLE_COVERAGE_VIEWS = Object.freeze([
  "overview",
  "orbit",
  "dolly",
  "first",
  "third",
]);
const finite = (n) => typeof n === "number" && Number.isFinite(n);
const counter = (n) => Number.isSafeInteger(n) && n >= 0;
/** Restore the recorded third-person firing angle after its camera resets. */
export function battleParticipationAim(view) {
  if (!["first", "third"].includes(view))
    throw new Error("Choose a participating battle view.");
  return { lookX: 0, lookY: view === "third" ? 0.17 : 0 };
}
export function battleParticipantWindow(samples, events = []) {
  const evidence = battleParticipantEvidence(
    samples.map((sample) => ({ ...sample, at: sample.wallMs ?? sample.at })),
    events,
  );
  const delta = (key) => {
    const values = samples.map((s) => s.battle?.stats?.[key]);
    return values.length >= 2 &&
      values.every(counter) &&
      values.every((v, i) => !i || v >= values[i - 1])
      ? values.at(-1) - values[0]
      : null;
  };
  let deaths = 0,
    lost = 0,
    validLost = samples.length >= 2;
  for (let i = 0; i < samples.length; i++) {
    const b = samples[i].battle?.participant,
      a = samples[i - 1]?.battle?.participant;
    if (!finite(b?.droppedSeconds) || b.droppedSeconds < 0) validLost = false;
    if (!a) continue;
    if (a.alive && !b?.alive) deaths++;
    const reset = events.some(
      (event) =>
        event.kind === "respawn-request" &&
        event.atMs >= (samples[i - 1].wallMs ?? samples[i - 1].at) &&
        event.atMs <= (samples[i].wallMs ?? samples[i].at),
    );
    if (b.droppedSeconds < a.droppedSeconds && !reset) validLost = false;
    if (finite(a.droppedSeconds) && finite(b.droppedSeconds))
      lost += Math.max(0, b.droppedSeconds - a.droppedSeconds);
  }
  const gpu = {
    attacks: delta("participantAttacks"),
    damage: delta("participantDamage"),
    kills: delta("participantKills"),
    incomingHits: delta("participantHits"),
  };
  return {
    ...evidence,
    gpu,
    observedDeaths: deaths,
    observedDroppedSeconds: validLost ? lost : null,
    ok:
      evidence.activeParticipation &&
      gpu.attacks > 0 &&
      gpu.damage > 0 &&
      validLost &&
      lost === 0,
    scope:
      "Timed sampled local movement and attack edges, plus increasing GPU accepted attacks and outgoing damage. Respawn jumps are excluded. These are real public controls; no player or NPC poses or health are injected. Sampled local dropped-time counters are checked across explicit life resets.",
  };
}
export function battleWindowAround60(row) {
  const summary = row.applicationPacing?.summary,
    wide = ["overview", "orbit"].includes(row.view),
    following = ["first", "third"].includes(row.view);
  const checks = [
    {
      id: "mean.completed-at-least-58hz",
      ok: finite(summary?.completedRateHz) && summary.completedRateHz >= 58,
    },
    {
      id: "mean.accepted-at-least-58hz",
      ok: finite(summary?.acceptedRateHz) && summary.acceptedRateHz >= 58,
    },
    {
      id: "simulation.no-measured-lost-time",
      ok:
        row.battleContinuity?.noDroppedTime === true &&
        summary?.wholeWindowDropped?.kernelDroppedSeconds === 0 &&
        summary?.wholeWindowDropped?.hostClampedSeconds === 0,
    },
    { id: "status.fresh", ok: row.statusFreshness?.ok === true },
    { id: "combat.advances", ok: row.combatProgress?.ok === true },
    { id: "overflow.no-loss", ok: row.overflowFidelity?.ok === true },
    {
      id: "visible.real-geometry",
      ok:
        row.populations?.visible?.min > 0 &&
        row.populations?.triangles?.min > 0,
    },
    {
      id: "wide.pool-visible",
      ok: !wide || row.populations?.visible?.min >= row.count * 0.95,
    },
    {
      id: "following.near-detail-visible",
      ok: !following || row.populations?.near?.min > 0,
    },
    {
      id: "following.real-participant-combat",
      ok: !following || row.participantEvidence?.ok === true,
    },
  ];
  return {
    ok: row.count > 0 && checks.every((c) => c.ok),
    minimumMeanHz: 58,
    acceptedHz: summary?.acceptedRateHz ?? null,
    completedHz: summary?.completedRateHz ?? null,
    checks,
    strictPacingPassed:
      row.pacing?.ok === true && row.applicationPacing?.ok === true,
    scope:
      "Experimental mean target around 60: at least 58 accepted and GPU-completed frames per second, with measured simulation continuity, fresh combat, overflow, and view-detail checks. This is not strict pacing qualification or physical display presentation.",
  };
}
export function battleAllViewsCoverage(windows, count) {
  const matrix = BATTLE_COVERAGE_VIEWS.map((view) => ({
    view,
    windows: windows.filter((w) => w.count === count && w.view === view),
  }));
  const checks = matrix.map((group) => {
    const repeats = group.windows.map((w) => w.repeat);
    return {
      view: group.view,
      windows: group.windows.length,
      ok:
        group.windows.length >= 2 &&
        repeats.every((n) => Number.isSafeInteger(n) && n >= 0) &&
        new Set(repeats).size === group.windows.length &&
        group.windows.every(
          (w) =>
            finite(w.observedMs) &&
            w.observedMs >= 30000 &&
            w.warmupMs >= 5000 &&
            w.around60?.ok === true,
        ),
    };
  });
  return {
    ok: count > 0 && checks.every((c) => c.ok),
    count,
    requiredViews: BATTLE_COVERAGE_VIEWS,
    minimumWindowsPerView: 2,
    minimumDurationMs: 30000,
    minimumWarmupMs: 5000,
    checks,
    scope:
      "All five actual Battle Lab camera buttons, with at least two independent sustained windows per view; first and third person require actual local movement, accepted GPU attacks, and damage. Legacy player-camera aliases do not fill missing views.",
  };
}
