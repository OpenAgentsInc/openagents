import { useAtomValue } from "@effect/atom-react";
import type { EditorService } from "../../../packages/editor-services/service";
import { editorAtoms } from "../../../packages/editor-services/state";
import { WORLD_PROFILE } from "../../../packages/editor-services/world-profile";
import { Button } from "./components/ui/button";
import "./spell-swarm-controls.css";

/** One play-session action; the renderer owns every creature's live state. */
export function SpellSwarmControls({ service }: { service: EditorService }) {
  const status = useAtomValue(editorAtoms.status);
  const pending = useAtomValue(editorAtoms.pending);
  if (WORLD_PROFILE.id !== "medieval-town-lab" || status.phase !== "ready")
    return null;
  const count = status.spellSwarm?.count ?? 0;
  return (
    <section className="spell-swarm-controls" aria-label="Spell creatures">
      <Button
        variant="secondary"
        type="button"
        aria-pressed={count > 0}
        disabled={pending > 0}
        onClick={() => service.spellSwarm(count > 0 ? 0 : 100000)}
      >
        {count > 0 ? "Clear spell swarm" : "Summon 100,000"}
      </Button>
      <span role="status" aria-live="polite" aria-atomic="true">
        {count > 0
          ? `${count.toLocaleString()} spell creatures`
          : "Spell swarm"}
      </span>
    </section>
  );
}
