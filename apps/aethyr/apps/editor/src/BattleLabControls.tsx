import { useEffect, useRef, useState } from "react";
import { useAtomValue } from "@effect/atom-react";
import type { EditorService } from "../../../packages/editor-services/service.ts";
import { editorAtoms } from "../../../packages/editor-services/state.ts";
import {
  canEditMmorpg,
  type BattleAction,
} from "../../../packages/editor-services/protocol.ts";
import { WORLD_PROFILE } from "../../../packages/editor-services/world-profile.ts";
import { Button } from "./components/ui/button";
import { Icon } from "./icons";
import { battleAttackControl } from "./battle-ability-state";
import "./battle-lab-controls.css";

const PRESETS = [
  1_000, 10_000, 20_000, 50_000, 100_000, 125_000, 200_000, 300_000, 500_000,
  750_000, 1_000_000,
] as const;
const number = (value?: number) =>
  value === undefined ? "—" : Math.round(value).toLocaleString();

/** Snapshot-only controls. The battle and its animation never advance in React. */
export function BattleLabControls({ service }: { service: EditorService }) {
  const status = useAtomValue(editorAtoms.status);
  const receipt = useAtomValue(editorAtoms.receipt);
  const error = useAtomValue(editorAtoms.error);
  const allPending = useAtomValue(editorAtoms.pending);
  const [open, setOpen] = useState(false);
  const [count, setCount] = useState("10000");
  const [pending, setPending] = useState<string | null>(null);
  const [team, setTeam] = useState<0 | 1>(0);
  const [playerClass, setPlayerClass] = useState<"paladin" | "sorceror">(
    "paladin",
  );
  const attackTimer = useRef<ReturnType<typeof setInterval> | null>(null);
  const attackHeld = useRef(false);
  const battle = status.battle;
  const participant = battle?.participant;
  const following = battle?.camera === "first" || battle?.camera === "third";
  const enabled = battle?.enabled === true;
  const attackControl = battleAttackControl(status, !!pending);
  const countValue = Number(count);
  const validCount =
    Number.isSafeInteger(countValue) &&
    countValue >= 100 &&
    countValue <= 1_000_000;
  useEffect(() => {
    if (enabled) setOpen(true);
  }, [enabled]);
  useEffect(() => {
    if (enabled && battle?.count) setCount(String(battle.count));
  }, [enabled, battle?.count]);
  useEffect(() => {
    if (pending && (receipt?.requestId === pending || (error && !allPending)))
      setPending(null);
  }, [pending, receipt, error, allPending]);
  useEffect(() => {
    const release = () => {
      if (!attackHeld.current && attackTimer.current === null) return;
      if (attackTimer.current !== null) clearInterval(attackTimer.current);
      attackTimer.current = null;
      attackHeld.current = false;
      if (!service.registry.get(editorAtoms.status).battle?.participant?.active)
        return;
      try {
        service.battle({
          op: "input",
          forward: 0,
          turn: 0,
          sprint: false,
          attack: false,
        });
      } catch {
        /* A closed scene no longer accepts input. */
      }
    };
    window.addEventListener("blur", release);
    if (
      !enabled ||
      !following ||
      status.phase !== "ready" ||
      status.mmorpg?.connection !== "online" ||
      !participant?.active ||
      !participant.ready ||
      !participant.alive
    )
      release();
    return () => {
      window.removeEventListener("blur", release);
      release();
    };
  }, [
    service,
    enabled,
    following,
    status.phase,
    status.mmorpg?.connection,
    participant?.active,
    participant?.ready,
    participant?.alive,
    participant?.attackKind,
  ]);
  const available =
    WORLD_PROFILE.id === "open-ground" &&
    canEditMmorpg(
      import.meta.env.VITE_PUBLIC_DEMO === "1",
      location.pathname,
      WORLD_PROFILE.readOnly,
    ) &&
    status.phase === "ready" &&
    status.mmorpg?.connection === "online";
  if (!available) return null;
  const send = (action: BattleAction) => {
    if (pending) return;
    try {
      setPending(service.battle(action));
    } catch (cause) {
      service.registry.set(
        editorAtoms.error,
        cause instanceof Error
          ? cause.message
          : "Couldn't update the battle. Try again.",
      );
    }
  };
  const focusWorld = () =>
    (
      document.getElementById("preview-frame") as HTMLIFrameElement | null
    )?.contentWindow?.focus();
  const enter = (at?: [number, number]) => {
    send({
      op: "join",
      team,
      class: playerClass,
      view: "third",
      ...(at ? { at } : {}),
    });
    setOpen(false);
    focusWorld();
  };
  const chooseView = (
    view: "overview" | "orbit" | "dolly" | "first" | "third",
  ) => {
    if ((view === "first" || view === "third") && !participant?.active)
      send({ op: "join", team, class: playerClass, view });
    else send({ op: "camera", view });
    focusWorld();
  };
  const attack = (held: boolean) => {
    if (attackTimer.current !== null) {
      clearInterval(attackTimer.current);
      attackTimer.current = null;
    }
    attackHeld.current = false;
    if (
      held &&
      !battleAttackControl(service.registry.get(editorAtoms.status), !!pending)
        .enabled
    )
      return;
    attackHeld.current = held;
    const submit = (pressed: boolean) => {
      const current = service.registry.get(editorAtoms.status);
      if (!current.battle?.participant?.active) return;
      if (pressed && !battleAttackControl(current).enabled) return;
      try {
        service.battle({
          op: "input",
          forward: 0,
          turn: 0,
          sprint: false,
          attack: pressed,
          attackKind: current.battle.participant.attackKind,
        });
      } catch (cause) {
        service.registry.set(
          editorAtoms.error,
          cause instanceof Error
            ? cause.message
            : "Couldn't attack. Try again.",
        );
      }
    };
    submit(held);
    if (held && participant?.attackKind !== "meteor_shower")
      attackTimer.current = setInterval(() => submit(true), 250);
  };
  const mapRadius = Math.max(12, battle?.radius ?? 100);
  const center = battle?.center ?? [0, 0, 0];
  const marker = participant?.active
    ? [
        Math.max(
          4,
          Math.min(
            252,
            128 +
              ((participant.position[0] - center[0]) / (mapRadius * 2)) * 256,
          ),
        ),
        Math.max(
          4,
          Math.min(
            156,
            80 +
              ((participant.position[2] - center[2]) / (mapRadius * 2)) * 160,
          ),
        ),
      ]
    : null;
  const stats = battle?.stats ?? {};
  const notice = error || (receipt && !receipt.ok ? receipt.message : null);
  return (
    <>
      <section
        className={`battle-lab${open ? " battle-lab--open" : ""}`}
        aria-label="Battle Lab"
      >
        <Button
          variant="secondary"
          className="battle-lab-toggle"
          aria-expanded={open}
          onClick={() => setOpen(!open)}
        >
          <span>Battle Lab</span>
          {enabled ? (
            <span className="battle-lab-badge">{number(battle.count)}</span>
          ) : null}
          {enabled && !open ? (
            <span className="battle-lab-badge">{number(status.fps)}/s</span>
          ) : null}
          <Icon name={open ? "close" : "chevron"} size={15} />
        </Button>
        {open ? (
          <div
            className="battle-lab-panel"
            onKeyDown={(event) => {
              if (event.key === "Escape") {
                event.preventDefault();
                event.stopPropagation();
                setOpen(false);
              }
            }}
            onFocusCapture={() =>
              service.send({
                type: "input",
                input: {
                  forward: 0,
                  right: 0,
                  up: 0,
                  lookX: 0,
                  lookY: 0,
                  sprint: false,
                  jump: false,
                  attack: false,
                  dance: false,
                },
              })
            }
          >
            <div className="battle-lab-heading">
              <strong>
                {enabled
                  ? `${number(battle.count)} fighters`
                  : "Choose your battle"}
              </strong>
              <output aria-label="Battle frame submissions">
                {number(status.fps)} submitted/s
              </output>
            </div>
            <div
              className="battle-lab-presets"
              role="group"
              aria-label="Battle population presets"
            >
              {PRESETS.map((value) => (
                <Button
                  variant="secondary"
                  key={value}
                  aria-pressed={countValue === value}
                  disabled={!!pending}
                  onClick={() => setCount(String(value))}
                >
                  {value === 1_000_000 ? "1M" : `${value / 1000}k`}
                </Button>
              ))}
            </div>
            <label className="battle-lab-count">
              Fighters
              <input
                aria-label="Battle fighters"
                type="number"
                min={100}
                max={1000000}
                step={100}
                value={count}
                disabled={!!pending}
                onChange={(event) => setCount(event.target.value)}
              />
            </label>
            <div className="battle-lab-actions">
              <Button
                variant="secondary"
                disabled={!!pending || !validCount}
                onClick={() =>
                  send({ op: "start", count: countValue, seed: 1 })
                }
              >
                {pending
                  ? "Working…"
                  : enabled
                    ? "Start new battle"
                    : "Start battle"}
              </Button>
              <Button
                variant="secondary"
                disabled={!!pending || !enabled}
                onClick={() => send({ op: "reset" })}
              >
                Reset
              </Button>
              <Button
                variant="secondary"
                disabled={!!pending || !enabled}
                onClick={() => send({ op: "stop" })}
              >
                Stop
              </Button>
            </div>
            {enabled ? (
              <>
                <div className="battle-lab-entry">
                  <div className="battle-lab-selectors">
                    <label>
                      Team
                      <select
                        aria-label="Battle team"
                        value={team}
                        onChange={(event) =>
                          setTeam(Number(event.target.value) as 0 | 1)
                        }
                      >
                        <option value={0}>Blue</option>
                        <option value={1}>Red</option>
                      </select>
                    </label>
                    <label>
                      Character
                      <select
                        aria-label="Battle character"
                        value={playerClass}
                        onChange={(event) =>
                          setPlayerClass(
                            event.target.value as "paladin" | "sorceror",
                          )
                        }
                      >
                        <option value="paladin">Paladin</option>
                        <option value="sorceror">Sorceror</option>
                      </select>
                    </label>
                  </div>
                  <button
                    type="button"
                    className="battle-lab-map"
                    disabled={!!pending || !battle.center || !battle.radius}
                    aria-label="Choose a spot on the battlefield to enter"
                    onClick={(event) => {
                      if (event.detail === 0) {
                        enter();
                        return;
                      }
                      const rect = event.currentTarget.getBoundingClientRect();
                      const u = Math.max(
                        0,
                        Math.min(1, (event.clientX - rect.left) / rect.width),
                      );
                      const v = Math.max(
                        0,
                        Math.min(1, (event.clientY - rect.top) / rect.height),
                      );
                      enter([
                        center[0] + (u - 0.5) * mapRadius * 2,
                        center[2] + (v - 0.5) * mapRadius * 2,
                      ]);
                    }}
                  >
                    <svg
                      viewBox="0 0 256 160"
                      role="img"
                      aria-label="Blue and red armies, with your current position"
                    >
                      <rect width="256" height="160" fill="#203b2b" />
                      {[32, 64, 96, 128, 160, 192, 224].map((x) => (
                        <path key={x} d={`M${x} 0V160`} stroke="#ffffff0d" />
                      ))}
                      {[32, 64, 96, 128].map((y) => (
                        <path key={y} d={`M0 ${y}H256`} stroke="#ffffff0d" />
                      ))}
                      <ellipse
                        cx="65"
                        cy="80"
                        rx="52"
                        ry="66"
                        fill="#5999e134"
                        stroke="#79aef080"
                      />
                      <ellipse
                        cx="191"
                        cy="80"
                        rx="52"
                        ry="66"
                        fill="#de737334"
                        stroke="#e8919180"
                      />
                      <path
                        d="M128 12V148"
                        stroke="#e3c785b0"
                        strokeDasharray="4 5"
                      />
                      <text x="65" y="84" textAnchor="middle">
                        Blue
                      </text>
                      <text x="191" y="84" textAnchor="middle">
                        Red
                      </text>
                      {marker ? (
                        <g transform={`translate(${marker[0]},${marker[1]})`}>
                          <circle r="7" fill="#0b1510" />
                          <circle r="4" fill="#ecf6ac" />
                        </g>
                      ) : null}
                    </svg>
                    <span>Click anywhere to enter</span>
                  </button>
                  <div className="battle-lab-entry-actions">
                    <Button
                      variant="secondary"
                      disabled={!!pending}
                      onClick={() => enter()}
                    >
                      Join the frontline
                    </Button>
                    {participant?.active ? (
                      <Button
                        variant="secondary"
                        disabled={!!pending}
                        onClick={() => send({ op: "leave" })}
                      >
                        Leave battle
                      </Button>
                    ) : null}
                  </div>
                </div>
                <dl className="battle-lab-stats">
                  {[
                    ["Alive", stats.alive],
                    ["Defeated", stats.dead],
                    ["Drawn", stats.visible],
                    ["Hits", stats.hits],
                  ].map(([label, value]) => (
                    <div key={label}>
                      <dt>{label}</dt>
                      <dd>{number(value as number | undefined)}</dd>
                    </div>
                  ))}
                </dl>
                <div
                  className="battle-lab-cameras"
                  role="group"
                  aria-label="Battle camera"
                >
                  {(
                    ["overview", "orbit", "dolly", "first", "third"] as const
                  ).map((view) => (
                    <Button
                      variant="secondary"
                      key={view}
                      aria-pressed={battle.camera === view}
                      disabled={!!pending}
                      onClick={() => chooseView(view)}
                    >
                      {view === "overview"
                        ? "Overview"
                        : view === "orbit"
                          ? "Orbit"
                          : view === "dolly"
                            ? "Dolly"
                            : view === "first"
                              ? "First"
                              : "Third"}
                    </Button>
                  ))}
                </div>
                <details className="battle-lab-details">
                  <summary>Performance details</summary>
                  <dl>
                    <dt>Triangles</dt>
                    <dd>{number(stats.triangles)}</dd>
                    <dt>Simulation ticks</dt>
                    <dd>{number(stats.simulationTicks)}</dd>
                    <dt>Attacks</dt>
                    <dd>{number(stats.attacks)}</dd>
                    <dt>Ranged shots</dt>
                    <dd>{number(stats.rangedShots)}</dd>
                    <dt>Flying projectiles</dt>
                    <dd>{number(stats.projectilesActive)}</dd>
                    <dt>Projectile hits</dt>
                    <dd>{number(stats.projectileHits)}</dd>
                    <dt>Siege shots / hits</dt>
                    <dd>
                      {number(stats.siegeShots)} / {number(stats.siegeHits)}
                    </dd>
                    <dt>Meteor casts / impacts</dt>
                    <dd>
                      {number(stats.meteorCasts)} /{" "}
                      {number(stats.meteorImpacts)}
                    </dd>
                    <dt>Meteor hits</dt>
                    <dd>{number(stats.meteorHits)}</dd>
                    <dt>Active meteors</dt>
                    <dd>{number(stats.meteorsActive)}</dd>
                    <dt>Projectile overflows</dt>
                    <dd
                      className={
                        stats.projectileOverflows > 0
                          ? "battle-lab-overflow"
                          : undefined
                      }
                    >
                      {number(stats.projectileOverflows)}
                    </dd>
                  </dl>
                </details>
              </>
            ) : null}
            {notice ? (
              <p className="battle-lab-error" role="alert">
                {notice}
              </p>
            ) : battle?.message ? (
              <p className="battle-lab-message" role="status">
                {battle.message}
              </p>
            ) : null}
          </div>
        ) : null}
      </section>
      {enabled && participant?.active && following ? (
        <>
          {participant.ready && participant.alive && !attackControl.meteor ? (
            <span className="battle-reticle" aria-hidden="true" />
          ) : null}
          <section
            className="battle-participant-hud"
            aria-label="Battle participant"
          >
            <div className="battle-participant-health">
              {!participant.ready ? (
                <strong role="status">Entering battle…</strong>
              ) : (
                <>
                  <div>
                    <strong>{participant.alive ? "Health" : "Defeated"}</strong>
                    <span>
                      {number(participant.health)} /{" "}
                      {number(participant.maxHealth)}
                    </span>
                  </div>
                  <div
                    className="battle-participant-bar"
                    role="progressbar"
                    aria-label="Your battle health"
                    aria-valuemin={0}
                    aria-valuemax={participant.maxHealth}
                    aria-valuenow={participant.health}
                  >
                    <span
                      style={{
                        width: `${Math.max(0, Math.min(100, (participant.health / participant.maxHealth) * 100))}%`,
                      }}
                    />
                  </div>
                </>
              )}
            </div>
            <div className="battle-participant-actions">
              {participant.ready && !participant.alive ? (
                <Button
                  variant="secondary"
                  disabled={!!pending}
                  onClick={() => {
                    send({ op: "respawn" });
                    focusWorld();
                  }}
                >
                  Respawn
                </Button>
              ) : (
                <>
                  <Button
                    variant="secondary"
                    disabled={!participant.ready || !!pending}
                    aria-pressed={participant.attackKind === "melee"}
                    onClick={() => {
                      attack(false);
                      send({ op: "weapon", attackKind: "melee" });
                      focusWorld();
                    }}
                  >
                    <kbd>1</kbd> Sword
                  </Button>
                  <Button
                    variant="secondary"
                    disabled={!participant.ready || !!pending}
                    aria-pressed={participant.attackKind === "ranged"}
                    onClick={() => {
                      attack(false);
                      send({ op: "weapon", attackKind: "ranged" });
                      focusWorld();
                    }}
                  >
                    <kbd>2</kbd> Firebolt
                  </Button>
                  <Button
                    variant="secondary"
                    disabled={!participant.ready || !!pending}
                    aria-pressed={attackControl.meteor}
                    onClick={() => {
                      attack(false);
                      send({ op: "weapon", attackKind: "meteor_shower" });
                      focusWorld();
                    }}
                  >
                    <kbd>3</kbd> Meteor Shower
                    {attackControl.seconds !== undefined &&
                    attackControl.seconds > 0 ? (
                      <span className="battle-ability-cooldown">
                        {Math.ceil(attackControl.seconds)}s
                      </span>
                    ) : null}
                  </Button>
                  <Button
                    variant="secondary"
                    disabled={!attackControl.enabled}
                    className="battle-participant-attack"
                    onPointerDown={(event) => {
                      event.preventDefault();
                      event.currentTarget.setPointerCapture(event.pointerId);
                      attack(true);
                    }}
                    onPointerUp={() => {
                      attack(false);
                      focusWorld();
                    }}
                    onPointerCancel={() => attack(false)}
                    onLostPointerCapture={() => attack(false)}
                    onKeyDown={(event) => {
                      if (
                        (event.key === " " || event.key === "Enter") &&
                        !event.repeat
                      ) {
                        event.preventDefault();
                        attack(true);
                      }
                    }}
                    onKeyUp={(event) => {
                      if (event.key === " " || event.key === "Enter") {
                        event.preventDefault();
                        attack(false);
                      }
                    }}
                  >
                    {attackControl.label}
                  </Button>
                </>
              )}
              <Button
                variant="secondary"
                disabled={!!pending}
                onClick={() =>
                  chooseView(battle.camera === "first" ? "third" : "first")
                }
              >
                <kbd>V</kbd> {battle.camera === "first" ? "Third" : "First"}
              </Button>
              <Button
                variant="secondary"
                disabled={!!pending}
                onClick={() => send({ op: "leave" })}
              >
                Leave
              </Button>
            </div>
            {attackControl.meteor ? (
              <p className="battle-participant-spell-status" role="status">
                {attackControl.note}
              </p>
            ) : null}
            <p>
              W/S move · A/D turn · Shift run · {battle.camera === "first" ? "Mouse look" : "Mouse/right-drag orbit · Wheel zoom"} · Space{" "}
              {attackControl.meteor ? "cast" : "attack"}
            </p>
            {attackControl.meteor ? (
              <p>3 Meteor Shower · aim at ground · click/Space to cast</p>
            ) : null}
          </section>
        </>
      ) : null}
    </>
  );
}
