import type {
  EnemySnapshot,
  EnemyPatrolsManifest,
} from "../../packages/runtime/enemies.ts";
import { assertNativeStartCapture } from "../../packages/runtime/capture-actors.ts";
import { EmptyWorld } from "../../packages/runtime/empty-world.ts";
import {
  preparePublishedWorld,
  type PublishedWorldStage,
} from "../../packages/runtime/published-world.ts";
import { roomRendererFeatures } from "../../packages/renderer/room-features.ts";
import {
  updateCharacterFrame,
  stepRuntime,
} from "../../packages/runtime/character-driver.ts";
import {
  planetEnvironment,
  planetCameraAllowed,
} from "../../packages/editor-services/planet-source.ts";
import { PlayerMode } from "../../packages/editor-services/player-mode.ts";
import {
  DEFAULT_SCENE_EFFECTS,
  validateSceneEffects,
  type SceneEffectsSettings,
} from "../../packages/runtime/scene-effects.ts";
import { CharacterCamera } from "../../packages/renderer/character-camera.ts";
import {
  QuickMmorpgCamera,
  QuickMmorpgLocomotion,
} from "../../packages/runtime/quick-mmorpg-controller.ts";
import {
  combatEnemyPoses,
  validQuickCombat,
  zombieSpawns,
  type QuickCombat,
} from "../../packages/runtime/quick-combat.ts";
import { JumpInput } from "../../packages/runtime/jump-input.ts";
import { MmorpgSession } from "../../packages/runtime/mmorpg-session.ts";
import { battleTerrain, battleCamera, BattleFollowCamera } from "../../packages/runtime/battle-view.ts";
import { battleMeteorTarget } from "../../packages/runtime/battle-spell-targeting.ts";
import { BattleParticipant, type BattleParticipantSnapshot, type BattleParticipantCrowdContext } from "../../packages/runtime/battle-participant.ts";
import type { BattleView, BattleParticipantStatus, BattleAttackKind } from "../../packages/editor-services/protocol.ts";
import { mmorpgHeight } from "../../packages/runtime/mmorpg-terrain.ts";
import {
  loadGroundProps,
  loadGroundCatalog,
  validateGroundDraft,
  type GroundPropsDocument,
} from "../../packages/runtime/ground-props.ts";
import type { MmoUiState } from "../../packages/runtime/mmorpg-protocol.ts";
import type { MmorpgWorldManifest } from "../../packages/assets/src/mmorpg.ts";
import { TaskOwnership } from "../../packages/runtime/task-ownership.ts";
import {
  AGENT_PROTOCOL,
  AGENT_CAPTURE_SCHEMA,
  AGENT_LIMITS,
  validateAgentAction,
  validateAgentExpectation,
  validateAgentCaptureRequest,
  validateAgentInspectOptions,
  validSpellSwarmCount,
  truncateAgentText,
  type AgentContext,
  type AgentExpectation,
  type AgentInspection,
  type AgentCaptureEnvelope,
} from "../../packages/runtime/agent-contract.ts";
import { CpuCullExperiment } from "../../packages/runtime/cpu-cull.ts";
import { frustumVisibility } from "../../packages/renderer/culling.ts";
import { liveViewport } from "../../packages/runtime/live-viewport.ts";
import {
  CellStream,
  type StreamPackageManifest,
} from "../../packages/runtime/cell-stream.ts";
import {
  StarterGame,
  createStarterGameConfig,
} from "../../packages/runtime/starter-game.ts";
import { type CharacterRenderPose } from "../../packages/renderer/character.ts";
import {
  CharacterLocomotion,
  characterAnimation,
  type CharacterPlayer,
} from "../../packages/runtime/character-state.ts";
import {
  installStaticCollision,
  type StaticCollisionData,
} from "../../packages/runtime/static-collision.ts";
import {
  type DestructibleObject,
  type DestructibleSceneManifest,
} from "../../packages/runtime/destructible-scene.ts";
import {
  DRUID_ABILITIES,
  isDruidAbility,
  type DruidAbility,
  type DruidStatus,
} from "../../packages/runtime/druid-abilities.ts";
import type { GameplayVisualState } from "../../packages/renderer/gameplay-effects.ts";
import { GameplayVisualSync } from "../../packages/runtime/gameplay-sync.ts";
import {
  prepareGameplayCast,
  type SpellForecastMetrics,
} from "../../packages/runtime/spell-forecast.ts";
import {
  pointerCameraRay,
  resolveSpellTarget,
  spellAimRadius,
  spellAimRange,
  type AimPointer,
  type SpellRay,
} from "../../packages/renderer/spell-targeting.ts";
import {
  configureAssetDownloads,
  readAsset,
} from "../../packages/assets/src/asset-download.ts";
import {
  C_TOWN_BACKEND,
  C_TOWN_BENCHMARK,
  assertCurrentControllerBackend,
} from "../../experiments/c-town/config.ts";
import {
  CTownMetrics,
  validateCTownRequest,
} from "../../experiments/c-town/metrics.ts";
import {
  FilmSessionState,
  cameraForCapture,
  environmentForCapture,
  validateFrameCaptureOptions,
  validateFilmCaptureSize,
} from "../../packages/renderer/film-frame.ts";
import {
  materialSource,
  materialKey,
  savedMaterialDocuments,
  checkMaterialTarget,
  type MaterialDocument,
} from "../../packages/editor-services/creator-material.ts";
import { checkAuthorFrame } from "../../packages/editor-services/host-budgets.ts";
import { ProjectAuthoring } from "../../packages/editor-services/project-authoring.ts";
import type { ProjectDocument } from "../../packages/editor-services/project-document.ts";
import type { IblManifest } from "../../packages/renderer/ibl.ts";
import { Renderer, type Camera } from "../../packages/renderer/renderer.ts";
import { MATERIAL } from "../../packages/renderer/shaders.ts";
import {
  PREVIEW_PROTOCOL,
  previewEventTransfers,
  DEFAULT_ENVIRONMENT,
  validateEnvironment,
  INITIAL_STATUS,
  validCharacterPerspective,
  validOrbitDistance,
  validMmorpgEditingRequest,
  validBattleRequest,
  type PreviewMessage,
  type PreviewEvent,
  type EngineStatus,
  type AssetSummary,
  type EditorCommand,
} from "../../packages/editor-services/protocol.ts";
import type {
  CompiledScene,
  CompiledCatalog,
  CatalogAsset,
  SceneLook,
} from "../../packages/assets/src/scene.ts";
import {
  add,
  scale,
  direction,
  viewAngles,
  multiply,
  perspective,
  lookAt,
  type V3,
} from "../../packages/renderer/math.ts";
import { rayBox, rayFromNdc, type Bounds } from "../../packages/renderer/picking.ts";
import { parseCommand } from "../../packages/editor-services/commands.ts";
import { EnvironmentProfiles } from "../../packages/editor-services/environment.ts";
import { selectedCameraVignette } from "../../packages/renderer/vignette.ts";
import { sourceBloomStrength } from "../../packages/renderer/source-bloom.ts";
import { sourceFilmicControls } from "../../packages/renderer/filmic.ts";
import { sunAngles } from "../../packages/renderer/lighting.ts";
import { Kernel } from "./kernel.ts";
import { referenceCameraLook } from "../../packages/renderer/reference-camera-look.ts";
import {
  WORLD_PROFILE,
  loadHauntProject,
} from "../../packages/editor-services/world-profile.ts";
import { WorldClock } from "../../packages/runtime/world-clock.ts";
import { FramePacer } from "../../packages/runtime/frame-pacer.ts";
import { prepareWorld as prepareRuntimeWorld } from "../../packages/runtime/world-loader.ts";
import type { OverviewPresentation } from "../../packages/runtime/overview.ts";
const nativeFetch = globalThis.fetch.bind(globalThis);
const assetDownloadLifetime = new AbortController();
configureAssetDownloads({
  signal: assetDownloadLifetime.signal,
  onRetry: () => report("A world file stopped downloading. Trying again…"),
});
let stagedResources: ReadonlyMap<string, Uint8Array> | undefined;
let activeResources: ReadonlyMap<string, Uint8Array> | undefined;
// Only cooked, hash-checked bytes replace local asset reads while a candidate prepares.
globalThis.fetch = ((input: RequestInfo | URL, init?: RequestInit) => {
  const url =
    typeof input === "string"
      ? input
      : input instanceof URL
        ? input.href
        : input.url;
  const key = new URL(url, self.location.href).pathname;
  const bytes = (stagedResources ?? activeResources)?.get(key);
  if (
    stagedResources &&
    (!bytes || new URL(url, self.location.href).origin !== self.location.origin)
  )
    return Promise.reject(
      new Error("This asset is missing from the verified area package."),
    );
  if (bytes && (!init?.method || init.method === "GET"))
    return Promise.resolve(
      new Response(bytes.slice().buffer, {
        headers: {
          "Content-Type": key.endsWith(".json")
            ? "application/json"
            : "application/octet-stream",
        },
      }),
    );
  return nativeFetch(input, init);
}) as typeof fetch;
let stream: CellStream | undefined;
let streamManifest: StreamPackageManifest | undefined;
let streamLoading = false;
let firstControlMs = 0,
  preparationMs = 0,
  gpuPeakBytes = 0;
function streamingStatus(): EngineStatus["streaming"] {
  if (!stream) return undefined;
  const stats = stream.snapshot();
  const uploads = renderer?.benchmarkInfo();
  return {
    activeCell: stats.activeCell,
    loading: streamLoading,
    quality:
      streamManifest?.cells.find((c) => c.id === stats.activeCell)?.quality ??
      "preparing",
    firstControlMs,
    stats: {
      ...Object.fromEntries(
        Object.entries(stats).filter(([, v]) => typeof v === "number"),
      ),
      preparedBufferUploadBytes: uploads?.preparedBufferUploadBytes ?? 0,
      materialBaseUploadBytes: uploads?.materialBaseUploadBytes ?? 0,
      layeredUploadBytes: uploads?.layeredUploadBytes ?? 0,
      layeredDecodedBytes: uploads?.layeredDecodedBytes ?? 0,
      gpuPeakBytes,
      gpuAccountedBytes: renderer?.gpuBytes ?? 0,
    },
  };
}
const scope = self as unknown as DedicatedWorkerGlobalScope;
const playerMode = new PlayerMode();
let session = "",
  renderer: Renderer | undefined,
  kernel: Kernel | undefined,
  scene: CompiledScene,
  activeNightScene: CompiledScene | undefined,
  attached: Promise<void> | undefined;
let environment = { ...DEFAULT_ENVIRONMENT },
  status: EngineStatus = { ...INITIAL_STATUS };
const runtimeOwnership = new TaskOwnership<{
  kernel: Kernel;
  renderer: Renderer;
}>();
let runtimeOwnersReady = false;
function bindRuntimeOwners() {
  if (!kernel || !renderer) return;
  const resources = { kernel, renderer };
  if (!runtimeOwnersReady) {
    runtimeOwnership.resource("kernel", resources.kernel, "preview");
    runtimeOwnership.resource("renderer", resources.renderer, "preview");
    runtimeOwnership.define({
      id: "frame",
      owner: "preview",
      writes: ["kernel", "renderer"],
    });
    runtimeOwnership.define({
      id: "activate",
      owner: "preview",
      writes: ["kernel", "renderer"],
    });
    runtimeOwnership.define({
      id: "probe",
      owner: "probe",
      reads: ["renderer"],
    });
    runtimeOwnersReady = true;
  } else
    runtimeOwnership.safePoint(() => {
      runtimeOwnership.replace("kernel", resources.kernel);
      runtimeOwnership.replace("renderer", resources.renderer);
    });
}
let activeCpuProbe:
  { pool: CpuCullExperiment; controller: AbortController } | undefined;
let environmentEdits: Partial<typeof environment> = {};
let sceneEffects: SceneEffectsSettings | undefined =
  WORLD_PROFILE.id === "open-ground" ? DEFAULT_SCENE_EFFECTS : undefined;
let activeShot = "MT-02";
let canvas: OffscreenCanvas;
let camera: Camera = { eye: [10, 4, 25], yaw: 0, pitch: -0.05 },
  mode: "fly" | "walk" = WORLD_PROFILE.mode;
const cTownMetrics = new CTownMetrics();
const framePacer = new FramePacer();
let cTownCamera: Camera | undefined;
let lastBattleSubmittedCamera: Camera | undefined;
let cTownStarted = 0;
let cTownStoppedCounters: ReturnType<typeof liveBenchmarkCounters> | undefined;
let lastSpellPreparation: SpellForecastMetrics | undefined;
function liveBenchmarkCounters() {
  return {
    frameAttempts: liveFrameAttempts,
    submittedFrames: liveSubmittedFrames,
    skippedFrames: liveSkippedFrames,
    rawCallbacks: liveRawCallbacks,
    intentionalPacedCallbacks: livePacedCallbacks,
    suspendedCallbacks: liveSuspendedCallbacks,
    kernelDroppedSeconds,
    hostClampedSeconds,
  };
}
function benchmarkResult(
  actorReference?: import("../../packages/runtime/capture-actors.ts").ActorCaptureReferenceMetadata,
) {
  return cTownMetrics.result(
    C_TOWN_BACKEND,
    {
      renderer: renderer?.benchmarkInfo(),
      battle: battleSummary(),
      kernel: kernel?.benchmarkInfo(),
      spellPreparation: lastSpellPreparation,
      worldRevision: snapshot.revision ?? 0,
      camera: camera.eye,
      mode,
      controllerProfile: "current-character-controller",
      recordingCamera: cTownCamera,
      battleCamera: lastBattleSubmittedCamera,
      routeStartedAtMs: cTownStarted,
      environment: effectiveEnvironment(),
      ...(actorReference ? { actorReference } : {}),
      frameCounters: cTownStoppedCounters ?? liveBenchmarkCounters(),
      timingScope: {
        stepMs:
          "Includes the JSON/Wasm request, gameplay and character-camera updates.",
        wasmRequestMs:
          "Rust request including JSON parsing, simulation, and response serialization.",
        commandEncodeMs:
          "Includes nested preparation callbacks between encoder creation and queue submit; overlaps cull and light work.",
        gpuMs: "Main HDR pass only when timestamp queries are available.",
        fullGpuMs:
          "Envelope of actual render/compute passes; excludes copies/uploads outside passes, query resolution, and browser composition.",
        completion:
          "Queue-fence callback notification, not compositor presentation.",
        presentation: "Unavailable; GPU submissions are not displayed frames.",
      },
    },
    (frame) => renderer?.gpuSample(frame) ?? null,
    (frame) => renderer?.gpuFrameSample(frame) ?? null,
    renderer?.completedFrameSnapshot(),
  );
}
const characterCamera = new CharacterCamera();
const jumpInput = new JumpInput();
let player: CharacterPlayer | undefined;
let locomotion: CharacterLocomotion | undefined;
let quickLocomotion: QuickMmorpgLocomotion | undefined;
let quickCamera: QuickMmorpgCamera | undefined;
let mmorpgSession: MmorpgSession | undefined;
let mmorpgWorld: MmorpgWorldManifest | undefined;
let mmorpgIcons: MmoUiState["icons"];
let mmorpgPresentation: MmoUiState | undefined;
let mmorpgEditing = false;
let battleView: BattleView = "overview";
let battleStarted = 0;
let battleField: ReturnType<typeof battleTerrain> | undefined;
let battleParticipant: BattleParticipant | undefined;
const battleFollowCamera = new BattleFollowCamera();
function battleLook(yaw: number, pitch: number) {
  if (!Number.isFinite(yaw) || !Number.isFinite(pitch)) return;
  if (battleFollowing() && battleView !== "first") battleFollowCamera.look(yaw, pitch);
  else { yawDelta += yaw; pitchDelta += pitch; }
}
function changeBattleView(view: BattleView) {
  const pose = battleParticipant?.snapshot;
  if (pose && view === "first" && battleView !== "first") {
    const orbit = battleFollowCamera.orbit;
    yawDelta = Math.atan2(Math.sin(orbit.yaw - pose.yaw), Math.cos(orbit.yaw - pose.yaw));
    pitchDelta = orbit.pitch - pose.pitch;
  } else if (pose && battleView === "first" && view !== "first") {
    battleFollowCamera.followFirst({yaw: pose.yaw + yawDelta, pitch: pose.pitch + pitchDelta});
    yawDelta = 0; pitchDelta = 0;
  }
  battleFollowCamera.cancelAttack();
  battleView = view;
}
let battleTeam: 0 | 1 = 0;
let battleAttack: BattleAttackKind = "melee";
let battleTarget: BattleParticipantStatus["target"];
let battleMeteorPending = false;
function requestBattleMeteor(pressed: boolean) {
  const gpu = renderer?.battleInfo()?.gpu;
  if (pressed && battleAttack === "meteor_shower" && battleTarget && battleFollowing() && battleParticipantReady() && gpu?.meteorCooldownTicks === 0 && !gpu.participantDead)
    battleMeteorPending = true;
}
let battleSpawn: { position: V3; yaw: number } | undefined;
let battleScriptInput: { forward: number; turn: number; sprint: boolean; attack: boolean; until: number } | undefined;
let battleParticipantEpoch = 0;
function battleCrowdContext(): BattleParticipantCrowdContext {
  const sample = renderer?.battleParticipantContacts() ?? null;
  const battle = renderer?.battleInfo();
  return {
    expectedEpoch: battleParticipantEpoch,
    currentTick: Math.max(battle?.simulationSteps ?? 0, battle?.gpu?.ticks ?? 0, sample?.tick ?? 0),
    ageSeconds: sample ? Math.min(10, Math.max(0, (performance.now() - sample.sampledAt) / 1000)) : 10,
    sample: sample ? { epoch: sample.epoch, tick: sample.tick, position: [...sample.position],
      contacts: sample.contacts.map((c) => ({id:c.id,position:[...c.position],radius:c.radius,velocity:[...c.velocity]})),overflow:sample.overflow } : null,
  };
}
function battleParticipantReady() { return (renderer?.battleInfo()?.gpu?.participantEpoch ?? 0) >= battleParticipantEpoch; }
function battleFollowing() {
  return !!battleParticipant?.snapshot && ["first", "third", "player"].includes(battleView);
}
function battleSpectating() {
  return !!renderer?.battleInfo()?.enabled && !battleFollowing() && battleView !== "player";
}
function participantStatus(): BattleParticipantStatus | undefined {
  const pose = battleParticipant?.snapshot;
  if (!pose) return undefined;
  const gpu = renderer?.battleInfo()?.gpu;
  const ready = battleParticipantReady();
  const health = ready ? gpu?.participantHealth ?? 200 : 200;
  return { active: true, class: pose.class, team: battleTeam, position: [...pose.position],
    yaw: pose.yaw, pitch: pose.pitch, health, maxHealth: 200,
    ready, alive: health > 0, attackKind: battleAttack,
    attackSequence: pose.attackSequence, tick: pose.tick, droppedSeconds: pose.droppedSeconds,
    ...(battleAttack === "meteor_shower" && health > 0 && battleTarget ? {target: battleTarget} : {}) };
}
function leaveBattleParticipant() {
  battleParticipant?.clear();
  battleParticipant = undefined;
  battleScriptInput = undefined;
  battleSpawn = undefined;
  battleParticipantEpoch = 0;
  battleTarget = undefined;
  battleMeteorPending = false;
  battleFollowCamera.reset();
  renderer?.setGameplayAim();
  renderer?.setBattleParticipant({ active: false, position: [0, 0, 0], yaw: 0, team: 0, attackSequence: 0, attack: "melee", aim: [0, 0, 0] });
}
function joinBattleParticipant(at?: readonly [number, number], team: 0 | 1 = 0, view: "first" | "third" = "third", kind: "paladin" | "sorceror" = "paladin") {
  const battle = renderer?.battleInfo();
  if (!kernel || !renderer || !battle?.enabled || !battleField) throw Error("Start a battle before entering it.");
  const position: V3 = [at?.[0] ?? battle.center[0] + (team === 0 ? -12 : 12), 0, at?.[1] ?? battle.center[2] - 8];
  if (Math.abs(position[0] - battle.center[0]) > battle.radius + 24 || Math.abs(position[2] - battle.center[2]) > battle.radius + 24)
    throw Error("Choose a spot inside this battlefield.");
  const yaw = team === 0 ? Math.PI / 2 : -Math.PI / 2;
  const candidate = new BattleParticipant(kernel);
  candidate.enter(battleField, { class: kind, position, yaw });
  battleParticipant = candidate;
  battleSpawn = { position, yaw };
  battleTeam = team;
  battleView = view;
  battleAttack = kind === "sorceror" ? "ranged" : "melee";
  battleTarget = undefined;
  battleMeteorPending = false;
  battleScriptInput = undefined;
  const pose = candidate.snapshot!;
  battleFollowCamera.reset(pose);
  yawDelta = 0; pitchDelta = 0;
  renderer.setBattleParticipant({ active: true, position: pose.position, yaw: pose.yaw, team, attackSequence: pose.attackSequence, attack: battleAttack, aim: pose.eye, firstPerson: view === "first" });
  battleParticipantEpoch = renderer.resetBattleParticipant();
}
function battleSummary() {
  const battle = renderer?.battleInfo();
  const gpu = battle?.gpu;
  return {
    enabled: !!battle?.enabled, count: battle?.count ?? 0, camera: battleView,
    center: battle?.center, radius: battle?.radius,
    stats: battle ? {
      gpuBytes: battle.accountedGpuBytes, droppedSeconds: battle.droppedSeconds,
      sampleSerial: battle.sampleSerial, simulationSteps: battle.simulationSteps,
      frameDraws: battle.frameDraws,
      ...(gpu ? Object.fromEntries(Object.entries(gpu).filter(([,v]) => typeof v === "number")) as Record<string, number> : {}),
      ...(gpu ? { simulationTicks: gpu.ticks } : {}),
    } : {},
  };
}
function battleStatus(): EngineStatus["battle"] {
  if (WORLD_PROFILE.id !== "open-ground") return undefined;
  const { enabled, count, camera, stats, center, radius } = battleSummary();
  return { enabled, count, camera, stats, ...(center ? {center: [...center] as V3, radius} : {}),
    ...(battleParticipant ? { participant: participantStatus() } : {}) };
}
let groundProps: GroundPropsDocument | undefined;
let groundDraftKey: string | undefined;
let combatEnabled = false;
let combatState: QuickCombat | undefined;
let combatAttackSequence = 0;
let characterPose: CharacterRenderPose | undefined;
const collisionScenes = new Map<
  "day" | "night",
  StaticCollisionData | undefined
