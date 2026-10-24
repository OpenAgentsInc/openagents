import { Button } from "./components/ui/button";
import { useEffect } from "react";
import { useAtom, useAtomValue } from "@effect/atom-react";
import type { EditorService } from "../../../packages/editor-services/service";
import { editorAtoms } from "../../../packages/editor-services/state";
import type { Tool } from "../../../packages/editor-services/protocol";
import { hasCharacterView } from "../../../packages/editor-services/protocol";
import { Panel } from "./controls";
import { Icon } from "./icons";
import { WORLD_PROFILE } from "../../../packages/editor-services/world-profile";
import { OverviewApp } from "./OverviewApp";
import { DruidControls } from "./PlayControls";
import { GameControls } from "./GameControls";
import { StreamControls } from "./StreamControls";
import { SpellSwarmControls } from "./SpellSwarmControls";
const shots = [
  ["MT-01", "Day / night comparison · reference only"],
  ["MT-02", "Town square · day"],
  ["MT-03", "Fountain courtyard"],
  ["MT-04", "Asset overview · reference only"],
  ["MT-05", "Winding street"],
  ["MT-06", "Square from the shade"],
  ["MT-07", "Blue doorway · day"],
  ["MT-08", "Lantern doorway · night"],
  ["MT-09", "Fountain detail · day"],
  ["MT-10", "Fountain detail · night"],
  ["MT-11", "Town square · night"],
  ["MT-12", "Gabled house · night"],
  ["MT-13", "Warm doorway"],
  ["MT-14", "Night courtyard"],
  ["MT-15", "Tavern square · night"],
  ["MT-16", "Café and ivy"],
  ["MT-17", "Castle skyline"],
  ["MT-18", "Townhouse and mountains"],
];
const tools: { id: Tool; icon: string; label: string; shortcut: string }[] = [
  { id: "select", icon: "select", label: "Select", shortcut: "Q" },
  { id: "assets", icon: "assets", label: "Assets", shortcut: "A" },
  { id: "build", icon: "build", label: "Build", shortcut: "B" },
  { id: "command", icon: "command", label: "Command", shortcut: "/" },
  { id: "settings", icon: "settings", label: "Lighting", shortcut: "L" },
  { id: "shader", icon: "command", label: "Shader", shortcut: "G" },
];
function editableTarget(target: EventTarget | null): boolean {
  return (
    target instanceof HTMLElement &&
    (target.isContentEditable ||
      /^(INPUT|TEXTAREA|SELECT|BUTTON)$/.test(target.tagName))
  );
}
export function App({ service }: { service: EditorService }) {
  return WORLD_PROFILE.readOnly ? (
    <OverviewApp service={service} />
  ) : (
    <TownEditor service={service} />
  );
}
function TownEditor({ service }: { service: EditorService }) {
  const [tool, setTool] = useAtom(editorAtoms.tool);
  const status = useAtomValue(editorAtoms.status);
  const env = useAtomValue(editorAtoms.environment);
  const shot = useAtomValue(editorAtoms.shot);
  const mode = useAtomValue(editorAtoms.mode);
  const perspective = status.character?.perspective ?? "third";
  const characterView = hasCharacterView(status.character);
  const error = useAtomValue(editorAtoms.error);
  const receipt = useAtomValue(editorAtoms.receipt);
  const pending = useAtomValue(editorAtoms.pending);
  useEffect(() => {
    const keydown = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        setTool(null);
        return;
      }
      if (
        editableTarget(event.target) ||
        event.altKey ||
        event.ctrlKey ||
        event.metaKey
      )
        return;
      if (
        event.code === "KeyV" &&
        !event.repeat &&
        mode === "walk" &&
        characterView
      ) {
        event.preventDefault();
        service.perspective(perspective === "third" ? "first" : "third");
        setTool(null);
        return;
      }
      const target = tools.find(
        (item) => item.shortcut.toLowerCase() === event.key.toLowerCase(),
      );
      if (target) {
        event.preventDefault();
        setTool(target.id);
      }
    };
    window.addEventListener("keydown", keydown);
    return () => window.removeEventListener("keydown", keydown);
  }, [setTool, service, mode, perspective, characterView]);
  return (
    <>
      <header className="editor-header">
        <div className="project-badge">
          <span className="forge-mark" aria-hidden="true">
            A
          </span>
          <div>
            <span className="brand">AETHYRFORGE</span>
            <h1>
              {WORLD_PROFILE.id === "starter-courtyard"
                ? "Starter Courtyard"
                : "Medieval Town Lab"}
            </h1>
          </div>
          <span
            className={`save-indicator ${status.saved ? "is-saved" : ""}`}
            title={
              status.saved
                ? "Saved on this device"
                : status.phase === "ready"
                  ? "Local preview"
                  : "Opening world"
            }
            aria-label={status.saved ? "Saved on this device" : "Local preview"}
          />
        </div>
        <div className="view-controls">
          <div className="day-night" aria-label="Time of day">
            <Button
              variant="secondary"
              aria-label="Day"
              aria-pressed={env.mode === "day"}
              className={env.mode === "day" ? "active" : ""}
              onClick={() => service.lightingMode("day")}
            >
              <Icon name="sun" size={17} />
              <span>Day</span>
            </Button>
            <Button
              variant="secondary"
              aria-label="Night"
              aria-pressed={env.mode === "night"}
              className={env.mode === "night" ? "active" : ""}
              onClick={() => service.lightingMode("night")}
            >
              <Icon name="moon" size={17} />
              <span>Night</span>
            </Button>
          </div>
          {WORLD_PROFILE.id !== "starter-courtyard" ? (
            <label className="shot-picker">
              <Icon name="camera" size={17} />
              <span className="sr-only">Camera composition</span>
              <select
                value={shot}
                onChange={(e) => service.camera(e.target.value)}
              >
                {shots.map(([id, label]) => (
                  <option
                    value={id}
                    key={id}
                    disabled={id === "MT-01" || id === "MT-04"}
                  >
                    {id} · {label}
                  </option>
                ))}
              </select>
              <Icon name="chevron" size={14} />
            </label>
          ) : null}
          <Button
            variant="secondary"
            className="capture-button"
            title="Save a screenshot"
            aria-label="Save a screenshot"
            disabled={status.phase !== "ready"}
            onClick={() => service.capture()}
          >
            <Icon name="camera" size={18} />
          </Button>
        </div>
      </header>
      <GameControls service={service} />
      <StreamControls service={service} />
      <SpellSwarmControls service={service} />
      {status.phase !== "ready" ? (
        <div
          className={`loading-card ${status.phase === "error" ? "has-error" : ""}`}
          role="status"
        >
          <span className="loading-orbit" />
          <strong>
            {status.phase === "error"
              ? "The world could not open"
              : "Opening the town"}
          </strong>
          <span>{status.message}</span>
          {status.phase === "error" ? (
            <Button
              variant="secondary"
              className="secondary-button"
              onClick={() => location.reload()}
            >
              Try again
            </Button>
          ) : null}
        </div>
      ) : null}
      <div
        className="world-reticle"
        aria-hidden="true"
        hidden={!!status.gameplay?.selected}
      >
        <i />
        <i />
      </div>
      {status.selection ? (
        <div className="selection-label">
          <span>{status.selection.name}</span>
          <Button
            variant="secondary"
            aria-label="Inspect selected object"
            onClick={() => setTool("select")}
          >
            <Icon name="settings" size={14} />
          </Button>
        </div>
      ) : null}
      <div className="bottom-ui">
        {error ? (
          <div className="notice notice--error" role="alert">
            <span>{error}</span>
            <Button
              variant="secondary"
              aria-label="Dismiss error"
              onClick={() => service.registry.set(editorAtoms.error, null)}
            >
              <Icon name="close" size={16} />
            </Button>
          </div>
        ) : receipt ? (
          <div
            className={`notice ${receipt.ok ? "" : "notice--error"}`}
            role="status"
          >
            <Icon name={receipt.ok ? "check" : "close"} size={16} />
            <span>{receipt.message}</span>
            <Button
              variant="secondary"
              aria-label="Dismiss result"
              onClick={() => service.registry.set(editorAtoms.receipt, null)}
            >
              <Icon name="close" size={16} />
            </Button>
          </div>
        ) : null}
        {tool ? <Panel tool={tool} service={service} /> : null}
        <DruidControls service={service} />
        <nav className="hotbar" aria-label="World tools">
          <Button
            variant="secondary"
            className={`walk-button ${mode === "walk" ? "is-walking" : ""}`}
            disabled={status.phase !== "ready"}
            title={
              mode === "walk"
                ? "Switch to fly navigation"
                : "Switch to walk navigation"
            }
            onClick={() => {
              service.mode(mode === "walk" ? "fly" : "walk");
              setTool(null);
            }}
          >
            <Icon name={mode === "walk" ? "walk" : "fly"} />
            <span>{mode === "walk" ? "Walk" : "Fly"}</span>
          </Button>
          {characterView ? (
            <Button
              variant="secondary"
              className="walk-button"
              title="Switch first or third person (V)"
              aria-label={
                perspective === "third"
                  ? "Switch to first person"
                  : "Switch to third person"
              }
              aria-pressed={perspective === "third"}
              disabled={status.phase !== "ready" || mode !== "walk"}
              onClick={() => {
                service.perspective(
                  perspective === "third" ? "first" : "third",
                );
                setTool(null);
              }}
            >
              <Icon name="camera" />
              <span>
                {perspective === "third" ? "Third person" : "First person"}
              </span>
              <kbd>V</kbd>
            </Button>
          ) : null}
          <span className="toolbar-divider" />
          {tools.map((item) => (
            <Button
              variant="secondary"
              key={item.id}
              className={`tool-button ${tool === item.id ? "is-active" : ""}`}
              aria-expanded={tool === item.id}
              title={`${item.label} (${item.shortcut})`}
              onClick={() => setTool(tool === item.id ? null : item.id)}
            >
              <Icon name={item.icon} />
              <span>{item.label}</span>
              <kbd>{item.shortcut}</kbd>
            </Button>
          ))}
          <span className="toolbar-divider" />
          <Button
            variant="secondary"
            className="history-button"
            title="Undo"
            aria-label="Undo"
            disabled={!status.canUndo}
            onClick={() => service.command({ kind: "history.undo" })}
          >
            <Icon name="undo" size={18} />
          </Button>
          <Button
            variant="secondary"
            className="history-button"
            title="Redo"
            aria-label="Redo"
            disabled={!status.canRedo}
            onClick={() => service.command({ kind: "history.redo" })}
          >
            <Icon name="redo" size={18} />
          </Button>
        </nav>
        <div className="footer-hints">
          <span>
            Click the world to look around <b>·</b> WASD move <b>·</b>{" "}
            {mode === "fly"
              ? "Space/C up/down"
              : "Shift run · Space jump · V view · Scroll zoom"}{" "}
            <b>·</b> Esc release
          </span>
          <details className="performance-details">
            <summary className="engine-status" title="Open performance details">
              <span className="sr-only">
                Frame submissions, not measured display frames.{" "}
              </span>
              {pending
                ? `${pending} change${pending > 1 ? "s" : ""}…`
                : status.phase === "ready"
                  ? `${Math.round(status.fps)} submitted/s`
                  : "Loading"}
              <i />
              {status.instances.toLocaleString()} objects
            </summary>
            <dl className="performance-readout">
              <dt>Frame attempts</dt>
              <dd>
                {(status.runtimeMetrics?.frameAttempts ?? 0).toLocaleString()}
              </dd>
              <dt>Sent to graphics</dt>
              <dd>
                {(status.runtimeMetrics?.submittedFrames ?? 0).toLocaleString()}
              </dd>
              <dt>Skipped while busy</dt>
              <dd>
                {(status.runtimeMetrics?.skippedFrames ?? 0).toLocaleString()}
              </dd>
              <dt>Simulation time dropped</dt>
              <dd>
                {(
                  (status.runtimeMetrics?.kernelDroppedSeconds ?? 0) * 1000
                ).toFixed(1)}{" "}
                ms
              </dd>
              <dt>Long frame time capped</dt>
              <dd>
                {(
                  (status.runtimeMetrics?.hostClampedSeconds ?? 0) * 1000
                ).toFixed(1)}{" "}
                ms
              </dd>
              <dt>Graphics memory counted</dt>
              <dd>
                {(
                  (status.runtimeMetrics?.gpuAccountedBytes ?? 0) /
                  (1024 * 1024)
                ).toFixed(1)}{" "}
                MiB
              </dd>
              <dt className="sr-only">Scope</dt>
              <dd className="performance-note">
                These counts do not measure screen display rate or all browser
                memory.
              </dd>
            </dl>
          </details>
        </div>
      </div>
    </>
  );
}
