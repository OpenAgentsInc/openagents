import {
  validMmoServerMessage,
  MMO_UI_EVENT_LIMIT,
  MAX_MMO_STATE_TIME,
  type MmoActor,
  type MmoClientMessage,
  type MmoEvent,
  type MmoPrediction,
  type MmoSnapshot,
  type MmoUiAction,
  type MmoUiState,
} from "./mmorpg-protocol.ts";

export interface MmoControls {
  forward: number;
  turn: number;
  sprint: boolean;
  attack: boolean;
  dance: boolean;
}
interface PredictionReply {
  ok: boolean;
  prediction?: MmoPrediction;
  player?: any;
  message?: string;
}
interface PendingInput {
  sequence: number;
  input: MmoControls;
  ticks: number;
}
interface FrameSample {
  at: number;
  snapshot: MmoSnapshot;
}
const cloneActor = (actor: MmoActor): MmoActor => ({
  ...actor,
  position: [...actor.position],
});
const angle = (from: number, to: number, t: number) =>
  from + Math.atan2(Math.sin(to - from), Math.cos(to - from)) * t;
const lerp = (from: number, to: number, t: number) => from + (to - from) * t;
const neutral: MmoControls = {
  forward: 0,
  turn: 0,
  sprint: false,
  attack: false,
  dance: false,
};

