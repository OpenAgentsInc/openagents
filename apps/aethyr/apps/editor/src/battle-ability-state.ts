import type { EngineStatus } from "../../../packages/editor-services/protocol.ts";

/** GPU cooldowns and the admitted terrain target drive this display; no local timer predicts readiness. */
export function battleAttackControl(
  status: Pick<EngineStatus, "phase" | "mmorpg" | "battle">,
  pending = false,
) {
  const participant = status.battle?.participant;
  const meteor = participant?.attackKind === "meteor_shower";
  const ticks = status.battle?.stats.meteorCooldownTicks;
  const knownCooldown =
    typeof ticks === "number" && Number.isFinite(ticks) && ticks >= 0;
  const seconds = knownCooldown ? ticks / 60 : undefined;
  const live =
    status.phase === "ready" &&
    status.mmorpg?.connection === "online" &&
    status.battle?.enabled === true &&
    ["first", "third"].includes(status.battle.camera) &&
    participant?.active === true &&
    participant.ready &&
    participant.alive;
  const note = !meteor
    ? undefined
    : !knownCooldown
      ? "Preparing spell…"
      : ticks > 0
        ? `Ready in ${Math.ceil(seconds!)}s`
        : !participant?.target
          ? "Aim at the ground"
          : "Ready to cast";
  return {
    meteor,
    label: meteor ? "Cast" : "Attack",
    seconds,
    enabled:
      !!live &&
      !pending &&
      (!meteor || (knownCooldown && ticks === 0 && !!participant?.target)),
    note,
  };
}
