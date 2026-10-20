// Run with node: the lazy footer canvas must use the latest chat on readiness.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import vm from "node:vm";

const source = readFileSync(new URL("../start.js", import.meta.url), "utf8");
const listeners = new Map();
const shown = [];
let imageSource;
let focused = false;
const context = vm.createContext({
  window: { addEventListener(name, callback) { listeners.set(name, callback); } },
  innerWidth: 1280,
  innerHeight: 1100,
  queueMicrotask(fn) { fn(); },
  chatInput: { focus() { focused = true; } },
});
vm.runInContext(source.slice(source.indexOf("var transcriptExtras = null;"), source.indexOf("// End transcript extras.")).replaceAll("import.meta.url", '"https://example.test/gpui/build/start.js"'), context);
let loads = 0;
context.loadExtras = () => { loads++; };
context.extrasBox = { style: {} };
const emit = (name, detail) => listeners.get(name)({ detail });
const first = { device: "a", chat: "one", generation: 1 };
const next = { device: "b", chat: "two", generation: 2 };
emit("oa-extras-context", JSON.stringify(first));
assert.equal(loads, 1);
// Leaving during a module load uses the new chat when the canvas is ready.
emit("oa-extras-context", JSON.stringify(next));
context.transcriptExtras = { show(value) { shown.push(JSON.parse(value)); }, readImage(...source) { imageSource = source; return Promise.resolve("image"); } };
emit("oa-extras-ready");
assert.equal(shown.at(-1).chat, "two");
emit("oa-extras-state", JSON.stringify({...next, open: true}));
assert.equal(context.extrasBox.style.visibility, "visible");
assert.equal(context.extrasBox.style.height, "442px");
emit("oa-extras-bounds", JSON.stringify({x:400, bottom:1090, width:704}));
assert.equal(context.extrasBox.style.left, "400px");
assert.equal(context.extrasBox.style.bottom, "10px");
emit("oa-extras-state", JSON.stringify({...first, open: false}));
assert.equal(context.extrasBox.style.height, "442px");
emit("oa-extras-usage", JSON.stringify({chat: "old", context_usage: {tokens: 99}}));
assert.equal(shown.at(-1).context_usage, undefined);
emit("oa-extras-usage", JSON.stringify({chat: "two", generation: 2, loading: false, context_usage: {tokens: 50, window: 200}, caught_up: true, connected: true}));
assert.equal(shown.at(-1).context_usage.tokens, 50);
assert.equal(shown.at(-1).connected, true);
assert.equal(shown.at(-1).loading, false);
emit("oa-extras-close");
assert.equal(context.extrasBox.style.height, "76px");
assert.equal(focused, true);
// A new cloud chat stops the old computer's watches, including when hidden.
emit("oa-extras-context", "{}");
assert.deepEqual(shown.at(-1), {});
assert.equal(context.extrasOpen, false);
assert.equal(context.extrasBox.style.visibility, "hidden");
// A read already started in an old transcript retains its owning host and chat.
assert.equal(await context.window.__oaReadTranscriptImage("a", "one", "/old/image.png"), "image");
assert.deepEqual(imageSource, ["a", "one", "/old/image.png"]);
console.log("transcript extras handoff: pass");
