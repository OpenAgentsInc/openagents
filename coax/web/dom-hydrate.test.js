// Firm reason: 1 (browser host check: verifies in-place SSR hydration reuses exact DOM nodes without flicker and preserves pre-hydration user input)
import test from "node:test";
import assert from "node:assert/strict";
import { createHydrator, readInitialState } from "./dom-hydrate.js";

function makeEl(tagName, { id = "", attrs = {}, text = "", value, defaultValue = "", type = "" } = {}) {
  const attrMap = new Map(Object.entries(attrs));
  if (id) attrMap.set("id", id);
  const el = {
    tagName: tagName.toUpperCase(),
    id,
    type,
    dataset: {},
    parentNode: null,
    childNodes: [],
    attrs: attrMap,
    textWrites: 0,
    attrWrites: 0,
    valueWrites: 0,
    removed: false,
    _text: text,
    _value: value !== undefined ? value : defaultValue,
    defaultValue,
    get textContent() { return this._text; },
    set textContent(v) { this._text = v; this.textWrites += 1; },
    get value() { return this._value; },
    set value(v) { this._value = v; this.valueWrites += 1; },
    get firstChild() { return this.childNodes[0] || null; },
    get nextSibling() {
      if (!this.parentNode) return null;
      const i = this.parentNode.childNodes.indexOf(this);
      return i >= 0 ? (this.parentNode.childNodes[i + 1] || null) : null;
    },
    getAttribute(k) { return this.attrs.has(k) ? this.attrs.get(k) : null; },
    setAttribute(k, v) { this.attrs.set(k, v); this.attrWrites += 1; },
    removeAttribute(k) { this.attrs.delete(k); },
    appendChild(child) {
      child.parentNode = this;
      this.childNodes.push(child);
      return child;
    },
    insertBefore(child, before) {
      if (child.parentNode) {
        const prev = child.parentNode.childNodes.indexOf(child);
        if (prev >= 0) child.parentNode.childNodes.splice(prev, 1);
      }
      child.parentNode = this;
      if (!before) { this.childNodes.push(child); return child; }
      const idx = this.childNodes.indexOf(before);
      if (idx < 0) this.childNodes.push(child);
      else this.childNodes.splice(idx, 0, child);
      return child;
    },
    remove() {
      this.removed = true;
      if (!this.parentNode) return;
      const i = this.parentNode.childNodes.indexOf(this);
      if (i >= 0) this.parentNode.childNodes.splice(i, 1);
      this.parentNode = null;
    },
    replaceChildren() {
      this.replaceChildrenCalls = (this.replaceChildrenCalls || 0) + 1;
      for (const c of [...this.childNodes]) c.remove();
    },
    querySelectorAll(sel) {
      const out = [];
      const walk = (n) => {
        for (const c of n.childNodes) {
          if (sel === "[id]" && c.id) out.push(c);
          walk(c);
        }
      };
      walk(this);
      return out;
    },
  };
  return el;
}

// Mirrors dom-host.js apply() logic over our lightweight DOM tree to prove zero churn.
function applyHydratedFrame(root, commands, hydrator) {
  const NONE = 0xffff;
  const els = [];
  let created = 0;
  let inserted = 0;
  for (const c of commands) {
    const [op, slot] = c;
    if (op === 1) {
      const [, , tag, id] = c;
      let el = hydrator.active ? hydrator.adopt(tag, id) : null;
      if (!el) {
        el = makeEl(tag, { id });
        created += 1;
      }
      el.id = id;
      el.dataset.slot = slot;
      els[slot] = el;
    } else if (op === 3) {
      const el = els[slot], p = c[2], a = c[3], parent = p === NONE ? root : els[p];
      const before = a === NONE ? parent.firstChild : els[a].nextSibling;
      if (before === el && el.parentNode === parent) continue;
      parent.insertBefore(el, before);
      inserted += 1;
    } else if (op === 4) {
      const el = els[slot];
      if (!hydrator.active || el.childNodes.length !== 0 || el.textContent !== c[2]) {
        el.textContent = c[2];
      }
    } else if (op === 5) {
      const el = els[slot];
      if (el.getAttribute("class") !== c[2]) el.setAttribute("class", c[2]);
    } else if (op === 6) {
      const el = els[slot], key = c[2], value = c[3];
      if (el.getAttribute(key) !== value) el.setAttribute(key, value);
      if (key === "value" && (el.tagName === "INPUT" || el.tagName === "TEXTAREA") && el.type !== "file") {
        if (!hydrator.hasDraft(el.id)) el.value = value;
      }
    } else if (op === 10) {
      const el = els[slot];
      if (!hydrator.hasDraft(el.id)) el.value = c[2];
    }
  }
  const drafts = hydrator.active ? hydrator.finish(els) : [];
  return { els, created, inserted, drafts };
}