>();
const gameplayScenes = new Map<
  "day" | "night",
  { objects: DestructibleObject[]; manifest: DestructibleSceneManifest }
>();
interface RuntimeGameplay extends GameplayVisualState {
  enemies?: EnemySnapshot;
  tick: number;
  version: number;
  castCount: number;
  objectCount: number;
  destroyedCount: number;
  pendingImpacts: number;
  pendingCollapse: number;
  activeCast: null | { ability: DruidAbility; age: number; life: number };
  lastImpact: V3 | null;
  castle?: NonNullable<GameplayVisualState["castle"]> & {
    total: number;
    remaining: number;
    destroyed: number;
    active: boolean;
    complete: boolean;
    cancelled: boolean;
    waves: number;
    impacts: number;
    elapsed: number;
    pendingImpacts: number;
    target: V3 | null;
  };
}
let gameplayState: RuntimeGameplay | undefined;
let enemyPatrols: EnemyPatrolsManifest | undefined;
const gameplayVisuals = new GameplayVisualSync();
let gameplayLoaded = false;
let preparingDestruction = false;
let castlePreparation = 0;
let selectedAbility: DruidAbility | null = null;
let gameplayTarget: DruidStatus["target"];
let gameplayRay: SpellRay | undefined;
let aimPointer: AimPointer = { x: 0, y: 0, locked: true };
let lastAimKey = "";
let impactCount = 0;
let effectIds = new Set<number>();
let lastGameImpact: DruidStatus["lastImpact"];
let collisionScenario: "day" | "night" | undefined;
function activateCollision(scenario: "day" | "night") {
  if (!kernel || collisionScenario === scenario) return;
  installStaticCollision(
    kernel,
    collisionScenes.get(scenario),
    gameplayScenes.get(scenario)?.objects.length,
  );
  collisionScenario = scenario;
  if (gameplayLoaded) initializeGameplay(scenario);
  characterCamera.resetCollision();
}
function initializeGameplay(scenario: "day" | "night") {
  if (WORLD_PROFILE.id === "open-ground") return;
  if (!kernel || !renderer) return;
  gameplayVisuals.reset();
  castlePreparation++;
  preparingDestruction = false;
  const objects = gameplayScenes.get(scenario)?.objects ?? [];
  for (let start = 0; start < Math.max(1, objects.length); start += 8192)
    checkResult(
      kernel.request({
        type: "gameplay_init",
        objects: objects.slice(start, start + 8192),
        seed: 12999,
        append: start > 0,
        complete: false,
      }),
    );
  const result = checkResult(
    kernel.request({
      type: "gameplay_init",
      objects: [],
      append: true,
      complete: true,
      targetGroups: gameplayScenes.get(scenario)?.manifest.targetGroups,
    }),
  );
  if (objects.length)
    renderer.setGameplayObjects(
      scenario,
      objects,
      gameplayScenes.get(scenario)?.manifest.targetGroups,
    );
  selectedAbility = null;
  impactCount = 0;
  effectIds.clear();
  lastGameImpact = undefined;
  gameplayTarget = undefined;
  gameplayLoaded = true;
  const enemies = enemyPatrols?.scenarios[scenario];
  const ready = enemies
    ? checkResult(kernel.request({ type: "gameplay_init_enemies", enemies }))
    : result;
  lastAimKey = "";
  syncGameplay(ready.gameplay);
}
function syncGameplay(value?: RuntimeGameplay) {
  if (!value || !gameplayLoaded || !renderer) return;
  gameplayState = value;
  renderer.setEnemies(value.enemies?.actors ?? []);
  for (const effect of value.effects)
    if (effect.kind === "impact" && !effectIds.has(effect.id)) {
      impactCount++;
      lastGameImpact = { point: effect.origin, radius: effect.radius };
    }
  if (value.effects.length)
    effectIds = new Set(value.effects.map((effect) => effect.id));
  else effectIds.clear();
  gameplayVisuals.apply(renderer, environment.mode, value);
}
function spellRay() {
  if (selectedAbility === "destroy_castle" && gameplayState?.castle?.target) {
    const origin =
      mode === "walk" && player
        ? ([...player.eye] as V3)
        : ([...camera.eye] as V3);
    const delta = gameplayState.castle.target.map(
      (v, i) => v - origin[i],
    ) as V3;
    return {
      origin,
      direction: scale(delta, 1 / Math.max(0.001, Math.hypot(...delta))),
    };
  }
  updateGameplayAim();
  if (!gameplayRay || !gameplayTarget)
    throw new Error("Point at a wall or the ground within reach, then cast.");
  return gameplayRay;
}
function updateGameplayAim(renderCamera: Camera = camera) {
  const origin = mode === "walk" && player ? player.eye : renderCamera.eye;
  const key = JSON.stringify([
    selectedAbility,
    environment.mode,
    gameplayState?.version,
    gameplayState?.enemies?.alive ? gameplayState.tick : 0,
    renderCamera.eye,
    renderCamera.yaw,
    renderCamera.pitch,
    renderCamera.fov,
    renderCamera.up,
    origin,
    aimPointer,
    width,
    height,
    mode,
    characterCamera.perspective,
    gameplayState?.castle?.complete,
  ]);
  if (key === lastAimKey) return;
  lastAimKey = key;
  gameplayTarget = undefined;
  gameplayRay = undefined;
  renderer?.setGameplayAim();
  if (!selectedAbility || !gameplayState?.objectCount || !kernel) return;
  if (selectedAbility === "destroy_castle") {
    const castle = gameplayState.castle;
    if (castle?.target && !castle.complete)
      gameplayTarget = {
        point: castle.target,
        normal: [0, 1, 0],
        distance: Math.hypot(...castle.target.map((v, i) => v - camera.eye[i])),
      };
    return;
  }
  const resolved = resolveSpellTarget(
    pointerCameraRay(
      renderCamera,
      width / height,
      aimPointer,
      mode === "walk" && characterCamera.perspective === "third",
    ),
    origin,
    spellAimRange(selectedAbility),
    (ray, maxDistance) =>
      checkResult(
        kernel!.request({
          type: "gameplay_aim",
          ...ray,
          maxDistance,
        }),
      ).aim ?? undefined,
  );
  if (!resolved) return;
  gameplayRay = resolved.ray;
  gameplayTarget = resolved.target;
  renderer?.setGameplayAim({
    point: resolved.target.point,
    normal: resolved.target.normal,
    radius: spellAimRadius(selectedAbility),
  });
}
function gameplaySummary(): DruidStatus | undefined {
  if (!gameplayState?.objectCount || !renderer) return undefined;
  const s = renderer.gameplayStats;
  return {
    enabled: true,
    preparing: preparingDestruction,
    selected: selectedAbility,
    destroyed: gameplayState.destroyedCount,
    debris: gameplayState.debris.length,
    rubble:
      gameplayState.rubble?.length ?? gameplayState.castle?.rubble?.length ?? 0,
    effects: gameplayState.effects.length,
    casts: gameplayState.castCount,
    impacts: impactCount,
    tick: gameplayState.tick,
    ...(gameplayState.enemies?.total
      ? {
          enemies: {
            total: gameplayState.enemies.total,
            alive: gameplayState.enemies.alive,
            killed: gameplayState.enemies.killed,
            modelLoaded: renderer.enemyStats.modelLoaded,
            rendered: renderer.enemyStats.submittedActors,
          },
        }
      : {}),
    ...(gameplayState.castle?.total
      ? {
          castle: {
            total: gameplayState.castle.total,
            remaining: gameplayState.castle.remaining,
            destroyed: gameplayState.castle.destroyed,
            active: gameplayState.castle.active,
            complete: gameplayState.castle.complete,
            cancelled: gameplayState.castle.cancelled,
            preparing:
              preparingDestruction && selectedAbility === "destroy_castle",
            waves: gameplayState.castle.waves,
            impacts: gameplayState.castle.impacts,
            elapsed: gameplayState.castle.elapsed,
            rubble:
              gameplayState.castle.rubbleCount ??
              gameplayState.castle.rubble?.length ??
              0,
          },
        }
      : {}),
    ...(gameplayTarget ? { target: gameplayTarget } : {}),
    ...(gameplayState.activeCast
      ? {
          casting: {
            ability: gameplayState.activeCast.ability,
            remaining: Math.max(
              0,
              gameplayState.activeCast.life - gameplayState.activeCast.age,
            ),
          },
        }
      : {}),
    ...(lastGameImpact ? { lastImpact: lastGameImpact } : {}),
    rendered: {
      maskedNodes: s.maskedNodes,
      fracturedSources: s.activeFracturedSources,
      standingTriangles: s.standingTriangles,
      debrisTriangles: s.debrisTriangles,
      effectTriangles: s.effectTriangles,
      aimTriangles: s.aimTriangles,
      rubbleCount: s.rubbleCount,
      rubbleTriangles: s.rubbleTriangles,
      geometryBytes: s.geometryBytes,
    },
  };
}
function publishGameplay() {
  updateGameplayAim();
  status = { ...status, gameplay: gameplaySummary() };
  send({ type: "status", status });
}
async function castDruid(requestId: string) {
  if (!kernel || !renderer || !gameplayState?.objectCount || !selectedAbility)
    throw new Error("Choose a spell after the town opens.");
  if (preparingDestruction)
    throw new Error("The spell is preparing. You can stop it below.");
  const ability = selectedAbility;
  const scenario = environment.mode;
  const generation = ++castlePreparation;
  const castKernel = kernel;
  const castRenderer = renderer;
  const cancelled = () =>
    generation !== castlePreparation ||
    kernel !== castKernel ||
    renderer !== castRenderer;
  const request = {
    castId: requestId,
    ability,
    ...spellRay(),
  };
  lastSpellPreparation = undefined;
  preparingDestruction = true;
  publishGameplay();
  try {
    const { plan, metrics } = await prepareGameplayCast(castKernel, request, {
      cancelled,
      prepareVisuals: async (plan) => {
        const prepared = await castRenderer.prepareGameplayDestructionAsync(
          scenario,
          plan.prospectiveDestroyed,
          cancelled,
        );
        if (cancelled()) throw new Error("Spell preparation stopped.");
        if (!prepared)
          throw new Error(
            castRenderer.gameplayLimitReason ||
              "This spell would break too much of the town at once. Restore the town to start again.",
          );
      },
    });
    lastSpellPreparation = metrics;
    const applied = checkResult(
      castKernel.request({
        type: "gameplay_cast",
        ...request,
        expectedVersion: plan.version,
        fragmentMasks: castRenderer.gameplayFragmentMasks(
          scenario,
          plan.prospectiveDestroyed,
        ),
        ...(ability === "destroy_castle"
          ? { castleFragments: castRenderer.castleFragmentMasks(scenario) }
          : {}),
      }),
    );
    syncGameplay(applied.gameplay);
    receipt(
      requestId,
      true,
      `${DRUID_ABILITIES.find((entry) => entry.id === ability)!.name} cast`,
    );
  } finally {
    if (generation === castlePreparation) preparingDestruction = false;
    publishGameplay();
  }
}
function updatePlayer(value: CharacterPlayer, elapsed: number) {
  player = value;
  if (mmorpgWorld && quickCamera) {
    const behindX = value.position[0] - Math.sin(value.yaw) * 15 * quickCamera.worldScale;
    const behindZ = value.position[2] + Math.cos(value.yaw) * 15 * quickCamera.worldScale;
    const framed = quickCamera.frame(value.position, value.yaw, mmorpgHeight(behindX, behindZ), elapsed);
    camera.eye = framed.eye;
    Object.assign(camera, viewAngles(framed.eye, framed.target));
    camera.near = framed.near; camera.far = framed.far; camera.fov = framed.fov; camera.up = framed.up;
    renderer?.setCharacter(undefined);
    return;
  }
  if (quickLocomotion && quickCamera && mode === "walk") {
    const framed = quickCamera.frame(
      value.position,
      value.yaw,
      WORLD_PROFILE.groundY,
      elapsed,
    );
    camera.eye = framed.eye;
    Object.assign(camera, viewAngles(framed.eye, framed.target));
    camera.near = framed.near;
    camera.far = framed.far;
    camera.fov = framed.fov;
    camera.up = framed.up;
    characterPose = quickLocomotion.pose(value, true);
    renderer?.setCharacter(characterPose);
    return;
  }
  characterPose = updateCharacterFrame(
    kernel!,
    value,
    mode,
    camera,
    characterCamera,
    locomotion,
    WORLD_PROFILE.groundY,
    elapsed,
  );
  renderer?.setCharacter(characterPose);
}

function characterStatus() {
  const localBattle = battleParticipant?.snapshot;
  if (localBattle) return { perspective: battleView === "first" ? "first" as const : "third" as const,
    position: localBattle.position, velocity: localBattle.velocity, grounded: localBattle.grounded,
    animation: localBattle.state, orbitDistance: battleFollowCamera.orbit.distance, modelLoaded: renderer?.mmorpgStats.modelLoaded ?? false,
    controller: "quick-mmorpg-v1" as const, animationTime: localBattle.animationTime };
  const shared = mmorpgPresentation?.player;
  if (mmorpgWorld && !shared) return undefined;
  if (mmorpgWorld && shared) return {
    perspective: "third" as const, position: [...shared.position] as V3,
    velocity: player?.velocity ?? [0, 0, 0] as V3, grounded: shared.prediction?.grounded ?? false,
    animation: shared.state, orbitDistance: 15 * mmorpgWorld.worldScale,
    modelLoaded: renderer?.mmorpgStats.modelLoaded ?? false,
    controller: "quick-mmorpg-v1" as const, animationTime: shared.animationTime,
  };
  return mode === "walk" && player
    ? {
        perspective: quickCamera ? ("third" as const) : characterCamera.perspective,
        position: [...player.position] as V3,
        velocity: [...player.velocity] as V3,
        grounded: player.grounded,
        animation: quickLocomotion?.animation ?? characterAnimation(player),
        orbitDistance: quickCamera
          ? 15 * quickCamera.worldScale
          : characterCamera.orbitDistance,
        modelLoaded: renderer?.hasCharacter ?? false,
        ...(quickCamera
          ? {
              controller: "quick-mmorpg-v1" as const,
              animationTime: quickLocomotion!.time,
            }
          : {}),
      }
    : undefined;
}
function syncCombat(value: unknown) {
  if (!combatEnabled || value == null) return;
  if (!validQuickCombat(value))
    throw new Error("The combat state could not update.");
  combatState = value;
  renderer?.setEnemies(combatEnemyPoses(value));
  if (!value.player.alive) quickLocomotion?.die();
}
let projectTemplate: ProjectDocument["template"] | undefined;
let projectSpawn: unknown;
let game: StarterGame | undefined;
function candidateGame(source: any) {
  const key = source.entities?.find((e: any) => e.id === "starter-key");
  const exit = source.entities?.find((e: any) => e.id === "starter-exit");
  if (!key || !exit) return undefined;
  const origin = (projectSpawn as { position: V3 }).position;
  const config = createStarterGameConfig(origin);
  config.key = { ...config.key, id: key.id, position: [...key.position] as V3 };
  config.exit = {
    ...config.exit,
    id: exit.id,
    position: [...exit.position] as V3,
  };
  return new StarterGame(config).config();
}
function syncGame() {
  const config = candidateGame(snapshot);
  if (!config) {
    game = undefined;
    return;
  }
  if (game) game.replaceConfig(config);
  else game = new StarterGame(config);
}
function gameStatus(): EngineStatus["game"] {
  if (!game) return undefined;
  const state = game.snapshot();
  const config = game.config();
  const target = state.keyCollected
    ? config.exit.position
    : config.key.position;
  const point = player?.position ?? config.spawn;
  const dx = target[0] - point[0],
    dz = target[2] - point[2];
  const angle = Math.atan2(dx, -dz) - (player?.yaw ?? 0);
  const heading =
    Math.cos(angle) > 0.7
      ? "ahead"
      : Math.cos(angle) < -0.7
        ? "behind you"
        : Math.sin(angle) > 0
          ? "to your right"
          : "to your left";
  const marker =
    WORLD_PROFILE.id === "medieval-town-lab"
      ? state.keyCollected
        ? "exit bench"
        : "key barrel"
      : state.keyCollected
        ? "green exit"
        : "brass key";
  return {
    phase: state.completed
      ? "complete"
      : state.keyCollected
        ? "find-exit"
        : "find-key",
    message: state.completed
      ? "Courtyard complete! Restart to play again."
      : `Find the ${marker}: ${Math.hypot(dx, dz).toFixed(1)} m ${heading}. Interact (E).`,
    keyId: config.key.id,
    exitId: config.exit.id,
    keyPosition: config.key.position,
    exitPosition: config.exit.position,
    hasKey: state.keyCollected,
  };
}
let snapshot: any = { revision: 0, entities: [], rooms: [], wallColliders: [] },
  selected: string | null = null;
let assets: AssetSummary[] = [],
  assetMeshes = new Map<string, number>(),
  catalog = new Map<string, CatalogAsset>();
