import assert from "node:assert/strict";
import test from "node:test";
import {
  battleLinkCount,
  installBattleLink,
  type BattleLinkAccess,
  type BattleLinkOwner,
  type BattleLinkPort,
} from "../apps/editor/src/battle-link.ts";
import {
  INITIAL_STATUS,
  validBattleAction,
  type EngineStatus,
} from "../packages/editor-services/protocol.ts";
import { validMmoUiAction } from "../packages/runtime/mmorpg-protocol.ts";

const creator: BattleLinkAccess = {
  profile: "open-ground",
  publicDemo: false,
  pathname: "/",
  readOnly: false,
};
function fixture() {
  let status: EngineStatus = { ...INITIAL_STATUS };
  let access = { ...creator };
  const receipts = new Map<string, { ok: boolean; message: string }>();
  const listeners = new Set<() => void>();
  const calls: { id: string; kind: string; action: unknown }[] = [];
  const errors: string[] = [];
  let timeout: (() => void) | undefined;
  const issue = (kind: "mmorpg" | "battle", action: unknown) => {
    assert.equal(
      kind === "mmorpg" ? validMmoUiAction(action) : validBattleAction(action),
      true,
    );
    const id = `request-${calls.length}`;
    calls.push({ id, kind, action });
    return id;
  };
  const port: BattleLinkPort = {
    snapshot: () => status,
    receipt: (id) => receipts.get(id),
    subscribe: (listener) => {
      listeners.add(listener);
      return () => {
        listeners.delete(listener);
      };
    },
    mmorpg: (action) => issue("mmorpg", action),
    battle: (action) => issue("battle", action),
    error: (message) => {
      errors.push(message);
    },
    timeout: (callback) => {
      timeout = callback;
      return () => {
        timeout = undefined;
      };
    },
  };
  const notify = () => {
    for (const listener of [...listeners]) listener();
  };
  const update = (patch: Partial<EngineStatus>) => {
    status = { ...status, ...patch };
    notify();
  };
  const connection = (
    value: "login" | "connecting" | "online" | "error" | "disconnected",
  ) =>
    update({
      phase: "ready",
      mmorpg: { connection: value, actors: [], events: [] },
    });
  const receipt = (index: number, ok = true) => {
    receipts.set(calls[index].id, {
      ok,
      message: ok ? "Ready" : "The server is full. Try again later.",
    });
    notify();
  };
  const owner: BattleLinkOwner = {};
  const install = (search = "?battle=10000") =>
    installBattleLink(owner, port, search, () => access);
  return {
    port,
    owner,
    calls,
    errors,
    listeners,
    install,
    update,
    connection,
    receipt,
    notify,
    access: (patch: Partial<BattleLinkAccess>) => {
      access = { ...access, ...patch };
      notify();
    },
    expire: () => timeout?.(),
    hasTimeout: () => !!timeout,
  };
}
const battle = {
  enabled: true,
  count: 10_000,
  camera: "overview" as const,
  stats: {},
};

test("battle links admit only bounded integers and use a small invalid-value default", () => {
  assert.equal(battleLinkCount("?other=10000", creator), undefined);
  for (const count of [100, 10_000, 1_000_000])
    assert.equal(battleLinkCount(`?battle=${count}`, creator), count);
  for (const value of [
    "",
    "99",
    "1000001",
    "NaN",
    "1e6",
    "100.5",
    "-100",
    "Infinity",
    "9999999999999999",
    "%3Cscript%3E",
  ])
    assert.equal(battleLinkCount(`?battle=${value}`, creator), 10_000);
  assert.equal(battleLinkCount("?battle=100&battle=1000000", creator), 10_000);
});

test("battle links never join or start public, player, read-only, or other worlds", () => {
  for (const patch of [
    { publicDemo: true },
    { pathname: "/player.html" },
    { pathname: "/nested/player.html" },
    { readOnly: true },
    { profile: "medieval-town-lab" },
  ]) {
    const f = fixture();
    f.access(patch);
    assert.equal(f.install(), undefined);
    assert.equal(f.calls.length, 0);
    assert.equal(f.listeners.size, 0);
  }
  const ordinary = fixture();
  assert.equal(ordinary.install("?shot=MT-02"), undefined);
  assert.equal(ordinary.calls.length, 0);
});

