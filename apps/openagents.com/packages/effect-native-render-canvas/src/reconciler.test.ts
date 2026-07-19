import { Effect } from "effect";
import { describe, expect, test } from "vite-plus/test";

import { applyOps } from "./backend";
import { makeHeadlessCanvasBackend } from "./headless";
import {
  deepEqual,
  diffScene,
  DuplicateNodeKeyError,
  flattenScene,
  type SceneOp,
} from "./reconciler";
import {
  basicMaterial,
  box,
  type CanvasScene,
  childrenOf,
  decodeScene,
  encodeScene,
  group,
  mesh,
  perspectiveCamera,
  scene,
  toLeaf,
} from "./scene";
import { buildThreeDescriptors, toThreeDescriptorProps } from "./three-backend";

// ---------------------------------------------------------------------------
// Shared fixtures
// ---------------------------------------------------------------------------

const cam = perspectiveCamera({
  position: [0, 0, 5],
  target: [0, 0, 0],
  fov: 60,
  near: 0.1,
  far: 1000,
});

const redBox = mesh({
  key: "a",
  geometry: box({ width: 1, height: 1, depth: 1 }),
  material: basicMaterial({ color: "#ff0000" }),
});
const greenBox = mesh({
  key: "b",
  geometry: box({ width: 1, height: 1, depth: 1 }),
  material: basicMaterial({ color: "#00ff00" }),
});

// ---------------------------------------------------------------------------
// deepEqual
// ---------------------------------------------------------------------------

describe("deepEqual", () => {
  test("primitives, arrays and nested objects compare structurally", () => {
    expect(deepEqual(1, 1)).toBe(true);
    expect(deepEqual("x", "x")).toBe(true);
    expect(deepEqual([1, 2, 3], [1, 2, 3])).toBe(true);
    expect(deepEqual({ a: 1, b: { c: 2 } }, { a: 1, b: { c: 2 } })).toBe(true);
    expect(deepEqual(null, null)).toBe(true);
  });

  test("distinguishes length, key-count, missing keys and values", () => {
    expect(deepEqual([1, 2], [1, 2, 3])).toBe(false);
    expect(deepEqual({ a: 1 }, { a: 1, b: 2 })).toBe(false);
    expect(deepEqual({ a: 1 }, { b: 1 })).toBe(false);
    expect(deepEqual({ a: 1 }, { a: 2 })).toBe(false);
    // Quirk of the implementation: a mismatched array pair falls through to the
    // object-key branch, so an array and an object with identical indexed keys
    // compare equal. Documented here so a mutation to that fall-through fails.
    expect(deepEqual([1, 2], { 0: 1, 1: 2 })).toBe(true);
    // But an array vs a longer object still differs by key count.
    expect(deepEqual([1, 2], { 0: 1, 1: 2, 2: 3 })).toBe(false);
  });

  test("NaN is not equal to NaN and null is not an object", () => {
    // a === b is false for NaN, and neither branch (array/object) applies.
    expect(deepEqual(NaN, NaN)).toBe(false);
    expect(deepEqual(null, {})).toBe(false);
    expect(deepEqual(1, "1")).toBe(false);
  });
});

// ---------------------------------------------------------------------------
// flattenScene + DuplicateNodeKeyError
// ---------------------------------------------------------------------------

describe("flattenScene", () => {
  test("indexes every node with parent/index/depth and stripped leaves", () => {
    const s = scene({ camera: cam }, [group({ key: "g" }, [redBox]), greenBox]);
    const flat = flattenScene(s);

    expect([...flat.keys()]).toEqual(["g", "a", "b"]);
    expect(flat.get("g")).toEqual({
      id: "g",
      parentId: null,
      index: 0,
      depth: 0,
      leaf: toLeaf(group({ key: "g" }, [redBox])),
    });
    expect(flat.get("a")).toEqual({ id: "a", parentId: "g", index: 0, depth: 1, leaf: redBox });
    expect(flat.get("b")).toEqual({ id: "b", parentId: null, index: 1, depth: 0, leaf: greenBox });
    // The group leaf must have had its children stripped.
    expect("children" in (flat.get("g")!.leaf as Record<string, unknown>)).toBe(false);
  });

  test("throws DuplicateNodeKeyError with the offending key", () => {
    const dupA = mesh({
      key: "dup",
      geometry: box({ width: 1, height: 1, depth: 1 }),
      material: basicMaterial({ color: "#ffffff" }),
    });
    const dupB = mesh({
      key: "dup",
      geometry: box({ width: 2, height: 2, depth: 2 }),
      material: basicMaterial({ color: "#000000" }),
    });
    const s = scene({ camera: cam }, [dupA, dupB]);

    expect(() => flattenScene(s)).toThrow(DuplicateNodeKeyError);
    try {
      flattenScene(s);
      throw new Error("expected throw");
    } catch (e) {
      expect(e).toBeInstanceOf(DuplicateNodeKeyError);
      expect((e as DuplicateNodeKeyError).key).toBe("dup");
      expect((e as DuplicateNodeKeyError)._tag).toBe("DuplicateNodeKeyError");
    }
  });
});