let width = 1280,
  height = 720,
  previous = 0,
  reportTime = 0,
  frameTimes: number[] = [],
  saved = WORLD_PROFILE.readOnly || WORLD_PROFILE.id === "medieval-town-lab";
let lastSubmission = 0;
let liveFrameAttempts = 0,
  liveRawCallbacks = 0,
  livePacedCallbacks = 0,
  liveSuspendedCallbacks = 0,
  liveSubmittedFrames = 0,
  liveSkippedFrames = 0;
let hostClampedSeconds = 0,
  kernelDroppedSeconds = 0;
let animationClock = new WorldClock();
let emptyWorld: EmptyWorld | undefined;
let emptyFilmSize: { width: number; height: number } | undefined;
let emptyCaptureBusy = false;
const filmState = new FilmSessionState();
let filmPending: Promise<Blob | ArrayBuffer> | undefined;
let filmTransition = false;
let filmRestore: { width: number; height: number; age: number } | undefined;
let deferredLiveSize: { width: number; height: number } | undefined;
function applyLiveSize() {
  if (!deferredLiveSize || filmState.manual || filmState.inFlight) return;
  ({ width, height } = deferredLiveSize);
  deferredLiveSize = undefined;
  renderer?.resize(width, height);
  if (emptyWorld && !emptyFilmSize) {
    emptyWorld.resize(width, height);
    emptyWorld.render();
  }
}
function resumeLiveFilmSession() {
  const restore = filmRestore;
  filmState.end();
  filmRestore = undefined;
  if (restore) {
    animationClock = new WorldClock(restore.age);
    animationClock.start(performance.now());
    if (!deferredLiveSize)
      deferredLiveSize = { width: restore.width, height: restore.height };
    applyLiveSize();
    previous = 0;
    framePacer.reset();
    lastSubmission = 0;
    frameTimes = [];
    if (!disposed) frameId = scope.requestAnimationFrame(draw);
  }
}
let look: SceneLook | undefined;
let overview: OverviewPresentation | undefined;
let environmentProfiles = new EnvironmentProfiles();
let lastWorkingShader = MATERIAL,
  commandQueue = Promise.resolve(),
  pendingCommands = 0;
let shaderChanging = 0;
let agentWorkSerial = 0;
let agentInput: AbortController | undefined;
const agentRequests = new Map<
  string,
  { started: number; ticket: number; type: string }
>();
let agentCaptureSequence = 0;
let observedAgentRenderer: Renderer | undefined;
let observedAgentVersion = -1,
  agentGeneration = 0;
const agentSourceHashes = new WeakMap<CompiledScene, Promise<string>>();
let input: {
    forward: number;
    right: number;
    up: number;
    sprint: boolean;
    jump: boolean;
    attack?: boolean;
    dance?: boolean;
  } = { forward: 0, right: 0, up: 0, sprint: false, jump: false },
  yawDelta = 0,
  pitchDelta = 0,
  disposed = false,
  frameId = 0;
const send = (
  event: Omit<PreviewEvent, "protocol" | "sessionId"> | Record<string, unknown>,
) =>
  scope.postMessage(
    {
      ...event,
      protocol: PREVIEW_PROTOCOL,
      sessionId: session,
    },
    previewEventTransfers(event),
  );
