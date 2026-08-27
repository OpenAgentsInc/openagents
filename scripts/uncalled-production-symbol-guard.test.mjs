import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdirSync, mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

const guard = fileURLToPath(new URL("./uncalled-production-symbol-guard.mjs", import.meta.url));

const fixture = (files) => {
  const root = mkdtempSync(path.join(tmpdir(), "uncalled-production-symbol-"));
  for (const [file, contents] of Object.entries(files)) {
    mkdirSync(path.dirname(path.join(root, file)), { recursive: true });
    writeFileSync(path.join(root, file), contents);
  }
  return root;
};

const run = (root, ...flags) =>
  spawnSync(process.execPath, [guard, root, ...flags], { encoding: "utf8" });

test("fails on an exported value whose only caller is a test", () => {
  const root = fixture({
    "packages/store/src/room.ts": "export const removeRoomMember = (id: string) => id\n",
    "packages/store/src/room.test.ts":
      'import { removeRoomMember } from "./room"\nremoveRoomMember("a")\n',
  });
  const result = run(root);
  assert.equal(result.status, 1, result.stdout);
  assert.match(result.stderr, /removeRoomMember/u);
  assert.match(result.stderr, /packages\/store\/src\/room\.ts/u);
  // The message must say what would satisfy the guard, not only that it failed.
  assert.match(result.stderr, /Call it from the live path, or delete it together with its test/u);
  assert.match(result.stderr, /"allowed"/u);
});

test("accepts an exported value a production module calls", () => {
  const root = fixture({
    "packages/store/src/room.ts": "export const removeRoomMember = (id: string) => id\n",
    "packages/store/src/routes.ts":
      'import { removeRoomMember } from "./room"\nremoveRoomMember("a")\n',
    "packages/store/src/room.test.ts":
      'import { removeRoomMember } from "./room"\nremoveRoomMember("a")\n',
  });
  assert.equal(run(root).status, 0, run(root).stderr);
});

test("accepts an exported value its own module wires into another export", () => {
  const root = fixture({
    "packages/store/src/room.ts":
      "export const removeRoomMember = (id: string) => id\nexport const handle = (id: string) => removeRoomMember(id)\n",
    "packages/store/src/room.test.ts":
      'import { removeRoomMember } from "./room"\nremoveRoomMember("a")\n',
  });
  assert.equal(run(root).status, 0, run(root).stderr);
});

test("ignores code no test references, which is ordinary dead code", () => {
  const root = fixture({
    "packages/store/src/room.ts": "export const removeRoomMember = (id: string) => id\n",
  });
  assert.equal(run(root).status, 0, run(root).stderr);
});

test("ignores exports a framework calls by convention", () => {
  const root = fixture({
    "apps/web/src/route.ts": "export const GET = () => new Response()\n",
    "apps/web/src/route.test.ts": 'import { GET } from "./route"\nGET()\n',
  });
  assert.equal(run(root).status, 0, run(root).stderr);
});

test("fails on a service-interface member only a test calls, and accepts one production calls", () => {
  const contract =
    "export interface RoomStore {\n  readonly removeParticipant: (id: string) => void\n}\n" +
    "class Store {\n  removeParticipant(id: string) {\n    void id\n  }\n}\n";
  const dead = fixture({
    "packages/store/src/store.ts": contract,
    "packages/store/src/store.test.ts":
      "declare const store: import('./store').RoomStore\nstore.removeParticipant('a')\n",
  });
  const deadResult = run(dead);
  assert.equal(deadResult.status, 1, deadResult.stdout);
  assert.match(deadResult.stderr, /RoomStore\.removeParticipant/u);

  const live = fixture({
    "packages/store/src/store.ts": contract,
    "packages/store/src/caller.ts":
      "declare const store: import('./store').RoomStore\nstore.removeParticipant('a')\n",
    "packages/store/src/store.test.ts":
      "declare const store: import('./store').RoomStore\nstore.removeParticipant('a')\n",
  });
  assert.equal(run(live).status, 0, run(live).stderr);
});