// ---------------------------------------------------------------------------
// diffScene — the crown jewel
// ---------------------------------------------------------------------------

describe("diffScene", () => {
  test("full mount from undefined: SetCamera then CreateNode, no SetBackground when no background", () => {
    const s = scene({ camera: cam }, [redBox]);
    const ops = diffScene(undefined, s);
    expect(ops).toEqual([
      { _tag: "SetCamera", camera: cam },
      { _tag: "CreateNode", id: "a", parentId: null, index: 0, node: redBox },
    ] satisfies ReadonlyArray<SceneOp>);
  });

  test("full mount emits SetBackground when a background is present", () => {
    const s = scene({ camera: cam, background: "#000000" }, [redBox]);
    const ops = diffScene(undefined, s);
    expect(ops).toEqual([
      { _tag: "SetCamera", camera: cam },
      { _tag: "SetBackground", color: "#000000" },
      { _tag: "CreateNode", id: "a", parentId: null, index: 0, node: redBox },
    ] satisfies ReadonlyArray<SceneOp>);
  });

  test("no camera/background ops when both are unchanged (deep-equal, different refs)", () => {
    const prev = scene({ camera: cam }, [redBox]);
    // A structurally identical but distinct camera object must not trigger SetCamera.
    const cam2 = perspectiveCamera({
      position: [0, 0, 5],
      target: [0, 0, 0],
      fov: 60,
      near: 0.1,
      far: 1000,
    });
    const next = scene({ camera: cam2 }, [redBox]);
    expect(diffScene(prev, next)).toEqual([]);
  });

  test("adding a node emits a single CreateNode at the new index", () => {
    const prev = scene({ camera: cam }, [redBox]);
    const next = scene({ camera: cam }, [redBox, greenBox]);
    expect(diffScene(prev, next)).toEqual([
      { _tag: "CreateNode", id: "b", parentId: null, index: 1, node: greenBox },
    ] satisfies ReadonlyArray<SceneOp>);
  });

  test("removing a node emits a single RemoveNode", () => {
    const prev = scene({ camera: cam }, [redBox, greenBox]);
    const next = scene({ camera: cam }, [redBox]);
    expect(diffScene(prev, next)).toEqual([
      { _tag: "RemoveNode", id: "b" },
    ] satisfies ReadonlyArray<SceneOp>);
  });

  test("reordering two nodes emits MoveNode for both in next-tree order", () => {
    const prev = scene({ camera: cam }, [redBox, greenBox]);
    const next = scene({ camera: cam }, [greenBox, redBox]);
    // nextMap iterates in next-tree order: b (now index 0) then a (now index 1).
    expect(diffScene(prev, next)).toEqual([
      { _tag: "MoveNode", id: "b", parentId: null, index: 0 },
      { _tag: "MoveNode", id: "a", parentId: null, index: 1 },
    ] satisfies ReadonlyArray<SceneOp>);
  });

  test("changing a prop emits UpdateNode carrying the new leaf", () => {
    const prev = scene({ camera: cam }, [redBox]);
    const recolored = mesh({
      key: "a",
      geometry: box({ width: 1, height: 1, depth: 1 }),
      material: basicMaterial({ color: "#123456" }),
    });
    const next = scene({ camera: cam }, [recolored]);
    expect(diffScene(prev, next)).toEqual([
      { _tag: "UpdateNode", id: "a", node: recolored },
    ] satisfies ReadonlyArray<SceneOp>);
  });

  test("removals are ordered deepest-first", () => {
    const prev = scene({ camera: cam }, [group({ key: "g" }, [redBox]), greenBox]);
    const next = scene({ camera: cam }, []);
    expect(diffScene(prev, next)).toEqual([
      { _tag: "RemoveNode", id: "a" }, // depth 1, removed first
      { _tag: "RemoveNode", id: "g" }, // depth 0
      { _tag: "RemoveNode", id: "b" }, // depth 0
    ] satisfies ReadonlyArray<SceneOp>);
  });

  test("creations are ordered shallowest-first (parent before child)", () => {
    const prev = scene({ camera: cam }, []);
    const g = group({ key: "g" }, [redBox]);
    const next = scene({ camera: cam }, [g]);
    expect(diffScene(prev, next)).toEqual([
      { _tag: "CreateNode", id: "g", parentId: null, index: 0, node: toLeaf(g) },
      { _tag: "CreateNode", id: "a", parentId: "g", index: 0, node: redBox },
    ] satisfies ReadonlyArray<SceneOp>);
  });

  test("background change and removal set the exact color", () => {
    const withBg = scene({ camera: cam, background: "#000000" }, [redBox]);
    const changed = scene({ camera: cam, background: "#111111" }, [redBox]);
    const cleared = scene({ camera: cam }, [redBox]);
    expect(diffScene(withBg, changed)).toEqual([{ _tag: "SetBackground", color: "#111111" }]);
    expect(diffScene(withBg, cleared)).toEqual([{ _tag: "SetBackground", color: undefined }]);
  });

  test("camera change emits SetCamera with the next camera only", () => {
    const prev = scene({ camera: cam }, [redBox]);
    const cam2 = perspectiveCamera({
      position: [1, 2, 3],
      target: [0, 0, 0],
      fov: 45,
      near: 0.1,
      far: 500,
    });
    const next = scene({ camera: cam2 }, [redBox]);
    expect(diffScene(prev, next)).toEqual([
      { _tag: "SetCamera", camera: cam2 },
    ] satisfies ReadonlyArray<SceneOp>);
  });
});