const report = (message: string) => {
  status = { ...status, message };
  send({ type: "status", status });
};
const receipt = (requestId: string, ok: boolean, message: string) => {
  message = truncateAgentText(message, 2048);
  const request = agentRequests.get(requestId);
  send({
    type: "receipt",
    requestId,
    ok,
    message,
    revision: snapshot.revision ?? 0,
    ...(request
      ? {
          agent: {
            requestId,
            ok,
            message,
            revision: snapshot.revision ?? 0,
            stage: ok
              ? request.type === "command" && saved
                ? "saved"
                : "activated"
              : "rejected",
            saved,
            diagnostics: ok ? [] : [truncateAgentText(message, 512)],
            timings: {
              receiptTotalMs: performance.now() - request.started,
              ...(ok && request.type === "command" ? runtimeEditMetrics : {}),
            },
          },
        }
      : {}),
  });
};
function sourceSkyLightIntensity() {
  return environmentProfiles.sourceIntensity(environment.mode);
}
function publishEnvironment() {
  send({
    type: "environment",
    settings: environment,
    sourceSkyLightIntensity: sourceSkyLightIntensity(),
  });
}
function configureEnvironmentLighting(ibl?: IblManifest) {
  environmentProfiles.configureSourceIntensity({
    day: ibl?.scenarios.day?.gain?.sourceSkyLightIntensity,
    night: ibl?.scenarios.night?.gain?.sourceSkyLightIntensity,
  });
  environmentProfiles.configureSourceBloom({
    day: sourceBloomStrength(look, "day", DEFAULT_ENVIRONMENT.bloomStrength),
    night: sourceBloomStrength(
      look,
      "night",
      DEFAULT_ENVIRONMENT.bloomStrength,
    ),
  });
  environment = environmentProfiles.initial(environment);
  publishEnvironment();
}
function checkResult(result: any) {
  if (!result.ok)
    throw new Error(
      result.message ??
        result.error?.message ??
        String(result.error ?? "The edit could not be applied."),
    );
  return result;
}
function prepareWorld(
  scene: CompiledScene,
  nightScene: CompiledScene,
  targetCanvas: OffscreenCanvas,
) {
  return prepareRuntimeWorld(scene, nightScene, targetCanvas, {
    report,
    presentationURL: WORLD_PROFILE.presentationURL,
    allowQuickCombat: WORLD_PROFILE.id === "open-ground",
  });
}
function adoptWorld(prepared: Awaited<ReturnType<typeof prepareWorld>>) {
  mmorpgSession?.disconnect(false); mmorpgSession = undefined;
  mmorpgWorld = prepared.mmorpg; mmorpgIcons = prepared.mmorpgIcons; mmorpgPresentation = undefined;
  renderer = prepared.renderer;
  if (sceneEffects) renderer.setSceneEffects(sceneEffects);
  bindRuntimeOwners();
  scene = prepared.scene;
  activeNightScene = prepared.nightScene;
  enemyPatrols = prepared.enemyPatrols;
  renderer.setEnemies(gameplayState?.enemies?.actors ?? []);
  look = prepared.look;
  overview = prepared.overview;
  environment = prepared.environment;
  environmentProfiles = prepared.profiles;
  collisionScenes.set("day", prepared.dayCollision);
  collisionScenes.set("night", prepared.nightCollision);
  gameplayScenes.clear();
  if (prepared.dayGameplay) gameplayScenes.set("day", prepared.dayGameplay);
  if (prepared.nightGameplay)
    gameplayScenes.set("night", prepared.nightGameplay);
  const sourceController =
    WORLD_PROFILE.id === "open-ground" ? prepared.character?.controller : undefined;
  quickLocomotion = sourceController
    ? new QuickMmorpgLocomotion(prepared.character!.clips)
    : undefined;
  quickCamera = sourceController || mmorpgWorld
    ? new QuickMmorpgCamera(sourceController?.worldScale ?? mmorpgWorld!.worldScale)
    : undefined;
  combatEnabled =
    !!sourceController &&
    prepared.enemyCharacter?.animationProfile === "quick-mmorpg-zombie-v1";
  combatState = undefined;
  locomotion =
    prepared.character && !quickLocomotion
      ? new CharacterLocomotion(prepared.character.clips)
      : undefined;
}
async function initialize() {
  assertCurrentControllerBackend(C_TOWN_BACKEND);
  report(`Opening ${WORLD_PROFILE.name}…`);
  const project = await loadHauntProject();
  if (project)
    projectTemplate = {
      world: WORLD_PROFILE.id,
      engine_release:
        import.meta.env.VITE_OPENAGENTS_ENGINE_RELEASE ?? "standalone",
      asset_set_sha256: import.meta.env.VITE_OPENAGENTS_ASSET_SET ?? "",
      scene_sha256: project.sourceSha256,
    };
  if (project) environment = validateEnvironment(project.environment);
  const started = performance.now();
  const packageURL = import.meta.env.VITE_STREAM_PACKAGE_URL as
    string | undefined;
  let prepared: Awaited<ReturnType<typeof prepareWorld>>;
  let published: PublishedWorldStage | undefined;
  const artifactURL = import.meta.env.VITE_PLAYER_ARTIFACT_URL;
  if (artifactURL) {
    if (artifactURL !== "/player-artifact.json" || packageURL || project)
      throw new Error(
        "Choose one local published world before opening this preview.",
      );
    const artifact = await readAsset(
      artifactURL,
      async (response) => {
        if (!response.ok)
          throw new Error("The published world could not open.");
        return response.json();
      },
      { fetcher: nativeFetch },
    );
    published = await preparePublishedWorld(artifact, canvas, {
      fetcher: nativeFetch,
      report,
      compatibility: {
        worldId: artifact.profile === "empty" ? undefined : WORLD_PROFILE.id,
      },
      rendererOptions:
        artifact.profile === "room"
          ? { features: roomRendererFeatures }
          : undefined,
    });
    if (published.artifact.profile === "empty") {
      emptyWorld = await EmptyWorld.create(canvas);
      emptyWorld.resize(width, height);
      emptyWorld.render();
      await emptyWorld.device.queue.onSubmittedWorkDone();
      firstControlMs = performance.now() - started;
      status = {
        ...status,
        phase: "ready",
        instances: 0,
        triangles: 0,
        frame: 1,
        fps: 0,
        message: "An empty world",
        runtimeMetrics: {
          firstControlMs,
          preparationMs: firstControlMs,
          gpuAccountedBytes: 0,
        },
      };
      send({
        type: "ready",
        assets: [],
        status,
        hasSourceSky: false,
        shaderSource: "",
      });
      return;
    }
    if (!published.prepared || !published.world || !published.kernelBytes)
      throw new Error("Open a playable artifact in the editor preview.");
    prepared = published.prepared;
    mode = published.world.mode;
    if (published.artifact.profile === "room")
      characterCamera.setPerspective("first", true);
  } else if (packageURL) {
    if (
      !["medieval-town-lab", "starter-courtyard"].includes(WORLD_PROFILE.id) ||
      !/^\/(?:private-assets|starter)\/[a-zA-Z0-9._-]+\.json$/.test(packageURL)
    )
      throw new Error("Choose a prepared local streaming package.");
    streamManifest = await readAsset(
      packageURL,
      async (response) => {
        if (!response.ok)
          throw new Error(
            "Cook the streaming package before opening this world.",
          );
        return response.json();
      },
      { fetcher: nativeFetch },
    );
    stream = new CellStream({ fetch: nativeFetch });
    stream.configure(streamManifest);
    let first: Awaited<ReturnType<typeof prepareWorld>> | undefined;
    const result = await stream.load("entry", {
      prepare: async ({ cell, resources }) => {
        stagedResources = resources;
        try {
          const decodeStarted = performance.now();
          const day = JSON.parse(
            new TextDecoder().decode(resources.get(cell.scenes.day)!),
          );
          const night = JSON.parse(
            new TextDecoder().decode(resources.get(cell.scenes.night)!),
          );
          const decodeMs = performance.now() - decodeStarted;
          const candidate = await prepareWorld(day, night, canvas);
          return {
            decodeMs,
            commit: () => {
              first = candidate;
              activeResources = resources;
            },
            dispose: () => candidate.renderer.dispose(first === candidate),
          };
        } finally {
          stagedResources = undefined;
        }
      },
    });
    if (!result.ok || !first)
      throw new Error("The entry package could not open.");
    prepared = first;
  } else {
    scene = await readAsset(
      project?.sceneURL ?? WORLD_PROFILE.scene,
      async (response) => {
        if (!response.ok)
          throw new Error("The world could not open. Reload to try again.");
        return response.json();
      },
    );
    const nightScene = await readAsset(
      project?.nightSceneURL ?? WORLD_PROFILE.nightScene,
      async (nightResponse) => {
        if (!nightResponse.ok)
          throw new Error("Prepare both day and night town scenes.");
        return nightResponse.json();
      },
    );
    prepared = await prepareWorld(scene, nightScene, canvas);
  }
  preparationMs = performance.now() - started;
  adoptWorld(prepared);
  renderer = prepared.renderer;
  mmorpgEditing = false;
  battleView = "overview";
  battleParticipant = undefined;
  battleFollowCamera.reset();
  battleField = undefined;
  battleScriptInput = undefined;
  battleParticipantEpoch = 0;
  groundProps = undefined;
  groundDraftKey = undefined;
  if (scene.openGroundPropsURL) {
    if (WORLD_PROFILE.id !== "open-ground" || !mmorpgWorld)
      throw Error("Open these decorations in the grass world.");
    groundProps = await loadGroundProps(scene.openGroundPropsURL, { fetcher: nativeFetch });
    groundDraftKey = "ground-props:" + scene.openGroundPropsURL;
    saved = false;
  }
  kernel = published?.kernelBytes
    ? await Kernel.fromModules("baseline", published.kernelBytes)
    : await Kernel.create();
  if (published) lastWorkingShader = renderer.workingShaderSource();
  bindRuntimeOwners();
  renderer.resize(width, height);
  activateCollision(environment.mode);
  if (project) {
    environment = validateEnvironment(project.environment);
    publishEnvironment();
  }
  camera.eye = scene.spawn.position as V3;
  Object.assign(camera, viewAngles(camera.eye, scene.spawn.target as V3));
  const spawn = published?.world?.spawn ?? {
    position: [camera.eye[0], camera.eye[1] - 1.62, camera.eye[2]],
    yaw: camera.yaw,
    pitch: camera.pitch,
    mode,
  };
  projectSpawn = structuredClone(spawn);
  const starterColliders =
    published?.world?.colliders ??
    (WORLD_PROFILE.id === "starter-courtyard"
      ? await readAsset("/starter/colliders.json", (r) => {
          if (!r.ok) throw new Error("Prepare the courtyard collision.");
          return r.json();
        })
      : []);
  let initial = checkResult(
    kernel.request({
      type: "init",
      entities: published?.world?.entities ?? project?.entities ?? groundProps?.entities ?? [],
      colliders: project?.colliders ?? starterColliders,
      rooms: published?.world?.rooms ?? project?.rooms ?? [],
      groundY: WORLD_PROFILE.groundY,
      spawn,
    }),
  );
  if (quickCamera && !mmorpgWorld)
    initial = checkResult(
      kernel.request({
        type: "set_character_controller",
        controller: {
          profile: "quick-mmorpg-v1",
          worldScale: quickCamera.worldScale,
        },
      }),
    );
  snapshot = initial;
  if (combatEnabled && quickCamera) {
    const ready = checkResult(
      kernel.request({
        type: "combat_init",
        worldScale: quickCamera.worldScale,
        zombies: zombieSpawns(initial.player.position, initial.player.yaw),
      }),
    );
    syncCombat(ready.combat);
  }
  updatePlayer(initial.player, 0);
  if (!WORLD_PROFILE.readOnly && (WORLD_PROFILE.id !== "open-ground" || groundProps)) {
    if (!scene.catalog)
      throw new Error(
        "Prepare the private asset catalog, then reopen the world.",
      );
    let compiled = published?.catalog;
    if (!compiled && groundProps) compiled = await loadGroundCatalog(scene.catalog, { fetcher: nativeFetch });
    if (!compiled) {
      compiled = await readAsset(scene.catalog, async (catalogResponse) => {
        if (!catalogResponse.ok)
          throw new Error("The private asset catalog could not open.");
        return (await catalogResponse.json()) as CompiledCatalog;
      });
    }
    if (compiled.schema !== "aethyrforge.private-catalog.v1")
      throw new Error("Prepare the catalog with the current asset tools.");
    for (const asset of compiled.assets) {
      if (!scene.meshes[asset.mesh])
        throw new Error("An asset is missing its prepared geometry.");
      catalog.set(asset.id, asset);
      assetMeshes.set(asset.id, asset.mesh);
      assets.push({
        id: asset.id,
        name: asset.name,
        category: groundProps ? "Haunted props" : asset.kind === "building" ? "Houses" : "Town props",
      });
    }
    if (groundProps) validateGroundDraft(groundProps, catalog);
    try {
      const cached = published || playerMode.active || import.meta.env.VITE_PUBLIC_DEMO === "1" ? undefined : await loadProject();
      const restored = cached && groundProps ? validateGroundDraft(cached, catalog) : cached;
      if (restored) {
        snapshot = checkResult(
          kernel.request({
            type: "init",
            ...restored,
            colliders: project?.colliders ?? starterColliders,
            groundY: WORLD_PROFILE.groundY,
            spawn,
          }),
        );
        updatePlayer(snapshot.player, 0);
        if (groundProps) saved = true;
      }
    } catch (e) {
      report(
        "The saved draft could not open. You can still explore the original world.",
      );
    }
    await renderer.updateAuthored(snapshot, assetMeshes, catalog);
  }
  if (mmorpgWorld) {
    mmorpgSession = new MmorpgSession({
      url: import.meta.env.VITE_MMORPG_URL ?? "ws://127.0.0.1:55319/v1/worlds/open-ground/ws",
      worldScale: mmorpgWorld.worldScale, release: mmorpgWorld.simulation.release, icons: mmorpgIcons,
      kernel,
      changed: value => {
        mmorpgPresentation = value;
        if (value.connection !== "online") {
          input = { forward: 0, right: 0, up: 0, sprint: false, jump: false };
          renderer?.setMmorpgActors([]);
        }
        status = { ...status, mmorpg: value };
        send({ type: "status", status });
      },
    });
    mmorpgPresentation = mmorpgSession.state;
    status = { ...status, mmorpg: mmorpgPresentation };
  }
  if (overview) setShot("MT-04");
  else if (WORLD_PROFILE.id === "medieval-town-lab") {
    const grade = referenceCameraLook("MT-02", environment.mode);
    if (grade) environment = { ...environment, ...grade };
    publishEnvironment();
    updatePlayer(checkResult(kernel.request({ type: "snapshot" })).player, 0);
  }
  initializeGameplay(environment.mode);
  if (published?.world?.game) game = new StarterGame(published.world.game);
  else syncGame();
  renderer.render(camera, environment, animationClock.time);
  await renderer.device.queue.onSubmittedWorkDone();
  animationClock.start(performance.now());
  firstControlMs = performance.now() - started;
  gpuPeakBytes = renderer.gpuBytes;
  status = {
    ...status,
    phase: "ready",
    mmorpgEditing,
    view: viewCheckpoint(),
    character: characterStatus(),
    combat: combatState,
    combatRendering: combatEnabled ? renderer.enemyStats : undefined,
    groundGrass: renderer.groundGrassStats,
    sceneEffects,
    effectPreview: renderer.effectPreviewStats,
    gameplay: gameplaySummary(),
    runtimeTables: renderer.runtimeTableInfo(),
    kernelWork: snapshot.runtimeWork,
    game: gameStatus(),
    streaming: streamingStatus(),
    runtimeMetrics: {
      firstControlMs,
      preparationMs,
      gpuAccountedBytes: renderer.gpuBytes,
      frameAttempts: liveFrameAttempts,
      submittedFrames: liveSubmittedFrames,
      skippedFrames: liveSkippedFrames,
      kernelDroppedSeconds,
      hostClampedSeconds,
    },
    message: combatEnabled
      ? "W/S move · A/D turn · Space attack"
      : "Click the world to explore · WASD · Space jump · Shift run · V view",
    instances: scene.instances.length,
    triangles: renderer.triangles,
    saved: WORLD_PROFILE.readOnly || WORLD_PROFILE.id === "medieval-town-lab" || !!(groundProps && saved),
  };
  send({
    type: "ready",
    assets,
    status,
    hasSourceSky: renderer.hasSourceSky,
    sourceSkyLightIntensity: sourceSkyLightIntensity(),
    shaderSource: lastWorkingShader,
  });
  frameId = scope.requestAnimationFrame(draw);
}
function viewCheckpoint() {
  const active = renderer?.battleInfo()?.enabled ? lastBattleSubmittedCamera : undefined;
  const eye = active?.eye ?? (mode === "walk" && player && !quickCamera ? player.eye : camera.eye);
  const ray = direction(active?.yaw ?? camera.yaw, active?.pitch ?? camera.pitch);
  return {
    eye: [...eye] as V3,
    target: add(eye, ray),
    mode,
    animationTime: animationClock.time,
  };
}
let renderedEnvironment: EngineStatus["renderedEnvironment"];
let runtimeEditMetrics: Record<string, number> | undefined;
function effectiveEnvironment() {
  const recipe =
    WORLD_PROFILE.id === "personal-planet"
      ? authoring.planetSources.current(snapshot)
      : undefined;
  return recipe
    ? planetEnvironment(recipe, environment, Date.now())
    : environment;
}
function draw(now: number) {
  const callbackEnteredAt = cTownMetrics.recording
    ? performance.now()
    : undefined;
  if (disposed || !runtimeOwnersReady || !renderer || !kernel) return;
  runtimeOwnership.run("frame", (access) =>
    access
      .write("kernel")
      .withView((ownedKernel) =>
        access
          .write("renderer")
          .withView((ownedRenderer) =>
            drawFrame(now, ownedKernel, ownedRenderer, callbackEnteredAt),
          ),
      ),
  );
}
function drawFrame(
  now: number,
  kernel: Kernel,
  renderer: Renderer,
  callbackEnteredAt?: number,
) {
  if (disposed || !renderer || !kernel) return;
  liveRawCallbacks++;
  if (filmState.manual || !filmState.liveAllowed) {
    liveSuspendedCallbacks++;
    cTownMetrics.observeCallback(null);
    if (!filmState.manual) frameId = scope.requestAnimationFrame(draw);
    return;
  }
  try {
    const due = framePacer.due(now);
    livePacedCallbacks += Number(!due);
    cTownMetrics.observeCallback(due);
    if (!due) {
      frameId = scope.requestAnimationFrame(draw);
      return;
    }
    const elapsed = previous ? Math.min((now - previous) / 1000, 0.1) : 1 / 60;
    const observedInterval = previous ? now - previous : 1000 / 60;
    previous = now;
    if (yawDelta || pitchDelta) camera.up = undefined;
    const measured = cTownMetrics.recording && cTownMetrics.instrumented;
    liveFrameAttempts++;
    hostClampedSeconds += Math.max(0, observedInterval / 1000 - elapsed);
    const stepStarted = measured ? performance.now() : 0;
    const wasAttacking = quickLocomotion?.animation === "attack";
    const sourceStep =
      mode === "walk"
        ? quickLocomotion?.advance(
            {
              forward: input.forward,
              turn: input.right,
              sprint: input.sprint,
              attack: input.attack === true,
              dance: input.dance === true,
            },
            elapsed,
          )
        : undefined;
    if (
      combatEnabled &&
      sourceStep?.animation === "attack" &&
      !wasAttacking
    ) {
      syncCombat(
        checkResult(
          kernel.request({
            type: "combat_attack",
            attackId: `sword-${++combatAttackSequence}`,
          }),
        ).combat,
      );
    }
    const characterElapsed = sourceStep?.elapsed ?? elapsed;
    hostClampedSeconds += elapsed - characterElapsed;
    const sharedFrame = mmorpgSession?.step(elapsed, {
      forward: mmorpgEditing || battleParticipant || battleSpectating() ? 0 : input.forward,
      turn: mmorpgEditing || battleParticipant || battleSpectating() ? 0 : input.right,
      sprint: !mmorpgEditing && !battleParticipant && !battleSpectating() && input.sprint,
      attack: !mmorpgEditing && !battleParticipant && !battleSpectating() && input.attack === true,
      dance: !mmorpgEditing && !battleParticipant && !battleSpectating() && input.dance === true,
    });
    const state = mmorpgSession ? snapshot : stepRuntime(kernel, characterElapsed, {
      ...input,
      turn: quickCamera && mode === "walk" ? input.right : 0,
      movementLocked: sourceStep?.movementLocked ?? false,
      yawDelta: quickCamera && mode === "walk" ? 0 : yawDelta,
      pitchDelta: quickCamera && mode === "walk" ? 0 : pitchDelta,
      jump: quickCamera && mode === "walk" ? false : jumpInput.sample(),
    });
    const bridge = measured ? kernel.lastRequestTimings : null;
    kernelDroppedSeconds = state.droppedSeconds ?? 0;
    syncGameplay(state.gameplay);
    syncCombat(state.combat);
    let localBattlePose: BattleParticipantSnapshot | undefined;
    if (battleParticipant) {
      if (battleScriptInput && performance.now() > battleScriptInput.until) {
        battleScriptInput = undefined;
        battleFollowCamera.cancelAttack();
      }
      const controls = battleFollowing() ? battleScriptInput ?? { forward: input.forward, turn: input.right, sprint: input.sprint, attack: input.attack === true } : { forward: 0, turn: 0, sprint: false, attack: false };
      const gpu = renderer.battleInfo()?.gpu;
      const before = battleParticipant.snapshot!;
      const meteorTarget = battleAttack === "meteor_shower" && battleFollowing() && battleField
        ? battleMeteorTarget(battleField, battleCamera(renderer.battleInfo()!.center, renderer.battleInfo()!.radius, battleView, 0, before, battleFollowCamera.orbit), width / height, aimPointer, before.eye, battleView !== "first")
        : undefined;
      const meteorReady = !!meteorTarget && battleParticipantReady() && gpu?.meteorCooldownTicks === 0 && !gpu.participantDead;
      let attack = battleAttack === "meteor_shower" ? (controls.attack || battleMeteorPending) && meteorReady : controls.attack;
      let bodyYaw = battleFollowing() ? Math.max(-Math.PI, Math.min(Math.PI, yawDelta)) : 0;
      let bodyPitch = battleFollowing() ? Math.max(-Math.PI, Math.min(Math.PI, pitchDelta)) : 0;
      if (battleFollowing() && battleView !== "first") {
        if (!before.alive || !battleParticipantReady() || before.state === "attack" || (battleAttack === "meteor_shower" && !meteorReady))
          battleFollowCamera.cancelAttack();
        const aim = battleFollowCamera.aimAttack(before.yaw, elapsed, attack && before.state !== "attack" && before.alive && battleParticipantReady(), performance.now());
        bodyYaw = aim.yawDelta; bodyPitch = 0; attack = aim.attack;
      }
      localBattlePose = battleParticipant.advance(elapsed, { forward: controls.forward, turn: controls.turn, sprint: controls.sprint, attack, dance: false },
        !battleParticipantReady() || !gpu?.participantDead,
        bodyYaw, bodyPitch,
        battleCrowdContext());
      if (battleView === "first") battleFollowCamera.followFirst(localBattlePose);
      if (localBattlePose.state === "attack") battleFollowCamera.cancelAttack();
      if (!meteorReady || localBattlePose.state === "attack") battleMeteorPending = false;
    }
    yawDelta = 0;
    pitchDelta = 0;
    if (sharedFrame) {
      const { local, prediction } = sharedFrame;
      const others = sharedFrame.actors.sort((a,b) => Math.hypot(a.position[0]-local.position[0],a.position[2]-local.position[2])-Math.hypot(b.position[0]-local.position[0],b.position[2]-local.position[2])).slice(0,63);
      if (!localBattlePose) renderer.setMmorpgActors([local, ...others], local.id, mmorpgSession!.state.events, mmorpgSession!.tick);
      mmorpgPresentation = mmorpgSession!.presentation(local, others);
      updatePlayer({ position: local.position, eye: [local.position[0], local.position[1] + 1.62, local.position[2]],
        yaw: local.yaw, pitch: 0, grounded: prediction.grounded,
        velocity: [Math.sin(local.yaw)*prediction.velocity,prediction.verticalSpeed,-Math.cos(local.yaw)*prediction.velocity],
        speed: Math.abs(prediction.velocity), ahead: input.forward, side: 0 }, elapsed);
    } else if (!mmorpgSession && state.player) {
      if (
        mode === "walk" &&
        stream &&
        player &&
        !walkAllowed(state.player.position)
      ) {
        const held = checkResult(
          kernel.request({
            type: "set_camera",
            position: player.position,
            yaw: state.player.yaw,
            pitch: state.player.pitch,
            mode,
          }),
        );
        updatePlayer(held.player, characterElapsed);
      } else updatePlayer(state.player, characterElapsed);
    }
    if (localBattlePose) {
      const health = participantStatus()!.health;
      const hero = { id: 0xfffffffe, name: "You", class: localBattlePose.class,
        position: localBattlePose.position, yaw: localBattlePose.yaw, state: localBattlePose.state,
        actionTime: localBattlePose.actionTime, animationTime: localBattlePose.animationTime,
        health, maxHealth: 200, alive: localBattlePose.alive,
        weapon: localBattlePose.class === "paladin" ? "weapon.sword1" as const : null };
      renderer.setMmorpgActors([hero], hero.id, [], localBattlePose.tick, battleView === "first");
    }
    const stepMs = measured ? performance.now() - stepStarted : null;
    let renderCamera = camera;
    if (
      cTownMetrics.recording &&
      cTownMetrics.route === "sway" &&
      cTownCamera
    ) {
      const phase = ((performance.now() - cTownStarted) / 20_000) * Math.PI * 2;
      const shift = Math.sin(phase);
      renderCamera = {
        ...cTownCamera,
        eye: [
          cTownCamera.eye[0] + Math.cos(cTownCamera.yaw) * shift,
          cTownCamera.eye[1] + Math.sin(phase * 2) * 0.15,
          cTownCamera.eye[2] + Math.sin(cTownCamera.yaw) * shift,
        ],
        yaw: cTownCamera.yaw + Math.sin(phase) * 0.12,
        pitch: cTownCamera.pitch + Math.sin(phase * 2) * 0.025,
      };
    }
    const activeBattle = renderer.battleInfo();
    if (activeBattle?.enabled && (battleView !== "player" || localBattlePose))
      renderCamera = battleCamera(activeBattle.center, activeBattle.radius, battleView,
        (performance.now() - battleStarted) / 1000, localBattlePose, battleFollowCamera.orbit);
    battleTarget = localBattlePose && localBattlePose.alive && battleFollowing() && battleField && battleAttack === "meteor_shower"
      ? battleMeteorTarget(battleField, renderCamera, width / height, aimPointer, localBattlePose.eye, battleView !== "first")
      : undefined;
    if (localBattlePose) {
      const ray = direction(renderCamera.yaw, renderCamera.pitch);
      renderer.setBattleParticipant({ active: true, position: localBattlePose.position,
        yaw: localBattlePose.yaw, team: battleTeam, attackSequence: localBattlePose.attackSequence,
        attack: battleAttack, aim: battleAttack === "meteor_shower" ? battleTarget?.point ?? localBattlePose.eye : add(renderCamera.eye, scale(ray, 90)),
        meteorTargetValid: !!battleTarget, firstPerson: battleView === "first" });
    }
    const renderedSettings = effectiveEnvironment();
    updateGameplayAim(renderCamera);
    if (battleParticipant) renderer.setGameplayAim(battleTarget ? {point: battleTarget.point, normal: battleTarget.normal, radius: 4, overlay: true} : undefined);
    const renderStarted = measured ? performance.now() : 0;
    const frameTime = animationClock.sample(performance.now());
    const submitted = renderer.render(
      renderCamera,
      renderedSettings,
      frameTime,
    );
    if (submitted) lastBattleSubmittedCamera = activeBattle?.enabled ? renderCamera : undefined;
    liveSubmittedFrames += Number(submitted);
    liveSkippedFrames += Number(!submitted);
    if (cTownMetrics.recording)
      cTownMetrics.record({
        frame: renderer.frame,
        at: now,
        callbackEnteredAt,
        elapsedMs: observedInterval,
        stepMs,
        renderCpuMs: measured ? performance.now() - renderStarted : null,
        cullMs: measured ? renderer.cullCpuMs : null,
        clusterMs: measured ? renderer.clusterCpuMs : null,
        bridgeEncodeMs: bridge?.encodeMs ?? null,
        wasmRequestMs: bridge?.wasmRequestMs ?? null,
        bridgeDecodeMs: bridge?.decodeMs ?? null,
        bridgeOverheadMs: bridge?.overheadMs ?? null,
        commandEncodeMs: measured ? renderer.commandEncodeMs : null,
        kernelDroppedSeconds,
        hostClampedSeconds,
        cameraEye: [...renderCamera.eye],
        cameraYaw: renderCamera.yaw,
        cameraPitch: renderCamera.pitch,
        animationTime: frameTime,
        submittedInstances: submitted
          ? renderer.runtimeInspection().submittedInstances
          : 0,
        submittedTriangles: submitted
          ? renderer.runtimeInspection().submittedTriangles
          : 0,
        ...(submitted && cTownMetrics.instrumented
          ? { sourceRenderCommands: renderer.sourceCommandCounts() }
          : {}),
        submitted,
        gpuMs: null,
      });
    if (submitted) {
      if (WORLD_PROFILE.id === "personal-planet")
        renderedEnvironment = {
          frame: renderer.frame,
          atUnixMs: Date.now(),
          settings: renderedSettings,
        };
      if (lastSubmission) frameTimes.push((now - lastSubmission) / 1000);
      lastSubmission = now;
      if (frameTimes.length > 90) frameTimes.shift();
    }
    if (now - reportTime > 200) {
      reportTime = now;
      updateGameplayAim();
      status = {
        ...status,
        phase: "ready",
        frame: renderer.frame,
        fps: frameTimes.length
          ? Math.round(
              frameTimes.length / frameTimes.reduce((a, b) => a + b, 0),
            )
          : 0,
        ...(renderedEnvironment ? { renderedEnvironment } : {}),
        ...(WORLD_PROFILE.id === "personal-planet" &&
        snapshot.authoring?.planetSource
          ? {
              personalWorld: {
                recipeSha256: snapshot.authoring.planetSource.sha256,
                entities: Object.fromEntries(
                  (snapshot.entities ?? []).map((e: any) => [
                    e.id,
                    {
                      asset: e.kind,
                      position: e.position,
                      yaw: e.yaw,
                      scale: e.scale,
                    },
                  ]),
                ),
              },
            }
          : {}),
        revision: snapshot.revision ?? 0,
        camera: camera.eye,
        view: viewCheckpoint(),
        character: characterStatus(),
        mmorpg: mmorpgPresentation,
        combat: combatState,
        combatRendering: combatEnabled ? renderer.enemyStats : undefined,
        groundGrass: renderer.groundGrassStats,
        sceneEffects,
        effectPreview: renderer.effectPreviewStats,
        gameplay: gameplaySummary(),
        spellSwarm: spellSwarmStatus(),
        battle: battleStatus(),
        runtimeTables: renderer.runtimeTableInfo(),
        kernelWork: snapshot.runtimeWork,
        runtimeEditMetrics,
        game: gameStatus(),
        streaming: streamingStatus(),
        runtimeMetrics: {
          firstControlMs,
          preparationMs,
          gpuAccountedBytes: renderer.gpuBytes,
          frameAttempts: liveFrameAttempts,
          submittedFrames: liveSubmittedFrames,
          skippedFrames: liveSkippedFrames,
          kernelDroppedSeconds,
          hostClampedSeconds,
          ...(mmorpgSession ? {
            mmorpgActors: renderer.mmorpgStats.actors,
            mmorpgClasses: renderer.mmorpgStats.classes,
            mmorpgTriangles: renderer.mmorpgStats.triangles,
            mmorpgParticles: renderer.mmorpgEffectStats?.particles ?? 0,
            mmorpgParticleDraws: renderer.mmorpgEffectStats?.drawCalls ?? 0,
            mmorpgTerrainUpdates: renderer.mmorpgTerrainStats?.updates ?? 0,
            mmorpgNameLabels: renderer.mmorpgStats.nameLabels,
            mmorpgNameTriangles: renderer.mmorpgStats.nameTriangles,
            mmorpgNameDrawCalls: renderer.mmorpgStats.nameDrawCalls,
            mmorpgNameTextureBytes: renderer.mmorpgStats.nameTextureBytes,
            mmorpgNameTextureLayers: renderer.mmorpgStats.nameTextureLayers,
            mmorpgNameUploads: renderer.mmorpgStats.nameUploads,
          } : {}),
        },
        authoring: ["haunted-house", "personal-planet"].includes(
          WORLD_PROFILE.id,
        )
          ? authoring.view()
          : undefined,
        canUndo: !!snapshot.canUndo,
        canRedo: !!snapshot.canRedo,
        saved,
        selection: selectionSummary(),
        mmorpgEditing,
      };
      send({ type: "status", status });
    }
  } catch (e) {
    status = { ...status, phase: "error", message: errorMessage(e) };
    send({ type: "error", message: status.message });
    return;
  }
  frameId = scope.requestAnimationFrame(draw);
}
function errorMessage(error: unknown) {
  return (error instanceof Error ? error.message : String(error)).slice(
    0,
    2048,
  );
}
function selectionSummary() {
  const entity = snapshot.entities?.find((e: any) => e.id === selected);
  return entity
    ? {
        id: entity.id,
        name: entity.kind,
        kind: entity.kind,
        position: entity.position,
        scale: entity.scale ?? 1,
      }
    : null;
}
let baseAuthorEnvironment: typeof environment | undefined;
const authoring = new ProjectAuthoring({
  restoreCamera: (value) => {
    const pose = value as { eye: V3; target: V3; mode: "walk" | "fly" };
    if (WORLD_PROFILE.id === "personal-planet" && !planetCameraAllowed(pose))
      return projectSpawn;
    return {
      position: [pose.eye[0], pose.eye[1] - 1.62, pose.eye[2]],
      ...viewAngles(pose.eye, pose.target),
      mode: pose.mode,
    };
  },
  fallbackCamera: () => projectSpawn,
  namespace: () =>
    WORLD_PROFILE.id === "personal-planet"
      ? "personal-planet"
      : "haunted-house",
  template: () => {
    if (
      !projectTemplate ||
      !/^[a-f0-9]{64}$/.test(projectTemplate.asset_set_sha256) ||
      !/^[a-f0-9]{64}$/.test(projectTemplate.scene_sha256)
    )
      throw new Error("This engine release has no editable project identity.");
    return projectTemplate;
  },
  restoreEntity: (id, entity) => {
    const asset = catalog.get(entity.asset);
    if (!asset || !assetMeshes.has(entity.asset))
      throw new Error("This save refers to an unavailable asset.");
    return {
      id,
      kind: entity.asset,
      position: entity.position,
      yaw: entity.yaw,
      scale: entity.scale,
      collidable: true,
      halfExtents: asset.bounds.max.map((v, i) =>
        Math.max(0.01, (v - asset.bounds.min[i]) / 2),
      ),
      collisionOffset: asset.bounds.max.map(
        (v, i) => (v + asset.bounds.min[i]) / 2,
      ),
      ...(asset.plan ? { buildingPlan: asset.plan } : {}),
    };
  },
  snapshot: () => snapshot,
  kernel: (request) => {
    if (
      request &&
      typeof request === "object" &&
      "type" in request &&
      ["command", "init"].includes(String(request.type))
    )
      return runtimeOwnership.safePoint(() =>
        runtimeOwnership.run("activate", (access) =>
          access
            .write("kernel")
            .withView((owned) => owned.request(request as object)),
        ),
      );
    return kernel!.request(request as object);
  },
  materialIds: () => renderer?.materialBindings(snapshot, assetMeshes) ?? [],
  hasAsset: (id) => assetMeshes.has(id),
  rendererSource: MATERIAL,
  shader: (source, current) => renderer!.replaceShader(source, current),
  prepare: async (candidate, source, current) => {
    renderer!.creatorImageProvider = (document) =>
      authoring.materialImages.bundle(document);
    const geometry = await renderer!.prepareAuthored(
      candidate,
      assetMeshes,
      catalog,
    );
    try {
      const shader =
        source === undefined
          ? undefined
          : await renderer!.prepareShader(source, current);
      if (!current() || (source !== undefined && !shader))
        throw new Error("A newer edit replaced this compilation.");
      return {
        ready: () =>
          current() && geometry.ready() && (!shader || shader.ready()),
        activate: () =>
          runtimeOwnership.safePoint(() =>
            runtimeOwnership.run("activate", (access) =>
              access.write("renderer").withView(() => {
                shader?.activate();
                if (shader && source !== undefined) lastWorkingShader = source;
                geometry.activate();
              }),
            ),
          ),
        discard: () => geometry.discard(),
      };
    } catch (error) {
      geometry.discard();
      throw error;
    }
  },
  activate: async (accepted, prepared) => {
    snapshot = accepted;
    baseAuthorEnvironment ??= { ...environment };
    environment = validateEnvironment({
      ...baseAuthorEnvironment,
      ...snapshot.authoring.environment,
    });
    publishEnvironment();
    renderer!.applyMaterialValues(snapshot.authoring.materials);
    if (!prepared)
      await renderer!.updateAuthored(snapshot, assetMeshes, catalog);
    saved = false;
  },
});
let materialViewSerial = 0,
  materialPreviewActive = false;
