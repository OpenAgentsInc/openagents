import type { EditorService } from "../../../packages/editor-services/service.ts";
import {
  editorAtoms,
  type Receipt,
} from "../../../packages/editor-services/state.ts";
import {
  canEditMmorpg,
  validBattleAction,
  type BattleAction,
  type EngineStatus,
} from "../../../packages/editor-services/protocol.ts";
import type { MmoUiAction } from "../../../packages/runtime/mmorpg-protocol.ts";

export interface BattleLinkAccess {
  profile: string;
  publicDemo: boolean;
  pathname: string;
  readOnly: boolean;
}
const permitted = (access: BattleLinkAccess) =>
  access.profile === "open-ground" &&
  canEditMmorpg(access.publicDemo, access.pathname, access.readOnly);

/** A malformed count opens the small default rather than allocating a large army. */
export function battleLinkCount(
  search: string,
  access: BattleLinkAccess,
): number | undefined {
  if (!permitted(access)) return;
  const values = new URLSearchParams(search).getAll("battle");
  if (!values.length) return;
  const value =
    values.length === 1 && /^\d{1,7}$/.test(values[0])
      ? Number(values[0])
      : NaN;
  return validBattleAction({ op: "start", count: value }) ? value : 10_000;
}

type LinkStatus = Pick<EngineStatus, "phase" | "mmorpg" | "battle">;
export interface BattleLinkPort {
  snapshot(): LinkStatus;
  receipt(id: string): Pick<Receipt, "ok" | "message"> | undefined;
  subscribe(listener: () => void): () => void;
  mmorpg(action: MmoUiAction): string;
  battle(action: BattleAction): string;
  error(message: string): void;
  timeout(callback: () => void, milliseconds: number): () => void;
}
export interface BattleLinkOwner {
  battleLink?: BattleLinkBootstrap;
}

/** One page-lifetime launch; UI remounts and Stop never repeat its commands. */
export class BattleLinkBootstrap {
  phase:
    | "waiting"
    | "joining"
    | "starting"
    | "camera"
    | "done"
    | "failed"
    | "cancelled" = "waiting";
  private pending?: string;
  private unsubscribe = () => {};
  private cancelTimeout = () => {};
  private advancing = false;
  constructor(
    private readonly count: number,
    private readonly port: BattleLinkPort,
    private readonly access: () => BattleLinkAccess,
  ) {}
  start(): void {
    this.unsubscribe = this.port.subscribe(() => this.advance());
    this.cancelTimeout = this.port.timeout(
      () => this.fail("Couldn't open Battle Lab. Reload to try again."),
      120_000,
    );
    this.advance();
  }
  dispose(): void {
    if (!this.finished()) this.phase = "cancelled";
    this.cleanup();
  }
  private finished(): boolean {
    return ["done", "failed", "cancelled"].includes(this.phase);
  }
  private cleanup(): void {
    this.unsubscribe();
    this.cancelTimeout();
  }
  private fail(message: string): void {
    if (this.finished()) return;
    this.phase = "failed";
    this.cleanup();
    this.port.error(message);
  }
  private send(
    phase: "joining" | "starting" | "camera",
    action: () => string,
  ): void {
    this.phase = phase;
    this.pending = action();
  }
  private advance(): void {
    if (this.finished() || this.advancing) return;
    this.advancing = true;
    try {
      if (!permitted(this.access())) {
        this.dispose();
        return;
      }
      const status = this.port.snapshot();
      if (status.phase === "error") {
        this.fail("Couldn't open the world. Reload to try again.");
        return;
      }
      if (status.phase !== "ready" || !status.mmorpg) return;
      const connection = status.mmorpg.connection;
      if (connection === "error" || connection === "disconnected") {
        this.fail("Couldn't join the world. Reload to try again.");
        return;
      }
      const receipt = this.pending
        ? this.port.receipt(this.pending)
        : undefined;
      if (receipt && !receipt.ok) {
        this.fail(
          receipt.message ||
            "Couldn't open Battle Lab. Try again from its controls.",
        );
        return;
      }
      if (this.phase === "waiting") {
        if (connection === "login") {
          this.send("joining", () =>
            this.port.mmorpg({
              type: "join",
              name: "Battle Guest",
              class: "paladin",
            }),
          );
          return;
        }
        if (connection !== "online") return;
        // A restored or manually started battle belongs to its current controls.
        if (status.battle?.enabled) {
          this.phase = "done";
          this.cleanup();
          return;
        }
        this.send("starting", () =>
          this.port.battle({ op: "start", count: this.count, seed: 1 }),
        );
      } else if (
        this.phase === "joining" &&
        receipt?.ok &&
        connection === "online"
      ) {
        this.send("starting", () =>
          this.port.battle({ op: "start", count: this.count, seed: 1 }),
        );
      } else if (
        this.phase === "starting" &&
        receipt?.ok &&
        status.battle?.enabled
      ) {
        this.send("camera", () =>
          this.port.battle({ op: "camera", view: "dolly" }),
        );
      } else if (this.phase === "camera" && receipt?.ok) {
        this.phase = "done";
        this.cleanup();
      }
    } catch {
      this.fail("Couldn't open Battle Lab. Try again from its controls.");
    } finally {
      this.advancing = false;
    }
  }
}

export function installBattleLink(
  owner: BattleLinkOwner,
  port: BattleLinkPort,
  search: string,
  access: () => BattleLinkAccess,
): BattleLinkBootstrap | undefined {
  if (owner.battleLink) return owner.battleLink;
  const count = battleLinkCount(search, access());
  if (count === undefined) return;
  const bootstrap = new BattleLinkBootstrap(count, port, access);
  // Retain before subscribing: immediate callbacks or hot refresh cannot duplicate it.
  owner.battleLink = bootstrap;
  bootstrap.start();
  return bootstrap;
}

export function editorBattleLinkPort(service: EditorService): BattleLinkPort {
  return {
    snapshot: () => service.registry.get(editorAtoms.status),
    receipt: (id) => service.receipts.get(id),
    subscribe: (listener) => {
      const status = service.registry.subscribe(editorAtoms.status, listener);
      const receipt = service.registry.subscribe(editorAtoms.receipt, listener);
      return () => {
        status();
        receipt();
      };
    },
    mmorpg: (action) => service.mmorpg(action),
    battle: (action) => service.battle(action),
    error: (message) => service.registry.set(editorAtoms.error, message),
    timeout: (callback, milliseconds) => {
      const timer = setTimeout(callback, milliseconds);
      return () => clearTimeout(timer);
    },
  };
}
