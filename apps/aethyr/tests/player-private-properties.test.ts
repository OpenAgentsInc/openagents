import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { test } from "node:test";
import { minifySync, transformSync } from "rolldown/experimental";
import {
  auditPlayerPrivateProperties,
  PLAYER_PRIVATE_PROPERTIES,
  playerPrivatePropertyMinify,
  type PropertyModule,
} from "../tools/player-private-properties.ts";

const id = fileURLToPath(
  new URL("../packages/renderer/renderer.ts", import.meta.url),
);
const code = readFileSync(id, "utf8");
const mappings = Object.fromEntries(
  PLAYER_PRIVATE_PROPERTIES.map(([name], index) => [name, `p${index}`]),
);
const emitted = [{ id: "player.js", code: 'export const schema = "public";' }];
const run = (
  source = code,
  modules: PropertyModule[] = [{ id, code: source }],
  output = emitted,
  map: Record<string, string | false> = mappings,
) =>
  auditPlayerPrivateProperties({
    renderer: { id, code: source },
    modules,
    emitted: output,
    mappings: map,
  });
const insert = (extra: string) =>
  code.replace(
    "export class Renderer {",
    `export class Renderer {\n${extra}\n`,
  );
const changed = (from: string, to: string) => {
  assert(
    code.includes(from),
    `The negative control no longer matches: ${from}.`,
  );
  return code.replace(from, to);
};

test("the explicit69-name ownership contract passes for original and transformed Renderer modules", () => {
  assert.equal(PLAYER_PRIVATE_PROPERTIES.length, 69);
  assert.equal(
    new Set(PLAYER_PRIVATE_PROPERTIES.map(([name]) => name)).size,
    69,
  );
  const result = run();
  assert(result.rendererIncluded);
  assert.equal(result.includedModules, 1);
  assert(!playerPrivatePropertyMinify().include.test("sourceBundlesEnabled"));
  assert(!playerPrivatePropertyMinify().include.test("farFrameBuffer"));
  const compiled = transformSync(id, code, { target: "es2022" });
  assert.equal(compiled.errors.length, 0);
  assert.doesNotThrow(() => run(code, [{ id, code: compiled.code }]));
  assert.doesNotThrow(() => run(insert("private stockPipelinesV2 = 1;")));
});

test("public, static, computed, renamed and accessor members fail closed", () => {
  for (const source of [
    changed("private stockPipelines", "stockPipelines"),
    changed("private stockPipelines", "private static stockPipelines"),
    changed("private stockPipelines", 'private ["stockPipelines"]'),
    changed(
      "private stockPipelines = new Map<string, GPURenderPipeline>();",
      "private get stockPipelines() { return new Map<string, GPURenderPipeline>(); }",
    ),
    code.replaceAll("stockPipelines", "stockPipelinesV2"),
  ])
    assert.throws(() => run(source));
});

test("private object keys, other receivers, unknown this bindings, and Renderer-this escapes reject", () => {
  for (const extra of [
    'private probe() { return this["stockPipelines"]; }',
    "private probe() { return this[chooseName()]; }",
    "private probe() { return { stockPipelines: this.stockPipelines }; }",
    "private probe() { const r = new Renderer(); return r.stockPipelines; }",
    "private probe() { const alias = this; return alias.stockPipelines; }",
    "private probe() { return function() { return this.stockPipelines; }; }",
    "private probe() { class Other { read() { return this.stockPipelines; } } }",
    "private probe() { return Object.getOwnPropertyNames(this); }",
    "private probe() { return { ...this }; }",
    "private probe() { return this; }",
    "private probe() { return { identity: this }; }",
    "private probe() { receive(this); }",
  ])
    assert.throws(() => run(insert(extra)), extra);
});

test("every included module parses, including modules without an original-name spelling", () => {
  assert.throws(
    () =>
      run(code, [
        { id, code },
        { id: "foreign.js", code: "export const = ;" },
      ]),
    /Cannot audit/,
  );
  const result = run(code, [
    { id, code },
    { id: "foreign.js", code: "// stockPipelines\nexport const value = 1;" },
  ]);
  assert.equal(result.includedModules, 2);
  for (const foreign of [
    "export const value = renderer.stockPipelines;",
    "export const schema = { stockPipelines: 1 };",
    'export const name = "stockPipelines";',
    "class Other { get stockPipelines() { return 1; } }",
    "function stockPipelines() {}",
    'export const value = renderer["stock" + "Pipelines"];',
    'const prefix = "stock"; export const value = renderer[prefix + "Pipelines"];',
    'export const value = renderer[`stock${"Pipe"}lines`];',
    'export const value = renderer["stock\\u0050ipelines"];',
    "export const value = renderer[`stock\\u0050ipelines`];",
  ])
    assert.throws(
      () =>
        run(code, [
          { id, code },
          { id: "foreign.js", code: foreign },
        ]),
      foreign,
    );
});

test("cooked strings, templates, and constant names also reject inside Renderer", () => {
  for (const extra of [
    'private probe() { return "stock\\u0050ipelines"; }',
    "private probe() { return `stock\\u0050ipelines`; }",
    'private probe() { return "stock" + "Pipelines"; }',
    'private probe() { const prefix = "stock"; return `${prefix}Pipelines`; }',
  ])
    assert.throws(() => run(insert(extra)), extra);
});