/** Network I/O, prediction, and interpolation live outside UI state and renders. */
export class MmorpgSession {
  get tick() {
    return this.latest?.tick ?? 0;
  }
  state: MmoUiState;
  private socket?: WebSocket;
  private sequence = 0;
  private history: PendingInput[] = [];
  private samples: FrameSample[] = [];
  private latest?: MmoSnapshot;
  private predicted?: MmoPrediction;
  private events: MmoEvent[] = [];
  private lastEvent = 0;
  private lastSent = -Infinity;
  private lastControls = "";
  private accumulator = 0;
  private pending = new Map<
    string,
    {
      resolve: () => void;
      reject: (error: Error) => void;
      timer: ReturnType<typeof setTimeout>;
    }
  >();
  private joining?: {
    resolve: () => void;
    reject: (error: Error) => void;
    timer: ReturnType<typeof setTimeout>;
  };
  private generation = 0;
  private visualCorrection = [0, 0, 0];
  constructor(
    private options: {
      url: string;
      worldScale: number;
      release?: string;
      icons: MmoUiState["icons"];
      kernel: { request(message: object): PredictionReply };
      changed: (state: MmoUiState) => void;
      now?: () => number;
      socket?: (url: string) => WebSocket;
    },
  ) {
    this.state = {
      connection: "login",
      actors: [],
      events: [],
      icons: options.icons,
    };
  }
  private now() {
    return this.options.now?.() ?? performance.now();
  }
  private publish() {
    this.options.changed(this.state);
  }
  private prediction(message: object) {
    const reply = this.options.kernel.request(message);
    if (!reply.ok || !reply.prediction)
      throw Error(reply.message ?? "The character could not move.");
    this.predicted = reply.prediction;
    return reply;
  }
  async action(action: MmoUiAction, requestId: string): Promise<void> {
    if (action.type === "join") return this.join(action);
    if (
      this.state.connection !== "online" ||
      !this.socket ||
      this.socket.readyState !== 1
    )
      throw Error("Join the world before doing that.");
    if (this.pending.size >= 32 || this.pending.has(requestId))
      throw Error("Wait for the last action to finish.");
    return new Promise((resolve, reject) => {
      const timer = setTimeout(() => {
        this.pending.delete(requestId);
        reject(Error("The world did not answer. Try again."));
      }, 8000);
      this.pending.set(requestId, { resolve, reject, timer });
      this.send({ ...action, requestId });
    });
  }
  private join(action: Extract<MmoUiAction, { type: "join" }>) {
    this.disconnect(false);
    const generation = ++this.generation;
    this.state = {
      connection: "connecting",
      actors: [],
      events: [],
      icons: this.options.icons,
    };
    this.publish();
    return new Promise<void>((resolve, reject) => {
      const timer = setTimeout(
        () => this.fail("The world took too long to answer. Try again."),
        10000,
      );
      this.joining = { resolve, reject, timer };
      try {
        const socket = (this.options.socket ?? ((url) => new WebSocket(url)))(
          this.options.url,
        );
        this.socket = socket;
        socket.onopen = () => {
          if (generation === this.generation) this.send(action);
        };
        socket.onmessage = (message) => {
          if (generation !== this.generation) return;
          try {
            if (
              typeof message.data !== "string" ||
              message.data.length > 1024 * 1024
            )
              throw Error("The world sent an unsupported update.");
            const value: unknown = JSON.parse(message.data);
            if (!validMmoServerMessage(value))
              throw Error("The world sent an incomplete update.");
            if (value.type === "error") throw Error(value.message);
            if (value.type === "receipt") {
              const pending = this.pending.get(value.requestId);
              if (!pending) return;
              clearTimeout(pending.timer);
              this.pending.delete(value.requestId);
              if (value.ok) pending.resolve();
              else
                pending.reject(
                  Error(value.message ?? "That action could not finish."),
                );
              return;
            }
            if (value.type === "welcome") {
              if (
                !this.joining ||
                value.playerId !== value.snapshot.player.id ||
                Math.fround(value.world.worldScale) !==
                  Math.fround(this.options.worldScale) ||
                value.world.seed !== 1 ||
                value.world.sourceCommit !==
                  "547884332ca650abe96264f7230702d36481b9bc" ||
                (this.options.release &&
                  this.options.release !== value.world.release) ||
                !value.snapshot.player.prediction
              )
                throw Error("The world and character versions do not match.");
              this.prediction({
                type: "mmorpg_prediction_init",
                worldScale: this.options.worldScale,
                class: value.snapshot.player.class,
                state: value.snapshot.player.prediction,
              });
              this.sequence = 0;
              this.history = [];
              this.samples = [];
              this.events = [];
              this.lastEvent = 0;
              this.accumulator = 0;
              this.lastSent = -Infinity;
              this.lastControls = "";
              this.visualCorrection = [0, 0, 0];
              this.receive(value.snapshot, true);
              const joining = this.joining;
              this.joining = undefined;
              clearTimeout(joining.timer);
              joining.resolve();
            } else {
              if (this.state.connection !== "online")
                throw Error("Join the world before receiving updates.");
              if (this.latest && value.tick <= this.latest.tick) return;
              this.receive(value, false);
            }
          } catch (error) {
            this.fail(
              error instanceof Error
                ? error.message
                : "The world could not update.",
            );
          }
        };
        socket.onerror = () => {
          if (generation === this.generation)
            this.fail("The world could not connect. Try again.");
        };
        socket.onclose = () => {
          if (generation === this.generation)
            this.fail(
              "The connection closed. Join again to return.",
              "disconnected",
            );
        };
      } catch (error) {
        this.fail(
          error instanceof Error
            ? error.message
            : "The world could not connect.",
        );
      }
    });
  }
  private receive(snapshot: MmoSnapshot, first: boolean) {
    const now = this.now();
    if (
      !first &&
      this.latest &&
      (snapshot.player.id !== this.latest.player.id ||
        snapshot.player.class !== this.latest.player.class)
    )
      throw Error(
        "The world changed this character unexpectedly. Join again to return.",
      );
    if (!snapshot.player.prediction)
      throw Error("The world did not include character movement.");
    if (!first) {
      this.history = this.history.filter(
        (input) => input.sequence > snapshot.ackInput,
      );
      if (!snapshot.player.alive || this.latest?.player.alive === false)
        this.history = [];
      const old = this.predicted?.position;
      this.prediction({
        type: "mmorpg_prediction_reconcile",
        state: snapshot.player.prediction,
        inputs: this.history.map(({ input, ticks }) => ({ input, ticks })),
      });
      if (
        old &&
        this.predicted &&
        Math.hypot(...old.map((v, i) => v - this.predicted!.position[i])) < 2
      )
        this.visualCorrection = old.map(
          (v, i) => v + this.visualCorrection[i] - this.predicted!.position[i],
        );
      else this.visualCorrection = [0, 0, 0];
    }
    this.latest = snapshot;
    this.samples.push({ at: now, snapshot });
    while (this.samples.length > 8) this.samples.shift();
    for (const event of snapshot.events)
      if (event.id > this.lastEvent) {
        this.events.push(event);
        this.lastEvent = event.id;
      }
    this.events = this.events.slice(-MMO_UI_EVENT_LIMIT);
    this.state = {
      connection: "online",
      player: snapshot.player,
      actors: snapshot.actors,
      events: this.events,
      population: snapshot.population,
      icons: this.options.icons,
    };
    this.publish();
  }
  private send(message: MmoClientMessage) {
    if (this.socket?.readyState === 1 && this.socket.bufferedAmount < 65536)
      this.socket.send(JSON.stringify(message));
  }
  step(elapsed: number, controls: MmoControls) {
    if (this.state.connection !== "online" || !this.latest || !this.predicted)
      return undefined;
    const now = this.now();
    this.accumulator = Math.min(
      this.accumulator + Math.max(0, elapsed),
      4 / 60,
    );
    const ticks = Math.floor((this.accumulator + 1e-9) * 60);
    this.accumulator -= ticks / 60;
    if (ticks) {
      const input = this.latest.player.alive ? controls : neutral;
      this.prediction({ type: "mmorpg_prediction_step", input, ticks });
      this.history.push({
        sequence: ++this.sequence,
        input: { ...input },
        ticks,
      });
      if (this.history.reduce((n, entry) => n + entry.ticks, 0) > 120) {
        this.fail(
          "The world stopped answering. Join again to return.",
          "disconnected",
        );
        return undefined;
      }
      const serialized = JSON.stringify(input);
      if (serialized !== this.lastControls || now - this.lastSent >= 50) {
        this.send({ type: "input", sequence: this.sequence, ...input });
        this.lastControls = serialized;
        this.lastSent = now;
      }
    }
    const prediction = this.predicted;
    const local = cloneActor(this.latest.player);
    const decay = Math.exp(-Math.max(0, elapsed) * 15);
    this.visualCorrection = this.visualCorrection.map((v) => v * decay);
    local.position = prediction.position.map(
      (v, i) => v + this.visualCorrection[i],
    ) as [number, number, number];
    local.yaw = prediction.yaw;
    local.state = prediction.state;
    local.actionTime = local.animationTime = prediction.stateTicks / 60;
    local.prediction = prediction;
    const target = now - 100;
    let older = this.samples[0],
      newer = this.samples.at(-1)!;
    for (let i = 1; i < this.samples.length; i++)
      if (this.samples[i].at >= target) {
        older = this.samples[i - 1];
        newer = this.samples[i];
        break;
      }
    const t = Math.max(
      0,
      Math.min(1, (target - older.at) / Math.max(1, newer.at - older.at)),
    );
    const previous = new Map(
      older.snapshot.actors.map((actor) => [actor.id, actor]),
    );
    const actors = newer.snapshot.actors.map((actor) => {
      const from = previous.get(actor.id);
      const result = cloneActor(actor);
      if (from) {
        result.position = result.position.map((v, i) =>
          lerp(from.position[i], v, t),
        ) as [number, number, number];
        result.yaw = angle(from.yaw, result.yaw, t);
      }
      const source = from && from.state === actor.state ? from : actor;
      result.actionTime = Math.min(
        MAX_MMO_STATE_TIME,
        source.actionTime +
          Math.max(0, target - (source === from ? older.at : newer.at)) / 1000,
      );
      result.animationTime = Math.min(
        MAX_MMO_STATE_TIME,
        source.animationTime +
          Math.max(0, target - (source === from ? older.at : newer.at)) / 1000,
      );
      return result;
    });
    return { local, actors, prediction };
  }
  presentation(local: MmoActor, actors: MmoActor[]) {
    return { ...this.state, player: local, actors };
  }
  disconnect(publish = true, failure?: Error) {
    this.generation++;
    this.socket?.close();
    this.socket = undefined;
    const error = failure ?? Error("The world connection closed.");
    if (this.joining) {
      clearTimeout(this.joining.timer);
      this.joining.reject(error);
      this.joining = undefined;
    }
    for (const pending of this.pending.values()) {
      clearTimeout(pending.timer);
      pending.reject(error);
    }
    this.pending.clear();
    this.history = [];
    this.samples = [];
    this.latest = undefined;
    this.predicted = undefined;
    if (publish) {
      this.state = { ...this.state, connection: "disconnected", actors: [] };
      this.publish();
    }
  }
  private fail(
    message: string,
    connection: "error" | "disconnected" = "error",
  ) {
    this.disconnect(false, Error(message));
    this.state = { ...this.state, connection, message, actors: [] };
    this.publish();
  }
}