test("SSR root hydrates in place with zero DOM creation, zero insertion, and preserved pre-hydration user input", () => {
  const root = makeEl("main", { id: "coaxial-root", attrs: { "data-coax-ssr": "1" } });
  const wrap = makeEl("div", { id: "chat", attrs: { class: "page" } });
  const heading = makeEl("h1", { id: "chat.title", text: "OpenAgents" });
  const composer = makeEl("textarea", {
    id: "chat.input",
    attrs: { class: "composer", placeholder: "Ask..." },
    defaultValue: "",
    value: "typed before wasm loaded",
  });
  const stale = makeEl("span", { id: "chat.ssr_only", text: "stale" });
  wrap.appendChild(heading);
  wrap.appendChild(composer);
  wrap.appendChild(stale);
  root.appendChild(wrap);

  const hydrator = createHydrator(root);
  assert.equal(hydrator.active, true);
  assert.equal(hydrator.hasDraft("chat.input"), true);

  // First Wasm frame matches SSR nodes (chat, chat.title, chat.input) and tries to set empty initial value.
  const commands = [
    [1, 0, "div", "chat"],
    [3, 0, 0xffff, 0xffff],
    [5, 0, "page"],
    [1, 1, "h1", "chat.title"],
    [3, 1, 0, 0xffff],
    [4, 1, "OpenAgents"],
    [1, 2, "textarea", "chat.input"],
    [3, 2, 0, 1],
    [5, 2, "composer"],
    [6, 2, "placeholder", "Ask..."],
    [6, 2, "value", ""],
    [10, 2, ""],
  ];

  const { els, created, inserted, drafts } = applyHydratedFrame(root, commands, hydrator);

  // Exact same DOM element instances adopted in place!
  assert.equal(els[0], wrap);
  assert.equal(els[1], heading);
  assert.equal(els[2], composer);
  assert.equal(created, 0);
  assert.equal(inserted, 0);
  assert.equal(root.replaceChildrenCalls || 0, 0);
  assert.equal(heading.textWrites, 0);
  assert.equal(wrap.attrWrites, 0);

  // Pre-hydration user input preserved and returned so dom-host.js dispatches send(2, slot, draft)!
  assert.equal(composer.value, "typed before wasm loaded");
  assert.equal(composer.valueWrites, 0);
  assert.deepEqual(drafts, [{ slot: 2, id: "chat.input", value: "typed before wasm loaded" }]);

  // Stale unadopted SSR node pruned, hydration marker updated.
  assert.equal(stale.removed, true);
  assert.equal(hydrator.adoptedCount, 3);
  assert.equal(hydrator.prunedCount, 1);
  assert.equal(root.getAttribute("data-coax-ssr"), null);
  assert.equal(root.getAttribute("data-coax-hydrated"), "1");
});

test("non-SSR roots bypass hydration cleanly and readInitialState parses coaxial-state script", () => {
  const plainRoot = makeEl("main", { id: "coaxial-root" });
  const hydrator = createHydrator(plainRoot);
  assert.equal(hydrator.active, false);
  assert.equal(hydrator.adopt("div", "x"), null);
  assert.deepEqual(hydrator.finish([]), []);

  const fakeDoc = {
    getElementById(id) {
      return id === "coaxial-state" ? { textContent: '{"page":2}' } : null;
    },
  };
  assert.equal(readInitialState(fakeDoc), '{"page":2}');
  assert.equal(readInitialState({ getElementById: () => null }), "");
});
