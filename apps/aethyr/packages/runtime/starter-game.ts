/** Bounded, reusable gameplay capabilities for the owned key-and-exit starter. */
export const STARTER_CAPABILITIES = Object.freeze([
  { id: "character.controller", version: 1, provider: "rust-kernel" },
  { id: "camera.first-third-person", version: 1, provider: "runtime-host" },
  { id: "interaction.proximity", version: 1, provider: "starter-rules" },
  { id: "objective.key-exit", version: 1, provider: "starter-rules" },
  { id: "game.restart", version: 1, provider: "starter-rules" },
  { id: "environment.sky", version: 1, provider: "webgpu-renderer" },
] as const);

export type StarterPosition = [number, number, number];
export type StarterCapabilityId = (typeof STARTER_CAPABILITIES)[number]["id"];
export interface StarterGameConfig {
  schema: "aethyrforge.starter-game.v1";
  id: string;
  title: string;
  spawn: StarterPosition;
  key: { id: string; position: StarterPosition; radius: number };
  exit: { id: string; position: StarterPosition; radius: number };
  requiredCapabilities: StarterCapabilityId[];
}
export interface StarterGameSnapshot {
  schema: "aethyrforge.starter-state.v1";
  /** Exact normalized configuration, not a cryptographic content identity. */
  configIdentity: string;
  sequence: number;
  keyCollected: boolean;
  completed: boolean;
  resets: number;
}
export type StarterGameAction = { type: "interact" } | { type: "reset" };
export interface StarterGameEvent {
  schema: "aethyrforge.starter-event.v1";
  sequence: number;
  configIdentity: string;
  action: StarterGameAction;
  position: StarterPosition;
  outcome: "key-collected" | "completed" | "reset";
}
export interface StarterGameStatus extends StarterGameSnapshot {
  title: string;
  objective: string;
  nearby: "key" | "exit" | null;
  prompt: string;
  target: { id: string; position: StarterPosition } | null;
}
export type StarterGameResult =
  | {
      ok: true;
      event: StarterGameEvent;
      state: StarterGameSnapshot;
      respawn?: StarterPosition;
    }
  | {
      ok: false;
      code:
        | "invalid-action"
        | "invalid-position"
        | "too-far"
        | "locked"
        | "complete";
      message: string;
      state: StarterGameSnapshot;
    };

const MAX_COORDINATE = 20_000;
const MAX_SEQUENCE = 1_000_000_000;
const clone = <T>(value: T): T => structuredClone(value);
function record(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}
function keys(value: Record<string, unknown>, allowed: string[]) {
  return Object.keys(value).every((key) => allowed.includes(key));
}
function identifier(value: unknown): value is string {
  return (
    typeof value === "string" && /^[a-zA-Z][a-zA-Z0-9_.:-]{0,63}$/.test(value)
  );
}
function position(value: unknown): value is StarterPosition {
  return (
    Array.isArray(value) &&
    value.length === 3 &&
    value.every(
      (n) =>
        typeof n === "number" &&
        Number.isFinite(n) &&
        Math.abs(n) <= MAX_COORDINATE,
    )
  );
}
function distance(a: StarterPosition, b: StarterPosition) {
  return Math.hypot(a[0] - b[0], a[1] - b[1], a[2] - b[2]);
}
function target(value: unknown): value is StarterGameConfig["key"] {
  return (
    record(value) &&
    keys(value, ["id", "position", "radius"]) &&
    identifier(value.id) &&
    position(value.position) &&
    typeof value.radius === "number" &&
    Number.isFinite(value.radius) &&
    value.radius >= 0.25 &&
    value.radius <= 5
  );
}

