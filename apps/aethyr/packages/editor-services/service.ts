import {
  validMmoUiAction,
  type MmoUiAction,
} from "../runtime/mmorpg-protocol.ts";
import { CaptureRequests } from "./capture-requests.ts";
import {
  sameSceneEffects,
  validateSceneEffects,
  type SceneEffectsSettings,
} from "../runtime/scene-effects.ts";
import {
  validateAgentAction,
  validateAgentExpectation,
  validateAgentCaptureRequest,
  validateAgentInspectOptions,
  validSpellSwarmCount,
  type AgentAction,
  type AgentExpectation,
  type AgentInspectOptions,
  type AgentInspection,
  type AgentDiscovery,
  type AgentCaptureRequest,
  type AgentCaptureEnvelope,
  type AgentReceipt,
  type SpellSwarmCount,
} from "../runtime/agent-contract.ts";
import { C_TOWN_BENCHMARK } from "../../experiments/c-town/config.ts";
import {
  validateCTownRequest,
  type CTownBenchmarkRequest,
  type CTownBenchmarkResult,
} from "../../experiments/c-town/metrics.ts";
import {
  validateFrameCaptureOptions,
  validateFramePixelOptions,
  validateFilmCaptureSize,
  type FrameCaptureOptions,
  type FramePixelOptions,
  type FilmCaptureSize,
} from "../renderer/film-frame.ts";
import type {
  MaterialViewRequest,
  MaterialViewResult,
} from "./creator-material.ts";
import { checkAuthorFrame } from "./host-budgets.ts";
import type { AuthorAction } from "./authoring.ts";
import type { ProjectDocument } from "./project-document.ts";
import { WORLD_PROFILE, profileSupportsShot } from "./world-profile";
import {
  isDruidAbility,
  type DruidAbility,
} from "../runtime/druid-abilities.ts";
import * as Effect from "effect/Effect";
import * as Fiber from "effect/Fiber";
import * as AtomRegistry from "effect/reactivity/AtomRegistry";
import { editorAtoms, ReceiptLedger } from "./state";
import {
  PREVIEW_PROTOCOL,
  isPreviewEvent,
  canEditMmorpg,
  validBattleAction,
  type BattleAction,
  validateEnvironment,
  validCharacterPerspective,
  validOrbitDistance,
  type CameraMode,
  type CharacterPerspective,
  type EditorCommand,
  type EnvironmentSettings,
  type PreviewMessage,
  type PreviewRequest,
} from "./protocol";