async function clearMaterialPreview() {
  if (materialPreviewActive && renderer) {
    await renderer.updateAuthored(snapshot, assetMeshes, catalog);
    materialPreviewActive = false;
  }
}
async function materialView(
  message: Extract<PreviewMessage, { type: "materialView" }>,
) {
  if (!renderer || !kernel) throw new Error("Wait for the world to open.");
  const ticket = (message as any).ticket;
  const current = () =>
    ticket === materialViewSerial && message.base === snapshot.revision;
  if (!current())
    throw new Error("The world changed. Review the current face material.");
  const view = message.view;
  if (view.mode === "clear") {
    await clearMaterialPreview();
    return { frame: renderer.frame };
  }
  const target =
    view.mode === "inspect" ? view.target : view.material.document.target;
  checkMaterialTarget(target, snapshot);
  if (view.mode === "inspect") {
    const document = savedMaterialDocuments(snapshot).find(
      (d) =>
        d.target.entity === target.entity &&
        d.target.surface === target.surface,
    ) ?? {
      schema: "openagents.verse.material.v1",
      target,
      generated: { image: null, visible: true, strength: 1 },
      handmade: { image: null, visible: true, strength: 1 },
      recipe: null,
    };
    return {
      material: authoring.materialImages.bundle(document),
      frame: renderer.frame,
    };
  }
  if (selected !== target.entity)
    throw new Error("Select this werewolf before previewing its face.");
  await authoring.materialImages.accept(view.material, snapshot);
  const key = await materialKey(target),
    source = await materialSource(view.material.document);
  const candidate = structuredClone(snapshot);
  (candidate.authoring.materialSources ??= {})[key] = source;
  renderer.creatorImageProvider = (d) => authoring.materialImages.bundle(d);
  const geometry = await renderer.prepareAuthored(
    candidate,
    assetMeshes,
    catalog,
  );
  try {
    if (!current() || selected !== target.entity)
      throw new Error(
        "The selection changed. Your applied material is still here.",
      );
    geometry.activate();
    materialPreviewActive = true;
    const adopted = renderer.frame;
    await new Promise<void>((resolve, reject) => {
      const deadline = performance.now() + 5000;
      const check = () => {
        if (!current()) {
          reject(new Error("A newer request replaced this face preview."));
          return;
        }
        if (renderer!.frame > adopted) {
          resolve();
          return;
        }
        if (performance.now() > deadline) {
          reject(
            new Error("The face preview is waiting for a rendered frame."),
          );
          return;
        }
        setTimeout(check, 16);
      };
      check();
    });
    await renderer.device.queue.onSubmittedWorkDone();
    if (!current())
      throw new Error("A newer request replaced this face preview.");
    return { frame: renderer.frame };
  } catch (error) {
    if (current()) await clearMaterialPreview();
    throw error;
  } finally {
    geometry.discard();
  }
}
async function command(command: EditorCommand, requestId: string) {
  if (!kernel || !renderer) throw new Error("Wait for the world to open.");
  if (mmorpgSession) {
    if (!mmorpgEditing || !groundProps)
      throw Error("Choose Edit before changing these props.");
    if (command.kind === "room.add" || command.kind === "wall.remove")
      throw Error("This clearing supports prop edits. Choose a prop to change.");
  }
  if (command.kind === "entity.select") {
    if (command.id && !snapshot.entities?.some((e: any) => e.id === command.id))
      throw new Error("Choose an object in this world.");
    selected = command.id;
    receipt(
      requestId,
      true,
      selected ? "Object selected" : "Selection cleared",
    );
    return;
  }
  let operations: any[] = [];
  let message = "Edit applied";
  if (command.kind === "prop.add") {
    const asset = catalog.get(command.assetId);
    if (
      !asset ||
      asset.kind === "building" ||
      !assetMeshes.has(command.assetId)
    )
      throw new Error("Choose an available prop.");
    if (!/^[a-zA-Z0-9][a-zA-Z0-9._:-]{0,95}$/.test(command.id))
      throw new Error(
        "Give the object a name of up to 96 letters, numbers, dots, or dashes.",
      );
    operations = [
      {
        op: "add_entity",
        entity: {
          id: command.id,
          kind: command.assetId,
          position: command.position,
          scale: command.scale ?? 1,
          yaw: 0,
          collidable: !mmorpgSession,
          halfExtents: asset.bounds.max.map((v, i) =>
            Math.max(0.01, (v - asset.bounds.min[i]) / 2),
          ),
          collisionOffset: asset.bounds.max.map(
            (v, i) => (v + asset.bounds.min[i]) / 2,
          ),
        },
      },
    ];
    message = `Added ${asset.name.toLowerCase()}`;
  } else if (command.kind === "asset.add") {
    if (
      !assetMeshes.has(command.assetId) ||
      !Number.isInteger(command.count) ||
      command.count < 1 ||
      command.count > 8
    )
      throw new Error("Choose an available asset and a count from 1 to 8.");
    const asset = catalog.get(command.assetId)!;
    const spacing = Math.max(
        2,
        Math.max(asset.dimensions[0], asset.dimensions[2]) + 1,
      ),
      distance =
        asset.kind === "building"
          ? Math.max(12, asset.dimensions[2] / 2 + 5)
          : 5;
    const f = direction(camera.yaw, 0),
      r: V3 = [Math.cos(camera.yaw), 0, Math.sin(camera.yaw)],
      offset =
        command.placement === "left"
          ? scale(r, -distance)
          : command.placement === "right"
            ? scale(r, distance)
            : scale(f, distance);
    for (let i = 0; i < command.count; i++) {
      const p = add(
        add(mode === "walk" && player ? player.eye : camera.eye, offset),
        scale(r, (i - (command.count - 1) / 2) * spacing),
      );
      p[1] =
        (mmorpgSession ? mmorpgHeight(p[0], p[2]) : mode === "walk" && player
          ? player.position[1]
          : WORLD_PROFILE.groundY) - asset.bounds.min[1];
      operations.push({
        op: "add_entity",
        entity: {
          id: crypto.randomUUID(),
          kind: command.assetId,
          position: p,
          yaw: -camera.yaw,
          scale: 1,
          collidable: !mmorpgSession,
          halfExtents: asset.bounds.max.map((v, i) =>
            Math.max(0.01, (v - asset.bounds.min[i]) / 2),
          ),
          collisionOffset: asset.bounds.max.map(
            (v, i) => (v + asset.bounds.min[i]) / 2,
          ),
          ...(asset.plan ? { buildingPlan: asset.plan } : {}),
        },
      });
    }
    message = `Added ${command.count} ${asset.name.toLowerCase()}${command.count > 1 ? "s" : ""}`;
  } else if (command.kind === "entity.remove") {
    operations = [{ op: "remove_entity", id: command.id }];
    message = "Object removed";
  } else if (command.kind === "entity.transform") {
    operations = [
      {
        op: "set_entity_transform",
        id: command.id,
        position: command.position,
        scale: command.scale,
      },
    ];
    message = "Object updated";
  } else if (command.kind === "room.add") {
    const a =
      camera.yaw +
      (command.side === "left"
        ? -Math.PI / 2
        : command.side === "right"
          ? Math.PI / 2
          : 0);
    const p = add(
      mode === "walk" && player ? player.eye : camera.eye,
      scale(direction(a, 0), command.size + 2),
    );
    operations = [
      {
        op: "add_room",
        room: {
          id: crypto.randomUUID(),
          center: [p[0], p[2]],
          size: [command.size, command.size],
          floorY: WORLD_PROFILE.groundY,
          height: 3,
        },
      },
    ];
    message = "Editable room added";
  } else if (command.kind === "wall.remove") {
    const ray = direction(camera.yaw, camera.pitch);
    let hit: any,
      distance = 16;
    for (const wall of snapshot.wallColliders ?? []) {
      const t = rayBox(
        camera.eye,
        ray,
        { min: wall.localMin ?? wall.min, max: wall.localMax ?? wall.max },
        wall.transform,
      );
      if (t !== null && t < distance) {
        distance = t;
        hit = wall;
      }
    }
    if (!hit)
      throw new Error("Aim at an editable house or room wall within16meters.");
    operations = [{ op: "remove_wall", wallId: hit.id }];
    message = "Wall removed";
  } else if (
    command.kind === "history.undo" ||
    command.kind === "history.redo"
  ) {
    operations = [{ op: command.kind === "history.undo" ? "undo" : "redo" }];
    message = command.kind === "history.undo" ? "Edit undone" : "Edit redone";
  } else if (command.kind === "text") {
    return commandAction(
      parseCommand(command.text, selectionSummary()),
      requestId,
    );
  }
  const result = await commitOperations(operations, requestId);
  snapshot = result;
  syncGame();
  selected = operations[0]?.entity?.id ?? selected;
  // Graphics are updated from the same accepted snapshot; the loop remains active.
  saved = false;
  try {
    await saveProject(snapshot);
    saved = ["medieval-town-lab", "starter-courtyard", "open-ground"].includes(
      WORLD_PROFILE.id,
    );
  } catch {
    message += " · Your edit is visible, but this draft could not save.";
  }
  receipt(requestId, true, message);
}
const commandAction = command;
async function commitOperations(operations: any[], requestId: string) {
  const started = performance.now();
  const transaction = {
    commandId: requestId,
    baseRevision: snapshot.revision ?? 0,
    operations,
  };
  const candidate = checkResult(
    kernel!.request({ type: "prepare", ...transaction }),
  );
  if (groundProps && mmorpgSession) validateGroundDraft(candidate, catalog);
  candidateGame(candidate); // Validate gameplay before committing any representation.
  const kernelPrepared = performance.now();
  const prepared = await renderer!.prepareAuthored(
    candidate,
    assetMeshes,
    catalog,
  );
  const rendererPrepared = performance.now();
  try {
    if (!prepared.ready() || !agentRequestCurrent(requestId))
      throw new Error("A newer edit replaced this update.");
    let committed = rendererPrepared;
    const accepted = runtimeOwnership.safePoint(() =>
      runtimeOwnership.run("activate", (access) =>
        access.write("kernel").withView((ownedKernel) =>
          access.write("renderer").withView(() => {
            const accepted = checkResult(
              ownedKernel.request({ type: "command", ...transaction }),
            );
            committed = performance.now();
            prepared.activate();
            return accepted;
          }),
        ),
      ),
    );
    const activated = performance.now();
    runtimeEditMetrics = {
      prepareKernelMs: kernelPrepared - started,
      prepareRendererMs: rendererPrepared - kernelPrepared,
      commitKernelMs: committed - rendererPrepared,
      activationMs: activated - committed,
      totalMs: activated - started,
    };
    return accepted;
  } finally {
    prepared.discard();
  }
}
async function gameAction(
  action: "start" | "interact" | "reset",
  requestId: string,
) {
  if (!["start", "interact", "reset"].includes(action))
    throw new Error("Choose start, interact, or restart.");
  if (!["starter-courtyard", "medieval-town-lab"].includes(WORLD_PROFILE.id))
    throw new Error("Open the courtyard or Medieval Town to play this quest.");
  if (!player || !kernel || !renderer)
    throw new Error("Wait for the world to open.");
  let saveWarning = "";
  if (action === "start" && !game) {
    if (playerMode.active)
      throw new Error(
        "Open this quest in the creator to place its markers first.",
      );
    const p = player.position;
    const forward = direction(player.yaw, 0);
    const keyKind = catalog.has("key") ? "key" : "barrel";
    const exitKind = catalog.has("exit") ? "exit" : "bench";
    if (!catalog.has(keyKind) || !catalog.has(exitKind))
      throw new Error("Prepare the quest markers first.");
    const template =
      WORLD_PROFILE.id === "starter-courtyard"
        ? new StarterGame(
            await fetch("/starter/game.json").then((r) => r.json()),
          ).config()
        : undefined;
    const roles = [
      ["starter-key", keyKind, 2.5],
      ["starter-exit", exitKind, 7],
    ] as const;
    const operations = roles
      .filter(([id]) => !snapshot.entities.some((e: any) => e.id === id))
      .map(([id, kind, distance]) => ({
        op: "add_entity",
        entity: {
          id,
          kind,
          position: template
            ? id === "starter-key"
              ? template.key.position
              : template.exit.position
            : [
                p[0] + forward[0] * distance,
                p[1],
                p[2] + forward[2] * distance,
              ],
          yaw: 0,
          scale: 1,
          collidable: false,
        },
      }));
    snapshot = await commitOperations(operations, requestId);
    syncGame();
    saved = false;
    try {
      await saveProject(snapshot);
      saved = true;
    } catch {
      saveWarning = " Your quest is playable, but this draft could not save.";
    }
  }
  if (!game) throw new Error("Start the quest first.");
  if (action === "reset") {
    const spawn = projectSpawn as {
      position: V3;
      yaw: number;
      pitch: number;
      mode: "walk" | "fly";
    };
    const next = checkResult(
      kernel.request({ type: "set_camera", ...spawn, mode: "walk" }),
    );
    input = { forward: 0, right: 0, up: 0, sprint: false, jump: false };
    jumpInput.reset();
    mode = "walk";
    characterCamera.resetCollision();
    const result = game.apply({ type: "reset" }, next.player.position);
    if (!result.ok) throw new Error(result.message);
    updatePlayer(next.player, 0);
  } else if (action === "interact") {
    if (mode !== "walk") throw new Error("Switch to Walk to interact.");
    const result = game.apply({ type: "interact" }, player.position);
    if (!result.ok) throw new Error(result.message);
  }
  status = {
    ...status,
    game: gameStatus(),
    character: characterStatus(),
    view: viewCheckpoint(),
    revision: snapshot.revision,
    saved,
  };
  send({ type: "status", status });
  receipt(requestId, true, status.game!.message + saveWarning);
}
function cellContains(id: string, point: readonly number[]) {
  const cell = stream?.cell(id);
  return (
    !!cell &&
    point[0] - 0.3 >= cell.bounds.min[0] &&
    point[0] + 0.3 <= cell.bounds.max[0] &&
    point[2] - 0.3 >= cell.bounds.min[1] &&
    point[2] + 0.3 <= cell.bounds.max[1]
  );
}
function walkAllowed(point: readonly number[]) {
  return (
    !stream ||
    (stream.canEnter([point[0] - 0.3, point[2] - 0.3]) &&
      stream.canEnter([point[0] + 0.3, point[2] + 0.3]))
  );
}
async function streamAction(
  action: "load" | "cancel" | "unload",
  cell: "entry" | "town" | undefined,
  requestId: string,
) {
  if (!stream)
    throw new Error("Open a cooked streaming package to switch areas.");
  if (action === "cancel") {
    stream.cancel();
    receipt(requestId, true, "Area loading canceled");
    return;
  }
  if (action === "unload") {
    stream.unloadInactive();
    receipt(requestId, true, "Unused area data released");
    return;
  }
  if (action !== "load" || !cell) throw new Error("Choose an available area.");
  if (
    preparingDestruction ||
    gameplayState?.castle?.active ||
    gameplayState?.castle?.pendingImpacts
  )
    throw new Error(
      "Stop the castle barrage and let its strikes land before changing areas.",
    );
  if (mode === "walk" && player && !cellContains(cell, player.position))
    throw new Error("Return to the entry area before unloading the town.");
  streamLoading = true;
  status = { ...status, streaming: streamingStatus() };
  send({ type: "status", status });
  try {
    const result = await stream.load(cell, {
      prepare: async ({ cell, resources, signal }) => {
        let candidate: Awaited<ReturnType<typeof prepareWorld>> | undefined;
        let activated = false;
        stagedResources = resources;
        try {
          const started = performance.now();
          const day = JSON.parse(
            new TextDecoder().decode(resources.get(cell.scenes.day)!),
          ) as CompiledScene;
          const night = JSON.parse(
            new TextDecoder().decode(resources.get(cell.scenes.night)!),
          ) as CompiledScene;
          const decodeMs = performance.now() - started;
          if (
            day.collisionURL !== scene.collisionURL ||
            day.gameplayURL !== scene.gameplayURL ||
            night.collisionURL !== activeNightScene?.collisionURL ||
            night.gameplayURL !== activeNightScene?.gameplayURL ||
            day.vertices !== scene.vertices ||
            day.indices !== scene.indices
          )
            throw new Error(
              "These areas need matching walking surfaces and geometry.",
            );
          candidate = await prepareWorld(
            day,
            night,
            new OffscreenCanvas(width, height),
          );
          const next = candidate;
          next.renderer.resize(width, height);
          await next.renderer.updateAuthored(snapshot, assetMeshes, catalog);
          if (
            lastWorkingShader !== MATERIAL &&
            !(await next.renderer.replaceShader(lastWorkingShader))
          )
            throw new Error("The current shader could not open in this area.");
          for (const [mode, description] of [
            ["day", next.dayGameplay],
            ["night", next.nightGameplay],
          ] as const)
            if (description?.objects.length)
              next.renderer.setGameplayObjects(
                mode,
                description.objects,
                description.manifest.targetGroups,
              );
          if (gameplayState?.objectCount) {
            if (
              !next.renderer.prepareGameplayDestruction(
                environment.mode,
                gameplayState.destroyed,
              )
            )
              throw new Error(
                "The damaged town could not fit in this prepared area.",
              );
            next.renderer.setGameplayState(environment.mode, gameplayState);
          }
          next.renderer.setCharacter(characterPose);
          next.renderer.render(
            camera,
            { ...next.environment, mode: environment.mode },
            animationClock.time,
          );
          await next.renderer.device.queue.onSubmittedWorkDone();
          gpuPeakBytes = Math.max(
            gpuPeakBytes,
            (renderer?.gpuBytes ?? 0) + next.renderer.gpuBytes,
          );
          if (signal.aborted) throw new Error("Area loading canceled.");
          if (gameplayState?.objectCount) {
            if (
              !next.renderer.prepareGameplayDestruction(
                environment.mode,
                gameplayState.destroyed,
              )
            )
              throw new Error(
                "The damaged town could not fit in this prepared area.",
              );
            next.renderer.setGameplayState(environment.mode, gameplayState);
          }
          return {
            decodeMs,
            commit: () => {
              if (
                mode === "walk" &&
                player &&
                !cellContains(cell.id, player.position)
              )
                throw new Error(
                  "Return to the entry area before unloading the town.",
                );
              const old = renderer!;
              if (gameplayState?.objectCount) {
                if (
                  !next.renderer.prepareGameplayDestruction(
                    environment.mode,
                    gameplayState.destroyed,
                  )
                )
                  throw new Error(
                    "The damaged town could not fit in this prepared area.",
                  );
                next.renderer.setGameplayState(environment.mode, gameplayState);
              }
              try {
                next.renderer.presentTo(canvas);
              } catch (error) {
                old.presentTo(canvas);
                throw error;
              }
              next.renderer.frame = old.frame;
              const selectedMode = environment.mode;
              adoptWorld(next);
              activeResources = resources;
              environment.mode = selectedMode;
              if (WORLD_PROFILE.id === "medieval-town-lab") {
                const grade = referenceCameraLook(activeShot, selectedMode);
                if (grade) environment = { ...environment, ...grade };
              }
              const sun = look?.scenarios[selectedMode].sun;
              if (sun) environment = { ...environment, ...sunAngles(sun) };
              environment = validateEnvironment({
                ...environment,
                ...environmentEdits,
              });
              if (player) updatePlayer(player, 0);
              activated = true;
              publishEnvironment();
            },
            dispose: () => next.renderer.dispose(activated),
          };
        } catch (error) {
          candidate?.renderer.dispose();
          throw error;
        } finally {
          stagedResources = undefined;
        }
      },
    });
    if (!result.ok) throw new Error(result.message);
    status = {
      ...status,
      triangles: renderer!.triangles,
      instances: scene.instances.length,
    };
    receipt(
      requestId,
      true,
      cell === "town" ? "Complete town ready" : "Entry area ready",
    );
  } finally {
    streamLoading = false;
    status = {
      ...status,
      streaming: streamingStatus(),
      runtimeMetrics: {
        firstControlMs,
        preparationMs,
        gpuAccountedBytes: renderer!.gpuBytes,
      },
    };
    send({
      type: "ready",
      status,
      assets,
      hasSourceSky: renderer?.hasSourceSky,
      shaderSource: lastWorkingShader,
      sourceSkyLightIntensity: sourceSkyLightIntensity(),
    });
  }
}
async function probeHash(bytes: Uint8Array<ArrayBuffer>) {
  return [...new Uint8Array(await crypto.subtle.digest("SHA-256", bytes))]
    .map((value) => value.toString(16).padStart(2, "0"))
    .join("");
}
async function cpuCullProbe(
  message: Extract<PreviewMessage, { type: "cpuCullProbe" }>,
) {
  if (!C_TOWN_BENCHMARK || !renderer || !runtimeOwnersReady || streamLoading)
    throw new Error(
      "Open a ready town benchmark build to collect measurements.",
    );
  if (
    ![0, 1, 2].includes(message.workers) ||
    !Number.isInteger(message.iterations) ||
    message.iterations < 1 ||
    message.iterations > 100 ||
    (message.timeoutMs !== undefined &&
      (!Number.isFinite(message.timeoutMs) ||
        message.timeoutMs < 1 ||
        message.timeoutMs > 5000))
  )
    throw new Error(
      "Choose zero, one, or two helpers and 1 to 100 measurements.",
    );
  activeCpuProbe?.controller.abort();
  activeCpuProbe?.pool.dispose();
  const pool = new CpuCullExperiment({ workers: message.workers });
  const controller = new AbortController();
  const deadline = setTimeout(() => controller.abort(), 10_000);
  const probe = { pool, controller };
  activeCpuProbe = probe;
  try {
    const sourceMode = environment.mode;
    const source = runtimeOwnership.safePoint(() =>
      runtimeOwnership.run("probe", (access) =>
        access
          .read("renderer")
          .withView((owned) => owned.cullProbeSnapshot(camera, sourceMode)),
      ),
    );
    const expected = new Uint8Array(
      frustumVisibility(source.bounds, source.matrix),
    );
    const referenceHash = await probeHash(expected);
    const boundsHashes = await Promise.all([
      probeHash(new Uint8Array(source.bounds.spheres.buffer)),
      probeHash(new Uint8Array(source.bounds.aabbs.buffer)),
    ]);
    const identity = await probeHash(
      new TextEncoder().encode(boundsHashes.join(":")),
    );
    const options = {
      signal: controller.signal,
      timeoutMs: message.timeoutMs ?? 5000,
    };
    const install = await pool.install(
      {
        datasetId: `source-${identity.slice(0, 24)}`,
        generation: source.generation,
        bounds: source.bounds,
      },
      options,
    );
    const measurements = [];
    const resultHashes: string[] = [];
    const started = performance.now();
    const frameBefore = renderer.frame;
    for (let i = 0; i < message.iterations; i++) {
      if (controller.signal.aborted || activeCpuProbe !== probe)
        throw new Error("A newer request replaced this measurement.");
      const result = await pool.run({
        matrix: source.matrix,
        mode: message.workers ? "workers" : "serial",
        sequence: i + 1,
        ...options,
      });
      if (
        result.mask.length !== expected.length ||
        result.mask.some((value, index) => value !== expected[index])
      )
        throw new Error(
          "The worker visibility result differs from the serial reference.",
        );
      const { mask, ...measurement } = result;
      measurements.push(measurement);
      resultHashes.push(await probeHash(new Uint8Array(mask)));
      // The control and worker paths both yield between jobs; neither gates the frame loop.
      await new Promise<void>((resolve) => setTimeout(resolve, 0));
    }
    if (activeCpuProbe !== probe || controller.signal.aborted)
      throw new Error("A newer request replaced this measurement.");
    send({
      type: "cpuCullProbeResult",
      requestId: message.requestId,
      result: {
        schema: "aethyrforge.cpu-cull-probe.v1",
        workers: message.workers,
        iterations: message.iterations,
        count: source.bounds.count,
        install,
        measurements,
        referenceHash,
        resultHashes,
        exact: true,
        frameBefore,
        frameAfter: renderer!.frame,
        elapsedMs: performance.now() - started,
        snapshotBytes:
          source.bounds.spheres.byteLength +
          source.bounds.aabbs.byteLength +
          source.matrix.byteLength,
        source: {
          generation: source.generation,
          mode: sourceMode,
          matrix: [...source.matrix],
          identity,
        },
        liveExecution: "serial",
      },
    });
  } finally {
    clearTimeout(deadline);
    pool.dispose();
    if (activeCpuProbe === probe) activeCpuProbe = undefined;
  }
}
function agentRequestCurrent(requestId: string) {
  const request = agentRequests.get(requestId);
  return !request || request.ticket === agentWorkSerial;
}
function clearAgentInput() {
  input = { forward: 0, right: 0, up: 0, sprint: false, jump: false };
  yawDelta = pitchDelta = 0;
  jumpInput.reset();
}
function agentCapabilities() {
  return [
    "inspect.v1",
    "capture.context.v1",
    "camera.v1",
    "input.bounded.v1",
    ...(!WORLD_PROFILE.readOnly &&
    WORLD_PROFILE.id !== "open-ground" &&
    !playerMode.active
      ? ["prop.edit.v1", "history.v1", "shader.wgsl.v1", "replay.v1"]
      : []),
    ...(["starter-courtyard", "medieval-town-lab"].includes(WORLD_PROFILE.id)
      ? ["game.key-exit.v1"]
      : []),
  ];
}
function checkAgentExpectation(actual: AgentContext, expect: AgentExpectation) {
  if (
    actual.sourceIdentity !== expect.sourceIdentity ||
    actual.revision !== expect.revision ||
    actual.generation !== expect.generation ||
    actual.shaderIdentity !== expect.shaderIdentity
  )
    throw new Error(
      "The world changed. Inspect it before trying this action again.",
    );
}
function currentAgentGeneration() {
  const version = renderer!.runtimeInspection().generation;
  if (observedAgentRenderer !== renderer || observedAgentVersion !== version) {
    observedAgentRenderer = renderer;
    observedAgentVersion = version;
    agentGeneration++;
  }
  return agentGeneration;
}
async function readAgentState(
  requestId: string,
  value: unknown = {},
): Promise<AgentInspection> {
  if (!renderer || !kernel || status.phase !== "ready")
    throw new Error("Wait for the world to open.");
  const options = validateAgentInspectOptions(value);
  const state = checkResult(kernel.request({ type: "snapshot" }));
  if (
    options.expectedRevision !== undefined &&
    state.revision !== options.expectedRevision
  )
    throw new Error("The world changed. Inspect its current revision.");
  const source = scene;
  let sourceHash = agentSourceHashes.get(source);
  if (!sourceHash) {
    sourceHash = probeHash(new TextEncoder().encode(JSON.stringify(source)));
    agentSourceHashes.set(source, sourceHash);
  }
  const code = renderer.workingShaderSource();
  const generation = currentAgentGeneration();
  const render = renderer.runtimeInspection();
  const pose = structuredClone(state.player);
  const liveView = {
    eye: [...camera.eye],
    target: add(camera.eye, direction(camera.yaw, camera.pitch)),
    up: camera.up ? [...camera.up] : [0, 1, 0],
    fov: camera.fov ?? 1,
    mode,
  } as AgentInspection["view"];
  const liveInput = { ...input, lookX: yawDelta, lookY: pitchDelta };
  const authored = state.entities ?? [];
  const cursor = options.cursor ?? 0,
    limit = options.limit ?? 64;
  const end = Math.min(authored.length, cursor + limit);
  const objects = [],
    readOnlyObjects = [];
  for (let i = cursor; i < end; i++) {
    if (i < authored.length) {
      const entity = authored[i];
      objects.push({
        id: entity.id,
        kind: entity.kind,
        position: entity.position,
        yaw: entity.yaw ?? 0,
        scale: entity.scale ?? 1,
        collidable: entity.collidable ?? false,
        halfExtents: entity.halfExtents,
        collisionOffset: entity.collisionOffset,
        handle: renderer.authoredHandle(entity.id),
      });
    }
  }
  for (
    let index = 0;
    index < Math.min(source.instances.length, limit - objects.length);
    index++
  ) {
    const instance = source.instances[index];
    readOnlyObjects.push({
      id: `source.${index}`,
      kind: `mesh.${instance.mesh}`,
      editable: false as const,
      position: [
        instance.model[12],
        instance.model[13],
        instance.model[14],
      ] as V3,
      sourceIndex: index,
    });
  }
  const [sourceIdentity, shaderIdentity, authoredIdentity] = await Promise.all([
    sourceHash,
    probeHash(new TextEncoder().encode(code)),
    probeHash(
      new TextEncoder().encode(
        JSON.stringify({
          entities: state.entities,
          rooms: state.rooms,
          buildings: state.buildings,
          colliders: state.colliders,
          removedWalls: state.removedWalls,
          authoring: state.authoring,
          groundY: state.groundY,
        }),
      ),
    ),
  ]);
  return {
    schema: AGENT_PROTOCOL,
    requestId,
    sessionId: session,
    context: {
      worldId: WORLD_PROFILE.id,
      sourceIdentity,
      revision: state.revision,
      generation,
      tick: state.tick,
      shaderIdentity,
    },
    authoredIdentity,
    objects,
    totalObjects: authored.length,
    readOnlyObjects,
    readOnlyTotal: source.instances.length,
    truncated:
      end < authored.length || readOnlyObjects.length < source.instances.length,
    ...(end < authored.length ? { nextCursor: end } : {}),
    player: {
      position: pose.position,
      yaw: pose.yaw,
      pitch: pose.pitch,
      velocity: pose.velocity,
      grounded: pose.grounded,
      perspective: characterCamera.perspective,
    },
    game: gameStatus(),
    ...(state.gameplay?.enemies
      ? { enemies: structuredClone(state.gameplay.enemies) }
      : {}),
    view: liveView,
    input: liveInput,
    render: {
      frame: render.frame,
      instances: render.submittedInstances,
      triangles: render.submittedTriangles,
    },
    capabilities: agentCapabilities(),
    metrics: {
      ...runtimeEditMetrics,
      captureReadbackAndValidationMs: runtimeAgentCaptureMs,
    },
    lastWorkingCode: {
      identity: shaderIdentity,
      ...(new TextEncoder().encode(code).length <= AGENT_LIMITS.shaderBytes
        ? { source: code }
        : {}),
    },
  };
}
async function captureAgentFrame(
  requestId: string,
  value: unknown,
): Promise<AgentCaptureEnvelope> {
  const options = validateAgentCaptureRequest(value);
  if (!renderer || shaderChanging || streamLoading || filmState.manual)
    throw new Error(
      "Finish the current edit or film session before taking this screenshot.",
    );
  const active = renderer;
  const oldSize = { width: active.width, height: active.height };
  filmState.enterFrame();
  try {
    const before = await readAgentState(requestId);
    checkAgentExpectation(before.context, options.expect);
    if (!agentRequestCurrent(requestId))
      throw new Error("This screenshot was canceled.");
    const generation = active.runtimeInspection().generation;
    active.resize(options.width, options.height);
    const started = performance.now();
    const capture = await active.capturePixelsDetailed(options.time, {
      camera: cameraForCapture(camera, {
        ...options.camera,
        vignette: options.camera.vignette ?? null,
      }),
      env: environmentForCapture(effectiveEnvironment(), options.exposure),
      actorReference: options.actorReference,
    });
    const after = await readAgentState(requestId);
    checkAgentExpectation(after.context, options.expect);
    if (
      active !== renderer ||
      capture.generation !== generation ||
      !agentRequestCurrent(requestId)
    )
      throw new Error(
        "The world changed during this screenshot. Inspect it and try again.",
      );
    const pixelSha256 = await probeHash(new Uint8Array(capture.pixels));
    const result: AgentCaptureEnvelope = {
      schema: AGENT_CAPTURE_SCHEMA,
      requestId,
      sessionId: session,
      sequence: ++agentCaptureSequence,
      context: before.context,
      frameBefore: capture.frameBefore,
      frameAfter: capture.frameAfter,
      camera: capture.camera,
      width: capture.width,
      height: capture.height,
      time: capture.time,
      exposure: capture.exposure,
      pixelSha256,
      pixels: capture.pixels,
      ...(capture.actors ? { actors: capture.actors } : {}),
    };
    runtimeAgentCaptureMs = performance.now() - started;
    return result;
  } finally {
    active.resize(oldSize.width, oldSize.height);
    filmState.finishFrame();
    applyLiveSize();
    previous = 0;
    framePacer.reset();
  }
}
let runtimeAgentCaptureMs = 0;
function spellSwarmStatus(): EngineStatus["spellSwarm"] {
  if (WORLD_PROFILE.id !== "medieval-town-lab" || !renderer) return undefined;
  const count = renderer.spellSwarmInfo()?.count ?? 0;
  if (!validSpellSwarmCount(count))
    throw new Error("The spell creature count is outside the supported limit.");
  return { count, enabled: count > 0 };
}
async function handle(message: PreviewMessage) {
  if (message.type === "initialize") {
    if (session && session !== message.sessionId) return;
    session = message.sessionId;
    playerMode.enter(message.player === true);
    if (attached) await attached;
    if (status.phase === "ready")
      send({
        type: "ready",
        requestId: message.requestId,
        assets,
        status,
        hasSourceSky: renderer?.hasSourceSky ?? false,
        sourceSkyLightIntensity: sourceSkyLightIntensity(),
        shaderSource: lastWorkingShader,
      });
    return;
  }
  if (message.sessionId !== session || message.protocol !== PREVIEW_PROTOCOL)
    return;
  if (["agentAction", "agentCapture", "agentInspect"].includes(message.type))
    agentRequests.set(message.requestId, {
      started: performance.now(),
      type:
        message.type === "agentAction" ? message.action?.type : message.type,
      ticket:
        (message as PreviewMessage & { agentTicket?: number }).agentTicket ??
        agentWorkSerial,
    });
  try {
    if (
      WORLD_PROFILE.id === "open-ground" &&
      !(message.type === "command" && groundProps && mmorpgEditing) &&
      [
        "author",
        "command",
        "setShader",
        "setEnvironment",
        "selectAbility",
        "castAbility",
        "cancelAbility",
        "resetGameplay",
        "spellSwarm",
        "game",
        "loadPlayable",
      ].includes(message.type)
    )
      throw new Error("Use the movement controls and Space to fight here.");
    if (emptyWorld) {
      if (message.type === "status") {
        send({ type: "status", status });
        return;
      }
      if (message.type === "input") return;
      if (message.type === "beginFilmCapture") {
        if (emptyFilmSize || emptyCaptureBusy)
          throw new Error("Finish the current screenshot first.");
        emptyFilmSize = validateFilmCaptureSize({
          width: message.width,
          height: message.height,
        });
        receipt(message.requestId, true, "Film capture ready");
        return;
      }
      if (message.type === "endFilmCapture") {
        if (emptyCaptureBusy)
          throw new Error("Let the current screenshot finish first.");
        emptyFilmSize = undefined;
        emptyWorld.resize(width, height);
        emptyWorld.render();
        receipt(message.requestId, true, "Film capture finished");
        return;
      }
      if (message.type === "capturePixels") {
        validateFrameCaptureOptions(
          message.time === undefined ? {} : { time: message.time },
        );
        if (emptyCaptureBusy)
          throw new Error("Let the current screenshot finish first.");
        const size = emptyFilmSize ?? { width, height };
        emptyCaptureBusy = true;
        try {
          const pixels = await emptyWorld.capturePixels(
            size.width,
            size.height,
          );
          send({ type: "capturePixels", requestId: message.requestId, pixels });
        } finally {
          emptyCaptureBusy = false;
        }
        return;
      }
      throw new Error("This empty world has no game or editing actions.");
    }
    if (!playerMode.allows(message.type))
      throw new Error("This player has no editing access.");
    if (
      streamLoading &&
      [
        "setShader",
        "setEnvironment",
        "setCamera",
        "mode",
        "setCameraPose",
        "spellSwarm",
        "beginFilmCapture",
      ].includes(message.type)
    )
      throw new Error(
        "Let the area finish loading before changing its view or lighting.",
      );
    if (message.type === "agentCancel") {
      agentWorkSerial++;
      agentInput?.abort();
      clearAgentInput();
      receipt(message.requestId, true, "Agent work canceled");
      return;
    }
    if (message.type === "cancelCpuCullProbe") {
      activeCpuProbe?.controller.abort();
      activeCpuProbe?.pool.dispose();
      activeCpuProbe = undefined;
      receipt(message.requestId, true, "Measurement canceled");
      return;
    }
    if (message.type === "cancelAuthoring") {
      authoring.supersede();
      receipt(message.requestId, true, "Pending edit canceled");
      return;
    }
    if (message.type === "input" && (filmState.manual || mmorpgEditing || battleSpectating())) return;
    if (filmTransition)
      throw new Error("Let the film session finish changing first.");
    if (
      filmState.manual &&
      ![
        "capture",
        "capturePixels",
        "endFilmCapture",
        "setEnvironment",
        "setCamera",
        "status",
        "cTownBenchmark",
        "cTownReload",
      ].includes(message.type)
    )
      throw new Error(
        "Finish the film capture before editing or moving in the world.",
      );
    if (
      filmState.inFlight &&
      !["endFilmCapture", "status"].includes(message.type)
    )
      throw new Error("Let the current screenshot finish first.");
    if (message.type === "agentDiscover") {
      const { context } = await readAgentState(message.requestId);
      send({
        type: "agentDiscovery",
        requestId: message.requestId,
        result: {
          schema: AGENT_PROTOCOL,
          sessionId: session,
          worldId: context.worldId,
          sourceIdentity: context.sourceIdentity,
          capabilities: agentCapabilities(),
          limits: AGENT_LIMITS,
          readOnly: WORLD_PROFILE.readOnly || playerMode.active,
        },
      });
      return;
    } else if (message.type === "agentInspect") {
      send({
        type: "agentInspection",
        requestId: message.requestId,
        result: await readAgentState(message.requestId, message.options),
      });
      return;
    } else if (message.type === "agentCapture") {
      send({
        type: "agentCapture",
        requestId: message.requestId,
        result: await captureAgentFrame(message.requestId, message.options),
      });
      return;
    } else if (message.type === "agentAction") {
      const action = validateAgentAction(message.action);
      const expect = validateAgentExpectation(message.expect);
      checkAgentExpectation(
        (await readAgentState(message.requestId)).context,
        expect,
      );
      if (!agentRequestCurrent(message.requestId))
        throw new Error("This agent action was canceled.");
      const base = {
        protocol: PREVIEW_PROTOCOL,
        sessionId: session,
        requestId: message.requestId,
      };
      if (action.type === "command")
        await handle({ ...base, type: "command", command: action.command });
      else if (action.type === "shader")
        await handle({ ...base, type: "setShader", source: action.source });
      else if (action.type === "camera")
        await handle({
          ...base,
          type: "setCameraPose",
          eye: action.eye,
          target: action.target,
          up: action.up,
          fov: action.fov,
          mode: action.mode,
        });
      else if (action.type === "game")
        await handle({ ...base, type: "game", action: action.action });
      else if (action.type === "spellSwarm")
        await handle({ ...base, type: "spellSwarm", count: action.count });
      else {
        const controller = new AbortController();
        agentInput = controller;
        try {
          await handle({ ...base, type: "input", input: action.input });
          await new Promise<void>((resolve, reject) => {
            const finish = () => {
              clearTimeout(timer);
              reject(new Error("This input action was canceled."));
            };
            const timer = setTimeout(() => {
              controller.signal.removeEventListener("abort", finish);
              resolve();
            }, action.durationMs);
            controller.signal.addEventListener("abort", finish, { once: true });
          });
          receipt(message.requestId, true, "Input finished");
        } finally {
          if (agentInput === controller) agentInput = undefined;
          clearAgentInput();
        }
      }
      return;
    } else if (message.type === "cpuCullProbe") {
      await cpuCullProbe(message);
      return;
    } else if (message.type === "cTownBenchmark") {
      if (!C_TOWN_BENCHMARK || !renderer || !kernel)
        throw new Error("Open a town benchmark build to collect measurements.");
      validateCTownRequest(message.options);
      if (message.options.op === "readBattle") {
        const battleProbe = await renderer.probeBattle();
        send({ type: "cTownBenchmarkResult", requestId: message.requestId,
          result: { ...benchmarkResult(), battleProbe } });
        return;
      }
      if (message.options.op === "readSwarm") {
        const swarmProbe = await renderer.probeSpellSwarm();
        send({
          type: "cTownBenchmarkResult",
          requestId: message.requestId,
          result: { ...benchmarkResult(), swarmProbe },
        });
        return;
      }
      if (message.options.op === "pinActors") {
        if (cTownMetrics.recording || filmState.manual)
          throw new Error(
            "Stop measurement or film capture before pinning actors.",
          );
        const context = (await readAgentState(message.requestId)).context;
        const canonical = message.options.canonical === true;
        if (canonical) {
          assertNativeStartCapture({
            diagnostic: C_TOWN_BENCHMARK,
            recording: cTownMetrics.recording,
            preparing: preparingDestruction,
            gameplay: gameplayState,
          });
          if (!enemyPatrols?.scenarios[environment.mode])
            throw new Error(
              "Prepare native knight routes before pinning their start poses.",
            );
          filmState.enterFrame();
        }
        try {
          if (canonical) {
            const ready = checkResult(
              kernel.request({
                type: "gameplay_init_enemies",
                enemies: enemyPatrols!.scenarios[environment.mode],
              }),
            );
            syncGameplay(ready.gameplay);
          }
          const actorReference = await renderer.pinCaptureActors(
            context.sourceIdentity,
            environment.mode,
            canonical,
          );
          send({
            type: "cTownBenchmarkResult",
            requestId: message.requestId,
            result: benchmarkResult(actorReference),
          });
        } finally {
          if (canonical) {
            filmState.finishFrame();
            previous = 0;
            framePacer.reset();
          }
        }
        return;
      }
      if (message.options.op === "start") {
        if (filmState.manual)
          throw new Error("Finish capture before measuring live frames.");
        cTownStoppedCounters = undefined;
        cTownMetrics.start(message.options);
        framePacer.configure(cTownMetrics.targetHz);
        renderer.startFrameRecording(cTownMetrics.capacity);
        cTownCamera = { ...camera, eye: [...camera.eye] };
        cTownStarted = performance.now();
        renderer.setBenchmarkEnabled(cTownMetrics.instrumented);
        kernel.setProfiling(cTownMetrics.instrumented);
      } else if (message.options.op === "stop") {
        cTownMetrics.stop();
        framePacer.configure("display");
        cTownStoppedCounters = liveBenchmarkCounters();
        renderer.stopFrameRecording();
        renderer.setBenchmarkEnabled(false);
        kernel.setProfiling(false);
      }
      if (message.options.drain) await renderer.drain();
      send({
        type: "cTownBenchmarkResult",
        requestId: message.requestId,
        result: benchmarkResult(),
      });
      return;
    } else if (message.type === "cTownReload") {
      if (
        !C_TOWN_BENCHMARK ||
        C_TOWN_BACKEND !== "c" ||
        !kernel ||
        cTownMetrics.recording
      )
        throw new Error(
          "Stop measurement in the C town before replacing its movement module.",
        );
      if (!["default", "changed", "invalid"].includes(message.candidate))
        throw new Error("Choose a prepared movement candidate.");
      const started = performance.now();
      let bytes: ArrayBuffer;
      if (message.candidate === "invalid")
        bytes = new Uint8Array([0, 97, 115, 109, 0]).buffer;
      else {
        const response = await fetch(
          message.candidate === "changed"
            ? "/c-town-simulation-faster.wasm"
            : "/c-town-simulation.wasm",
          { cache: "no-store" },
        );
        if (!response.ok)
          throw new Error("Build the movement candidate before loading it.");
        bytes = await response.arrayBuffer();
      }
      const result = await kernel.reloadSimulation(bytes);
      send({
        type: "cTownReloadResult",
        requestId: message.requestId,
        result: {
          ...result,
          elapsedMs: performance.now() - started,
          bytes: bytes.byteLength,
        },
      });
      return;
    } else if (message.type === "beginFilmCapture") {
      if (
        !renderer ||
        !kernel ||
        status.phase !== "ready" ||
        pendingCommands ||
        shaderChanging
      )
        throw new Error("Wait for the current world changes to finish.");
      const size = validateFilmCaptureSize({
        width: message.width,
        height: message.height,
      });
      const age = animationClock.sample(performance.now());
      filmState.begin(size);
      filmTransition = true;
      filmRestore = { width, height, age };
      scope.cancelAnimationFrame(frameId);
      input = { forward: 0, right: 0, up: 0, sprint: false, jump: false };
      jumpInput.reset();
      renderer.setCharacter(undefined);
      yawDelta = 0;
      pitchDelta = 0;
      try {
        await renderer.device.queue.onSubmittedWorkDone();
        width = size.width;
        height = size.height;
        renderer.resize(width, height);
      } catch (error) {
        try {
          resumeLiveFilmSession();
        } catch {}
        throw error;
      } finally {
        filmTransition = false;
      }
      receipt(message.requestId, true, "Film capture ready");
      return;
    }
    if (message.type === "endFilmCapture") {
      filmTransition = true;
      try {
        await filmPending?.catch(() => {});
        if (filmState.manual) resumeLiveFilmSession();
      } finally {
        filmTransition = false;
      }
      receipt(message.requestId, true, "Film capture finished");
      return;
    }
    if (message.type === "loadPlayable") {
      if (
        message.action.type !== "restore_material_project" ||
        !/^[a-f0-9]{64}$/.test(message.id)
      )
        throw new Error("Open a complete playable version.");
      if (playerMode.begin(message.id)) {
        try {
          await clearMaterialPreview();
          await authoring.execute(
            message.action,
            message.base,
            message.requestId,
            message.ticket!,
          );
          playerMode.finish(message.id);
        } catch (error) {
          playerMode.fail();
          throw error;
        }
      }
      receipt(message.requestId, true, "Playable version opened.");
      return;
    }
    if (
      WORLD_PROFILE.readOnly &&
      ([
        "author",
        "command",
        "setShader",
        "setEnvironment",
        "setSceneEffects",
        "previewImpact",
        "spellSwarm",
      ].includes(message.type) ||
        ((message.type === "mode" || message.type === "setCameraPose") &&
          message.mode === "walk"))
    )
      throw new Error(
        "The overview is for exploring. Open Medieval Town to edit.",
      );
    if (message.type === "spellSwarm") {
      if (
        Object.keys(message).some(
          (key) =>
            !["protocol", "sessionId", "requestId", "type", "count"].includes(
              key,
            ),
        )
      )
        throw new Error("Choose a supported spell creature request.");
      if (playerMode.active)
        throw new Error("This player has no editing access.");
      if (WORLD_PROFILE.id !== "medieval-town-lab")
        throw new Error("Open Medieval Town to summon spell creatures.");
      if (!renderer || status.phase !== "ready")
        throw new Error("Wait for the world to open.");
      const action = validateAgentAction({
        type: "spellSwarm",
        count: message.count,
      });
      if (action.type !== "spellSwarm" || !validSpellSwarmCount(action.count))
        throw new Error("Choose a supported spell creature count.");
      await renderer.setSpellSwarm(action.count);
      status = { ...status, spellSwarm: spellSwarmStatus() };
      send({ type: "status", status });
      receipt(
        message.requestId,
        true,
        action.count
          ? `${action.count.toLocaleString()} spell creatures summoned`
          : "Spell creatures cleared",
      );
    } else if (message.type === "materialView") {
      const result = await materialView(message);
      send({ type: "materialView", requestId: message.requestId, result });
    } else if (message.type === "exportProject") {
      send({
        type: "projectDocument",
        requestId: message.requestId,
        document: authoring.export(),
      });
    } else if (message.type === "mesh") {
      if (!renderer) throw new Error("Wait for the world to open.");
      await renderer.meshView(message.enabled);
      status = { ...status, mesh: message.enabled };
      send({ type: "status", status });
      receipt(
        message.requestId,
        true,
        message.enabled ? "Mesh overlay on" : "Shaded view",
      );
    } else if (message.type === "author") {
      await clearMaterialPreview();
      await authoring.execute(
        message.action,
        message.base,
        message.commandId,
        (message as any).ticket,
      );
      receipt(
        message.requestId,
        true,
        message.action.type === "draft"
          ? "Source draft updated"
          : "Change applied",
      );
    } else if (message.type === "stream") {
      await streamAction(message.action, message.cell, message.requestId);
    } else if (message.type === "battle") {
      if (!validBattleRequest(message) || playerMode.active ||
          import.meta.env.VITE_PUBLIC_DEMO === "1" || WORLD_PROFILE.id !== "open-ground")
        throw Error("Open Battle Lab in the local grass creator.");
      if (!renderer || status.phase !== "ready" || mmorpgSession?.state.connection !== "online")
        throw Error("Join the grass world before starting a battle.");
      const action = message.action;
      const continuingView = action.op === "camera" && battleFollowing() && ["first", "third", "player"].includes(action.view);
      if (action.op !== "input" && action.op !== "weapon" && !continuingView) {
        input = { forward: 0, right: 0, up: 0, sprint: false, jump: false };
        battleScriptInput = undefined;
        jumpInput.reset();
      }
      if (action.op === "start") {
        const origin = player?.position ?? [-10, 0, 0];
        const center: [number, number, number] = [origin[0], 0, origin[2] + 100];
        center[1] = mmorpgHeight(center[0], center[2]);
        const terrain = battleTerrain([center[0], center[2]]);
        await renderer.startBattle(action.count, action.seed ?? 1, terrain, center);
        leaveBattleParticipant();
        battleField = terrain;
        mmorpgEditing = false;
        battleView = action.count > 500_000 ? "dolly" : "overview";
        battleStarted = performance.now();
      } else if (action.op === "stop") {
        leaveBattleParticipant(); renderer.stopBattle(); battleField = undefined; battleView = "overview";
      }
      else if (action.op === "reset") {
        leaveBattleParticipant();
        renderer.resetBattle();
        battleView = (renderer.battleInfo()?.count ?? 0) > 500_000 ? "dolly" : "overview";
        battleStarted = performance.now();
      } else if (action.op === "join") {
        joinBattleParticipant(action.at, action.team, action.view, action.class);
      } else if (action.op === "leave") {
        leaveBattleParticipant(); battleView = "overview";
      } else if (action.op === "respawn") {
        if (!battleParticipant || !battleSpawn) throw Error("Enter the battle before returning to it.");
        battleParticipant.respawn(battleSpawn);
        battleFollowCamera.reset(battleParticipant.snapshot!);
        yawDelta = 0; pitchDelta = 0;
        battleMeteorPending = false;
        battleParticipantEpoch = renderer.resetBattleParticipant();
      } else if (action.op === "input") {
        if (!battleParticipant || !battleFollowing()) throw Error("Enter the battle in first or third person before moving.");
        battleScriptInput = { forward: action.forward, turn: action.turn, sprint: action.sprint, attack: action.attack, until: performance.now() + 1000 };
        if (action.attackKind && action.attackKind !== battleAttack) { battleMeteorPending = false; battleTarget = undefined; battleAttack = action.attackKind; battleFollowCamera.cancelAttack(); }
        requestBattleMeteor(action.attack);
        battleLook(action.lookX ?? 0, action.lookY ?? 0);
      } else if (action.op === "weapon") {
        if (!battleParticipant) throw Error("Enter the battle before choosing an attack.");
        battleAttack = action.attackKind;
        battleTarget = undefined;
        battleMeteorPending = false;
        battleFollowCamera.cancelAttack();
      } else if (action.op === "camera") {
        if (!renderer.battleInfo()?.enabled) throw Error("Start a battle before choosing its view.");
        if (["first", "third"].includes(action.view) && !battleParticipant)
          joinBattleParticipant(undefined, 0, action.view as "first" | "third");
        else changeBattleView(action.view);
      }
      status = { ...status, mmorpgEditing, battle: battleStatus() };
      send({ type: "status", status });
      receipt(message.requestId, true, action.op === "stop" ? "Battle stopped" : action.op === "input" ? "Battle input updated" : "Battle ready");
    } else if (message.type === "mmorpgEditing") {
      if (!validMmorpgEditingRequest(message) || playerMode.active ||
          import.meta.env.VITE_PUBLIC_DEMO === "1" || WORLD_PROFILE.id !== "open-ground")
        throw Error("Open the grass world in the creator to edit its props.");
      if (!groundProps || !catalog.size || status.phase !== "ready" ||
          mmorpgSession?.state.connection !== "online")
        throw Error("Enter the prepared grass world before editing.");
      mmorpgEditing = message.editing;
      input = { forward: 0, right: 0, up: 0, sprint: false, jump: false };
      jumpInput.reset();
      status = { ...status, mmorpgEditing };
      send({ type: "status", status });
      receipt(message.requestId, true, mmorpgEditing ? "Edit props" : "Back to playing");
    } else if (message.type === "mmorpg") {
      if (!mmorpgSession || status.phase !== "ready") throw Error("Open the game before joining.");
      await mmorpgSession.action(message.action, message.requestId);
      receipt(message.requestId, true, message.action.type === "join" ? "Welcome" : "Done");
    } else if (message.type === "restartCombat") {
      if (
        WORLD_PROFILE.id !== "open-ground" ||
        !combatEnabled ||
        !kernel ||
        !renderer ||
        status.phase !== "ready"
      )
        throw new Error("Open the zombie encounter before restarting.");
      if (
        Object.keys(message).some(
          (key) => !["protocol", "sessionId", "requestId", "type"].includes(key),
        )
      )
        throw new Error("Choose the encounter's Restart control.");
      const reset = checkResult(kernel.request({ type: "combat_reset" }));
      quickLocomotion?.reset();
      jumpInput.reset();
      input = { forward: 0, right: 0, up: 0, sprint: false, jump: false };
      yawDelta = pitchDelta = 0;
      syncCombat(reset.combat);
      updatePlayer(reset.player, 0);
      status = { ...status, character: characterStatus(), combat: combatState };
      send({ type: "status", status });
      receipt(message.requestId, true, "Encounter restarted");
    } else if (message.type === "game") {
      await gameAction(message.action, message.requestId);
    } else if (message.type === "command") {
      await clearMaterialPreview();
      await command(message.command, message.requestId);
    } else if (
      message.type === "setSceneEffects" ||
      message.type === "previewImpact"
    ) {
      if (WORLD_PROFILE.id !== "open-ground" || playerMode.active)
        throw new Error("Open Ground has the live effects controls.");
      if (!renderer || status.phase !== "ready")
        throw new Error("Wait for the world to open.");
      const allowed = [
        "protocol",
        "sessionId",
        "requestId",
        "type",
        ...(message.type === "setSceneEffects" ? ["settings"] : []),
      ];
      if (Object.keys(message).some((key) => !allowed.includes(key)))
        throw new Error("Choose a supported effects action.");
      if (message.type === "setSceneEffects") {
        const candidate = validateSceneEffects(message.settings);
        renderer.setSceneEffects(candidate);
        sceneEffects = candidate;
      } else {
        const forward = direction(camera.yaw, 0);
        const origin = player?.position ?? camera.eye;
        renderer.previewImpact(
          [
            origin[0] + forward[0] * 6,
            WORLD_PROFILE.groundY,
            origin[2] + forward[2] * 6,
          ],
          animationClock.time,
        );
      }
      status = {
        ...status,
        sceneEffects,
        effectPreview: renderer.effectPreviewStats,
      };
      send({ type: "status", status });
      receipt(
        message.requestId,
        true,
        message.type === "setSceneEffects"
          ? "Effects updated"
          : "Impact preview playing",
      );
    } else if (message.type === "setEnvironment") {
      if (
        message.restoreSourceFill !== undefined &&
        typeof message.restoreSourceFill !== "boolean"
      )
        throw new Error(
          "Choose whether to restore this sky's lighting settings.",
        );
      const checkedSettings = validateEnvironment(message.settings);
      const changedMode = checkedSettings.mode !== environment.mode;
      if (
        changedMode &&
        gameplayState &&
        (gameplayState.destroyedCount ||
          preparingDestruction ||
          gameplayState.castle?.active ||
          gameplayState.activeCast ||
          gameplayState.pendingImpacts ||
          gameplayState.debris.length)
      )
        throw new Error(
          "Restore the town before switching between day and night.",
        );
      for (const key of Object.keys(
        checkedSettings,
      ) as (keyof typeof environment)[])
        if (checkedSettings[key] !== environment[key])
          Object.assign(environmentEdits, { [key]: checkedSettings[key] });
      environment = environmentProfiles.apply(
        environment,
        message.settings,
        message.restoreSourceFill === true,
      );
      if (changedMode) activateCollision(environment.mode);
      if (changedMode) {
        const sun = look?.scenarios[environment.mode].sun;
        if (sun) environment = { ...environment, ...sunAngles(sun) };
      }
      publishEnvironment();
      receipt(message.requestId, true, "Lighting updated");
    } else if (message.type === "input") {
      jumpInput.update(message.input.jump === true);
      input = {
        forward: message.input.forward,
        right: message.input.right,
        up: message.input.up,
        sprint: message.input.sprint,
        jump: message.input.jump === true,
        attack: message.input.attack === true,
        dance: message.input.dance === true,
      };
      battleLook(message.input.lookX, message.input.lookY);
    } else if (message.type === "selectAbility") {
      if (
        !gameplayState?.objectCount ||
        (message.ability !== null && !isDruidAbility(message.ability))
      )
        throw new Error("These spells are available in Medieval Town.");
      if (message.ability === "destroy_castle" && !gameplayState.castle?.total)
        throw new Error(
          "Prepare the distant castle before using this ability.",
        );
      if (preparingDestruction && message.ability !== selectedAbility) {
        castlePreparation++;
        preparingDestruction = false;
      }
      selectedAbility = message.ability;
      if (
        selectedAbility === "destroy_castle" &&
        gameplayState.castle?.target &&
        player
      ) {
        const angles = viewAngles(player.eye, gameplayState.castle.target);
        characterCamera.yawOffset = 0;
        yawDelta += angles.yaw - player.yaw;
        pitchDelta += angles.pitch - player.pitch;
      }
      publishGameplay();
      receipt(
        message.requestId,
        true,
        selectedAbility
          ? selectedAbility === "destroy_castle"
            ? "The distant castle is targeted. Click Cast or press Enter to begin."
            : "Aim at the town, then click to cast."
          : "Spell targeting canceled",
      );
    } else if (message.type === "castAbility") {
      await castDruid(message.requestId);
    } else if (message.type === "cancelAbility") {
      const wasPreparing = preparingDestruction;
      castlePreparation++;
      preparingDestruction = false;
      const stopped = checkResult(
        kernel!.request({ type: "gameplay_cancel_cast" }),
      );
      syncGameplay(stopped.gameplay);
      publishGameplay();
      receipt(
        message.requestId,
        true,
        wasPreparing
          ? "Spell preparation stopped."
          : "Barrage stopped. Falling strikes will still land.",
      );
    } else if (message.type === "resetGameplay") {
      if (!gameplayState?.objectCount)
        throw new Error("Open Medieval Town to restore its buildings.");
      castlePreparation++;
      preparingDestruction = false;
      const reset = checkResult(kernel!.request({ type: "gameplay_reset" }));
      renderer!.resetGameplay(environment.mode);
      selectedAbility = null;
      impactCount = 0;
      effectIds.clear();
      lastGameImpact = undefined;
      syncGameplay(reset.gameplay);
      publishGameplay();
      receipt(message.requestId, true, "Town restored");
    } else if (message.type === "mode") {
      if (!["walk", "fly"].includes(message.mode))
        throw new Error("Choose Walk or Fly.");
      if (mode === message.mode) {
        receipt(
          message.requestId,
          true,
          mode === "walk" ? "Walk camera" : "Fly camera",
        );
        return;
      }
      if (
        message.mode === "walk" &&
        !walkAllowed([camera.eye[0], camera.eye[1] - 1.62, camera.eye[2]])
      )
        throw new Error("Load this area before walking into it.");
      const next = checkResult(
        kernel!.request({
          type: "set_camera",
          position: [camera.eye[0], camera.eye[1] - 1.62, camera.eye[2]],
          yaw: camera.yaw,
          pitch: camera.pitch,
          mode: message.mode,
        }),
      );
      mode = message.mode;
      input = { forward: 0, right: 0, up: 0, sprint: false, jump: false };
      jumpInput.reset();
      characterCamera.resetCollision();
      updatePlayer(next.player, 0);
      receipt(
        message.requestId,
        true,
        mode === "fly" ? "Fly camera" : "Walk camera",
      );
    } else if (message.type === "perspective") {
      if (quickCamera) throw new Error("Paladin uses a following camera.");
      if (!renderer?.hasCharacter)
        throw new Error("This world uses first-person walking.");
      if (!validCharacterPerspective(message.perspective))
        throw new Error("Choose First person or Third person.");
      characterCamera.setPerspective(message.perspective);
      receipt(
        message.requestId,
        true,
        message.perspective === "first" ? "First person" : "Third person",
      );
    } else if (message.type === "orbitDistance") {
      if (!validOrbitDistance(message.distance))
        throw new Error("Choose a camera distance from 2.5 to 40 meters.");
      if (battleFollowing() && battleView !== "first") battleFollowCamera.zoom(message.distance);
      else {
        if (quickCamera) throw new Error("Use third person to zoom around the battle player.");
        characterCamera.setOrbitDistance(message.distance);
      }
      receipt(message.requestId, true, "Camera distance updated");
    } else if (message.type === "setCamera") {
      setShot(message.shotId);
      receipt(message.requestId, true, `Camera ${message.shotId}`);
    } else if (message.type === "setCameraPose") {
      const { eye, target } = message;
      validateFrameCaptureOptions({
        camera: { eye, target, up: message.up, fov: message.fov },
      });
      if (
        ![eye, target].every(
          (p) =>
            Array.isArray(p) &&
            p.length === 3 &&
            p.every((n) => Number.isFinite(n) && Math.abs(n) < 20000),
        ) ||
        Math.hypot(...eye.map((n, i) => n - target[i])) < 0.01
      )
        throw new Error("Choose a valid camera position and target.");
      if (
        message.fov !== undefined &&
        (!Number.isFinite(message.fov) ||
          message.fov < 0.15 ||
          message.fov > 2.5)
      )
        throw new Error(
          "Choose a camera field of view from 0.15 to 2.5 radians.",
        );
      if (
        WORLD_PROFILE.id === "personal-planet" &&
        !planetCameraAllowed({ eye, mode: message.mode ?? "fly" })
      )
        throw new Error(
          "Keep walking inside the courtyard, or reload to return to its entrance.",
        );
      const angles = viewAngles(eye, target),
        nextMode = message.mode ?? "fly";
      if (nextMode === "walk" && !walkAllowed([eye[0], eye[1] - 1.62, eye[2]]))
        throw new Error("Load this area before walking into it.");
      const moved = checkResult(
        kernel!.request({
          type: "set_camera",
          position: [eye[0], eye[1] - 1.62, eye[2]],
          yaw: angles.yaw,
          pitch: angles.pitch,
          mode: nextMode,
        }),
      );
      camera.eye = eye;
      camera.up = undefined;
      camera.vignette = undefined;
      camera.fov = message.fov ?? 1;
      Object.assign(camera, angles);
      mode = nextMode;
      characterCamera.resetCollision();
      updatePlayer(moved.player, 0);
      if (message.up) camera.up = [...message.up];
      receipt(message.requestId, true, "Camera moved");
    } else if (message.type === "capture" || message.type === "capturePixels") {
      if (!renderer) throw new Error("Wait for the world to open.");
      if (message.type === "capturePixels" && "format" in message)
        throw Error("Pixel capture returns raw RGBA without an image format.");
      const options = validateFrameCaptureOptions({
        ...(message.time === undefined ? {} : { time: message.time }),
        ...(message.camera === undefined ? {} : { camera: message.camera }),
        ...(message.exposure === undefined
          ? {}
          : { exposure: message.exposure }),
        ...(message.transforms === undefined
          ? {}
          : { transforms: message.transforms }),
        ...(message.actorReference === undefined
          ? {}
          : { actorReference: message.actorReference }),
        ...(message.type === "capture" && message.format !== undefined
          ? { format: message.format }
          : {}),
      });
      filmState.enterFrame();
      try {
        const time =
            options.time ??
            (filmState.manual ? animationClock.time : undefined),
          view = {
            camera: cameraForCapture(camera, options.camera),
            env: environmentForCapture(
              effectiveEnvironment(),
              options.exposure,
            ),
            actorReference: options.actorReference,
          };
        if (message.type === "capturePixels") {
          const pending = renderer.capturePixels(
            time,
            view,
            options.transforms,
          );
          filmPending = pending;
          const pixels = await pending;
          send({ type: "capturePixels", requestId: message.requestId, pixels });
        } else {
          const pending = renderer.capture(
            time,
            view,
            options.format ?? "png",
            options.transforms,
          );
          filmPending = pending;
          const blob = await pending;
          send({ type: "capture", requestId: message.requestId, blob });
        }
      } finally {
        filmPending = undefined;
        filmState.finishFrame();
        applyLiveSize();
        framePacer.reset();
      }
    } else if (message.type === "setShader") {
      if (!renderer) throw new Error("Wait for the world to open.");
      shaderChanging++;
      try {
        const applied = await renderer.replaceShader(message.source, () =>
          agentRequestCurrent(message.requestId),
        );
        if (applied) lastWorkingShader = message.source;
        receipt(
          message.requestId,
          applied,
          applied
            ? "Shader applied"
            : "A newer shader edit is already pending.",
        );
      } finally {
        shaderChanging--;
      }
    } else if (message.type === "status")
      send({ type: "status", requestId: message.requestId, status });
  } catch (e) {
    receipt(message.requestId, false, errorMessage(e));
  } finally {
    if (["agentAction", "agentCapture", "agentInspect"].includes(message.type))
      agentRequests.delete(message.requestId);
  }
}
function setShot(id: string) {
  camera.near = undefined;
  characterPose = undefined;
  renderer?.setCharacter(undefined);
  characterCamera.resetCollision();
  if (WORLD_PROFILE.readOnly) {
    if (!overview || id !== "MT-04")
      throw new Error("Choose the asset overview camera.");
    camera.eye = [...overview.camera.position] as V3;
    camera.up = [...overview.camera.up] as V3;
    camera.fov = (overview.camera.verticalFovDegrees * Math.PI) / 180;
    camera.vignette = undefined;
    Object.assign(camera, viewAngles(camera.eye, overview.camera.target as V3));
    mode = "fly";
    kernel?.request({
      type: "set_camera",
      position: [camera.eye[0], camera.eye[1] - 1.62, camera.eye[2]],
      yaw: camera.yaw,
      pitch: camera.pitch,
      mode,
    });
    return;
  }
  const shots: Record<
    string,
    { eye: V3; target: V3; fov?: number; night?: boolean }
  > = {
    "MT-02": { eye: [10, 4, 25], target: [10, 5, -8] },
    "MT-03": { eye: [16, 3, 8], target: [9, 4, 0] },
    "MT-05": { eye: [10, 3, 30], target: [10, 4, -25] },
    "MT-06": { eye: [0, 3, 18], target: [10, 4, 0] },
    "MT-07": {
      // Fixed-FOV, zero-roll image fit; background agreement remains unqualified.
      eye: [23.6999203, 2.8484634, 30.3531501],
      target: [29.8156586, 4.0289923, 22.5298454],
      fov: 0.48231,
    },
    "MT-08": {
      eye: [24.02835, 3.03284, 30.15633],
      target: [27.07628, 3.25379, 26.1989],
      fov: 0.51783,
      night: true,
    },
    "MT-09": { eye: [11, 2.8, 8], target: [10, 3, 3] },
    "MT-10": { eye: [11, 2.8, 8], target: [10, 3, 3], night: true },
    "MT-11": { eye: [10, 4, 25], target: [10, 5, -8], night: true },
    "MT-12": { eye: [10, 3, 12], target: [0, 9, -8], night: true },
    "MT-13": { eye: [3, 3, 4], target: [0, 3, -2], night: true },
    "MT-14": { eye: [16, 3, 8], target: [9, 4, 0], night: true },
    "MT-15": { eye: [12, 3, 16], target: [5, 6, -5], night: true },
    "MT-16": { eye: [14, 3, 8], target: [6, 4, -3] },
    "MT-17": { eye: [10, 4, 25], target: [10, 24, -100] },
    "MT-18": { eye: [18, 5, 16], target: [0, 10, -10] },
  };
  const sourceMatch = look?.galleryShots[id];
  const sourceCamera = sourceMatch
    ? look?.scenarios[sourceMatch.scenario].cameras.find(
        (camera) => camera.id === sourceMatch.cameraId,
      )
    : undefined;
  const shot = sourceCamera
    ? {
        eye: sourceCamera.position as V3,
        target: sourceCamera.target as V3,
        fov: (sourceCamera.verticalFovDegrees * Math.PI) / 180,
        night: sourceMatch!.scenario === "night",
      }
    : shots[id];
  if (!shot) throw new Error("Choose a scenic reference camera.");
  activeShot = id;
  camera.eye = shot.eye;
  camera.fov = shot.fov ?? 1;
  Object.assign(camera, viewAngles(shot.eye, shot.target));
  const nextMode = shot.night ? "night" : "day";
  const changedMode = environment.mode !== nextMode;
  environment = environmentProfiles.apply(
    environment,
    { ...environment, mode: nextMode },
    true,
  );
  if (changedMode) {
    const sun = look?.scenarios[nextMode].sun;
    if (sun) environment = { ...environment, ...sunAngles(sun) };
  }
  if (WORLD_PROFILE.id === "medieval-town-lab") {
    const grade = referenceCameraLook(id, nextMode);
    if (grade)
      environment = {
        ...environment,
        exposure: grade.exposure,
        skyBackgroundIntensity: grade.skyBackgroundIntensity,
      };
  }
  camera.vignette = selectedCameraVignette(sourceCamera);
  if (sourceCamera) camera.up = sourceCamera.up as V3;
  else camera.up = undefined;
  publishEnvironment();
  activateCollision(environment.mode);
  kernel?.request({
    type: "set_camera",
    position: [shot.eye[0], shot.eye[1] - 1.62, shot.eye[2]],
    yaw: camera.yaw,
    pitch: camera.pitch,
    mode: "fly",
  });
  mode = "fly";
}
const db = () =>
  new Promise<IDBDatabase>((resolve, reject) => {
    const request = indexedDB.open(WORLD_PROFILE.database, 1);
    request.onupgradeneeded = () =>
      request.result.createObjectStore("projects");
    request.onsuccess = () => resolve(request.result);
    request.onerror = () => reject(request.error);
  });
