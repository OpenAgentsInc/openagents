import assert from "node:assert/strict";
import test from "node:test";
import { Renderer } from "../packages/renderer/renderer.ts";

type Draw = {
  material: number;
  rasterOrientation?: "positive" | "negative" | "degenerate";
  wind?: boolean;
  indexCount: number;
  count: number;
  firstIndex: number;
  baseVertex: number;
  firstInstance: number;
};
const item = (
  material: number,
  orientation: Draw["rasterOrientation"],
  i: number,
): Draw => ({
  material,
  rasterOrientation: orientation,
  wind: i % 2 === 0,
  indexCount: 3 * (i + 1),
  count: 1 + (i % 3),
  firstIndex: i * 5,
  baseVertex: i - 4,
  firstInstance: i * 7,
});
function fixture() {
  const a = { name: "material-a" },
    b = { name: "material-b" },
    c = { name: "material-c" };
  const pipelines = {
    positive: { name: "positive" },
    negative: { name: "negative" },
    degenerate: { name: "degenerate" },
  };
  const groups = [
    a,
    b,
    c,
    b,
    { name: "blend" },
    { name: "flame" },
    { name: "glass" },
  ];
  const sourceDraws = [
    item(0, "positive", 0),
    item(1, "positive", 1),
    item(1, "negative", 2),
    item(1, "negative", 3),
    item(3, "negative", 4),
    item(2, "positive", 5),
    item(0, "positive", 6),
    item(4, "positive", 7),
    item(5, "negative", 8),
    item(6, "degenerate", 9),
  ];
  const authoredDraws = [
    item(1, "positive", 10),
    item(3, "positive", 11),
    item(3, "negative", 12),
    item(3, "negative", 13),
    item(2, "positive", 14),
    item(4, "negative", 15),
    item(5, "positive", 16),
    item(6, "positive", 17),
  ];
  const lookups: unknown[] = [];
  const renderer = Object.assign(Object.create(Renderer.prototype), {
    vertices: { name: "vertices" },
    instances: { name: "source instances" },
    vertexColors: { name: "colors" },
    vertexUV1: { name: "uv1" },
    indices: { name: "indices" },
    meshIndices: { name: "wire indices" },
    zeroWind: { name: "zero wind" },
    materials: groups,
    renderMaterialInfo: [
      {},
      {},
      {},
      {},
      { alphaMode: "BLEND" },
      { flame: {} },
      { glass: {} },
    ],
    draws: sourceDraws,
    authoredInstances: { name: "authored instances" },
    authoredDraws,
    submittedGeometry: { instances: 0, triangles: 0, drawCalls: 0 },
    materialPipeline(
      group: unknown,
      transparent: boolean,
      additive: boolean,
      kind: string,
      orientation: Draw["rasterOrientation"] = "degenerate",
    ) {
      lookups.push({ group, transparent, additive, kind, orientation });
      return pipelines[orientation];
    },
  }) as any;
  const source = {
    instances: renderer.instances,
    wind: { name: "source wind" },
    draws: sourceDraws,
  };
  return { renderer, source, lookups, pipelines };
}
function pass(initial = { name: "caller pipeline" }) {
  const calls: any[][] = [],
    draws: unknown[] = [];
  let pipeline: unknown = initial;
  const groups = new Map<number, unknown>(),
    buffers = new Map<number, unknown>();
  let indices: unknown;
  return {
    calls,
    draws,
    setPipeline(value: unknown) {
      calls.push(["pipeline", value]);
      pipeline = value;
    },
    setBindGroup(index: number, value: unknown) {
      calls.push(["group", index, value]);
      groups.set(index, value);
    },
    setVertexBuffer(index: number, value: unknown) {
      calls.push(["vertex", index, value]);
      buffers.set(index, value);
    },
    setIndexBuffer(value: unknown, format: string) {
      calls.push(["index", value, format]);
      indices = value;
    },
    drawIndexed(...args: number[]) {
      calls.push(["draw", ...args]);
      draws.push({
        args,
        pipeline,
        material: groups.get(1),
        vertices: [...buffers.entries()],
        indices,
      });
    },
  };
}
// Previous direct loops, retained to compare effective state at each original draw.
function reference(
  renderer: any,
  p: any,
  transparent: boolean,
  additive = false,
  source?: any,
  wire = false,
  countSubmission = false,
  specialized?: string,
) {
  p.setVertexBuffer(0, renderer.vertices);
  p.setVertexBuffer(1, source?.instances ?? renderer.instances);
  p.setVertexBuffer(2, renderer.vertexColors);
  p.setVertexBuffer(3, renderer.vertexUV1);
  p.setIndexBuffer(wire ? renderer.meshIndices : renderer.indices, "uint32");
  const include = (draw: Draw) =>
    Boolean(renderer.renderMaterialInfo[draw.material].flame) === additive &&
    (additive ||
      (Boolean(renderer.renderMaterialInfo[draw.material].glass) ||
        renderer.renderMaterialInfo[draw.material].alphaMode === "BLEND") ===
        transparent);
  const submit = (draw: Draw) => {
    p.drawIndexed(
      draw.indexCount * (wire ? 2 : 1),
      draw.count,
      draw.firstIndex * (wire ? 2 : 1),
      draw.baseVertex,
      draw.firstInstance,
    );
    if (countSubmission) {
      renderer.submittedGeometry.instances += draw.count;
      renderer.submittedGeometry.triangles +=
        (draw.indexCount * draw.count) / 3;
      renderer.submittedGeometry.drawCalls++;
    }
  };
  let previous = -1,
    previousOrientation: Draw["rasterOrientation"],
    previousWind: unknown;
  for (const draw of source?.draws ?? renderer.draws) {
    if (!include(draw)) continue;
    const wind = draw.wind
      ? (source?.wind ?? renderer.zeroWind)
      : renderer.zeroWind;
    if (wind !== previousWind) {
      p.setVertexBuffer(4, wind);
      previousWind = wind;
    }
    if (
      draw.material !== previous ||
      draw.rasterOrientation !== previousOrientation
    ) {
      if (specialized)
        p.setPipeline(
          renderer.materialPipeline(
            renderer.materials[draw.material],
            transparent,
            additive,
            specialized,
            draw.rasterOrientation,
          ),
        );
      p.setBindGroup(1, renderer.materials[draw.material]);
      previous = draw.material;
      previousOrientation = draw.rasterOrientation;
    }
    submit(draw);
  }
  if (renderer.authoredInstances && !source?.dynamicOnly) {
    p.setVertexBuffer(4, renderer.zeroWind);
    p.setVertexBuffer(1, renderer.authoredInstances);
    for (const draw of renderer.authoredDraws) {
      if (!include(draw)) continue;
      if (specialized)
        p.setPipeline(
          renderer.materialPipeline(
            renderer.materials[draw.material],
            transparent,
            additive,
            specialized,
            draw.rasterOrientation,
          ),
        );
      p.setBindGroup(1, renderer.materials[draw.material]);
      submit(draw);
    }
  }
}
function compare(
  settings: {
    transparent?: boolean;
    additive?: boolean;
    wire?: boolean;
    specialized?: string;
    source?: boolean;
    dynamicOnly?: boolean;
    empty?: boolean;
    generic?: boolean;
  } = {},
) {
  const current = fixture(),
    old = fixture(),
    actual = pass(),
    expected = pass();
  if (settings.empty) for (const f of [current, old]) f.source.draws.length = 0;
  if (settings.generic)
    for (const f of [current, old]) {
      const generic = { name: "custom generic" };
      f.renderer.materialPipeline = (...args: unknown[]) => {
        f.lookups.push(args);
        return generic;
      };
    }
  const sourceA =
    settings.source === false
      ? undefined
      : { ...current.source, dynamicOnly: settings.dynamicOnly };
  const sourceB =
    settings.source === false
      ? undefined
      : { ...old.source, dynamicOnly: settings.dynamicOnly };
  const args = [
    settings.transparent ?? false,
    settings.additive ?? false,
    sourceA,
    settings.wire ?? false,
    true,
    settings.specialized,
  ] as const;
  current.renderer.draw(actual, ...args);
  reference(
    old.renderer,
    expected,
    args[0],
    args[1],
    sourceB,
    args[3],
    args[4],
    args[5],
  );
  assert.deepEqual(actual.draws, expected.draws);
  assert.deepEqual(current.lookups, old.lookups);
  assert.deepEqual(
    current.renderer.submittedGeometry,
    old.renderer.submittedGeometry,
  );
  assert.deepEqual(
    actual.calls.filter((call) => !["pipeline", "group"].includes(call[0])),
    expected.calls.filter((call) => !["pipeline", "group"].includes(call[0])),
  );
  return { current, old, actual, expected };
}