// ---------------------------------------------------------------------------
// scene.ts constructors, toLeaf/childrenOf, decode/encode
// ---------------------------------------------------------------------------

describe("scene constructors", () => {
  test("box / perspectiveCamera build exact tagged descriptors", () => {
    expect(box({ width: 2, height: 3, depth: 4 })).toEqual({
      _tag: "Box",
      width: 2,
      height: 3,
      depth: 4,
    });
    expect(
      perspectiveCamera({ position: [0, 0, 5], target: [0, 0, 0], fov: 60, near: 0.1, far: 1000 }),
    ).toEqual({
      _tag: "Perspective",
      position: [0, 0, 5],
      target: [0, 0, 0],
      fov: 60,
      near: 0.1,
      far: 1000,
    });
  });

  test("constructors reject invalid values on construction", () => {
    expect(() => box({ width: -1, height: 1, depth: 1 })).toThrow();
    expect(() => basicMaterial({ color: "not-a-hex" })).toThrow();
    expect(() =>
      perspectiveCamera({ position: [0, 0, 5], target: [0, 0, 0], fov: 200, near: 0.1, far: 1000 }),
    ).toThrow();
  });

  test("toLeaf strips group children; childrenOf reads them", () => {
    const g = group({ key: "g" }, [redBox, greenBox]);
    const leaf = toLeaf(g) as Record<string, unknown>;
    expect("children" in leaf).toBe(false);
    expect(leaf._tag).toBe("Group");
    expect(childrenOf(g)).toEqual([redBox, greenBox]);
    // Non-group leaves pass through unchanged and have no children.
    expect(toLeaf(redBox)).toBe(redBox);
    expect(childrenOf(redBox)).toEqual([]);
  });

  test("encodeScene → decodeScene round-trips; decodeScene rejects invalid input", () => {
    const s: CanvasScene = scene({ camera: cam, background: "#0a0a0a" }, [
      group({ key: "g" }, [redBox]),
      greenBox,
    ]);
    expect(decodeScene(encodeScene(s))).toEqual(s);
    expect(() => decodeScene({ nope: true })).toThrow();
    expect(() =>
      decodeScene({ _tag: "Scene", catalogVersion: "bad-version", camera: cam, children: [] }),
    ).toThrow();
  });
});

