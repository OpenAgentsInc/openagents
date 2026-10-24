import { Button } from "./components/ui/button";
import { useEffect } from "react";
import { useAtomValue } from "@effect/atom-react";
import type { EditorService } from "../../../packages/editor-services/service";
import { editorAtoms } from "../../../packages/editor-services/state";
import { WORLD_PROFILE } from "../../../packages/editor-services/world-profile";
import "./game-controls.css";

/** Small play-session controls shared by the editor and published player. */
export function GameControls({ service }: { service: EditorService }) {
  const status = useAtomValue(editorAtoms.status);
  const pending = useAtomValue(editorAtoms.pending);
  const supported =
    WORLD_PROFILE.id === "medieval-town-lab" ||
    WORLD_PROFILE.id === "starter-courtyard";
  const game = status.game;
  useEffect(() => {
    if (!supported || !game || game.phase === "complete") return;
    const keydown = (event: KeyboardEvent) => {
      const target = event.target;
      if (
        event.code !== "KeyE" ||
        event.repeat ||
        event.altKey ||
        event.ctrlKey ||
        event.metaKey ||
        status.phase !== "ready" ||
        (target instanceof HTMLElement &&
          (target.isContentEditable ||
            /^(INPUT|TEXTAREA|SELECT|BUTTON)$/.test(target.tagName)))
      )
        return;
      event.preventDefault();
      service.send({ type: "game", action: "interact" }, true);
    };
    window.addEventListener("keydown", keydown);
    return () => window.removeEventListener("keydown", keydown);
  }, [service, supported, Boolean(game), game?.phase, status.phase]);
  if (!supported || status.phase !== "ready") return null;
  return (
    <section className="game-controls" aria-label="Courtyard quest">
      {game ? (
        <>
          <p role="status" aria-live="polite" aria-atomic="true">
            {game.message}
          </p>
          {game.phase !== "complete" ? (
            <Button
              variant="secondary"
              type="button"
              disabled={pending > 0}
              title="Interact with the key or exit (E)"
              onClick={() =>
                service.send({ type: "game", action: "interact" }, true)
              }
            >
              Interact <kbd>E</kbd>
            </Button>
          ) : null}
          <Button
            variant="secondary"
            type="button"
            disabled={pending > 0}
            onClick={() =>
              service.send({ type: "game", action: "reset" }, true)
            }
          >
            Restart
          </Button>
        </>
      ) : (
        <Button
          variant="secondary"
          type="button"
          disabled={pending > 0}
          onClick={() => service.send({ type: "game", action: "start" }, true)}
        >
          Play quest
        </Button>
      )}
    </section>
  );
}
