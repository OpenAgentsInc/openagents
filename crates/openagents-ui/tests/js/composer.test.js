// The composer's send rule in static/components/shell.js (#11685). Run:
// node --test crates/openagents-ui/tests/js/composer.test.js (the crate's
// tests run it when node is installed).
"use strict";
var test = require("node:test");
var assert = require("node:assert");
var shell = require("../../static/components/shell.js");

function key(extra) {
  var event = { key: "Enter", shiftKey: false, altKey: false, ctrlKey: false, metaKey: false, isComposing: false, keyCode: 13 };
  for (var name in extra) event[name] = extra[name];
  return event;
}

test("Enter sends now when the chat is free", function () {
  assert.strictEqual(shell.sendAction(key({}), true, 0), "send");
});

test("Enter while an answer is being written goes first in line, never last", function () {
  assert.strictEqual(shell.sendAction(key({}), false, 0), "first");
  assert.strictEqual(shell.sendAction(key({}), false, 3), "first");
  // Messages already waiting: this one goes ahead of them.
  assert.strictEqual(shell.sendAction(key({}), true, 2), "first");
});

test("Ctrl+Enter and Cmd+Enter queue for later", function () {
  assert.strictEqual(shell.sendAction(key({ ctrlKey: true }), false, 0), "last");
  assert.strictEqual(shell.sendAction(key({ metaKey: true }), false, 1), "last");
  assert.strictEqual(shell.sendAction(key({ ctrlKey: true }), true, 0), "last");
});

test("Shift+Enter, Alt+Enter, IME and other keys are left alone", function () {
  assert.strictEqual(shell.sendAction(key({ shiftKey: true }), true, 0), "");
  assert.strictEqual(shell.sendAction(key({ altKey: true }), true, 0), "");
  assert.strictEqual(shell.sendAction(key({ isComposing: true }), true, 0), "");
  assert.strictEqual(shell.sendAction(key({ keyCode: 229 }), true, 0), "");
  assert.strictEqual(shell.sendAction(key({ key: "a" }), true, 0), "");
});

test("the hint says Cmd on a Mac", function () {
  assert.strictEqual(
    shell.macHint("Enter to send, Ctrl+Enter to queue for later"),
    "Enter to send, ⌘+Enter to queue for later"
  );
});
