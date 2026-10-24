import { useEffect, useRef, useState } from "react";
import { useAtom, useAtomValue } from "@effect/atom-react";
import type { EditorService } from "../../../packages/editor-services/service.ts";
import { editorAtoms } from "../../../packages/editor-services/state.ts";
import {
  canEditMmorpg,
  type Tool,
} from "../../../packages/editor-services/protocol.ts";
import { WORLD_PROFILE } from "../../../packages/editor-services/world-profile.ts";
import { Button } from "./components/ui/button";
import { Panel } from "./controls";
import { Icon } from "./icons";
import { mmoDraftLabel } from "./mmorpg-ui-model.ts";
import "./ground-edit-controls.css";

const EDIT_TOOLS = [
  { id: "assets", label: "Assets", icon: "assets" },
  { id: "select", label: "Select", icon: "select" },
  { id: "command", label: "Command", icon: "command" },
] as const;
const isEditTool = (tool: Tool | null) =>
  tool === "assets" || tool === "select" || tool === "command";

/** Local draft tools share the live preview without changing its player or camera. */
export function GroundEditControls({ service }: { service: EditorService }) {
  const status = useAtomValue(editorAtoms.status);
  const receipt = useAtomValue(editorAtoms.receipt);
  const error = useAtomValue(editorAtoms.error);
  const pending = useAtomValue(editorAtoms.pending);
  const assets = useAtomValue(editorAtoms.assets);
  const [tool, setTool] = useAtom(editorAtoms.tool);
  const [request, setRequest] = useState<{
    id: string;
    editing: boolean;
  } | null>(null);
  const editing = status.mmorpgEditing === true;
  const wasEditing = useRef(false);
  const online = status.mmorpg?.connection === "online";
  const available = canEditMmorpg(
    import.meta.env.VITE_PUBLIC_DEMO === "1",
    location.pathname,
    WORLD_PROFILE.readOnly,
  );
  useEffect(() => {
    const previous = wasEditing.current;
    wasEditing.current = editing;
    if (editing && !previous) setTool("assets");
    if (!editing && previous) {
      setTool(null);
      document.getElementById("preview-frame")?.focus();
    }
    if (request && request.editing === editing) setRequest(null);
  }, [editing, request, setTool]);
  useEffect(() => {
    if (request && (receipt?.requestId === request.id || (error && !pending)))
      setRequest(null);
  }, [receipt, request, error, pending]);
  const changeEditing = (next: boolean) => {
    if (request) return;
    try {
      setRequest({ id: service.mmorpgEditing(next), editing: next });
    } catch (cause) {
      service.registry.set(
        editorAtoms.error,
        cause instanceof Error
          ? cause.message
          : "Couldn't change modes. Try again.",
      );
    }
  };
  useEffect(() => {
    if (!editing || !available) return;
    const onKey = (event: KeyboardEvent) => {
      if (event.key !== "Escape") return;
      event.preventDefault();
      event.stopImmediatePropagation();
      if (tool) {
        setTool(null);
        document.getElementById("preview-frame")?.focus();
      } else changeEditing(false);
    };
    window.addEventListener("keydown", onKey, true);
    return () => window.removeEventListener("keydown", onKey, true);
  }, [available, editing, tool, request, service, setTool]);
  if (!available || !online || assets.length === 0) return null;
  if (!editing)
    return (
      <Button
        variant="secondary"
        className="ground-edit-entry"
        disabled={!!request || status.phase !== "ready"}
        onClick={() => changeEditing(true)}
      >
        <Icon name="settings" size={16} />
        {request ? "Opening editor…" : "Edit"}
      </Button>
    );
  const result = error || (receipt && !receipt.ok ? receipt.message : null);
  return (
    <section className="ground-edit" aria-label="World editor">
      <div className="ground-edit-caption">
        <strong>Edit mode</strong>
        <span>{mmoDraftLabel(status.saved)}</span>
      </div>
      {result ? (
        <p className="ground-edit-notice" role="alert">
          {result}
        </p>
      ) : receipt?.ok ? (
        <p className="ground-edit-notice" role="status">
          {receipt.message}
        </p>
      ) : null}
      {isEditTool(tool) ? (
        <Panel tool={tool!} service={service} propsOnly />
      ) : null}
      <nav className="ground-edit-hotbar" aria-label="Edit tools">
        {EDIT_TOOLS.map((item) => (
          <Button
            variant="secondary"
            key={item.id}
            aria-expanded={tool === item.id}
            className={tool === item.id ? "is-active" : undefined}
            onClick={() => setTool(tool === item.id ? null : item.id)}
          >
            <Icon name={item.icon} size={17} />
            <span>{item.label}</span>
          </Button>
        ))}
        <span className="ground-edit-divider" />
        <Button
          variant="secondary"
          aria-label="Undo edit"
          title="Undo"
          disabled={!status.canUndo || !!pending}
          onClick={() => service.command({ kind: "history.undo" })}
        >
          <Icon name="undo" size={17} />
        </Button>
        <Button
          variant="secondary"
          aria-label="Redo edit"
          title="Redo"
          disabled={!status.canRedo || !!pending}
          onClick={() => service.command({ kind: "history.redo" })}
        >
          <Icon name="redo" size={17} />
        </Button>
        <Button
          variant="secondary"
          className="ground-edit-done"
          disabled={!!request}
          onClick={() => changeEditing(false)}
        >
          <Icon name="check" size={17} />
          {request ? "Closing…" : "Done"}
        </Button>
      </nav>
      <p className="ground-edit-hint">Click a placed prop to select it.</p>
    </section>
  );
}