async function saveProject(s: any) {
  if (
    WORLD_PROFILE.readOnly ||
    ["haunted-house", "personal-planet"].includes(WORLD_PROFILE.id)
  )
    return;
  const database = await db();
  await new Promise<void>((resolve, reject) => {
    const tx = database.transaction("projects", "readwrite");
    tx.objectStore("projects").put(
      { entities: s.entities, rooms: s.rooms, removedWalls: s.removedWalls },
      groundDraftKey ?? WORLD_PROFILE.key,
    );
    tx.oncomplete = () => resolve();
    tx.onerror = () => reject(tx.error);
  });
  database.close();
}
async function loadProject() {
  if (
    WORLD_PROFILE.readOnly ||
    ["haunted-house", "personal-planet"].includes(WORLD_PROFILE.id)
  )
    return undefined;
  const database = await db();
  const value = await new Promise<any>((resolve, reject) => {
    const request = database
      .transaction("projects")
      .objectStore("projects")
      .get(groundDraftKey ?? WORLD_PROFILE.key);
    request.onsuccess = () => resolve(request.result);
    request.onerror = () => reject(request.error);
  });
  database.close();
  return value;
}
scope.onmessage = (event) => {
  const m = event.data;
  if (m.type === "attach") {
    canvas = m.canvas;
    const size = liveViewport(m.width, m.height);
    if (size) ({ width, height } = size);
    attached = Promise.resolve()
      .then(() => {
        animationClock = new WorldClock(
          m.animationTime === undefined ? 0 : m.animationTime,
        );
        return initialize();
      })
      .catch((e) => {
        assetDownloadLifetime.abort();
        status = { ...status, phase: "error", message: errorMessage(e) };
        send({ type: "error", message: status.message });
      });
  } else if (m.type === "resize") {
    const size = liveViewport(m.width, m.height);
    if (size) {
      deferredLiveSize = size;
      applyLiveSize();
    }
  } else if (m.type === "rawInput") {
    if (filmState.liveAllowed && !mmorpgEditing && !battleSpectating()) {
      jumpInput.update(m.jump === true);
      requestBattleMeteor(m.attack === true);
      input = {
        forward: m.forward,
        right: m.right,
        up: m.up,
        sprint: m.sprint,
        jump: m.jump === true,
        attack: m.attack === true,
        dance: m.dance === true,
      };
      battleScriptInput = undefined;
    }
  } else if (m.type === "look") {
    if (!filmState.liveAllowed) return;
    if (quickCamera && mode === "walk" && !battleFollowing()) return;
    battleLook(m.yaw, m.pitch);
  } else if (m.type === "clearBattleInput") {
    battleFollowCamera.cancelAttack();
    battleMeteorPending = false;
    battleScriptInput = undefined;
  } else if (m.type === "aim") {
    if (
      !filmState.liveAllowed ||
      !Number.isFinite(m.x) ||
      !Number.isFinite(m.y)
    )
      return;
    aimPointer = {
      x: Math.max(-1, Math.min(1, m.x)),
      y: Math.max(-1, Math.min(1, m.y)),
      locked: m.locked === true,
    };
  } else if (m.type === "dispose") {
    mmorpgSession?.disconnect(false);
    disposed = true;
    assetDownloadLifetime.abort();
    activeCpuProbe?.controller.abort();
    activeCpuProbe?.pool.dispose();
    activeCpuProbe = undefined;
    input = { forward: 0, right: 0, up: 0, sprint: false, jump: false };
    jumpInput.reset();
    runtimeOwnership.close();
    agentWorkSerial++;
    agentInput?.abort();
    clearAgentInput();
    scope.cancelAnimationFrame(frameId);
    activeResources = undefined;
    stagedResources = undefined;
    stream?.dispose();
    emptyWorld?.dispose();
    emptyWorld = undefined;
    renderer?.context.unconfigure();
    renderer?.dispose();
    scope.close();
  } else if (m.type === "pick") {
    if (!filmState.liveAllowed || playerMode.active ||
        (mmorpgSession && !mmorpgEditing)) return;
    let ray = direction(camera.yaw, camera.pitch);
    if (m.ndc !== undefined) {
      try {
        ray = rayFromNdc(multiply(
          perspective(camera.fov ?? 1, width / height, camera.near ?? .1, camera.far),
          lookAt(camera.eye, add(camera.eye, ray), camera.up ?? [0, 1, 0]),
        ), m.ndc);
      } catch { return; }
    }
    let closest = 80;
    selected = null;
    materialViewSerial++;
    commandQueue = commandQueue.then(() => clearMaterialPreview());
    for (const entity of snapshot.entities ?? []) {
      const bounds = catalog.get(entity.kind)?.bounds;
      if (!bounds) continue;
      const distance = rayBox(camera.eye, ray, bounds as Bounds, {
        position: entity.position,
        yaw: entity.yaw,
        scale: entity.scale,
      });
      if (distance !== null && distance < closest) {
        closest = distance;
        selected = entity.id;
      }
    }
  } else if (
    m.type === "agentAction" ||
    m.type === "agentInspect" ||
    m.type === "agentCapture" ||
    (m.type === "stream" && m.action !== "cancel") ||
    m.type === "game" ||
    m.type === "spellSwarm" ||
    m.type === "battle" ||
    m.type === "command" ||
    m.type === "author" ||
    m.type === "loadPlayable" ||
    m.type === "mesh" ||
    m.type === "materialView"
  ) {
    if (m.type.startsWith("agent")) m.agentTicket = agentWorkSerial;
    if (m.type === "materialView") {
      try {
        checkAuthorFrame(m, m);
      } catch (error) {
        receipt(m.requestId, false, errorMessage(error));
        return;
      }
      (m as any).ticket = ++materialViewSerial;
    } else materialViewSerial++;
    if (m.type === "author" || m.type === "loadPlayable") {
      try {
        checkAuthorFrame(m.action, m);
      } catch (error) {
        receipt(
          m.requestId,
          false,
          error instanceof Error ? error.message : "This change is too large.",
        );
        return;
      }
      m.ticket = authoring.supersede();
    }
    if (pendingCommands >= 16) {
      receipt(m.requestId, false, "Let the current edits finish first.");
      return;
    }
    pendingCommands++;
    commandQueue = commandQueue
      .then(() => handle(m))
      .finally(() => {
        pendingCommands--;
      });
  } else void handle(m);
};
