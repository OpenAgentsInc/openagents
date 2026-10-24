import { useAtomValue } from "@effect/atom-react";
import type { EditorService } from "../../../packages/editor-services/service";
import { editorAtoms } from "../../../packages/editor-services/state";
import { hasCharacterView } from "../../../packages/editor-services/protocol";
import { Button } from "./components/ui/button";
import { EffectsControls } from "./EffectsControls";
import { CombatHud } from "./CombatHud";
import { MmorpgUI } from "./MmorpgUI";
import { GroundEditControls } from "./GroundEditControls";
import { BattleLabControls } from "./BattleLabControls";
import "./ground-zone-controls.css";

/** Presentation-only controls over the existing preview and input service. */
export function GroundZoneControls({ service }: { service: EditorService }) {
  const status = useAtomValue(editorAtoms.status);
  const error = useAtomValue(editorAtoms.error);
  const receipt = useAtomValue(editorAtoms.receipt);
  const ready = status.phase === "ready";
  const failed = status.phase === "error";
  const characterView = hasCharacterView(status.character);
  const quickMmorpg = status.character?.controller === "quick-mmorpg-v1";
  const third = status.character?.perspective === "third";
  const viewError = !failed && (error || (receipt && !receipt.ok));

  if (status.mmorpg)
    return (
      <>
        <div
          className="ground-mmo-ui"
          hidden={status.battle?.enabled && (status.battle.camera !== "player" || status.battle.participant?.active)}
        >
          <MmorpgUI
            service={service}
            status={status}
            receipt={receipt}
            error={error}
            editing={status.mmorpgEditing === true}
          />
        </div>
        {!status.battle?.enabled ? (
          <GroundEditControls service={service} />
        ) : null}
        <BattleLabControls service={service} />
      </>
    );

  return (
    <>
      <header className="ground-zone-title">
        <h1>Open Ground</h1>
        <span role="status" aria-live="polite">
          {failed ? "Unavailable" : ready ? "Ready" : "Opening…"}
        </span>
      </header>
      <output className="ground-zone-fps" aria-label="Frames per second">
        {ready && status.fps > 0 ? Math.round(status.fps) : "—"} FPS
      </output>
      {ready && status.combat ? (
        <CombatHud combat={status.combat} service={service} />
      ) : null}
      {import.meta.env.VITE_PUBLIC_DEMO !== "1" &&
      !location.pathname.endsWith("/player.html") ? (
        <EffectsControls service={service} disabled={!ready} />
      ) : null}
      {failed ? (
        <div className="ground-zone-error" role="alert">
          <p>Couldn't open Open Ground. Reload to try again.</p>
          <Button
            type="button"
            variant="secondary"
            onClick={() => location.reload()}
          >
            Reload
          </Button>
        </div>
      ) : (
        <nav className="ground-zone-controls" aria-label="Movement controls">
          {characterView && !quickMmorpg ? (
            <Button
              type="button"
              variant="secondary"
              aria-label={
                third ? "Switch to first person" : "Switch to third person"
              }
              aria-pressed={third}
              disabled={!ready}
              title="Switch first or third person (V)"
              onClick={() => service.perspective(third ? "first" : "third")}
            >
              {third ? "Third person" : "First person"}
              <kbd>V</kbd>
            </Button>
          ) : null}
          <p className="ground-zone-hint ground-zone-hint--keyboard">
            {quickMmorpg ? (
              <>
                <span>W/S move · A/D turn · Shift run</span>
                <span>Space attack · Backspace dance · Esc release mouse</span>
              </>
            ) : (
              <>
                <span>Click to look · WASD move · Shift run · Space jump</span>
                <span>
                  {characterView ? "V first / third person · " : ""}Scroll zoom
                  · Esc release mouse
                </span>
              </>
            )}
          </p>
          <p className="ground-zone-hint ground-zone-hint--touch">
            {quickMmorpg
              ? "Drag left to move and turn"
              : "Drag left to move · Drag right to look"}
          </p>
          {viewError ? (
            <p className="ground-zone-view-error" role="alert">
              Couldn't update the view. Try again.
            </p>
          ) : null}
        </nav>
      )}
    </>
  );
}
