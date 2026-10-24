import { Button } from "./components/ui/button";
import { useAtomValue } from "@effect/atom-react";
import type { EditorService } from "../../../packages/editor-services/service";
import { editorAtoms } from "../../../packages/editor-services/state";
import "./stream-controls.css";

/** Cell controls subscribe to summaries without participating in rendering. */
export function StreamControls({ service }: { service: EditorService }) {
  const status = useAtomValue(editorAtoms.status);
  const streaming = status.streaming;
  if (status.phase !== "ready" || !streaming) return null;
  const complete = streaming.activeCell === "town";
  return (
    <section className="stream-controls" aria-label="Town loading">
      <span role="status" aria-live="polite" aria-atomic="true">
        {streaming.loading
          ? "Loading town…"
          : complete
            ? "Full town"
            : "Entry area"}
      </span>
      {streaming.loading ? (
        <Button
          variant="secondary"
          type="button"
          onClick={() =>
            service.send({ type: "stream", action: "cancel" }, true)
          }
        >
          Cancel
        </Button>
      ) : (
        <Button
          variant="secondary"
          type="button"
          onClick={() =>
            service.send(
              {
                type: "stream",
                action: "load",
                cell: complete ? "entry" : "town",
              },
              true,
            )
          }
        >
          {complete ? "Return to entry" : "Complete town"}
        </Button>
      )}
    </section>
  );
}
