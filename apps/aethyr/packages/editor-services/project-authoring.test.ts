import { createHash } from "node:crypto";
import { test } from "node:test";
import assert from "node:assert/strict";
import { DEFAULT_EFFECT, materialEffect } from "./authoring.ts";
import { ProjectAuthoring } from "./project-authoring.ts";
import { MATERIAL } from "../renderer/shaders.ts";
const base = MATERIAL;
function fixture(rendererSource = base) {
  let snapshot: any = {
    revision: 0,
    canUndo: false,
    authoring: { environment: {}, materials: {}, effect: null },
  };
  let active = rendererSource;
  let held: (() => void) | undefined;
  const author = new ProjectAuthoring({
    snapshot: () => snapshot,
    materialIds: () => [{ id: "trim", objects: ["wolf"] }],
    hasAsset: () => false,
    rendererSource,
    kernel: (r: any) => {
      const next = {
        ...snapshot,
        revision: r.baseRevision + 1,
        authoring:
          r.operations[0]?.state ??
          r.operations[0]?.authoring ??
          snapshot.authoring,
        ok: true,
      };
      return next;
    },
    shader: async (source, current) => {
      if (held)
        await new Promise<void>((resolve) => {
          held = resolve;
        });
      if (!current()) return false;
      if (source.includes("syntax_error"))
        throw new Error("Line 2: invalid source");
      active = source;
      return true;
    },
    activate: async (accepted) => {
      snapshot = accepted;
    },
  });
  return {
    author,
    get snapshot() {
      return snapshot;
    },
    get active() {
      return active;
    },
    hold() {
      held = () => {};
    },
    release() {
      held?.();
      held = undefined;
    },
  };
}
test("world effect uses the current renderer UV interface", () => {
  const shader = materialEffect(DEFAULT_EFFECT, MATERIAL);
  assert.match(
    shader,
    /oa_material\(base,v\.world,v\.normal\.xyz,v\.rawUV,frame\.eye\.w\)/,
  );
  assert.doesNotMatch(shader, /oa_material\(base,v\.world,v\.normal,v\.uv,/);
});
test("material hook rejects interface drift before replacing the prior effect", async () => {
  for (const rendererSource of [
    "renamedAlpha=1.0;",
    base + base,
    base.replaceAll("rawUV", "renamedUV"),
  ]) {
    assert.throws(
      () => materialEffect(DEFAULT_EFFECT, rendererSource),
      /interface changed/,
    );
    const f = fixture(rendererSource),
      prior = f.active;
    f.author.draft = DEFAULT_EFFECT.replace(
      "return base;",
      "return base * 0.5;",
    );
    await assert.rejects(
      f.author.execute(
        {
          type: "compile",
          asset: "world-effect",
          source_sha256: createHash("sha256")
            .update(f.author.draft)
            .digest("hex"),
        },
        0,
        "compile",
        f.author.supersede(),
      ),
      /interface changed/,
    );
    assert.equal(f.active, prior);
    assert.equal(f.snapshot.revision, 0);
    assert.notEqual(f.author.view().draft, f.author.view().active_source);
    assert.match(f.author.view().diagnostics[0], /interface changed/);
  }
});
test("valid effect, syntax failure, repair and superseded compilation preserve working state", async () => {
  const f = fixture();
  f.author.draft = DEFAULT_EFFECT.replace("return base;", "return base * 0.5;");
  await f.author.execute(
    {
      type: "compile",
      asset: "world-effect",
      source_sha256: createHash("sha256").update(f.author.draft).digest("hex"),
    },
    0,
    "good",
    f.author.supersede(),
  );
  const good = f.active;
  f.author.draft = DEFAULT_EFFECT.replace("return base;", "syntax_error");
  await assert.rejects(
    f.author.execute(
      {
        type: "compile",
        asset: "world-effect",
        source_sha256: createHash("sha256")
          .update(f.author.draft)
          .digest("hex"),
      },
      1,
      "bad",
      f.author.supersede(),
    ),
    /invalid source/,
  );
  assert.equal(f.active, good);
  assert.equal(f.snapshot.revision, 1);
  f.author.draft = DEFAULT_EFFECT;
  f.hold();
  const old = f.author.execute(
    {
      type: "compile",
      asset: "world-effect",
      source_sha256: createHash("sha256").update(f.author.draft).digest("hex"),
    },
    1,
    "slow",
    f.author.supersede(),
  );
  f.author.supersede();
  f.release();
  await assert.rejects(old, /newer edit/);
  assert.equal(f.active, good);
  assert.equal(f.snapshot.revision, 1);
  await f.author.execute(
    {
      type: "compile",
      asset: "world-effect",
      source_sha256: createHash("sha256").update(f.author.draft).digest("hex"),
    },
    1,
    "repair",
    f.author.supersede(),
  );
  assert.equal(f.snapshot.revision, 2);
  assert.equal(f.author.view().draft, f.author.view().active_source);
});
test("material interface rejects bindings, loops and calls into the private shader", () => {
  for (const text of [
    "loop {}",
    "for(var i=0;i<4;i++){}",
    "while true {}",
    "textureSample(baseImage,texSampler,uv);",
    "privateEngineFunction();",
  ]) {
    assert.throws(() =>
      materialEffect(
        DEFAULT_EFFECT.replace("return base;", text + "return base;"),
        base,
      ),
    );
  }
  assert.match(materialEffect(DEFAULT_EFFECT, base), /base=clamp\(oa_material/);
});
test("stale edits and invented material IDs cannot advance the project", async () => {
  const f = fixture();
  await assert.rejects(
    f.author.execute(
      {
        type: "propose",
        operations: [
          { type: "material", target: "unknown", parameter: "red", value: 1 },
        ],
      },
      0,
      "bad",
      f.author.supersede(),
    ),
    /material in this world/,
  );
  await assert.rejects(
    f.author.execute({ type: "undo" }, 9, "stale", f.author.supersede()),
    /world changed/,
  );
  assert.equal(f.snapshot.revision, 0);
});

test("environment values fail before the kernel when outside the renderer contract", async () => {
  const f = fixture();
  await assert.rejects(
    f.author.execute(
      {
        type: "propose",
        operations: [
          {
            type: "material",
            target: "environment",
            parameter: "fogDensity",
            value: 0.08,
          },
        ],
      },
      0,
      "bad-fog",
      f.author.supersede(),
    ),
    /supported material or lighting value/,
  );
  assert.equal(f.snapshot.revision, 0);
});

test("compile consent binds the source bytes even though draft writes do not advance world history", async () => {
  const f = fixture();
  const approved = createHash("sha256").update(f.author.draft).digest("hex");
  f.author.draft = f.author.draft.replace("return base;", "return base * 0.5;");
  await assert.rejects(
    f.author.execute(
      { type: "compile", asset: "world-effect", source_sha256: approved },
      0,
      "stale-source",
      f.author.supersede(),
    ),
    /source draft changed/,
  );
  assert.equal(f.snapshot.revision, 0);
  assert.equal(f.active, base);
});

test("staged GPU failure and stale completion cannot commit a saved project", async () => {
  let commits = 0,
    activates = 0,
    releases = 0,
    ready = true;
  const snapshot: any = {
    revision: 0,
    entities: [],
    rooms: [],
    removedWalls: [],
    authoring: { environment: {}, materials: {}, effect: null },
  };
  const template = {
    world: "house",
    engine_release: "fixture",
    asset_set_sha256: "a".repeat(64),
    scene_sha256: "b".repeat(64),
  };
  let fail = true;
  const author = new ProjectAuthoring({
    snapshot: () => snapshot,
    materialIds: () => [],
    hasAsset: () => true,
    rendererSource: base,
    template: () => template,
    restoreEntity: (id, e) => ({ id, kind: e.asset, ...e }),
    kernel: (r: any) => {
      if (r.type === "command") commits++;
      return { ok: true, revision: 1, authoring: r.operations[0].authoring };
    },
    shader: async () => {
      throw Error("unexpected immediate shader adoption");
    },
    prepare: async () => {
      if (fail) throw Error("GPU capacity exhausted");
      return {
        ready: () => ready,
        activate: () => {
          activates++;
        },
        discard: () => {
          releases++;
        },
      };
    },
    activate: async () => {},
  });
  const document = author.export();
  document.source_draft = "unfinished draft";
  const action = { type: "restore_project" as const, document };
  await assert.rejects(
    author.execute(action, 0, "failed", author.supersede()),
    /GPU capacity/,
  );
  assert.equal(commits, 0);
  assert.equal(activates, 0);
  assert.equal(snapshot.revision, 0);
  fail = false;
  ready = false;
  await assert.rejects(
    author.execute(action, 0, "stale", author.supersede()),
    /newer edit/,
  );
  assert.equal(commits, 0);
  assert.equal(activates, 0);
  assert.equal(releases, 1);
  ready = true;
  await author.execute(action, 0, "good", author.supersede());
  assert.equal(commits, 1);
  assert.equal(activates, 1);
  assert.equal(author.draft, "unfinished draft");
  assert.equal(author.export().template.scene_sha256, template.scene_sha256);
});

test("pretty reordered material sources restore without rewriting their text; changed values fail before commit", async () => {
  const { materialSource, materialHash, sameMaterialResource } =
    await import("./creator-material.ts");
  const material: any = {
    schema: "openagents.verse.material.v1",
    target: {
      entity: "wolf-a",
      asset: "custom:haunt:werewolf",
      surface: "werewolf.face",
      material: "custom:haunt:werewolf:face",
      uv_set: 0,
      size: [256, 256],
      uv_bounds: [0, 0, 1, 1],
    },
    generated: { image: null, visible: true, strength: 1 },
    handmade: { image: null, visible: true, strength: 1 },
    recipe: null,
  };
  const original = await materialSource(material);
  const reorder = (value: any): any =>
    Array.isArray(value)
      ? value.map(reorder)
      : value && typeof value === "object"
        ? Object.fromEntries(
            Object.keys(value)
              .reverse()
              .map((k) => [k, reorder(value[k])]),
          )
        : value;
  const text = JSON.stringify(reorder(material), null, 2).replaceAll(
    "\n",
    "\n" + " ".repeat(160),
  );
  assert.ok(new TextEncoder().encode(text).length > 4096);
  assert.ok(new TextEncoder().encode(text).length <= 24 * 1024);
  const source = {
    name: original.name,
    text,
    sha256: await materialHash(new TextEncoder().encode(text)),
  };
  const template = {
    world: "house",
    engine_release: "fixture-6",
    asset_set_sha256: "a".repeat(64),
    scene_sha256: "b".repeat(64),
  };
  const document: any = {
    schema: "openagents.verse.project.v1",
    namespace: "haunted-house",
    template,
    source_revision: 0,
    entities: {
      "wolf-a": {
        asset: "custom:haunt:werewolf",
        position: [0, 0, 0],
        yaw: 0,
        scale: 1,
      },
    },
    rooms: {},
    removed_walls: [],
    materials: {},
    environment: {},
    source_draft: DEFAULT_EFFECT,
    active_source: DEFAULT_EFFECT,
    layers: [],
    originals: [source],
  };
  let snapshot: any = {
    revision: 0,
    entities: [],
    authoring: { environment: {}, materials: {}, effect: DEFAULT_EFFECT },
  };
  let commits = 0;
  const author = new ProjectAuthoring({
    template: () => template,
    snapshot: () => snapshot,
    restoreEntity: (id, e) => ({ id, kind: e.asset, ...e }),
    materialIds: () => [],
    hasAsset: () => true,
    rendererSource: MATERIAL,
    shader: async () => true,
    kernel: (request: any) => {
      if (request.type === "command") commits++;
      const op = request.operations[0];
      return {
        ok: true,
        revision: snapshot.revision + 1,
        entities: op.entities,
        authoring: op.authoring,
      };
    },
    prepare: async () => ({ ready: () => true, activate() {}, discard() {} }),
    activate: async (accepted) => {
      snapshot = accepted;
    },
  });
  await author.execute(
    {
      type: "restore_material_project",
      document,
      materials: [{ document: material, generated: null, handmade: null }],
    },
    0,
    "reordered",
    author.supersede(),
  );
  assert.equal(commits, 1);
  assert.deepEqual(author.export().originals, [source]);
  assert.equal(author.export().originals[0].text, text);
  const accepted = structuredClone(snapshot);
  const changed = structuredClone(material);
  changed.generated.strength = 0.5;
  await assert.rejects(
    author.execute(
      {
        type: "restore_material_project",
        document,
        materials: [{ document: changed, generated: null, handmade: null }],
      },
      1,
      "different",
      author.supersede(),
    ),
    /sources do not match/,
  );
  assert.deepEqual(snapshot, accepted);
  assert.equal(commits, 1);
  assert.equal(
    sameMaterialResource(
      { sha256: "a", bytes: 5, media_type: "image/png" },
      { media_type: "image/png", bytes: 5, sha256: "a" },
    ),
    true,
  );
  assert.equal(
    sameMaterialResource(
      { sha256: "a", bytes: 5, media_type: "image/png" },
      { media_type: "image/png", bytes: 6, sha256: "a" },
    ),
    false,
  );
});

test("creator face color is composed before lighting through the existing texture array", () => {
  const hook = MATERIAL.indexOf(
    "if stockMaterialFeature(64u,material.creator.x>0.5)",
  );
  assert.ok(hook > 0);
  assert.ok(hook < MATERIAL.indexOf("var direct=vec3(0.0)"));
  assert.match(
    MATERIAL.slice(hook, hook + 220),
    /textureSampleLevel\(LC/,
  );
});