test("direct loops preserve effective draw state and every indexed field across passes, subsets and wind", () => {
  for (const specialized of [undefined, "main", "planar", "depth"])
    for (const [transparent, additive] of [
      [false, false],
      [true, false],
      [true, true],
    ])
      for (const source of [true, false])
        for (const dynamicOnly of [false, true])
          compare({ specialized, transparent, additive, source, dynamicOnly });
  compare({ wire: true });
  compare({ wire: true, specialized: "main" });
  compare({ empty: true, specialized: "depth" });
});

test("actual pipeline and material identities independently eliminate redundant calls without reordering", () => {
  const { actual, expected } = compare({ specialized: "main" });
  const count = (p: ReturnType<typeof pass>, kind: string) =>
    p.calls.filter((call) => call[0] === kind).length;
  assert.equal(count(expected, "pipeline"), 11);
  assert.equal(count(actual, "pipeline"), 6);
  assert.equal(count(expected, "group"), 11);
  assert.equal(count(actual, "group"), 6);
  assert.equal(actual.draws.length, 12);
  const generic = compare({ specialized: "main", generic: true });
  assert.equal(count(generic.actual, "pipeline"), 2);
  assert.equal(count(generic.expected, "pipeline"), 11);
  const unspecialized = compare();
  assert.equal(count(unspecialized.actual, "pipeline"), 0);
  assert.equal(count(unspecialized.expected, "pipeline"), 0);
});