/** Validate on a copy before replacing either authored settings or live progress. */
export function validateStarterGameConfig(
  candidate: unknown,
): StarterGameConfig {
  const value = clone(candidate);
  if (
    !record(value) ||
    !keys(value, [
      "schema",
      "id",
      "title",
      "spawn",
      "key",
      "exit",
      "requiredCapabilities",
    ]) ||
    value.schema !== "aethyrforge.starter-game.v1" ||
    !identifier(value.id) ||
    typeof value.title !== "string" ||
    value.title.trim().length === 0 ||
    value.title.length > 120 ||
    !position(value.spawn) ||
    !target(value.key) ||
    !target(value.exit) ||
    value.key.id === value.exit.id ||
    !Array.isArray(value.requiredCapabilities) ||
    value.requiredCapabilities.length !== STARTER_CAPABILITIES.length ||
    new Set(value.requiredCapabilities).size !==
      value.requiredCapabilities.length ||
    !STARTER_CAPABILITIES.every((capability) =>
      (value.requiredCapabilities as unknown[]).includes(capability.id),
    )
  )
    throw new Error(
      "Choose supported starter capabilities, distinct key and exit IDs, and valid positions.",
    );
  // Fixed field ordering makes equivalent inputs produce the same replay identity.
  return {
    schema: "aethyrforge.starter-game.v1",
    id: value.id,
    title: value.title.trim(),
    spawn: [...value.spawn],
    key: {
      id: value.key.id,
      position: [...value.key.position],
      radius: value.key.radius,
    },
    exit: {
      id: value.exit.id,
      position: [...value.exit.position],
      radius: value.exit.radius,
    },
    requiredCapabilities: STARTER_CAPABILITIES.map(
      (capability) => capability.id,
    ),
  };
}

/** Place a short objective near an existing controller spawn, without changing its pose. */
export function createStarterGameConfig(
  spawn: StarterPosition = [0, 0, 4],
  lookAt: StarterPosition = [0, 0, -1],
): StarterGameConfig {
  if (!position(spawn) || !position(lookAt))
    throw new Error("Choose valid starter spawn and look positions.");
  const dx = lookAt[0] - spawn[0],
    dz = lookAt[2] - spawn[2];
  const length = Math.hypot(dx, dz);
  const forward: StarterPosition =
    length > 0.001 ? [dx / length, 0, dz / length] : [0, 0, -1];
  const right: StarterPosition = [-forward[2], 0, forward[0]];
  const point = (ahead: number, side: number): StarterPosition => [
    spawn[0] + forward[0] * ahead + right[0] * side,
    spawn[1],
    spawn[2] + forward[2] * ahead + right[2] * side,
  ];
  return validateStarterGameConfig({
    schema: "aethyrforge.starter-game.v1",
    id: "town-key-and-exit",
    title: "The courtyard key",
    spawn,
    key: { id: "starter-key", position: point(3, -2), radius: 1.8 },
    exit: { id: "starter-exit", position: point(7, 2), radius: 1.8 },
    requiredCapabilities: STARTER_CAPABILITIES.map(
      (capability) => capability.id,
    ),
  });
}

