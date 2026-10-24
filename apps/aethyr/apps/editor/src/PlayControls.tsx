import { Button } from "./components/ui/button";
import { useAtomValue } from "@effect/atom-react";
import { useEffect } from "react";
import type { EditorService } from "../../../packages/editor-services/service";
import { editorAtoms } from "../../../packages/editor-services/state";
import { hasCharacterView } from "../../../packages/editor-services/protocol";
import { Icon } from "./icons";
import { GameControls } from "./GameControls";
import "./play-controls.css";
import { DRUID_ABILITIES } from "../../../packages/runtime/druid-abilities.ts";

export function DruidControls({ service }: { service: EditorService }) {
  const status = useAtomValue(editorAtoms.status);
  const game = status.gameplay;
  const castleRunning = !!(
    game?.castle?.active ||
    game?.preparing ||
    game?.castle?.preparing
  );
  useEffect(() => {
    if (!game?.enabled) return;
    const onKey = (event: KeyboardEvent) => {
      if (
        event.repeat ||
        event.altKey ||
        event.ctrlKey ||
        event.metaKey ||
        (event.target instanceof HTMLElement &&
          (event.target.isContentEditable ||
            /^(INPUT|TEXTAREA|SELECT)$/.test(event.target.tagName)))
      )
        return;
      const ability = DRUID_ABILITIES.find(
        (entry) =>
          event.code === `Digit${entry.key}` ||
          event.code === `Numpad${entry.key}`,
      );
      if (ability) {
        if (ability.id === "destroy_castle" && !game.castle) return;
        event.preventDefault();
        service.selectAbility(game.selected === ability.id ? null : ability.id);
      } else if (event.code === "Escape") {
        if (castleRunning) service.cancelAbility();
        if (game.selected) service.selectAbility(null);
      } else if (event.code === "Enter" && game.selected && !castleRunning) {
        event.preventDefault();
        service.castAbility();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [service, game?.enabled, game?.selected, !!game?.castle, castleRunning]);
  if (!game?.enabled) return null;
  const selected = DRUID_ABILITIES.find(
    (ability) => ability.id === game.selected,
  );
  return (
    <div className="druid-controls" role="group" aria-label="Druid abilities">
      <div className="druid-buttons">
        {DRUID_ABILITIES.filter(
          (ability) => ability.id !== "destroy_castle" || game.castle,
        ).map((ability) => (
          <Button
            variant="secondary"
            key={ability.id}
            type="button"
            aria-pressed={game.selected === ability.id}
            title={`${ability.hint} (${ability.key})`}
            disabled={status.phase !== "ready" || !!game.preparing}
            onClick={() =>
              service.selectAbility(
                game.selected === ability.id ? null : ability.id,
              )
            }
          >
            <kbd>{ability.key}</kbd>
            <span>{ability.name}</span>
          </Button>
        ))}
        {selected ? (
          <Button
            variant="secondary"
            type="button"
            className="druid-cast"
            onClick={() => service.castAbility()}
            disabled={
              status.phase !== "ready" ||
              castleRunning ||
              (selected.id !== "destroy_castle" && !game.target) ||
              (selected.id === "destroy_castle" && !!game.castle?.complete)
            }
          >
            Cast
          </Button>
        ) : null}
        {castleRunning ? (
          <Button
            variant="secondary"
            type="button"
            onClick={() => service.cancelAbility()}
          >
            {game.preparing ? "Stop preparing" : "Stop barrage"}
          </Button>
        ) : null}
        <Button
          variant="secondary"
          type="button"
          aria-label="Restore town"
          title="Restore the town, revive knights, and clear spell effects"
          disabled={
            status.phase !== "ready" ||
            (!game.enemies?.killed &&
              !game.destroyed &&
              !game.effects &&
              !game.debris &&
              !game.preparing)
          }
          onClick={() => service.resetGameplay()}
        >
          Restore town
        </Button>
      </div>
      <span className="druid-hint" role="status">
        {game.preparing
          ? "Preparing the spell… · Esc stop"
          : game.castle?.active
            ? `Castle barrage · ${game.castle.destroyed} / ${game.castle.total} pieces broken · Esc stop`
            : selected?.id === "destroy_castle"
              ? game.castle?.complete
                ? "Castle reduced to rubble · Restore town to rebuild it"
                : "Distant castle targeted · Click Cast or press Enter to begin"
              : game.casting
                ? `Casting ${DRUID_ABILITIES.find((a) => a.id === game.casting?.ability)?.name}…`
                : selected
                  ? `${selected.name} · Point at a knight, wall, or the ground, then click or press Enter · Esc cancel`
                  : `Choose a spell · 1 / 2 / 3${game.castle ? " / 4" : ""}`}
        {game.enemies?.total
          ? ` · Knights ${game.enemies.alive} · Defeated ${game.enemies.killed}`
          : ""}
        {game.destroyed ? ` · ${game.destroyed} pieces broken` : ""}
      </span>
    </div>
  );
}

/** Compact quest, spell, and view controls for a published world. */
export function PlayControls({ service }: { service: EditorService }) {
  const status = useAtomValue(editorAtoms.status);
  const mode = useAtomValue(editorAtoms.mode);
  const receipt = useAtomValue(editorAtoms.receipt);
  const character = status.character;
  const characterView =
    mode === "walk" && character && hasCharacterView(character);
  const third = character?.perspective === "third";
  return (
    <>
      <GameControls service={service} />
      {characterView ? (
        <nav className="play-controls" aria-label="Play controls">
          <Button
            variant="secondary"
            type="button"
            aria-label={
              third ? "Switch to first person" : "Switch to third person"
            }
            aria-pressed={third}
            disabled={status.phase !== "ready"}
            title="Switch first or third person (V)"
            onClick={() => service.perspective(third ? "first" : "third")}
          >
            <Icon name="camera" size={18} />
            <span>{third ? "Third person" : "First person"}</span>
            <kbd>V</kbd>
          </Button>
          <DruidControls service={service} />
          {receipt && !receipt.ok ? (
            <span className="druid-error" role="alert">
              {receipt.message}
            </span>
          ) : null}
          <span className="play-hint play-hint--keyboard">
            WASD move · Shift run · Space jump · Scroll zoom
          </span>
          <span className="play-hint play-hint--touch">
            Drag left to move · Drag right to look
          </span>
        </nav>
      ) : null}
    </>
  );
}