test("the indirect pipeline parameter/list cannot be shadowed or reassigned", () => {
  const from = "SHADER_PIPELINE_FIELDS.map((field) => this[field])";
  for (const to of [
    "SHADER_PIPELINE_FIELDS.map((field) => { { const field = chooseName(); return this[field]; } })",
    "SHADER_PIPELINE_FIELDS.map((field) => { { const {field} = input; return this[field]; } })",
    "SHADER_PIPELINE_FIELDS.map((field) => { field = chooseName(); return this[field]; })",
    "SHADER_PIPELINE_FIELDS.map((field) => { for (field of chooseFields()) return this[field]; })",
    "SHADER_PIPELINE_FIELDS.map((field) => { for (field in chooseFields()) return this[field]; })",
    "unknownFields.map((field) => this[field])",
  ])
    assert.throws(() => run(changed(from, to)));
  assert.throws(() => run(changed('  "pipeline",', '  "stockPipelines",')));
  assert.throws(() =>
    run(insert("private probe() { const SHADER_PIPELINE_FIELDS = []; }")),
  );
  assert.throws(() =>
    run(insert('private probe() { SHADER_PIPELINE_FIELDS.push("another"); }')),
  );
  assert.throws(() =>
    run(insert("private probe() { const alias = SHADER_PIPELINE_FIELDS; }")),
  );
});

test("Object.assign is allowed only as a bare statement from one fixed immutable binding", () => {
  const call = "Object.assign(this, selected);";
  assert.throws(
    () => run(changed(call, "return Object.assign(this, selected);")),
    /returned assignment/,
  );
  assert.throws(
    () => run(changed(call, "receive(Object.assign(this, selected));")),
    /returned assignment/,
  );
  assert.throws(() => run(changed(call, "Object.assign(this, select());")));
  assert.throws(
    () =>
      run(
        changed(
          call,
          "{ const selected = {}; Object.assign(this, selected); }",
        ),
      ),
    /shadowed/,
  );
  assert.throws(() =>
    run(changed(call, "selected = select(); Object.assign(this, selected);")),
  );
  assert.throws(() =>
    run(
      changed(
        call,
        "for (selected of select()) {} Object.assign(this, selected);",
      ),
    ),
  );
  assert.throws(() =>
    run(
      changed(
        call,
        "for (selected in select()) {} Object.assign(this, selected);",
      ),
    ),
  );
  assert.throws(() =>
    run(
      changed(
        "instances: this.instances,\n        draws: this.draws,",
        "stockPipelines: this.stockPipelines,\n        draws: this.draws,",
      ),
    ),
  );
});

test("actual emitted JavaScript and the minifier mapping are checked without changing schemas", () => {
  assert.throws(
    () =>
      run(
        code,
        [{ id, code }],
        [{ id: "player.js", code: "export const = ;" }],
      ),
    /Cannot audit/,
  );
  assert.throws(() =>
    run(
      code,
      [{ id, code }],
      [{ id: "player.js", code: 'export const x = "stock" + "Pipelines";' }],
    ),
  );
  assert.throws(
    () => run(code, [{ id, code }], [...emitted, ...emitted]),
    /one JavaScript chunk/,
  );
  assert.throws(() =>
    run(code, [{ id, code }], emitted, {
      ...mappings,
      createRenderPipeline: "host",
    }),
  );
  assert.throws(() => run(code, [{ id, code }], emitted, {}));
  assert.throws(() =>
    run(code, [{ id, code }], emitted, { ...mappings, stockPipelines: false }),
  );
  assert.throws(() =>
    run(code, [{ id, code }], emitted, {
      ...mappings,
      stockPipelines: mappings.stockPipelineTasks,
    }),
  );
  const empty = run(
    code,
    [{ id: "empty.js", code: "export const ready = true;" }],
    emitted,
    {},
  );
  assert.equal(empty.rendererIncluded, false);
});

test("the actual minifier preserves host/schema/future names and an executed program", async () => {
  const source = `export function check() { class Test { stockPipelines = 3; read() { return this.stockPipelines; } } const host = {createRenderPipeline: 7, stockPipelinesV2: 9, schema: "still-public"}; return [new Test().read(), host.createRenderPipeline, host.stockPipelinesV2, host.schema]; }`;
  const result = minifySync("guard-control.mjs", source, {
    module: true,
    mangleProps: playerPrivatePropertyMinify(),
  });
  assert.equal(result.errors.length, 0);
  assert.deepEqual(Object.keys(result.mangleCache!), ["stockPipelines"]);
  assert.notEqual(result.mangleCache!.stockPipelines, "stockPipelines");
  for (const name of ["createRenderPipeline", "stockPipelinesV2", "schema"])
    assert(result.code.includes(name));
  assert.deepEqual(
    (
      await import(`data:text/javascript,${encodeURIComponent(result.code)}`)
    ).check(),
    [3, 7, 9, "still-public"],
  );
});
