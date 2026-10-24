import { CharacterCamera } from "../renderer/character-camera.ts";
import type { Camera } from "../renderer/renderer.ts";
import {
  direction,
  add,
  scale,
  viewAngles,
  type V3,
} from "../renderer/math.ts";
import { selectedCameraVignette } from "../renderer/vignette.ts";
import { sunAngles } from "../renderer/lighting.ts";
import {
  FrameMetrics,
  validateFrameRequest,
  type FrameBenchmarkRequest,
} from "./frame-metrics.ts";
import { referenceCameraLook } from "../renderer/reference-camera-look.ts";
import { CharacterLocomotion } from "./character-state.ts";
import {
  updateCharacterFrame,
  stepRuntime,
  runtimeResult,
  pointerLook,
} from "./character-driver.ts";
import { RuntimeKernel } from "./kernel.ts";
import { installStaticCollision } from "./static-collision.ts";
import { StarterGame } from "./starter-game.ts";
import { WorldClock, validWorldAnimationTime } from "./world-clock.ts";
import { JumpInput } from "./jump-input.ts";
import { GameplayVisualSync } from "./gameplay-sync.ts";
import { FramePacer } from "./frame-pacer.ts";
import {
  prepareGameplayCast,
  type SpellForecastMetrics,
} from "./spell-forecast.ts";
import { assertNativeStartCapture } from "./capture-actors.ts";
import { isDruidAbility, type DruidAbility } from "./druid-abilities.ts";
import type { preparePublishedWorld } from "./published-world.ts";
import type { DetailDriver } from "./detail-driver.ts";

type Published = Awaited<ReturnType<typeof preparePublishedWorld>>;

/** One standalone world owner. UI callbacks supply intentions and never own time. */
export class WorldSession {
  readonly characterCamera = new CharacterCamera();
  readonly clock = new WorldClock();
  readonly jump = new JumpInput();
  readonly camera: Camera;
  readonly kernel: RuntimeKernel;
  readonly renderer: NonNullable<Published["prepared"]>["renderer"];
  readonly environment: NonNullable<Published["prepared"]>["environment"];
  readonly game?: StarterGame;
  private locomotion?: CharacterLocomotion;
  private current: any;
  private disposed = false;
  private casting = 0;
  private castPending = false;
  private lastSpellPreparation?: SpellForecastMetrics;
  private mode: "walk" | "fly";
  private previous = 0;
  private sequence = 0;
  private gameplay: any;
  private readonly gameplayVisuals = new GameplayVisualSync();
  private readonly frameMetrics = new FrameMetrics();
  private readonly framePacer = new FramePacer();
  private detailDriver?: DetailDriver;
  private viewport?: { width: number; height: number };
  /** GPU timestamps resolve a few frames late; read this far back. */
  private static readonly GPU_SAMPLE_LAG = 4;
  private benchmarkCamera?: Camera;
  private benchmarkStarted = 0;
  private hostClampedSeconds = 0;
  private stoppedBenchmarkCounters?: ReturnType<
    WorldSession["benchmarkCounters"]
  >;
  private benchmarkCounters() {
    return {
      frameAttempts: this.spans.attempts,
      submittedFrames: this.spans.submissions,
      skippedFrames: this.spans.skipped,
      rawCallbacks: this.spans.rawCallbacks,
      intentionalPacedCallbacks: this.spans.pacedCallbacks,
      kernelDroppedSeconds: this.spans.droppedSeconds,
      hostClampedSeconds: this.hostClampedSeconds,
    };
  }
  private held = { forward: 0, right: 0, up: 0, sprint: false, jump: false };
  private yawDelta = 0;
  private pitchDelta = 0;
  readonly spans = {
    rawCallbacks: 0,
    pacedCallbacks: 0,
    attempts: 0,
    submissions: 0,
    skipped: 0,
    stepMs: 0,
    renderCpuMs: 0,
    droppedSeconds: 0,
  };

