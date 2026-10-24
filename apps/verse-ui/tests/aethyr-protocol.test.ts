// The TypeScript side of the aethyr.ui.v1 contract: the generated types
// match the Rust schema, and the validator agrees with Rust on every shared
// fixture in crates/aethyr-ui/fixtures.
import { test } from "node:test";
import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { readFileSync, readdirSync } from "node:fs";
import { join } from "node:path";
import {
  ELEMENT_KINDS,
  MAX_DEPTH,
  MAX_NODES,
  MAX_TEXT_BYTES,
  MAX_VIEW_BYTES,
  SCHEMA,
  SCHEMA_DOCUMENT,
  SCHEMA_SHA256,
  type Intent,
  type View,
} from "../src/aethyr/protocol";
import {
  acceptIntent,
  children,
  isIntent,
  parseView,
} from "../src/aethyr/validate";

const crate = join(import.meta.dirname, "../../../crates/aethyr-ui");
const read = (path: string) => readFileSync(join(crate, path), "utf8");
const json = (path: string): unknown => JSON.parse(read(path));

test("the generated types come from the current Rust schema", () => {
  const schema = read("schema/aethyr.ui.v1.schema.json");
  const digest = createHash("sha256").update(schema).digest("hex");
  assert.equal(
    digest,
    SCHEMA_SHA256,
    "src/aethyr/protocol.ts is stale; run `cargo run -p aethyr-ui -- --write`",
  );
  assert.deepEqual(SCHEMA_DOCUMENT, JSON.parse(schema));
});

test("one fixture per node kind, each valid and unchanged by a round trip", () => {
  const files = readdirSync(join(crate, "fixtures/kinds")).sort();
  assert.deepEqual(
    files.map((f) => f.replace(/\.json$/, "")),
    [...ELEMENT_KINDS].sort(),
  );
  for (const file of files) {
    const text = read(`fixtures/kinds/${file}`);
    const checked = parseView(text);
    assert.ok(checked.ok, `${file}: ${!checked.ok && checked.code}`);
    assert.deepEqual(
      JSON.parse(JSON.stringify(checked.value)),
      JSON.parse(text),
    );
  }
});

test("invalid fixtures fail with the same code as in Rust", () => {
  const cases = json("fixtures/invalid.json") as Array<{
    name: string;
    error: string;
    view: unknown;
  }>;
  assert.ok(cases.length > 10);
  for (const { name, error, view } of cases) {
    const checked = parseView(JSON.stringify(view));
    assert.equal(checked.ok ? "ok" : checked.code, error, name);
  }
});

test("intents are accepted or refused as in Rust", () => {
  const fixture = json("fixtures/intents.json") as {
    view: View;
    cases: Array<{ name: string; intent: Intent; expect: string }>;
  };
  const view = parseView(JSON.stringify(fixture.view));
  assert.ok(view.ok);
  for (const { name, intent, expect } of fixture.cases) {
    assert.ok(isIntent(intent), name);
    const result = acceptIntent(view.value, intent);
    assert.equal(result.ok ? "ok" : result.code, expect, name);
    if (result.ok) assert.equal(result.value.id, intent.node, name);
  }
  assert.equal(
    isIntent({ instance: "a", revision: 1, node: "b", kind: "hover" }),
    false,
  );
});

const text = (id: string) => ({
  id,
  element: { kind: "text", value: "x", role: "body" },
});
const wide = (count: number) => ({
  schema: SCHEMA,
  instance: "limits",
  revision: 1,
  root: {
    id: "root",
    element: {
      kind: "stack",
      axis: "vertical",
      children: Array.from({ length: count - 1 }, (_, i) => text(`n${i + 1}`)),
    },
  },
});
const deep = (depth: number) => {
  let node: object = text(`d${depth}`);
  for (let level = depth - 1; level >= 1; level--)
    node = {
      id: `d${level}`,
      element: { kind: "stack", axis: "vertical", children: [node] },
    };
  return { schema: SCHEMA, instance: "limits", revision: 1, root: node };
};
const code = (value: unknown) => {
  const checked = parseView(
    typeof value === "string" ? value : JSON.stringify(value),
  );
  return checked.ok ? "ok" : checked.code;
};

test("1,024 nodes and depth 16 are the limits", () => {
  assert.equal(MAX_NODES, 1024);
  assert.equal(MAX_DEPTH, 16);
  assert.equal(code(wide(MAX_NODES)), "ok");
  assert.equal(code(wide(MAX_NODES + 1)), "node_limit");
  assert.equal(code(deep(MAX_DEPTH)), "ok");
  assert.equal(code(deep(MAX_DEPTH + 1)), "depth_limit");
  assert.equal(code("x".repeat(MAX_VIEW_BYTES + 1)), "view_limit");
  const long = wide(2) as {
    root: { element: { children: Array<{ element: { value: string } }> } };
  };
  long.root.element.children[0]!.element.value = "x".repeat(MAX_TEXT_BYTES + 1);
  assert.equal(code(long), "text_limit");
});

test("children are visited for every container kind", () => {
  const panel = json("fixtures/kinds/panel.json") as View;
  assert.equal(children(panel.root.element).length, 3);
});
