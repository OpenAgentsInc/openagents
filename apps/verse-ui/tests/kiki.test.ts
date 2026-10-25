// The pure half of Kiki's house editor desk: the card command grammar, the
// desk's open and moved cards, the guest one-change rule, and payout lines.
import { test } from "node:test";
import assert from "node:assert/strict";
import { CARDS, parseCommand } from "../src/kiki/commands";
import {
  GRIP,
  append,
  apply,
  blocksChange,
  clamp,
  initialDesk,
  isOpen,
  move,
  payoutLine,
  restore,
  revertControl,
  MAX_MESSAGES,
  type Message,
} from "../src/kiki/desk";

test("card commands parse as a whole sentence of the grammar", () => {
  assert.deepEqual(parseCommand("show inventory"), {
    verb: "open",
    card: "inventory",
  });
  assert.deepEqual(parseCommand("Open tools"), { verb: "open", card: "tools" });
  assert.deepEqual(parseCommand("close chat."), {
    verb: "close",
    card: "chat",
  });
  assert.deepEqual(parseCommand("please hide the voice bot"), {
    verb: "close",
    card: "voice",
  });
  assert.deepEqual(parseCommand("show my edits and payouts card"), {
    verb: "open",
    card: "payouts",
  });
  assert.deepEqual(parseCommand("open the world actions window"), {
    verb: "open",
    card: "actions",
  });
  assert.deepEqual(parseCommand("Hide chat!"), { verb: "close", card: "chat" });
  // Anything else is a request for the editing agent, not a command.
  for (const text of [
    "make the lamps glow green",
    "show me the inventory of the shed",
    "can you open the chat",
    "open",
    "chat",
    "close the door",
    "",
  ])
    assert.equal(parseCommand(text), undefined, text);
});

test("the desk opens, closes, and moves cards", () => {
  const desk = initialDesk(false);
  for (const card of CARDS) assert(isOpen(desk, card));
  const phone = initialDesk(true);
  assert.deepEqual(
    CARDS.filter((card) => isOpen(phone, card)),
    ["tools", "voice"],
  );
  const closed = apply(desk, { verb: "close", card: "chat" });
  assert(!isOpen(closed, "chat"));
  assert.deepEqual(apply(closed, { verb: "close", card: "chat" }).closed, [
    "chat",
  ]);
  assert(isOpen(apply(closed, { verb: "open", card: "chat" }), "chat"));
  // On a phone, one desk card at a time; the tools and voice stay as they are.
  const phoneChat = apply(phone, { verb: "open", card: "chat" }, true);
  assert.deepEqual(
    CARDS.filter((card) => isOpen(phoneChat, card)),
    ["tools", "voice", "chat"],
  );
  const phoneInventory = apply(
    phoneChat,
    { verb: "open", card: "inventory" },
    true,
  );
  assert.deepEqual(
    CARDS.filter((card) => isOpen(phoneInventory, card)),
    ["tools", "voice", "inventory"],
  );
  const moved = move(desk, "tools", { x: 10, y: 20, w: 400 });
  assert.deepEqual(moved.moved.tools, { x: 10, y: 20, w: 400 });
  // Saved desks come back, and anything else falls back.
  assert.deepEqual(restore(JSON.stringify(moved), desk), moved);
  assert.deepEqual(
    restore(
      '{"closed":["chat","nope"],"moved":{"tools":{"x":"a","y":1}}}',
      desk,
    ),
    { closed: ["chat"], moved: {} },
  );
  assert.equal(restore("not json", desk), desk);
  assert.equal(restore(null, desk), desk);
});

test("a moved card's header stays on screen", () => {
  const view = { width: 1000, height: 700 };
  const size = { width: 300 };
  assert.deepEqual(clamp({ x: 100.4, y: 50.6 }, size, view), { x: 100, y: 51 });
  assert.deepEqual(clamp({ x: -900, y: -40 }, size, view), {
    x: GRIP - 300,
    y: 0,
  });
  assert.deepEqual(clamp({ x: 5000, y: 5000 }, size, view), {
    x: 1000 - GRIP,
    y: 700 - GRIP,
  });
});

test("guests keep one change at a time", () => {
  assert(!blocksChange({ guest: true, changes: 0, saving: false }));
  assert(blocksChange({ guest: true, changes: 1, saving: false }));
  assert(!blocksChange({ guest: true, changes: 1, saving: true }));
  assert(!blocksChange({ guest: false, changes: 3, saving: false }));
  const on = { shown: true, enabled: true };
  const off = { shown: true, enabled: false };
  assert.equal(revertControl({ undo: on, revert: on }), "edit-undo");
  assert.equal(revertControl({ undo: off, revert: on }), "project-revert");
  assert.equal(revertControl({ undo: off, revert: off }), undefined);
});

test("chat keeps the newest messages; payouts show 0 for nothing paid", () => {
  let log: ReadonlyArray<Message> = [];
  for (let i = 0; i < MAX_MESSAGES + 5; i++)
    log = append(log, {
      id: `${i}`,
      name: "You",
      text: `${i}`,
      at: i,
      mine: true,
    });
  assert.equal(log.length, MAX_MESSAGES);
  assert.equal(log[0]!.id, "5");
  assert.equal(payoutLine("Earnings: Not recorded"), "Paid: 0");
  assert.equal(payoutLine("Not recorded"), "Paid: 0");
  assert.equal(
    payoutLine("Earnings: 2,100 msat paid"),
    "Earnings: 2,100 msat paid",
  );
  assert.equal(payoutLine("Earnings unavailable"), "Earnings unavailable");
});
