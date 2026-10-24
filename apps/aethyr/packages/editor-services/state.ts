import * as Atom from "effect/reactivity/Atom";
import { WORLD_PROFILE } from "./world-profile.ts";
import {
  DEFAULT_SCENE_EFFECTS,
  type SceneEffectsSettings,
} from "../runtime/scene-effects.ts";
import {
  DEFAULT_ENVIRONMENT,
  INITIAL_STATUS,
  type AssetSummary,
  type CameraMode,
  type EngineStatus,
  type EnvironmentSettings,
  type Tool,
} from "./protocol.ts";

export interface Receipt {
  requestId: string;
  ok: boolean;
  message: string;
  revision: number;
}
const makeEditorAtoms = () => ({
  tool: Atom.make<Tool | null>(null).pipe(Atom.keepAlive),
  mode: Atom.make<CameraMode>(WORLD_PROFILE.mode).pipe(Atom.keepAlive),
  status: Atom.make<EngineStatus>(INITIAL_STATUS).pipe(Atom.keepAlive),
  assets: Atom.make<AssetSummary[]>([]).pipe(Atom.keepAlive),
  sourceSkyAvailable: Atom.make(false).pipe(Atom.keepAlive),
  sourceSkyLightIntensity: Atom.make(1).pipe(Atom.keepAlive),
  sceneEffects: Atom.make<SceneEffectsSettings>(DEFAULT_SCENE_EFFECTS).pipe(
    Atom.keepAlive,
  ),
  environment: Atom.make<EnvironmentSettings>(DEFAULT_ENVIRONMENT).pipe(
    Atom.keepAlive,
  ),
  shot: Atom.make("MT-02").pipe(Atom.keepAlive),
  receipt: Atom.make<Receipt | null>(null).pipe(Atom.keepAlive),
  error: Atom.make<string | null>(null).pipe(Atom.keepAlive),
  shaderSource: Atom.make("").pipe(Atom.keepAlive),
  activeShader: Atom.make("").pipe(Atom.keepAlive),
  initialShader: Atom.make("").pipe(Atom.keepAlive),
  pending: Atom.make(0).pipe(Atom.keepAlive),
});
type EditorAtoms = ReturnType<typeof makeEditorAtoms>;
// The bridge outlives UI reloads and must keep writing to the same atoms.
const previousAtoms = import.meta.hot?.data.editorAtoms as
  Partial<EditorAtoms> | undefined;
export const editorAtoms: EditorAtoms = {
  ...makeEditorAtoms(),
  ...previousAtoms,
};
if (import.meta.hot) import.meta.hot.data.editorAtoms = editorAtoms;

/** Retains command outcomes independently of disposable pose summaries. */
export class ReceiptLedger {
  readonly #entries = new Map<string, Receipt>();
  readonly #limit: number;
  constructor(limit = 128) {
    this.#limit = limit;
  }
  record(receipt: Receipt): void {
    this.#entries.delete(receipt.requestId);
    this.#entries.set(receipt.requestId, receipt);
    while (this.#entries.size > this.#limit)
      this.#entries.delete(this.#entries.keys().next().value!);
  }
  get(requestId: string): Receipt | undefined {
    return this.#entries.get(requestId);
  }
  values(): Receipt[] {
    return Array.from(this.#entries.values());
  }
}