  private constructor(
    readonly source: Published,
    kernel: RuntimeKernel,
  ) {
    const prepared = source.prepared!,
      world = source.world!;
    this.kernel = kernel;
    this.mode = world.mode;
    this.renderer = prepared.renderer;
    this.environment = { ...prepared.environment };
    if (source.artifact.worldId === "medieval-town-lab")
      Object.assign(
        this.environment,
        referenceCameraLook("MT-02", this.environment.mode),
      );
    this.camera = {
      eye: [...prepared.scene.spawn.position] as V3,
      ...viewAngles(
        prepared.scene.spawn.position as V3,
        prepared.scene.spawn.target as V3,
      ),
    };
    if (prepared.character)
      this.locomotion = new CharacterLocomotion(prepared.character.clips);
    if (source.artifact.profile === "room")
      this.characterCamera.setPerspective("first", true);
    const gameplay =
      this.environment.mode === "night"
        ? prepared.nightGameplay
        : prepared.dayGameplay;
    installStaticCollision(
      kernel,
      this.environment.mode === "night"
        ? prepared.nightCollision
        : prepared.dayCollision,
      gameplay?.objects.length,
    );
    this.current = runtimeResult(
      kernel.request({
        type: "init",
        entities: world.entities,
        colliders: world.colliders,
        rooms: world.rooms,
        groundY: world.groundY,
        spawn: world.spawn,
      }),
    );
    if (world.game) this.game = new StarterGame(world.game);
    this.update(this.current.player, 0);
    this.initializeGameplay();
  }

  static async create(source: Published) {
    if (!source.prepared || !source.world || !source.kernelBytes)
      throw new Error("Open a playable world before starting its controller.");
    let session: WorldSession | undefined;
    try {
      const kernel = await RuntimeKernel.fromBytes(source.kernelBytes);
      session = new WorldSession(source, kernel);
      const catalog = source.catalog;
      const assets = new Map(
        (catalog?.assets ?? []).map((asset) => [asset.id, asset]),
      );
      await session.renderer.updateAuthored(
        session.current,
        new Map([...assets].map(([id, asset]) => [id, asset.mesh])),
        assets,
      );
      return session;
    } catch (error) {
      session?.dispose();
      source.prepared.renderer.dispose();
      throw error;
    }
  }

  private initializeGameplay() {
    this.gameplayVisuals.reset();
    const data =
      this.environment.mode === "night"
        ? this.source.prepared!.nightGameplay
        : this.source.prepared!.dayGameplay;
    const objects = data?.objects ?? [];
    for (let start = 0; start < Math.max(1, objects.length); start += 8192)
      runtimeResult(
        this.kernel.request({
          type: "gameplay_init",
          objects: objects.slice(start, start + 8192),
          seed: 12999,
          append: start > 0,
          complete: false,
        }),
      );
    const state = runtimeResult(
      this.kernel.request({
        type: "gameplay_init",
        objects: [],
        append: true,
        complete: true,
        targetGroups: data?.manifest.targetGroups,
      }),
    );
    if (objects.length)
      this.renderer.setGameplayObjects(
        this.environment.mode,
        objects,
        data?.manifest.targetGroups,
      );
    const enemies =
      this.source.prepared!.enemyPatrols?.scenarios[this.environment.mode];
    const ready = enemies
      ? runtimeResult(
          this.kernel.request({ type: "gameplay_init_enemies", enemies }),
        )
      : state;
    this.syncGameplay(ready.gameplay);
  }

  private syncGameplay(value: any) {
    if (!value) return;
    this.gameplay = value;
    if (value.enemies) this.renderer.setEnemies(value.enemies.actors);
    this.gameplayVisuals.apply(this.renderer, this.environment.mode, value);
  }

  private update(player: any, elapsed: number) {
    this.current.player = player;
    this.renderer.setCharacter(
      updateCharacterFrame(
        this.kernel,
        player,
        this.mode,
        this.camera,
        this.characterCamera,
        this.locomotion,
        this.source.world!.groundY,
        elapsed,
      ),
    );
  }