// ---------------------------------------------------------------------------
// three-backend pure descriptor mapping
// ---------------------------------------------------------------------------

describe("three-backend descriptors", () => {
  test("toThreeDescriptorProps lowercases the kind and strips _tag/key", () => {
    expect(toThreeDescriptorProps(redBox)).toEqual({
      kind: "mesh",
      props: {
        geometry: { _tag: "Box", width: 1, height: 1, depth: 1 },
        material: { _tag: "Basic", color: "#ff0000" },
      },
    });
    expect(toThreeDescriptorProps(toLeaf(group({ key: "g" }, [])))).toEqual({
      kind: "group",
      props: {},
    });
  });

  test("buildThreeDescriptors nests children and sorts by index, omitting empty children", () => {
    const groupLeaf = toLeaf(group({ key: "g" }, []));
    const stored = [
      // deliberately inserted out of order to prove the index sort
      { id: "b", parentId: "g", index: 1, node: greenBox },
      { id: "a", parentId: "g", index: 0, node: redBox },
      { id: "g", parentId: null, index: 0, node: groupLeaf },
    ];
    expect(buildThreeDescriptors(stored)).toEqual([
      {
        id: "g",
        kind: "group",
        props: {},
        children: [
          {
            id: "a",
            kind: "mesh",
            props: {
              geometry: { _tag: "Box", width: 1, height: 1, depth: 1 },
              material: { _tag: "Basic", color: "#ff0000" },
            },
          },
          {
            id: "b",
            kind: "mesh",
            props: {
              geometry: { _tag: "Box", width: 1, height: 1, depth: 1 },
              material: { _tag: "Basic", color: "#00ff00" },
            },
          },
        ],
      },
    ]);
  });
});

// ---------------------------------------------------------------------------
// headless backend end-to-end: diffScene → applyOps → snapshot
// ---------------------------------------------------------------------------

describe("headless backend reconcile", () => {
  test("mount then add reconciles into the exact recorded snapshot", () => {
    const s1 = scene({ camera: cam, background: "#020202" }, [redBox]);
    const s2 = scene({ camera: cam, background: "#020202" }, [redBox, greenBox]);

    const { ops1, snap1, snap2 } = Effect.runSync(
      Effect.scoped(
        Effect.gen(function* () {
          const h = yield* makeHeadlessCanvasBackend();
          yield* applyOps(h.backend, diffScene(undefined, s1));
          const snap1 = yield* h.snapshot;
          const ops1 = yield* h.ops;
          yield* applyOps(h.backend, diffScene(s1, s2));
          const snap2 = yield* h.snapshot;
          return { ops1, snap1, snap2 };
        }),
      ),
    );

    // The applied op log for the initial mount is exact and ordered.
    expect(ops1).toEqual([
      { _tag: "SetCamera", camera: cam },
      { _tag: "SetBackground", color: "#020202" },
      { _tag: "CreateNode", id: "a", parentId: null, index: 0, node: redBox },
    ]);

    expect(snap1).toEqual({
      camera: cam,
      background: "#020202",
      nodes: [{ id: "a", node: redBox, children: [] }],
      frames: 0,
      disposed: false,
    });

    expect(snap2.nodes).toEqual([
      { id: "a", node: redBox, children: [] },
      { id: "b", node: greenBox, children: [] },
    ]);
    expect(snap2.background).toBe("#020202");
    expect(snap2.disposed).toBe(false);
  });

  test("nested group reconcile builds a child tree in the snapshot", () => {
    const s = scene({ camera: cam }, [group({ key: "g" }, [redBox])]);
    const snap = Effect.runSync(
      Effect.scoped(
        Effect.gen(function* () {
          const h = yield* makeHeadlessCanvasBackend();
          yield* applyOps(h.backend, diffScene(undefined, s));
          return yield* h.snapshot;
        }),
      ),
    );
    expect(snap.nodes).toEqual([
      {
        id: "g",
        node: toLeaf(group({ key: "g" }, [])),
        children: [{ id: "a", node: redBox, children: [] }],
      },
    ]);
    expect(snap.camera).toEqual(cam);
  });
});
