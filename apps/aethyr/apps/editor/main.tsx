import { StrictMode } from "react";
import { createRoot, type Root } from "react-dom/client";
import { RegistryContext } from "@effect/atom-react";
import { EditorService } from "../../packages/editor-services/service";
import { AgentClient } from "../../packages/editor-services/agent-client.ts";
import { editorAtoms } from "../../packages/editor-services/state";
import { installOpenAgentsHost } from "../../packages/editor-services/openagents-host";
import { App } from "./src/App";
import { PlayControls } from "./src/PlayControls";
import { GroundZoneControls } from "./src/GroundZoneControls";
import "./src/theme.css";
import "./src/editor.css";
import "./src/zinc.css";
import { WORLD_PROFILE } from "../../packages/editor-services/world-profile";
import {
  editorBattleLinkPort,
  installBattleLink,
  type BattleLinkBootstrap,
} from "./src/battle-link";

declare global {
  interface Window {
    aethyrforgeEditor?: {
      service: EditorService;
      root: Root;
      battleLink?: BattleLinkBootstrap;
      snapshot: () => unknown;
    };
    aethyrforge?: {
      agent: AgentClient;
      snapshot: () => unknown;
      command: EditorService["command"];
      environment: EditorService["environment"];
      sceneEffects: EditorService["sceneEffects"];
      previewImpact: EditorService["previewImpact"];
      camera: EditorService["camera"];
      cameraPose: EditorService["cameraPose"];
      shader: EditorService["shader"];
      selectAbility: EditorService["selectAbility"];
      castAbility: EditorService["castAbility"];
      cancelAbility: EditorService["cancelAbility"];
      resetGameplay: EditorService["resetGameplay"];
      restartCombat: EditorService["restartCombat"];
      mmorpg: EditorService["mmorpg"];
      mmorpgEditing: EditorService["mmorpgEditing"];
      battle: EditorService["battle"];
      capture: EditorService["capture"];
      captureBlob: EditorService["captureBlob"];
      capturePixels: EditorService["capturePixels"];
      beginFilmCapture: EditorService["beginFilmCapture"];
      endFilmCapture: EditorService["endFilmCapture"];
      cancelCapture: EditorService["cancelCapture"];
      cTownBenchmark: EditorService["cTownBenchmark"];
      cTownReload: EditorService["cTownReload"];
      cpuCullProbe: EditorService["cpuCullProbe"];
      cancelCpuCullProbe: EditorService["cancelCpuCullProbe"];
      game: (action: "start" | "interact" | "reset") => string;
    };
  }
}
const element = document.getElementById("ui-root");
const frame = document.getElementById("preview-frame");
if (!element || !(frame instanceof HTMLIFrameElement))
  throw new Error("The editor needs its controls and preview surface.");
const publicDemo = import.meta.env.VITE_PUBLIC_DEMO === "1";
const player = location.pathname.endsWith("/player.html");
const playOnly = publicDemo || player;
element.hidden = WORLD_PROFILE.id === "haunted-house" && !playOnly;
document.title = `${WORLD_PROFILE.name} · OpenAgents`;
frame.title = WORLD_PROFILE.name;
const existing = window.aethyrforgeEditor;
const service =
  existing?.service ??
  new EditorService(
    frame,
    import.meta.env.VITE_PREVIEW_URL || "http://127.0.0.1:55305/preview.html",
  );
const root = existing?.root ?? createRoot(element);
service.start();
root.render(
  <StrictMode>
    <RegistryContext.Provider value={service.registry}>
      {WORLD_PROFILE.id === "open-ground" ? (
        <GroundZoneControls service={service} />
      ) : playOnly ? (
        <PlayControls service={service} />
      ) : (
        <App service={service} />
      )}
    </RegistryContext.Provider>
  </StrictMode>,
);
window.aethyrforgeEditor = {
  service,
  root,
  battleLink: existing?.battleLink,
  snapshot: () => ({
    status: service.registry.get(editorAtoms.status),
    environment: service.registry.get(editorAtoms.environment),
    sceneEffects: service.registry.get(editorAtoms.sceneEffects),
    receipts: service.receipts.values(),
  }),
};
window.aethyrforge = {
  agent: new AgentClient({
    discover: () => service.agentDiscover(),
    inspect: (options) => service.agentInspect(options),
    execute: (action, expect) => service.agentExecute(action, expect),
    result: (id) => service.agentResult(id),
    capture: (options) => service.agentCapture(options),
    close: () => service.cancelAgentWork(),
  }),
  snapshot: window.aethyrforgeEditor.snapshot,
  command: (command) => service.command(command),
  environment: (settings) => service.environment(settings),
  sceneEffects: (settings) => service.sceneEffects(settings),
  previewImpact: () => service.previewImpact(),
  camera: (shotId) => service.camera(shotId),
  cameraPose: (eye, target, fov, mode) =>
    service.cameraPose(eye, target, fov, mode),
  shader: (source) => service.shader(source),
  selectAbility: (ability) => service.selectAbility(ability),
  castAbility: () => service.castAbility(),
  cancelAbility: () => service.cancelAbility(),
  resetGameplay: () => service.resetGameplay(),
  restartCombat: () => service.restartCombat(),
  mmorpg: (action) => service.mmorpg(action),
  mmorpgEditing: (editing) => service.mmorpgEditing(editing),
  battle: (action) => service.battle(action),
  capture: (time) => service.capture(time),
  captureBlob: (options) => service.captureBlob(options),
  capturePixels: (options) => service.capturePixels(options),
  beginFilmCapture: (size) => service.beginFilmCapture(size),
  endFilmCapture: () => service.endFilmCapture(),
  cancelCapture: () => service.cancelCapture(),
  cTownBenchmark: (options) => service.cTownBenchmark(options),
  cTownReload: (candidate) => service.cTownReload(candidate),
  cpuCullProbe: (options) => service.cpuCullProbe(options),
  cancelCpuCullProbe: () => service.cancelCpuCullProbe(),
  game: (action) => service.send({ type: "game", action }, true),
};
await installOpenAgentsHost(service);
installBattleLink(
  window.aethyrforgeEditor,
  editorBattleLinkPort(service),
  location.search,
  () => ({
    profile: WORLD_PROFILE.id,
    publicDemo,
    pathname: location.pathname,
    readOnly: WORLD_PROFILE.readOnly,
  }),
);
if (import.meta.hot) import.meta.hot.accept();
