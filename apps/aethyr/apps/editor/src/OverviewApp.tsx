import { Button } from "./components/ui/button";
import { useAtomValue } from "@effect/atom-react";
import type { EditorService } from "../../../packages/editor-services/service";
import { editorAtoms } from "../../../packages/editor-services/state";
import { WORLD_PROFILE } from "../../../packages/editor-services/world-profile";
import { Icon } from "./icons";

/** The overview offers navigation and screenshots without an editing toolbar. */
export function OverviewApp({ service }: { service: EditorService }) {
  const status = useAtomValue(editorAtoms.status);
  const error = useAtomValue(editorAtoms.error);
  const ready = status.phase === "ready";
  return (
    <>
      <header className="editor-header">
        <div className="project-badge">
          <span className="forge-mark" aria-hidden="true">
            A
          </span>
          <div>
            <span className="brand">AETHYRFORGE</span>
            <h1>{WORLD_PROFILE.name}</h1>
          </div>
        </div>
        <div className="view-controls">
          <div className="day-night" aria-label="Overview navigation">
            <Button
              variant="secondary"
              aria-label="Reset view"
              disabled={!ready}
              onClick={() => service.camera("MT-04")}
            >
              <Icon name="camera" size={17} />
              <span>Reset view</span>
            </Button>
            <Button
              variant="secondary"
              aria-label="Explore"
              disabled={!ready}
              onClick={() => service.mode("fly")}
            >
              <Icon name="fly" size={17} />
              <span>Explore</span>
            </Button>
          </div>
          <Button
            variant="secondary"
            className="capture-button"
            title="Save a screenshot"
            aria-label="Save a screenshot"
            disabled={!ready}
            onClick={() => service.capture()}
          >
            <Icon name="camera" size={18} />
          </Button>
        </div>
      </header>
      {!ready ? (
        <div
          className={`loading-card ${status.phase === "error" ? "has-error" : ""}`}
          role="status"
        >
          <span className="loading-orbit" />
          <strong>
            {status.phase === "error"
              ? "The overview could not open"
              : "Opening the overview"}
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
        ) : null}
        <div className="footer-hints">
          <span>
            Click the world to look around <b>·</b> WASD move <b>·</b> Space/C
            up/down <b>·</b> Esc release
          </span>
          {ready ? (
            <span>{status.instances.toLocaleString()} pieces</span>
          ) : null}
        </div>
      </div>
    </>
  );
}
