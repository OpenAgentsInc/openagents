// Run with node: exercise the page boundary between the palette and file canvas.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import vm from "node:vm";

const source = readFileSync(new URL("../start.js", import.meta.url), "utf8");
const listeners = new Map();
const opened = [];
const shown = [];
const mainInput = { isConnected: true, focus() { shown.push("main"); } };
const otherInput = { isConnected: true, focus() { shown.push("other"); } };
let forwarded = 0;
const context = vm.createContext({
  window: { addEventListener(name, callback) { listeners.set(name, callback); } },
  document: { activeElement: otherInput },
  queueMicrotask(fn) { fn(); },
  paletteShortcut() { forwarded++; return true; },
});
vm.runInContext(source.slice(source.indexOf('var chatInput = null;'), source.indexOf('// A computer chat\'s live transcript')).replaceAll('import.meta.url', '"https://example.test/gpui/build/start.js"'), context);
context.chatInput = mainInput;
context.filesBox = { hidden: true };
context.filesReady = true;
context.files = { show(...args) { shown.push(args); }, open_file(...args) { opened.push(args); } };
const emit = (name, detail) => listeners.get(name)({ detail });
emit("oa-host", "computer-a\nchat-a");
emit("oa-files-open", JSON.stringify({ device: "computer-a", chat: "chat-a", path: "src/lib.rs", line: 42 }));
assert.equal(context.filesBox.hidden, false);
assert.deepEqual(opened.pop(), ["computer-a", "chat-a", "src/lib.rs", 42]);
// A second hit must reveal the pane, rather than toggle it closed.
emit("oa-files-open", JSON.stringify({ device: "computer-a", chat: "chat-a", path: "README.md" }));
assert.equal(context.filesBox.hidden, false);
assert.deepEqual(opened.pop(), ["computer-a", "chat-a", "README.md", undefined]);
// mount returns before its GPUI window exists. Keep the hit until ready.
context.filesReady = false;
emit("oa-files-open", JSON.stringify({ device: "computer-a", chat: "chat-a", path: "src/main.rs", line: 9 }));
assert.equal(opened.length, 0);
emit("oa-files-ready");
assert.equal(opened.pop()[3], 9);
// Switching chats during a module load discards the old result.
context.filesReady = false;
emit("oa-files-open", JSON.stringify({ device: "computer-a", chat: "chat-a", path: "old.rs" }));
emit("oa-host", "computer-b\nchat-b");
emit("oa-files-ready");
assert.deepEqual(shown.at(-1), ["computer-b", "chat-b"]);
assert.equal(opened.length, 0);
emit("oa-files-open", JSON.stringify({ device: "computer-a", chat: "chat-a", path: "wrong.rs" }));
assert.equal(opened.length, 0);
// The palette sits above the file pane and gives focus back when dismissed.
context.filesBox.hidden = false;
emit("oa-palette-open");
assert.equal(context.filesBox.hidden, true);
assert.equal(shown.at(-1), "main");
emit("oa-palette-close");
assert.equal(context.filesBox.hidden, false);
assert.equal(shown.at(-1), "other");
const key = { ctrlKey: true, key: "k", preventDefault() { this.prevented = true; } };
listeners.get("keydown")(key);
assert.equal(key.prevented, true);
assert.equal(forwarded, 1);
listeners.get("keydown")({ ...key, defaultPrevented: true });
listeners.get("keydown")({ ...key, isComposing: true });
assert.equal(forwarded, 1);
console.log("palette/file handoff: pass");