  resize(width: number, height: number) {
    this.assertOpen();
    this.viewport = { width, height };
    const scale = this.renderer.detail.renderScale;
    this.renderer.resize(
      Math.max(1, Math.round(width * scale)),
      Math.max(1, Math.round(height * scale)),
    );
    this.detailDriver?.controller.reset();
  }
  /**
   * Let a detail driver steer the renderer's quality switches from measured
   * frames (ADR 0007). Without one the renderer stays at full detail.
   */
  attachDetail(driver: DetailDriver | undefined) {
    this.assertOpen();
    this.detailDriver = driver;
    this.applyDetail();
  }
  get detail() {
    return this.detailDriver;
  }
  /** Pin Detail, for a capture; `undefined` resumes control. */
  holdDetail(value: number | undefined) {
    if (!this.detailDriver) return;
    const changed =
      value === undefined
        ? this.detailDriver.release()
        : this.detailDriver.hold(value);
    if (changed) this.applyDetail();
  }
  private applyDetail() {
    const next = this.detailDriver?.renderer();
    if (!next) return;
    const scaleChanged = next.renderScale !== this.renderer.detail.renderScale;
    this.renderer.setDetail(next);
    if (scaleChanged && this.viewport)
      this.resize(this.viewport.width, this.viewport.height);
  }
  /** The profile and current Detail, for frame and failure reports. */
  detailReport() {
    return this.detailDriver?.report() ?? null;
  }
  input(value: Partial<typeof this.held>) {
    if (this.disposed) return;
    if (
      !value ||
      typeof value !== "object" ||
      Array.isArray(value) ||
      Object.keys(value).some(
        (key) => !["forward", "right", "up", "sprint", "jump"].includes(key),
      )
    )
      throw new Error("Choose supported movement controls.");
    for (const key of ["forward", "right", "up"] as const)
      if (
        key in value &&
        (typeof value[key] !== "number" ||
          !Number.isFinite(value[key]) ||
          Math.abs(value[key]!) > 1)
      )
        throw new Error("Use movement values from -1 to 1.");
    for (const key of ["sprint", "jump"] as const)
      if (key in value && typeof value[key] !== "boolean")
        throw new Error("Use pressed or released movement buttons.");
    this.held = { ...this.held, ...value };
    this.jump.update(this.held.jump);
  }
  clearInput() {
    this.held = { forward: 0, right: 0, up: 0, sprint: false, jump: false };
    this.yawDelta = this.pitchDelta = 0;
    this.jump.reset();
    this.previous = 0;
    this.framePacer.reset();
  }
  look(dx: number, dy: number) {
    this.assertOpen();
    const change = pointerLook(dx, dy);
    this.yawDelta += change.yawDelta;
    this.pitchDelta += change.pitchDelta;
    this.camera.up = undefined;
  }
  perspective(value: "first" | "third") {
    this.assertOpen();
    if (value !== "first" && value !== "third")
      throw new Error("Choose First person or Third person.");
    if (this.source.artifact.profile === "room" && value !== "first")
      throw new Error("This world uses a first-person view.");
    this.characterCamera.setPerspective(value);
    this.update(this.current.player, 0);
  }