test("cold launch waits for assets, joins once, then waits for both authoritative status and matching receipts", () => {
  const f = fixture();
  const link = f.install()!;
  f.notify();
  f.update({ phase: "ready" });
  assert.equal(f.calls.length, 0);
  f.connection("login");
  f.notify();
  assert.deepEqual(f.calls[0].action, {
    type: "join",
    name: "Battle Guest",
    class: "paladin",
  });
  f.connection("connecting");
  f.receipt(0);
  f.notify();
  assert.equal(f.calls.length, 1);
  f.connection("online");
  assert.deepEqual(f.calls[1].action, { op: "start", count: 10000, seed: 1 });
  f.update({ battle });
  assert.equal(f.calls.length, 2);
  f.receipt(1);
  assert.deepEqual(f.calls[2].action, { op: "camera", view: "dolly" });
  f.update({ battle: { ...battle, camera: "dolly" } });
  f.receipt(2);
  assert.equal(link.phase, "done");
  assert.equal(f.listeners.size, 0);
  assert.equal(f.hasTimeout(), false);
});

test("Strict Mode-style installation, hot refresh, Stop, and later reconnect do not restart a completed link", () => {
  const f = fixture();
  f.connection("online");
  const first = f.install()!;
  assert.equal(f.install(), first);
  assert.equal(f.calls.length, 1);
  f.update({ battle });
  f.receipt(0);
  f.receipt(1);
  assert.equal(first.phase, "done");
  f.update({ battle: { ...battle, enabled: false, count: 0 } });
  f.connection("login");
  f.connection("online");
  assert.equal(f.install("?battle=1000000"), first);
  assert.equal(f.calls.length, 2);
});

test("an existing online battle is preserved and an in-progress guest connection is not duplicated", () => {
  const running = fixture();
  running.connection("online");
  running.update({ battle });
  assert.equal(running.install()!.phase, "done");
  assert.equal(running.calls.length, 0);
  const joining = fixture();
  joining.connection("connecting");
  joining.install();
  joining.notify();
  assert.equal(joining.calls.length, 0);
  joining.connection("online");
  assert.equal(joining.calls[0].kind, "battle");
});

test("join/start rejection and disconnection stop the launch without retrying", () => {
  for (const online of [false, true]) {
    const f = fixture();
    f.connection(online ? "online" : "login");
    const link = f.install()!;
    f.receipt(0, false);
    f.notify();
    assert.equal(link.phase, "failed");
    assert.deepEqual(f.errors, ["The server is full. Try again later."]);
    assert.equal(f.calls.length, 1);
    assert.equal(f.listeners.size, 0);
  }
  const disconnected = fixture();
  disconnected.connection("connecting");
  const link = disconnected.install()!;
  disconnected.connection("disconnected");
  assert.equal(link.phase, "failed");
  assert.equal(disconnected.calls.length, 0);
});

test("permission changes, disposal, and loading timeout release subscriptions and cannot issue later commands", () => {
  const revoked = fixture();
  const link = revoked.install()!;
  revoked.access({ publicDemo: true });
  revoked.connection("online");
  assert.equal(link.phase, "cancelled");
  assert.equal(revoked.calls.length, 0);
  const closed = fixture();
  const handle = closed.install()!;
  handle.dispose();
  closed.connection("online");
  assert.equal(closed.calls.length, 0);
  assert.equal(closed.hasTimeout(), false);
  const slow = fixture();
  const timed = slow.install()!;
  slow.expire();
  slow.connection("online");
  assert.equal(timed.phase, "failed");
  assert.equal(slow.calls.length, 0);
  assert.equal(
    slow.errors[0],
    "Couldn't open Battle Lab. Reload to try again.",
  );
});