// The guard counted mentions in the raw file text until 2026-08-03, so writing
// a doc comment about a test-only export silenced the guard that exists to find
// test-only exports. It was found by accident: an agent added a TSDoc `{@link}`
// while working near a flagged symbol and watched the finding disappear.
test("a TSDoc link in the declaring file is prose, not the module wiring its own export up", () => {
  const root = fixture({
    "packages/store/src/room.ts":
      "/** Removes a member. See {@link removeRoomMember} for the receipt shape. */\nexport const removeRoomMember = (id: string) => id\n",
    "packages/store/src/room.test.ts":
      'import { removeRoomMember } from "./room"\nremoveRoomMember("a")\n',
  });
  const result = run(root);
  assert.equal(result.status, 1, result.stdout);
  assert.match(result.stderr, /removeRoomMember/u);
});

test("a comment or a string in another production module is not a production caller", () => {
  const commented = fixture({
    "packages/store/src/room.ts": "export const removeRoomMember = (id: string) => id\n",
    "packages/store/src/routes.ts":
      "// TODO: call removeRoomMember once the seat ledger lands.\nexport const routes = []\n",
    "packages/store/src/room.test.ts":
      'import { removeRoomMember } from "./room"\nremoveRoomMember("a")\n',
  });
  const commentedResult = run(commented);
  assert.equal(commentedResult.status, 1, commentedResult.stdout);
  assert.match(commentedResult.stderr, /removeRoomMember/u);

  const quoted = fixture({
    "packages/store/src/room.ts": "export const removeRoomMember = (id: string) => id\n",
    "packages/store/src/audit.ts":
      'export const covered = ["removeRoomMember", `removeRoomMember`]\n',
    "packages/store/src/room.test.ts":
      'import { removeRoomMember } from "./room"\nremoveRoomMember("a")\n',
  });
  const quotedResult = run(quoted);
  assert.equal(quotedResult.status, 1, quotedResult.stdout);
  assert.match(quotedResult.stderr, /removeRoomMember/u);
});

// `Effect.fn("RoomStore.removeParticipant")` names a span. It reads like a call,
// it is not one, and it sits beside almost every service member in this repo, so
// it exempted service members wholesale.
test("a span name and a comment are not dot-access calls on a service member", () => {
  const root = fixture({
    "packages/store/src/store.ts":
      "export interface RoomStore {\n  readonly removeParticipant: (id: string) => void\n}\n" +
      'const removeParticipant = Effect.fn("RoomStore.removeParticipant")((id: string) => id)\n' +
      "// store.removeParticipant is wired up in a later slice.\n",
    "packages/store/src/store.test.ts":
      "declare const store: import('./store').RoomStore\nstore.removeParticipant('a')\n",
  });
  const result = run(root);
  assert.equal(result.status, 1, result.stdout);
  assert.match(result.stderr, /RoomStore\.removeParticipant/u);
});

// The other half of the fix: masking prose must not eat code. A caller hidden by
// an over-eager mask fails the build for code that is fine, which is the failure
// mode that gets a guard deleted rather than fixed.
test("code that only looks like prose still counts: template substitutions and JSX", () => {
  const substituted = fixture({
    "packages/store/src/room.ts": "export const removeRoomMember = (id: string) => id\n",
    "packages/store/src/routes.ts":
      'import { removeRoomMember } from "./room"\nexport const line = `removed ${removeRoomMember("a")}`\n',
    "packages/store/src/room.test.ts":
      'import { removeRoomMember } from "./room"\nremoveRoomMember("a")\n',
  });
  assert.equal(run(substituted).status, 0, run(substituted).stderr);

  // The apostrophe in JSX text opens a string literal for a plain scanner, which
  // closes it at the next quote and swallows the `<badge.RoomBadge />` between.
  const rendered = fixture({
    "packages/store/src/badge.tsx": "export const RoomBadge = () => <span />\n",
    "packages/store/src/panel.tsx":
      'import * as badge from "./badge"\n' +
      "export const Panel = () => (\n  <div>\n    <p>It's fine</p><badge.RoomBadge title='ok' />\n  </div>\n)\n",
    "packages/store/src/badge.test.tsx": 'import { RoomBadge } from "./badge"\nRoomBadge()\n',
  });
  assert.equal(run(rendered).status, 0, run(rendered).stderr);
});