/** Live progress is session state. Gameplay never mutates the authored world. */
export class StarterGame {
  private authored: StarterGameConfig;
  private live: StarterGameSnapshot;
  constructor(config: unknown = createStarterGameConfig()) {
    this.authored = validateStarterGameConfig(config);
    this.live = {
      schema: "aethyrforge.starter-state.v1",
      configIdentity: JSON.stringify(this.authored),
      sequence: 0,
      keyCollected: false,
      completed: false,
      resets: 0,
    };
  }
  config(): StarterGameConfig {
    return clone(this.authored);
  }
  snapshot(): StarterGameSnapshot {
    return clone(this.live);
  }
  status(player: StarterPosition): StarterGameStatus {
    const valid = position(player);
    const key = this.authored.key,
      exit = this.authored.exit;
    const nearKey =
      valid &&
      !this.live.keyCollected &&
      distance(player, key.position) <= key.radius;
    const nearExit = valid && distance(player, exit.position) <= exit.radius;
    const nearby = this.live.completed
      ? null
      : nearKey
        ? "key"
        : nearExit
          ? "exit"
          : null;
    return {
      ...this.snapshot(),
      title: this.authored.title,
      objective: this.live.completed
        ? "You escaped. Restart to play again."
        : this.live.keyCollected
          ? "Take the key to the green exit."
          : "Find the brass key in the courtyard.",
      nearby,
      prompt:
        nearby === "key"
          ? "Collect key"
          : nearby === "exit"
            ? this.live.keyCollected
              ? "Unlock exit"
              : "Find the key first"
            : "",
      target: this.live.completed
        ? null
        : clone(this.live.keyCollected ? exit : key),
    };
  }
  apply(action: StarterGameAction, player: StarterPosition): StarterGameResult {
    const fail = (
      code: Extract<StarterGameResult, { ok: false }>["code"],
      message: string,
    ): StarterGameResult => ({
      ok: false,
      code,
      message,
      state: this.snapshot(),
    });
    if (
      !record(action) ||
      !keys(action, ["type"]) ||
      !["interact", "reset"].includes(action.type)
    )
      return fail("invalid-action", "Choose Interact or Restart.");
    if (!position(player))
      return fail(
        "invalid-position",
        "Move to a valid position before interacting.",
      );
    if (
      this.live.sequence >= MAX_SEQUENCE ||
      (action.type === "reset" && this.live.resets >= MAX_SEQUENCE)
    )
      return fail("invalid-action", "Start a new play session to continue.");
    let outcome: StarterGameEvent["outcome"];
    if (action.type === "reset") outcome = "reset";
    else {
      if (this.live.completed)
        return fail("complete", "You escaped. Restart to play again.");
      const status = this.status(player);
      if (!status.nearby)
        return fail("too-far", "Move closer to the key or exit.");
      if (status.nearby === "exit" && !this.live.keyCollected)
        return fail("locked", "Find the key before opening the exit.");
      outcome = status.nearby === "key" ? "key-collected" : "completed";
    }
    const event: StarterGameEvent = {
      schema: "aethyrforge.starter-event.v1",
      sequence: this.live.sequence + 1,
      configIdentity: this.live.configIdentity,
      action: { type: action.type },
      position: [...player],
      outcome,
    };
    this.live = {
      ...this.live,
      sequence: event.sequence,
      keyCollected:
        outcome === "reset"
          ? false
          : outcome === "key-collected" || this.live.keyCollected,
      completed: outcome === "completed",
      resets: this.live.resets + (outcome === "reset" ? 1 : 0),
    };
    return {
      ok: true,
      event,
      state: this.snapshot(),
      ...(outcome === "reset"
        ? { respawn: [...this.authored.spawn] as StarterPosition }
        : {}),
    };
  }
  /** Compatible transform/title edits preserve progress. Replacing objective IDs needs a new session. */
  replaceConfig(candidate: unknown): StarterGameSnapshot {
    const next = validateStarterGameConfig(candidate);
    if (
      next.id !== this.authored.id ||
      next.key.id !== this.authored.key.id ||
      next.exit.id !== this.authored.exit.id
    )
      throw new Error(
        "Restart in a new play session after replacing the objective IDs.",
      );
    this.authored = next;
    this.live = { ...this.live, configIdentity: JSON.stringify(next) };
    return this.snapshot();
  }
  /** Loading progress validates every field first and never replaces authored settings. */
  restore(candidate: unknown): StarterGameSnapshot {
    const value = clone(candidate);
    if (
      !record(value) ||
      !keys(value, [
        "schema",
        "configIdentity",
        "sequence",
        "keyCollected",
        "completed",
        "resets",
      ]) ||
      value.schema !== "aethyrforge.starter-state.v1" ||
      value.configIdentity !== this.live.configIdentity ||
      typeof value.sequence !== "number" ||
      !Number.isSafeInteger(value.sequence) ||
      value.sequence < 0 ||
      value.sequence > MAX_SEQUENCE ||
      typeof value.resets !== "number" ||
      !Number.isSafeInteger(value.resets) ||
      value.resets < 0 ||
      value.resets > value.sequence ||
      typeof value.keyCollected !== "boolean" ||
      typeof value.completed !== "boolean" ||
      (value.completed && !value.keyCollected) ||
      (value.keyCollected && value.sequence < value.resets + 1) ||
      (value.completed && value.sequence < value.resets + 2)
    )
      throw new Error(
        "This starter save does not match the current game or contains invalid progress.",
      );
    this.live = value as unknown as StarterGameSnapshot;
    return this.snapshot();
  }
}

/** Replay accepted actions with exact configuration, sequence, input, and outcome checks. */
export function replayStarterGame(
  config: unknown,
  events: readonly StarterGameEvent[],
): StarterGameSnapshot {
  if (!Array.isArray(events) || events.length > 10_000)
    throw new Error("Replay at most 10,000 starter actions at a time.");
  const game = new StarterGame(config);
  for (const candidate of events) {
    const event = clone(candidate);
    if (
      !record(event) ||
      !keys(event, [
        "schema",
        "sequence",
        "configIdentity",
        "action",
        "position",
        "outcome",
      ]) ||
      event.schema !== "aethyrforge.starter-event.v1" ||
      event.sequence !== game.snapshot().sequence + 1 ||
      event.configIdentity !== game.snapshot().configIdentity
    )
      throw new Error(
        "The starter replay contains an out-of-order or incompatible action.",
      );
    const result = game.apply(
      event.action as StarterGameAction,
      event.position as StarterPosition,
    );
    if (!result.ok || result.event.outcome !== event.outcome)
      throw new Error(
        "The starter replay action does not produce its recorded outcome.",
      );
  }
  return game.snapshot();
}
