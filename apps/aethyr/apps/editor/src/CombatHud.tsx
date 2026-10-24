import { useEffect, useRef } from "react";
import type { EngineStatus } from "../../../packages/editor-services/protocol";
import type { EditorService } from "../../../packages/editor-services/service";
import { Button } from "./components/ui/button";

/** Combat snapshots update this overlay; the engine owns health and attacks. */
export function CombatHud({
  combat,
  service,
}: {
  combat: NonNullable<EngineStatus["combat"]>;
  service: EditorService;
}) {
  const flash = useRef<HTMLDivElement>(null);
  const previousHealth = useRef(combat.player.health);
  const health = combat.player.health;
  const maxHealth = combat.player.maxHealth;
  const defeated = !combat.player.alive;
  const cleared = !defeated && combat.actors.length > 0 && combat.alive === 0;

  useEffect(() => {
    const damaged = health < previousHealth.current;
    previousHealth.current = health;
    if (!damaged || matchMedia("(prefers-reduced-motion: reduce)").matches)
      return;
    const animation = flash.current?.animate(
      [{ opacity: 0 }, { opacity: 0.22, offset: 0.15 }, { opacity: 0 }],
      { duration: 420, easing: "ease-out" },
    );
    return () => animation?.cancel();
  }, [health]);

  return (
    <>
      <div ref={flash} className="combat-damage-flash" aria-hidden="true" />
      <section className="combat-hud" aria-label="Combat">
        <div className="combat-health-label">
          <span>Health</span>
          <output aria-label="Player health">
            {Math.ceil(health)} <span>/ {maxHealth}</span>
          </output>
        </div>
        <div
          className="combat-health-track"
          role="progressbar"
          aria-label="Health"
          aria-valuemin={0}
          aria-valuemax={maxHealth}
          aria-valuenow={health}
          aria-valuetext={`${Math.ceil(health)} of ${maxHealth} health`}
          data-low={health <= maxHealth * 0.25}
        >
          <span style={{ width: `${(health / maxHealth) * 100}%` }} />
        </div>
        <div className="combat-counts" aria-label="Zombies">
          <span>
            <b>{combat.alive}</b> alive
          </span>
          <span>
            <b>{combat.killed}</b> defeated
          </span>
        </div>
        {defeated || cleared ? (
          <p className="combat-result" role="status">
            {defeated ? "You fell. Try again." : "Field cleared."}
          </p>
        ) : null}
        <Button
          type="button"
          variant="secondary"
          size="sm"
          onClick={() => service.restartCombat()}
        >
          Restart
        </Button>
      </section>
    </>
  );
}