test("new passes and the source-to-authored boundary always establish their own required state", () => {
  const f = fixture();
  f.source.draws = [item(0, "positive", 0)];
  f.renderer.authoredDraws = [item(0, "positive", 1)];
  for (let i = 0; i < 2; i++) {
    const p = pass();
    f.renderer.draw(p, false, false, f.source, false, true, "main");
    assert.equal(p.calls.filter((call) => call[0] === "pipeline").length, 2);
    assert.equal(
      p.calls.filter((call) => call[0] === "group" && call[1] === 1).length,
      2,
    );
    assert.equal(p.draws.length, 2);
    const switchAt = p.calls.findIndex(
      (call) =>
        call[0] === "vertex" &&
        call[1] === 1 &&
        call[2] === f.renderer.authoredInstances,
    );
    assert.equal(p.calls[switchAt + 1][0], "pipeline");
    assert.equal(p.calls[switchAt + 2][0], "group");
  }
});

test("failed state resolution or pass commands retain the original draw prefix and leave no cross-call state", () => {
  for (const failure of ["resolve", "pipeline", "group"]) {
    const actual = fixture(),
      old = fixture(),
      a = pass(),
      b = pass();
    for (const [f, p] of [
      [actual, a],
      [old, b],
    ] as const) {
      if (failure === "resolve") {
        const resolve = f.renderer.materialPipeline.bind(f.renderer);
        f.renderer.materialPipeline = (group: unknown, ...args: unknown[]) => {
          if (group === f.renderer.materials[2]) throw Error("resolve failed");
          return resolve(group, ...args);
        };
      } else if (failure === "pipeline") {
        const set = p.setPipeline.bind(p);
        p.setPipeline = (value: unknown) => {
          if (value === f.pipelines.negative) throw Error("pipeline failed");
          set(value);
        };
      } else {
        const set = p.setBindGroup.bind(p);
        p.setBindGroup = (index: number, value: unknown) => {
          if (index === 1 && value === f.renderer.materials[2])
            throw Error("group failed");
          set(index, value);
        };
      }
    }
    assert.throws(
      () =>
        actual.renderer.draw(
          a,
          false,
          false,
          actual.source,
          false,
          true,
          "main",
        ),
      new RegExp(failure),
    );
    assert.throws(
      () =>
        reference(
          old.renderer,
          b,
          false,
          false,
          old.source,
          false,
          true,
          "main",
        ),
      new RegExp(failure),
    );
    assert.deepEqual(a.draws, b.draws);
    assert.deepEqual(
      actual.renderer.submittedGeometry,
      old.renderer.submittedGeometry,
    );
    actual.source.draws = [item(0, "positive", 0)];
    actual.renderer.authoredDraws = [];
    const retry = pass();
    actual.renderer.draw(
      retry,
      false,
      false,
      actual.source,
      false,
      true,
      "main",
    );
    assert.equal(
      retry.calls.filter((call) => call[0] === "pipeline").length,
      1,
    );
    assert.equal(
      retry.calls.filter((call) => call[0] === "group" && call[1] === 1).length,
      1,
    );
    assert.equal(retry.draws.length, 1);
  }
});

test("a missing first internal binding still reaches the pass validator", () => {
  for (const field of ["material", "pipeline"]) {
    const f = fixture(),
      p = pass();
    f.source.draws = [item(0, "positive", 0)];
    f.renderer.authoredDraws = [];
    if (field === "material") {
      f.renderer.materials[0] = undefined;
      const set = p.setBindGroup.bind(p);
      p.setBindGroup = (index: number, value: unknown) => {
        if (index === 1 && value === undefined) throw Error("invalid material");
        set(index, value);
      };
    } else {
      f.renderer.materialPipeline = () => undefined;
      const set = p.setPipeline.bind(p);
      p.setPipeline = (value: unknown) => {
        if (value === undefined) throw Error("invalid pipeline");
        set(value);
      };
    }
    assert.throws(
      () => f.renderer.draw(p, false, false, f.source, false, true, "main"),
      new RegExp("invalid " + field),
    );
    assert.equal(p.draws.length, 0);
  }
});