test("a baselined finding passes, and stops passing once it is no longer a finding", () => {
  const root = fixture({
    "packages/store/src/room.ts": "export const removeRoomMember = (id: string) => id\n",
    "packages/store/src/room.test.ts":
      'import { removeRoomMember } from "./room"\nremoveRoomMember("a")\n',
  });
  const seeded = run(root, "--seed");
  assert.equal(seeded.status, 0, seeded.stderr);
  assert.equal(run(root).status, 0, run(root).stderr);

  // Wiring the symbol up makes the ledger entry stale, and the ledger may only shrink.
  writeFileSync(
    path.join(root, "packages/store/src/routes.ts"),
    'import { removeRoomMember } from "./room"\nremoveRoomMember("a")\n',
  );
  const stale = run(root);
  assert.equal(stale.status, 1, stale.stdout);
  assert.match(stale.stderr, /no longer flags/u);

  const pruned = run(root, "--prune");
  assert.equal(pruned.status, 0, pruned.stderr);
  const ledger = JSON.parse(
    readFileSync(path.join(root, "scripts/uncalled-production-symbol-baseline.json"), "utf8"),
  );
  assert.deepEqual(ledger.inheritedDebt, []);
  assert.equal(run(root).status, 0);
});

test("--prune may only remove entries, so a new finding cannot be laundered into the ledger", () => {
  const root = fixture({
    "packages/store/src/room.ts": "export const removeRoomMember = (id: string) => id\n",
    "packages/store/src/room.test.ts":
      'import { removeRoomMember } from "./room"\nremoveRoomMember("a")\n',
  });
  run(root, "--seed");
  writeFileSync(
    path.join(root, "packages/store/src/seat.ts"),
    "export const releaseSeat = (id: string) => id\n",
  );
  writeFileSync(
    path.join(root, "packages/store/src/seat.test.ts"),
    'import { releaseSeat } from "./seat"\nreleaseSeat("a")\n',
  );

  const pruned = run(root, "--prune");
  assert.equal(pruned.status, 0, pruned.stderr);
  const afterPrune = run(root);
  assert.equal(afterPrune.status, 1, "prune must not absorb the new finding");
  assert.match(afterPrune.stderr, /releaseSeat/u);

  const reseed = run(root, "--seed");
  assert.equal(reseed.status, 1, "--seed must refuse to overwrite an existing ledger");
  assert.match(reseed.stderr, /Refusing to --seed/u);
});

test("an allowlist entry needs a written reason, and may not outlive its finding", () => {
  const files = {
    "packages/store/src/room.ts": "export const removeRoomMember = (id: string) => id\n",
    "packages/store/src/room.test.ts":
      'import { removeRoomMember } from "./room"\nremoveRoomMember("a")\n',
  };
  const ref = "packages/store/src/room.ts#removeRoomMember";

  const unreasoned = fixture({
    ...files,
    "scripts/uncalled-production-symbol-baseline.json": JSON.stringify({
      inheritedDebt: [],
      allowed: [{ ref, reason: "later" }],
    }),
  });
  const unreasonedResult = run(unreasoned);
  assert.equal(unreasonedResult.status, 1, unreasonedResult.stdout);
  assert.match(unreasonedResult.stderr, /without a usable reason/u);

  const reasoned = fixture({
    ...files,
    "scripts/uncalled-production-symbol-baseline.json": JSON.stringify({
      inheritedDebt: [],
      allowed: [
        {
          ref,
          reason: "Published package API; the caller is an external consumer, not this repo.",
        },
      ],
    }),
  });
  assert.equal(run(reasoned).status, 0, run(reasoned).stderr);

  const stale = fixture({
    "packages/store/src/room.ts": "export const removeRoomMember = (id: string) => id\n",
    "packages/store/src/routes.ts":
      'import { removeRoomMember } from "./room"\nremoveRoomMember("a")\n',
    "packages/store/src/room.test.ts":
      'import { removeRoomMember } from "./room"\nremoveRoomMember("a")\n',
    "scripts/uncalled-production-symbol-baseline.json": JSON.stringify({
      inheritedDebt: [],
      allowed: [
        {
          ref,
          reason: "Published package API; the caller is an external consumer, not this repo.",
        },
      ],
    }),
  });
  const staleResult = run(stale);
  assert.equal(staleResult.status, 1, staleResult.stdout);
  assert.match(staleResult.stderr, /no longer flags/u);
});