export class EditorService {
  private acceptSceneEffects(settings?: SceneEffectsSettings) {
    if (
      settings &&
      !sameSceneEffects(this.registry.get(editorAtoms.sceneEffects), settings)
    )
      this.registry.set(
        editorAtoms.sceneEffects,
        validateSceneEffects(settings),
      );
  }
  readonly #agentPending = new Map<
    string,
    {
      resolve: (value: any) => void;
      reject: (error: Error) => void;
      timer: ReturnType<typeof setTimeout>;
      receipt: boolean;
      promise: Promise<any>;
    }
  >();
  readonly #agentReceipts = new Map<string, AgentReceipt>();
  private failAgentRequests(error: Error) {
    for (const held of this.#agentPending.values()) {
      clearTimeout(held.timer);
      held.reject(error);
    }
    this.#agentPending.clear();
  }
  private holdAgentRequest<T>(id: string, receipt = false): Promise<T> {
    const existing = this.#agentPending.get(id);
    if (existing) return existing.promise;
    if (this.#closed || this.#agentPending.size >= 16)
      return Promise.reject(
        new Error("Let the current agent requests finish first."),
      );
    let resolve!: (value: T) => void, reject!: (error: Error) => void;
    const promise = new Promise<T>((yes, no) => {
      resolve = yes;
      reject = no;
    });
    const timer = setTimeout(() => {
      this.#agentPending.delete(id);
      reject(
        new Error(
          "This agent request took too long. Inspect its result before retrying.",
        ),
      );
    }, 15000);
    this.#agentPending.set(id, { resolve, reject, timer, receipt, promise });
    return promise;
  }
  readonly #benchmarks = new Map<
    string,
    {
      resolve: (value: any) => void;
      reject: (error: Error) => void;
      timer: ReturnType<typeof setTimeout>;
    }
  >();
  readonly registry = AtomRegistry.make();
  readonly receipts = new ReceiptLedger();
  readonly sessionId = crypto.randomUUID();
  readonly previewOrigin: string;
  readonly #frame: HTMLIFrameElement;
  readonly #mmorpgChatListeners = new Set<() => void>();
  readonly #materialViews = new Map<
    string,
    {
      resolve: (value: MaterialViewResult) => void;
      reject: (error: Error) => void;
      timer: ReturnType<typeof setTimeout>;
    }
  >();
  readonly #pending = new Map<string, ReturnType<typeof setTimeout>>();
  #fiber: Fiber.Fiber<never, never> | undefined;
  #started = false;
  #closed = false;
  #timedOutRequest: string | undefined;
  readonly #shaderRequests = new Map<string, string>();
  readonly #sceneEffectsRequests = new Map<string, SceneEffectsSettings>();
  readonly #projects = new Map<
    string,
    {
      resolve: (document: ProjectDocument) => void;
      reject: (error: Error) => void;
      timer: ReturnType<typeof setTimeout>;
    }
  >();
  readonly #captures = new CaptureRequests((id) => this.settle(id));
  readonly #filmRequests = new Map<
    string,
    {
      resolve: () => void;
      reject: (error: Error) => void;
      timer: ReturnType<typeof setTimeout>;
    }
  >();

  constructor(frame: HTMLIFrameElement, previewUrl: string) {
    this.#frame = frame;
    this.previewOrigin = new URL(previewUrl).origin;
    const url = new URL(previewUrl);
    url.hash = new URLSearchParams({
      session: this.sessionId,
      editorOrigin: window.location.origin,
    }).toString();
    frame.src = url.href;
    if (WORLD_PROFILE.readOnly) {
      this.registry.set(editorAtoms.shot, "MT-04");
      this.registry.set(editorAtoms.mode, "fly");
    }
  }

  start(): void {
    if (this.#started) return;
    this.#started = true;
    const receive = (event: MessageEvent) => {
      if (this.#closed) return;
      if (
        event.origin !== this.previewOrigin ||
        event.source !== this.#frame.contentWindow ||
        !isPreviewEvent(event.data, this.sessionId)
      )
        return;
      const message = event.data;
      if (message.type === "mmorpgFocusChat") {
        if (
          this.registry.get(editorAtoms.status).mmorpg?.connection !== "online"
        )
          return;
        this.send({
          type: "input",
          input: {
            forward: 0,
            right: 0,
            up: 0,
            lookX: 0,
            lookY: 0,
            sprint: false,
            jump: false,
            attack: false,
            dance: false,
          },
        });
        for (const listener of this.#mmorpgChatListeners) listener();
        return;
      }
      if (
        message.type === "agentDiscovery" ||
        message.type === "agentInspection" ||
        message.type === "agentCapture"
      ) {
        const held = this.#agentPending.get(message.requestId!);
        if (held) {
          clearTimeout(held.timer);
          this.#agentPending.delete(message.requestId!);
          held.resolve(message.result);
        }
        return;
      }
      if (
        message.type === "cTownBenchmarkResult" ||
        message.type === "cTownReloadResult" ||
        message.type === "cpuCullProbeResult"
      ) {
        const held = this.#benchmarks.get(message.requestId!);
        if (held) {
          clearTimeout(held.timer);
          this.#benchmarks.delete(message.requestId!);
          held.resolve(message.result);
        }
        return;
      } else if (message.type === "ready") {
        this.failAgentRequests(
          new Error("The world reopened during the agent request."),
        );
        for (const held of this.#benchmarks.values()) {
          clearTimeout(held.timer);
          held.reject(new Error("The world reopened during measurement."));
        }
        this.#benchmarks.clear();
        this.#captures.cancel(new Error("The world reopened during capture."));
        for (const held of this.#filmRequests.values()) {
          clearTimeout(held.timer);
          held.reject(new Error("The world reopened during capture."));
        }
        this.#filmRequests.clear();
        this.registry.set(editorAtoms.assets, message.assets.slice(0, 1024));
        this.registry.set(
          editorAtoms.sourceSkyAvailable,
          message.hasSourceSky ?? false,
        );
        this.registry.set(editorAtoms.status, message.status);
        this.acceptSceneEffects(message.status.sceneEffects);
        if (message.status.view)
          this.registry.set(editorAtoms.mode, message.status.view.mode);
        if (message.sourceSkyLightIntensity !== undefined)
          this.registry.set(
            editorAtoms.sourceSkyLightIntensity,
            message.sourceSkyLightIntensity,
          );
        this.registry.set(editorAtoms.error, null);
        if (message.shaderSource !== undefined) {
          this.registry.set(editorAtoms.activeShader, message.shaderSource);
          if (!this.registry.get(editorAtoms.initialShader))
            this.registry.set(editorAtoms.initialShader, message.shaderSource);
          if (!this.registry.get(editorAtoms.shaderSource))
            this.registry.set(editorAtoms.shaderSource, message.shaderSource);
        }
      } else if (message.type === "status") {
        this.registry.set(editorAtoms.status, message.status);
        this.acceptSceneEffects(message.status.sceneEffects);
        if (message.status.view)
          this.registry.set(editorAtoms.mode, message.status.view.mode);
      } else if (message.type === "environment") {
        this.registry.set(
          editorAtoms.environment,
          validateEnvironment(message.settings),
        );
        if (message.sourceSkyLightIntensity !== undefined)
          this.registry.set(
            editorAtoms.sourceSkyLightIntensity,
            message.sourceSkyLightIntensity,
          );
      } else if (message.type === "receipt") {
        const requestId = message.requestId ?? "unknown";
        const receipt = {
          requestId,
          ok: message.ok,
          message: message.message,
          revision: message.revision,
        };
        const shaderSource = this.#shaderRequests.get(requestId);
        const effects = this.#sceneEffectsRequests.get(requestId);
        if (effects) {
          if (message.ok) this.acceptSceneEffects(effects);
          this.#sceneEffectsRequests.delete(requestId);
        }
        if (shaderSource !== undefined) {
          if (message.ok)
            this.registry.set(editorAtoms.activeShader, shaderSource);
          this.#shaderRequests.delete(requestId);
        }
        const film = this.#filmRequests.get(requestId);
        if (film) {
          clearTimeout(film.timer);
          this.#filmRequests.delete(requestId);
          if (message.ok) film.resolve();
          else film.reject(new Error(message.message));
        }
        if (!message.ok)
          this.#captures.fail(requestId, new Error(message.message));
        this.receipts.record(receipt);
        const agent = message.agent ?? {
          ...receipt,
          stage: message.ok ? ("activated" as const) : ("rejected" as const),
        };
        const agentWait = this.#agentPending.get(requestId);
        if (message.agent || agentWait?.receipt) {
          this.#agentReceipts.set(requestId, agent);
          while (this.#agentReceipts.size > 128)
            this.#agentReceipts.delete(
              this.#agentReceipts.keys().next().value!,
            );
        }
        if (agentWait) {
          clearTimeout(agentWait.timer);
          this.#agentPending.delete(requestId);
          if (agentWait.receipt) agentWait.resolve(agent);
          else agentWait.reject(new Error(message.message));
        }
        this.registry.set(editorAtoms.receipt, receipt);
        this.settle(requestId);
        if (message.ok && this.#timedOutRequest === requestId) {
          this.#timedOutRequest = undefined;
          this.registry.set(editorAtoms.error, null);
        }
        if (!message.ok) {
          const benchmark = this.#benchmarks.get(requestId);
          if (benchmark) {
            clearTimeout(benchmark.timer);
            this.#benchmarks.delete(requestId);
            benchmark.reject(new Error(message.message));
          }
          this.registry.set(editorAtoms.error, message.message);
          const material = this.#materialViews.get(requestId);
          if (material) {
            clearTimeout(material.timer);
            this.#materialViews.delete(requestId);
            material.reject(new Error(message.message));
          }
        }
      } else if (message.type === "error") {
        if (!message.requestId)
          this.failAgentRequests(new Error(message.message));
        else {
          const held = this.#agentPending.get(message.requestId);
          if (held) {
            clearTimeout(held.timer);
            this.#agentPending.delete(message.requestId);
            held.reject(new Error(message.message));
          }
        }
        if (!message.requestId) {
          for (const held of this.#benchmarks.values()) {
            clearTimeout(held.timer);
            held.reject(new Error(message.message));
          }
          this.#benchmarks.clear();
          this.#captures.cancel(new Error(message.message));
          for (const held of this.#filmRequests.values()) {
            clearTimeout(held.timer);
            held.reject(new Error(message.message));
          }
          this.#filmRequests.clear();
        }
        this.registry.set(editorAtoms.error, message.message);
        if (!message.requestId)
          this.registry.set(editorAtoms.status, {
            ...this.registry.get(editorAtoms.status),
            phase: "error",
            message: message.message,
          });
        if (message.requestId) {
          const benchmark = this.#benchmarks.get(message.requestId);
          if (benchmark) {
            clearTimeout(benchmark.timer);
            this.#benchmarks.delete(message.requestId);
            benchmark.reject(new Error(message.message));
          }
          this.#captures.fail(message.requestId, new Error(message.message));
          const film = this.#filmRequests.get(message.requestId);
          if (film) {
            clearTimeout(film.timer);
            this.#filmRequests.delete(message.requestId);
            film.reject(new Error(message.message));
          }
          this.settle(message.requestId);
          this.#shaderRequests.delete(message.requestId);
          this.#sceneEffectsRequests.delete(message.requestId);
        }
      } else if (message.type === "materialView") {
        const requested = this.#materialViews.get(message.requestId!);
        if (requested) {
          clearTimeout(requested.timer);
          this.#materialViews.delete(message.requestId!);
          requested.resolve(message.result);
        }
      } else if (message.type === "projectDocument") {
        const requested = this.#projects.get(message.requestId!);
        if (requested) {
          clearTimeout(requested.timer);
          this.#projects.delete(message.requestId!);
          requested.resolve(message.document);
        }
      } else if (message.type === "capturePixels") {
        this.#captures.completePixels(message.requestId, message.pixels);
        if (message.requestId) this.settle(message.requestId);
        return;
      } else if (message.type === "capture") {
        const intent = this.#captures.complete(message.requestId, message.blob);
        if (message.requestId) this.settle(message.requestId);
        if (intent !== "download") return;
        const href = URL.createObjectURL(message.blob);
        const a = document.createElement("a");
        a.href = href;
        a.download = `aethyrforge-${this.registry.get(editorAtoms.shot)}.png`;
        a.click();
        setTimeout(() => URL.revokeObjectURL(href), 5000);
      }
    };
    const initialize = () =>
      this.send({
        type: "initialize",
        player: location.pathname.endsWith("/player.html"),
        animationTime: this.registry.get(editorAtoms.status).view
          ?.animationTime,
      });
    const resetInput = () =>
      this.send({
        type: "input",
        input: {
          forward: 0,
          right: 0,
          up: 0,
          lookX: 0,
          lookY: 0,
          sprint: false,
          jump: false,
        },
      });
    const lifetime = Effect.acquireRelease(
      Effect.sync(() => {
        window.addEventListener("message", receive);
        window.addEventListener("blur", resetInput);
        this.#frame.addEventListener("load", initialize);
        initialize();
      }),
      () =>
        Effect.sync(() => {
          window.removeEventListener("message", receive);
          window.removeEventListener("blur", resetInput);
          this.#frame.removeEventListener("load", initialize);
        }),
    ).pipe(
      Effect.flatMap(() => Effect.never),
      Effect.scoped,
    );
    this.#fiber = Effect.runFork(lifetime);
  }

  send(request: PreviewRequest, track = false): string {
    const requestId = crypto.randomUUID();
    if (this.#closed) return requestId;
    if (request.type === "game") this.registry.set(editorAtoms.error, null);
    const message = {
      ...request,
      protocol: PREVIEW_PROTOCOL,
      sessionId: this.sessionId,
      requestId,
    } as PreviewMessage;
    if (request.type === "author" || request.type === "loadPlayable")
      checkAuthorFrame(request.action, message);
    if (request.type === "materialView") checkAuthorFrame(request, message);
    if (track) {
      if (this.#pending.size >= 16) {
        this.registry.set(
          editorAtoms.error,
          "Let the current changes finish before adding another.",
        );
        return requestId;
      }
      const timeout = setTimeout(() => {
        this.settle(requestId);
        this.#timedOutRequest = requestId;
        this.registry.set(
          editorAtoms.error,
          "This change is taking longer than expected. Check the world before retrying.",
        );
      }, 15000);
      this.#pending.set(requestId, timeout);
      this.registry.set(editorAtoms.pending, this.#pending.size);
    }
    this.#frame.contentWindow?.postMessage(message, this.previewOrigin);
    return requestId;
  }
  private requireEditing(): void {
    if (WORLD_PROFILE.readOnly)
      throw new Error(
        "The overview is for exploring. Open Medieval Town to edit.",
      );
  }
  author(action: AuthorAction, base: number, commandId: string): string {
    this.requireEditing();
    return this.send({ type: "author", action, base, commandId }, true);
  }
  command(command: EditorCommand): string {
    this.requireEditing();
    return Effect.runSync(
      Effect.sync(() => {
        this.registry.set(editorAtoms.error, null);
        return this.send({ type: "command", command }, true);
      }),
    );
  }
  environment(settings: EnvironmentSettings): void {
    this.applyEnvironment(settings, false);
  }
  lightingMode(mode: EnvironmentSettings["mode"]): void {
    this.applyEnvironment(
      { ...this.registry.get(editorAtoms.environment), mode },
      true,
    );
  }
  skySource(skySource: EnvironmentSettings["skySource"]): void {
    this.applyEnvironment(
      { ...this.registry.get(editorAtoms.environment), skySource },
      true,
    );
  }
  private applyEnvironment(
    settings: EnvironmentSettings,
    restoreSourceFill: boolean,
  ): void {
    this.requireEditing();
    try {
      Effect.runSync(
        Effect.sync(() => {
          const checked = validateEnvironment(settings);
          if (
            checked.skySource === "kit" &&
            !this.registry.get(editorAtoms.sourceSkyAvailable)
          )
            throw new Error("The kit sky is not available in this world.");
          this.registry.set(editorAtoms.environment, checked);
          this.send({
            type: "setEnvironment",
            settings: checked,
            restoreSourceFill,
          });
        }),
      );
    } catch (cause) {
      this.registry.set(
        editorAtoms.error,
        cause instanceof Error
          ? cause.message
          : "These settings could not be applied.",
      );
    }
  }
  camera(shotId: string): string | undefined {
    if (!profileSupportsShot(WORLD_PROFILE, shotId)) return;
    this.registry.set(editorAtoms.shot, shotId);
    this.registry.set(editorAtoms.mode, "fly");
    this.registry.set(editorAtoms.environment, {
      ...this.registry.get(editorAtoms.environment),
      mode: [
        "MT-08",
        "MT-10",
        "MT-11",
        "MT-12",
        "MT-13",
        "MT-14",
        "MT-15",
      ].includes(shotId)
        ? "night"
        : "day",
    });
    return this.send({ type: "setCamera", shotId });
  }
  /** Explicit inspection camera; vertical field of view uses radians. */
  cameraPose(
    eye: [number, number, number],
    target: [number, number, number],
    fov?: number,
    mode: CameraMode = "fly",
  ): string | undefined {
    if (WORLD_PROFILE.readOnly && mode !== "fly") this.requireEditing();
    if (
      ![...eye, ...target].every(
        (value) => Number.isFinite(value) && Math.abs(value) <= 1_000_000,
      ) ||
      Math.hypot(...eye.map((value, index) => value - target[index])) <
        0.0001 ||
      (fov !== undefined && (!Number.isFinite(fov) || fov < 0.1 || fov > 3)) ||
      (mode !== "fly" && mode !== "walk")
    ) {
      this.registry.set(
        editorAtoms.error,
        "Choose a valid camera position, target, and field of view.",
      );
      return;
    }
    this.registry.set(editorAtoms.mode, mode);
    return this.send({ type: "setCameraPose", eye, target, fov, mode });
  }
  mode(mode: CameraMode): void {
    if (WORLD_PROFILE.readOnly && mode !== "fly") this.requireEditing();
    this.registry.set(editorAtoms.mode, mode);
    this.send({ type: "mode", mode });
    this.#frame.focus();
  }
  perspective(perspective: CharacterPerspective): string | undefined {
    this.requireEditing();
    if (!validCharacterPerspective(perspective)) {
      this.registry.set(editorAtoms.error, "Choose first or third person.");
      return;
    }
    const requestId = this.send({ type: "perspective", perspective });
    this.#frame.focus();
    return requestId;
  }
  selectAbility(ability: DruidAbility | null): string | undefined {
    if (ability !== null && !isDruidAbility(ability)) return;
    const requestId = this.send({ type: "selectAbility", ability });
    this.#frame.focus();
    return requestId;
  }
  castAbility(): string {
    const requestId = this.send({ type: "castAbility" });
    this.#frame.focus();
    return requestId;
  }
  spellSwarm(count: SpellSwarmCount): string | undefined {
    this.requireEditing();
    if (!validSpellSwarmCount(count)) {
      this.registry.set(
        editorAtoms.error,
        "Choose 0, 10,000, 100,000, or 200,000 spell creatures.",
      );
      return;
    }
    return this.send({ type: "spellSwarm", count }, true);
  }
  private requireEffectEditing(): void {
    this.requireEditing();
    if (this.#closed)
      throw new Error("Open the world before changing effects.");
    if (
      import.meta.env?.VITE_PUBLIC_DEMO === "1" ||
      (typeof location !== "undefined" &&
        location.pathname.endsWith("/player.html"))
    )
      throw new Error("Open the editor to change effects.");
    if (this.#pending.size >= 16)
      throw new Error(
        "Let the current changes finish before changing effects.",
      );
  }
  sceneEffects(settings: SceneEffectsSettings): string {
    this.requireEffectEditing();
    const checked = validateSceneEffects(settings);
    this.registry.set(editorAtoms.error, null);
    const requestId = this.send(
      { type: "setSceneEffects", settings: checked },
      true,
    );
    if (this.#pending.has(requestId))
      this.#sceneEffectsRequests.set(requestId, checked);
    return requestId;
  }
  previewImpact(): string {
    this.requireEffectEditing();
    this.registry.set(editorAtoms.error, null);
    return this.send({ type: "previewImpact" }, true);
  }
  resetGameplay(): string {
    return this.send({ type: "resetGameplay" });
  }
  onMmorpgFocusChat(listener: () => void): () => void {
    if (this.#closed) return () => {};
    this.#mmorpgChatListeners.add(listener);
    return () => {
      this.#mmorpgChatListeners.delete(listener);
    };
  }
  mmorpg(action: MmoUiAction): string {
    if (!validMmoUiAction(action))
      throw new Error(
        "Check your name, message, or inventory selection and try again.",
      );
    this.registry.set(editorAtoms.error, null);
    return this.send({ type: "mmorpg", action }, true);
  }
  mmorpgEditing(editing: boolean): string {
    this.requireEditing();
    if (
      !canEditMmorpg(
        import.meta.env?.VITE_PUBLIC_DEMO === "1",
        typeof location === "undefined" ? "/" : location.pathname,
        WORLD_PROFILE.readOnly,
      )
    )
      throw new Error("Open the editor to change this world.");
    if (typeof editing !== "boolean") throw new Error("Choose Play or Edit.");
    if (
      this.#closed ||
      this.registry.get(editorAtoms.status).mmorpg?.connection !== "online"
    )
      throw new Error("Enter the world before editing.");
    this.registry.set(editorAtoms.error, null);
    return this.send({ type: "mmorpgEditing", editing }, true);
  }
  battle(action: BattleAction): string {
    this.requireEditing();
    if (
      !canEditMmorpg(
        import.meta.env?.VITE_PUBLIC_DEMO === "1",
        typeof location === "undefined" ? "/" : location.pathname,
        WORLD_PROFILE.readOnly,
      )
    )
      throw new Error("Open the creator to use Battle Lab.");
    if (!validBattleAction(action))
      throw new Error("Choose a valid battle action. Armies can have 100 to 1,000,000 fighters.");
    if (
      this.#closed ||
      this.registry.get(editorAtoms.status).mmorpg?.connection !== "online"
    )
      throw new Error("Enter the world before opening Battle Lab.");
    this.registry.set(editorAtoms.error, null);
    return this.send({ type: "battle", action }, true);
  }
  restartCombat(): string {
    return this.send({ type: "restartCombat" }, true);
  }
  cancelAbility(): string {
    return this.send({ type: "cancelAbility" });
  }
  orbitDistance(distance: number): string | undefined {
    this.requireEditing();
    if (!validOrbitDistance(distance)) {
      this.registry.set(
        editorAtoms.error,
        "Choose a camera distance between 2.5 and 40 meters.",
      );
      return;
    }
    return this.send({ type: "orbitDistance", distance });
  }
  shader(source: string): string | undefined {
    this.requireEditing();
    if (!source.trim() || new TextEncoder().encode(source).byteLength > 65536) {
      this.registry.set(
        editorAtoms.error,
        "Use a WGSL material shader up to 64 KiB.",
      );
      return;
    }
    this.registry.set(editorAtoms.error, null);
    if (this.#pending.size >= 16 || this.#shaderRequests.size >= 16) {
      this.registry.set(
        editorAtoms.error,
        "Let the current shader changes finish before adding another.",
      );
      return;
    }
    const requestId = this.send({ type: "setShader", source }, true);
    this.#shaderRequests.set(requestId, source);
    return requestId;
  }
  captureBlob(options: FrameCaptureOptions = {}): Promise<Blob> {
    if (this.#closed || this.#captures.busy)
      return Promise.reject(
        new Error("Let the current screenshot finish first."),
      );
    try {
      validateFrameCaptureOptions(options);
    } catch (error) {
      return Promise.reject(error);
    }
    const id = this.send({ type: "capture", ...options });
    return this.#captures.blob(id);
  }
  private experimentRequest<T>(request: PreviewRequest): Promise<T> {
    if (!C_TOWN_BENCHMARK || this.#closed || this.#benchmarks.size >= 2)
      return Promise.reject(
        new Error("Open a town benchmark build to use these measurements."),
      );
    const id = this.send(request);
    return new Promise((resolve, reject) => {
      const timer = setTimeout(() => {
        this.#benchmarks.delete(id);
        reject(new Error("The town experiment took too long to respond."));
      }, 30000);
      this.#benchmarks.set(id, { resolve, reject, timer });
    });
  }
  cTownBenchmark(
    options: CTownBenchmarkRequest,
  ): Promise<CTownBenchmarkResult> {
    try {
      validateCTownRequest(options);
    } catch (error) {
      return Promise.reject(error);
    }
    return this.experimentRequest({ type: "cTownBenchmark", options });
  }
  cTownReload(
    candidate: "default" | "changed" | "invalid",
  ): Promise<Record<string, unknown>> {
    if (!["default", "changed", "invalid"].includes(candidate))
      return Promise.reject(new Error("Choose a prepared movement candidate."));
    return this.experimentRequest({ type: "cTownReload", candidate });
  }
  cpuCullProbe(options: {
    workers: 0 | 1 | 2;
    iterations: number;
    timeoutMs?: number;
  }): Promise<Record<string, unknown>> {
    return this.experimentRequest({ type: "cpuCullProbe", ...options });
  }
  cancelCpuCullProbe(): void {
    this.send({ type: "cancelCpuCullProbe" });
  }
  agentDiscover(): Promise<AgentDiscovery> {
    return this.holdAgentRequest(this.send({ type: "agentDiscover" }));
  }
  agentInspect(options: AgentInspectOptions = {}): Promise<AgentInspection> {
    return this.holdAgentRequest(
      this.send({
        type: "agentInspect",
        options: validateAgentInspectOptions(options),
      }),
    );
  }
  agentExecute(action: AgentAction, expectation: AgentExpectation): string {
    if (this.#closed || this.#pending.size >= 16)
      throw new Error("Let the current agent requests finish first.");
    const checked = validateAgentAction(action);
    if (
      checked.type === "command" ||
      checked.type === "shader" ||
      checked.type === "spellSwarm"
    )
      this.requireEditing();
    return this.send(
      {
        type: "agentAction",
        action: checked,
        expect: validateAgentExpectation(expectation),
      },
      true,
    );
  }
  agentResult(id: string): Promise<AgentReceipt> {
    const saved = this.#agentReceipts.get(id) ?? this.receipts.get(id);
    return saved
      ? Promise.resolve(structuredClone(saved))
      : this.holdAgentRequest(id, true);
  }
  agentCapture(options: AgentCaptureRequest): Promise<AgentCaptureEnvelope> {
    return this.holdAgentRequest(
      this.send({
        type: "agentCapture",
        options: validateAgentCaptureRequest(options),
      }),
    );
  }
  cancelAgentWork(): void {
    this.send({ type: "agentCancel" });
    this.failAgentRequests(new Error("Agent work canceled."));
  }
  capturePixels(options: FramePixelOptions = {}): Promise<ArrayBuffer> {
    if (this.#closed || this.#captures.busy)
      return Promise.reject(
        new Error("Let the current screenshot finish first."),
      );
    try {
      validateFramePixelOptions(options);
    } catch (error) {
      return Promise.reject(error);
    }
    const id = this.send({ type: "capturePixels", ...options });
    return this.#captures.pixels(id);
  }
  cancelCapture(): void {
    this.#captures.cancel();
  }
  private filmRequest(request: PreviewRequest): Promise<void> {
    if (this.#closed || this.#filmRequests.size)
      return Promise.reject(
        new Error("Let the film session finish changing first."),
      );
    const id = this.send(request);
    return new Promise((resolve, reject) => {
      const timer = setTimeout(() => {
        this.#filmRequests.delete(id);
        reject(new Error("The film session took too long to respond."));
      }, 30000);
      this.#filmRequests.set(id, { resolve, reject, timer });
    });
  }
  beginFilmCapture(size: FilmCaptureSize): Promise<void> {
    try {
      validateFilmCaptureSize(size);
    } catch (error) {
      return Promise.reject(error);
    }
    if (this.#captures.busy || this.#pending.size)
      return Promise.reject(
        new Error("Let the current changes finish before starting the film."),
      );
    return this.filmRequest({ type: "beginFilmCapture", ...size });
  }
  endFilmCapture(): Promise<void> {
    this.cancelCapture();
    return this.filmRequest({ type: "endFilmCapture" });
  }
  materialView(
    view: MaterialViewRequest,
    base: number,
  ): Promise<MaterialViewResult> {
    if (this.#closed || this.#materialViews.size >= 8)
      return Promise.reject(
        new Error("Let the current face preview finish first."),
      );
    const id = this.send({ type: "materialView", view, base });
    return new Promise((resolve, reject) => {
      const timer = setTimeout(() => {
        this.#materialViews.delete(id);
        reject(
          new Error(
            "The face preview took too long. Your applied material has not changed.",
          ),
        );
      }, 14000);
      this.#materialViews.set(id, { resolve, reject, timer });
    });
  }
  projectDocument(): Promise<ProjectDocument> {
    if (this.#closed || this.#projects.size)
      return Promise.reject(new Error("Let the current export finish first."));
    const id = this.send({ type: "exportProject" });
    return new Promise((resolve, reject) => {
      const timer = setTimeout(() => {
        this.#projects.delete(id);
        reject(
          new Error(
            "The export took too long. Your saved project has not changed.",
          ),
        );
      }, 14000);
      this.#projects.set(id, { resolve, reject, timer });
    });
  }
  capture(time?: number): void {
    if (this.#closed || this.#captures.busy) {
      this.registry.set(
        editorAtoms.error,
        "Let the current screenshot finish first.",
      );
      return;
    }
    if (
      time !== undefined &&
      (!Number.isFinite(time) || time < 0 || time > 86400)
    ) {
      this.registry.set(
        editorAtoms.error,
        "Choose a screenshot time between zero and 24 hours.",
      );
      return;
    }
    const id = this.send(
      { type: "capture", ...(time === undefined ? {} : { time }) },
      true,
    );
    this.#captures.download(id);
  }
  settle(requestId: string): void {
    const timer = this.#pending.get(requestId);
    if (!timer) return;
    clearTimeout(timer);
    this.#pending.delete(requestId);
    this.#sceneEffectsRequests.delete(requestId);
    this.registry.set(editorAtoms.pending, this.#pending.size);
  }
  close(): void {
    this.#mmorpgChatListeners.clear();
    this.failAgentRequests(new Error("The world closed."));
    if (!this.#closed) {
      this.send({
        type: "input",
        input: {
          forward: 0,
          right: 0,
          up: 0,
          lookX: 0,
          lookY: 0,
          sprint: false,
          jump: false,
        },
      });
      // Unloading the preview stops its worker and releases the graphics context.
      this.#frame.src = "about:blank";
    }
    for (const held of this.#benchmarks.values()) {
      clearTimeout(held.timer);
      held.reject(new Error("The world closed."));
    }
    this.#benchmarks.clear();
    for (const request of this.#materialViews.values()) {
      clearTimeout(request.timer);
      request.reject(new Error("The world closed."));
    }
    this.#materialViews.clear();
    for (const request of this.#projects.values()) {
      clearTimeout(request.timer);
      request.reject(new Error("The world closed before export finished."));
    }
    this.#projects.clear();
    this.#captures.cancel(new Error("The world closed."));
    for (const held of this.#filmRequests.values()) {
      clearTimeout(held.timer);
      held.reject(new Error("The world closed."));
    }
    this.#filmRequests.clear();
    if (this.#closed) return;
    this.#closed = true;
    for (const timer of this.#pending.values()) clearTimeout(timer);
    this.#pending.clear();
    this.#shaderRequests.clear();
    this.#sceneEffectsRequests.clear();
    if (this.#fiber) Effect.runFork(Fiber.interrupt(this.#fiber));
    this.registry.dispose();
  }
}