  tick(now: number) {
    const callbackEnteredAt = this.frameMetrics.recording
      ? performance.now()
      : undefined;
    if (this.disposed) return false;
    if (
      !Number.isFinite(now) ||
      now < 0 ||
      now > Number.MAX_SAFE_INTEGER ||
      (this.previous && now < this.previous)
    )
      throw new Error("Use a monotonic finite frame timestamp.");
    const due = this.framePacer.due(now);
    this.spans.rawCallbacks++;
    this.spans.pacedCallbacks += Number(!due);
    this.frameMetrics.observeCallback(due);
    if (!due) return false;
    const time = this.clock.sample(now);
    const observedInterval = this.previous ? now - this.previous : 1000 / 60;
    const elapsed = Math.min(observedInterval / 1000, 0.1);
    this.hostClampedSeconds += Math.max(0, observedInterval / 1000 - elapsed);
    this.previous = now;
    const measured =
      !this.frameMetrics.recording || this.frameMetrics.instrumented;
    const started = measured ? performance.now() : 0;
    this.current = stepRuntime(this.kernel, elapsed, {
      ...this.held,
      yawDelta: this.yawDelta,
      pitchDelta: this.pitchDelta,
      jump: this.jump.sample(),
    });
    const bridge =
      this.frameMetrics.recording && this.frameMetrics.instrumented
        ? this.kernel.lastRequestTimings
        : null;
    this.yawDelta = this.pitchDelta = 0;
    this.update(this.current.player, elapsed);
    this.syncGameplay(this.current.gameplay);
    const stepMs = measured ? performance.now() - started : null;
    this.spans.stepMs += stepMs ?? 0;
    this.spans.droppedSeconds = this.current.droppedSeconds ?? 0;
    let camera = this.camera;
    if (
      this.frameMetrics.recording &&
      this.frameMetrics.route === "sway" &&
      this.benchmarkCamera
    ) {
      const base = this.benchmarkCamera;
      const phase =
        ((performance.now() - this.benchmarkStarted) / 20_000) * Math.PI * 2;
      const shift = Math.sin(phase);
      camera = {
        ...base,
        eye: [
          base.eye[0] + Math.cos(base.yaw) * shift,
          base.eye[1] + Math.sin(phase * 2) * 0.15,
          base.eye[2] + Math.sin(base.yaw) * shift,
        ],
        yaw: base.yaw + Math.sin(phase) * 0.12,
        pitch: base.pitch + Math.sin(phase * 2) * 0.025,
      };
    }
    const renderStarted = measured ? performance.now() : 0;
    const submitted = this.render(time, camera);
    if (this.detailDriver && submitted && !this.frameMetrics.recording) {
      const gpuMs = this.renderer.gpuFrameSample(
        this.renderer.frame - WorldSession.GPU_SAMPLE_LAG,
      );
      if (this.detailDriver.observe({ interval_ms: observedInterval, gpu_ms: gpuMs }))
        this.applyDetail();
    }
    if (this.frameMetrics.recording) {
      const profiled = this.frameMetrics.instrumented;
      const geometry = this.renderer.runtimeInspection();
      this.frameMetrics.record({
        frame: this.renderer.frame,
        at: now,
        callbackEnteredAt,
        elapsedMs: observedInterval,
        stepMs: profiled ? stepMs : null,
        renderCpuMs: profiled ? performance.now() - renderStarted : null,
        cullMs: profiled ? this.renderer.cullCpuMs : null,
        clusterMs: profiled ? this.renderer.clusterCpuMs : null,
        bridgeEncodeMs: bridge?.encodeMs ?? null,
        wasmRequestMs: bridge?.wasmRequestMs ?? null,
        bridgeDecodeMs: bridge?.decodeMs ?? null,
        bridgeOverheadMs: bridge?.overheadMs ?? null,
        commandEncodeMs: profiled ? this.renderer.commandEncodeMs : null,
        kernelDroppedSeconds: this.spans.droppedSeconds,
        hostClampedSeconds: this.hostClampedSeconds,
        cameraEye: [...camera.eye],
        cameraYaw: camera.yaw,
        cameraPitch: camera.pitch,
        animationTime: time,
        submittedInstances: submitted ? geometry.submittedInstances : 0,
        submittedTriangles: submitted ? geometry.submittedTriangles : 0,
        ...(submitted && profiled ? { sourceRenderCommands: this.renderer.sourceCommandCounts() } : {}),
        submitted,
        gpuMs: null,
      });
    }
    return submitted;
  }
  render(time: number, camera = this.camera) {
    if (this.disposed) return false;
    if (!validWorldAnimationTime(time))
      throw new Error("Choose a supported world animation time.");
    const measured =
      !this.frameMetrics.recording || this.frameMetrics.instrumented;
    const started = measured ? performance.now() : 0;
    const submitted = this.renderer.render(camera, this.environment, time);
    this.spans.attempts++;
    this.spans.submissions += Number(submitted);
    this.spans.skipped += Number(!submitted);
    this.spans.renderCpuMs += measured ? performance.now() - started : 0;
    return submitted;
  }
  async benchmark(
    request: FrameBenchmarkRequest,
    diagnosticNativeStart = false,
  ) {
    this.assertOpen();
    validateFrameRequest(request);
    const actorReference =
      request.op === "pinActors"
        ? await (async () => {
            if (this.frameMetrics.recording)
              throw new Error("Stop measurement before pinning actors.");
            if (request.canonical) {
              assertNativeStartCapture({
                diagnostic: diagnosticNativeStart,
                recording: this.frameMetrics.recording,
                preparing: !!this.castPending,
                gameplay: this.gameplay,
              });
              const enemies =
                this.source.prepared!.enemyPatrols?.scenarios[
                  this.environment.mode
                ];
              if (!enemies)
                throw new Error(
                  "Prepare native knight routes before pinning their start poses.",
                );
              this.syncGameplay(
                runtimeResult(
                  this.kernel.request({
                    type: "gameplay_init_enemies",
                    enemies,
                  }),
                ).gameplay,
              );
            }
            return this.renderer.pinCaptureActors(
              this.source.artifact.sourceIdentity,
              this.environment.mode,
              request.canonical === true,
            );
          })()
        : undefined;
    if (request.op === "start") {
      // Measured frames run at one fixed Detail so results stay comparable.
      this.detailDriver?.hold();
      this.stoppedBenchmarkCounters = undefined;
      this.frameMetrics.start(request);
      this.framePacer.configure(this.frameMetrics.targetHz);
      this.benchmarkCamera = structuredClone(this.camera);
      this.benchmarkStarted = performance.now();
      this.renderer.startFrameRecording(this.frameMetrics.capacity);
      this.renderer.setBenchmarkEnabled(this.frameMetrics.instrumented);
      this.kernel.setProfiling(this.frameMetrics.instrumented);
    } else if (request.op === "stop") {
      this.frameMetrics.stop();
      this.framePacer.configure("display");
      this.stoppedBenchmarkCounters = this.benchmarkCounters();
      this.renderer.stopFrameRecording();
      this.renderer.setBenchmarkEnabled(false);
      this.kernel.setProfiling(false);
    }
    if (request.drain) await this.renderer.drain();
    const detail = this.detailReport();
    if (request.op === "stop") this.holdDetail(undefined);
    const result = this.frameMetrics.result(
      "player",
      {
        renderer: this.renderer.benchmarkInfo(),
        kernel: this.kernel.benchmarkInfo(),
        profile: this.source.artifact.profile,
        worldId: this.source.artifact.worldId,
        sourceIdentity: this.source.artifact.sourceIdentity,
        ...(actorReference ? { actorReference } : {}),
        recordingCamera: this.benchmarkCamera,
        routeStartedAtMs: this.benchmarkStarted,
        environment: { ...this.environment },
        frameCounters:
          this.stoppedBenchmarkCounters ?? this.benchmarkCounters(),
        timingScope: {
          gpuMs: "Main HDR pass only.",
          fullGpuMs:
            "Envelope of actual render/compute passes; excludes copies/uploads outside passes, query resolution, and browser composition.",
          completion:
            "Queue-fence callback notification, not compositor presentation.",
          presentation: "Unavailable.",
        },
      },
      (frame) => this.renderer.gpuSample(frame),
      (frame) => this.renderer.gpuFrameSample(frame),
      this.renderer.completedFrameSnapshot(),
    );
    return { ...result, detail };
  }
  start(now: number) {
    this.assertOpen();
    this.clock.start(now);
    this.previous = 0;
    this.framePacer.reset();
  }
  inspect() {
    this.assertOpen();
    const state = this.kernel.request<any>({ type: "snapshot" });
    return {
      worldId: this.source.artifact.worldId,
      sourceIdentity: this.source.artifact.sourceIdentity,
      profile: this.source.artifact.profile,
      mode: this.mode,
      state,
      game: this.game?.snapshot(),
      gameConfig: this.game?.config(),
      camera: structuredClone(this.camera),
      perspective: this.characterCamera.perspective,
      input: { ...this.held },
      render: this.renderer.runtimeInspection(),
      enemyRender: this.renderer.enemyStats,
      renderer: this.renderer.benchmarkInfo(),
      kernel: this.kernel.benchmarkInfo(),
      spellPreparation: this.lastSpellPreparation,
      spans: { ...this.spans },
      detail: this.detailReport(),
    };
  }
  interact() {
    this.assertOpen();
    if (this.mode !== "walk") throw new Error("Switch to Walk to interact.");
    if (!this.game) throw new Error("This world has no nearby interaction.");
    const result = this.game.apply(
      { type: "interact" },
      this.current.player.position,
    );
    if (!result.ok) throw new Error(result.message);
    return result;
  }
  reset() {
    this.assertOpen();
    const world = this.source.world!;
    const mode = this.game ? "walk" : world.spawn.mode;
    const result = runtimeResult(
      this.kernel.request({ type: "set_camera", ...world.spawn, mode }),
    );
    this.casting++;
    this.mode = mode;
    this.current = result;
    this.clearInput();
    this.characterCamera.resetCollision();
    this.update(result.player, 0);
    return this.game?.apply({ type: "reset" }, result.player.position);
  }
  /** Verification camera admission still passes through the same Rust controller. */
  setCamera(eye: V3, target: V3, mode: "walk" | "fly" = "walk") {
    this.assertOpen();
    if (
      ![eye, target].every(
        (vector) =>
          Array.isArray(vector) &&
          vector.length === 3 &&
          vector.every((v) => Number.isFinite(v) && Math.abs(v) < 20_000),
      ) ||
      Math.hypot(...target.map((v, i) => v - eye[i])) < 0.01 ||
      !["walk", "fly"].includes(mode)
    )
      throw new Error(
        "Choose a valid camera position, target, and movement mode.",
      );
    const angles = viewAngles(eye, target);
    const value = runtimeResult(
      this.kernel.request({
        type: "set_camera",
        position: [eye[0], eye[1] - 1.62, eye[2]],
        ...angles,
        mode,
      }),
    );
    this.casting++;
    this.mode = mode;
    this.clearInput();
    this.characterCamera.resetCollision();
    this.current = value;
    this.camera.up = undefined;
    this.camera.fov = undefined;
    this.camera.vignette = undefined;
    this.update(value.player, 0);
    return value;
  }
  /** Scenario changes preserve the controller and require an undamaged source world. */
  scenario(value: "day" | "night") {
    this.assertOpen();
    if (value !== "day" && value !== "night")
      throw new Error("Choose day or night.");
    if (value === this.environment.mode) return;
    if (
      this.castPending ||
      this.gameplay?.destroyedCount ||
      this.gameplay?.castle?.active ||
      this.gameplay?.activeCast ||
      this.gameplay?.pendingImpacts ||
      this.gameplay?.debris?.length
    )
      throw new Error(
        "Restore the town before switching between day and night.",
      );
    const prepared = this.source.prepared!;
    const next = prepared.profiles.apply(
      this.environment,
      { ...this.environment, mode: value },
      true,
    );
    const sun = prepared.look?.scenarios[value].sun;
    if (sun) Object.assign(next, sunAngles(sun));
    installStaticCollision(
      this.kernel,
      value === "day" ? prepared.dayCollision : prepared.nightCollision,
      (value === "day" ? prepared.dayGameplay : prepared.nightGameplay)?.objects
        .length,
    );
    Object.assign(this.environment, next);
    this.clearInput();
    this.characterCamera.resetCollision();
    this.initializeGameplay();
    this.update(this.current.player, 0);
  }
  restoreGameplay() {
    this.assertOpen();
    if (this.castPending)
      throw new Error("Wait for the current spell to finish preparing.");
    const restored = runtimeResult(
      this.kernel.request({ type: "gameplay_reset" }),
    );
    this.casting++;
    this.clearInput();
    this.current = restored;
    this.gameplayVisuals.reset();
    this.renderer.resetGameplay(this.environment.mode);
    this.syncGameplay(restored.gameplay);
  }
  /** Only cameras from this published source can select a reference composition. */
  referenceShot(id: string) {
    this.assertOpen();
    const look = this.source.prepared!.look;
    const match = typeof id === "string" ? look?.galleryShots[id] : undefined;
    const camera =
      match &&
      look?.scenarios[match.scenario].cameras.find(
        (item) => item.id === match.cameraId,
      );
    if (!match || !camera) throw new Error("Choose a source reference camera.");
    const fov = (camera.verticalFovDegrees * Math.PI) / 180;
    if (
      !Number.isFinite(fov) ||
      fov <= 0 ||
      fov >= Math.PI ||
      !Array.isArray(camera.up) ||
      camera.up.length !== 3 ||
      !camera.up.every((value) => Number.isFinite(value)) ||
      Math.hypot(...camera.up) < 0.001
    )
      throw new Error("This source camera has invalid lens settings.");
    if (
      ![camera.position, camera.target].every(
        (vector) =>
          Array.isArray(vector) &&
          vector.length === 3 &&
          vector.every(
            (value) => Number.isFinite(value) && Math.abs(value) < 20_000,
          ),
      )
    )
      throw new Error("This source camera has an invalid position or target.");
    const heading = camera.target.map(
      (value, index) => value - camera.position[index],
    );
    const cross = [
      heading[1] * camera.up[2] - heading[2] * camera.up[1],
      heading[2] * camera.up[0] - heading[0] * camera.up[2],
      heading[0] * camera.up[1] - heading[1] * camera.up[0],
    ];
    if (Math.hypot(...heading) < 0.01 || Math.hypot(...cross) < 0.001)
      throw new Error(
        "This source camera needs a valid viewing direction and up vector.",
      );
    const vignette = selectedCameraVignette(camera);
    this.scenario(match.scenario);
    this.setCamera(camera.position as V3, camera.target as V3, "fly");
    this.camera.fov = fov;
    this.camera.up = [...camera.up] as V3;
    this.camera.vignette = vignette;
    this.camera.near = undefined;
    const grade = referenceCameraLook(id, match.scenario);
    if (grade) Object.assign(this.environment, grade);
    return {
      id,
      scenario: match.scenario,
      camera: structuredClone(this.camera),
      environment: { ...this.environment },
    };
  }
  aim() {
    this.assertOpen();
    return runtimeResult(
      this.kernel.request({
        type: "gameplay_aim",
        origin: this.camera.eye,
        direction: direction(this.camera.yaw, this.camera.pitch),
        maxDistance: 80,
      }),
    ).aim;
  }
  async cast(ability: DruidAbility) {
    this.assertOpen();
    if (!isDruidAbility(ability)) throw new Error("Choose an available spell.");
    if (this.castPending)
      throw new Error("Wait for the current spell to finish preparing.");
    if (!this.gameplay?.objectCount)
      throw new Error("This world has no spell targets.");
    const ticket = ++this.casting;
    this.castPending = true;
    try {
      const origin = [
        ...(this.mode === "walk" ? this.current.player.eye : this.camera.eye),
      ] as V3;
      const forward = direction(this.camera.yaw, this.camera.pitch);
      const aim = runtimeResult(
        this.kernel.request({
          type: "gameplay_aim",
          origin: this.camera.eye,
          direction: forward,
          maxDistance: 80,
        }),
      ).aim;
      const destination =
        ability === "destroy_castle" && this.gameplay.castle?.target
          ? this.gameplay.castle.target
          : (aim?.point ?? add(this.camera.eye, scale(forward, 30.48)));
      const delta = destination.map(
        (v: number, i: number) => v - origin[i],
      ) as V3;
      const request = {
        castId: `player-${++this.sequence}`,
        ability,
        origin,
        direction:
          Math.hypot(...delta) > 0.001
            ? scale(delta, 1 / Math.hypot(...delta))
            : forward,
      };
      this.lastSpellPreparation = undefined;
      const cancelled = () => this.disposed || this.casting !== ticket;
      const { plan, metrics } = await prepareGameplayCast(
        this.kernel,
        request,
        {
          cancelled,
          prepareVisuals: (plan) =>
            this.renderer.prepareGameplayDestructionAsync(
              this.environment.mode,
              plan.prospectiveDestroyed,
              cancelled,
            ),
        },
      );
      this.lastSpellPreparation = metrics;
      const result = runtimeResult(
        this.kernel.request({
          type: "gameplay_cast",
          ...request,
          expectedVersion: plan.version,
          fragmentMasks: this.renderer.gameplayFragmentMasks(
            this.environment.mode,
            plan.prospectiveDestroyed,
          ),
          ...(ability === "destroy_castle"
            ? {
                castleFragments: this.renderer.castleFragmentMasks(
                  this.environment.mode,
                ),
              }
            : {}),
        }),
      );
      this.syncGameplay(result.gameplay);
      return result;
    } finally {
      this.castPending = false;
    }
  }
  cancelCast() {
    this.assertOpen();
    this.casting++;
    const result = runtimeResult(
      this.kernel.request({ type: "gameplay_cancel_cast" }),
    );
    this.syncGameplay(result.gameplay);
    return result;
  }
  private assertOpen() {
    if (this.disposed) throw new Error("This world has closed.");
  }
  dispose() {
    if (this.disposed) return;
    this.disposed = true;
    this.casting++;
    this.clearInput();
    this.renderer.dispose();
  }
}
