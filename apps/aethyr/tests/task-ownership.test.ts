import test from "node:test";
import assert from "node:assert/strict";
import {
  TaskOwnership,
  type ResourceBorrow,
  type TaskContext,
} from "../packages/runtime/task-ownership.ts";

interface RuntimeResources {
  input: Float32Array;
  pose: { x: number };
  bounds: Float64Array;
  view: Float32Array;
  visibility: Uint8Array;
  gpu: { submissions: number };
}
function fixture() {
  const scope = new TaskOwnership<RuntimeResources>();
  const input = scope.resource("input", new Float32Array([2]), "input");
  const pose = scope.resource("pose", { x: 0 }, "simulation");
  const bounds = scope.resource(
    "bounds",
    new Float64Array([1, 2, 3, 4]),
    "renderer",
  );
  scope.resource("view", new Float32Array([1]), "renderer");
  scope.resource("visibility", new Uint8Array(4), "renderer");
  scope.resource("gpu", { submissions: 0 }, "renderer");
  scope.define({
    id: "step",
    owner: "simulation",
    reads: ["input"],
    writes: ["pose"],
  });
  scope.define({
    id: "camera",
    owner: "renderer",
    reads: ["pose"],
    writes: ["view"],
  });
  scope.define({
    id: "cull",
    owner: "renderer",
    reads: ["bounds", "view"],
    writes: ["visibility"],
  });
  scope.define({
    id: "encode",
    owner: "renderer",
    reads: ["visibility"],
    writes: ["gpu"],
  });
  scope.define({ id: "inspect-pose", owner: "tools", reads: ["pose"] });
  scope.define({ id: "inspect-bounds", owner: "tools", reads: ["bounds"] });
  scope.define({ id: "other-inspect", owner: "tools", reads: ["bounds"] });
  return { scope, input, pose, bounds };
}

test("declared runtime owners execute synchronously and read access does not grant write access", () => {
  const { scope } = fixture();
  const value = scope.run("step", (context) => {
    const input = context.read("input").withView((view) => view[0]);
    context.write("pose").withView((pose) => {
      pose.x += input;
    });
    return context.read("pose").withView((pose) => pose.x);
  });
  assert.equal(value, 2);
  scope.run("inspect-pose", (context) => {
    assert.equal(
      context.read("pose").withView((pose) => pose.x),
      2,
    );
    assert.throws(() => context.write("pose"), /did not declare write/);
    assert.throws(() => context.read("gpu"), /did not declare read/);
  });
  assert.equal(scope.snapshot().activeTasks, 0);
  assert.equal(scope.snapshot().accessErrors, 2);
  assert.equal(scope.snapshot().runs, 2);
  scope.close();
});

test("definitions reject undeclared resources, duplicate accesses, and writes by another owner", () => {
  const { scope } = fixture(),
    before = scope.snapshot().tasks;
  assert.throws(
    () => scope.define({ id: "bad-owner", owner: "tools", writes: ["gpu"] }),
    /another runtime owner/,
  );
  assert.throws(
    () =>
      scope.define({
        id: "missing",
        owner: "renderer",
        reads: ["not-registered" as never],
      }),
    /not registered/,
  );
  assert.throws(
    () =>
      scope.define({
        id: "duplicate-read",
        owner: "tools",
        reads: ["pose", "pose"],
      }),
    /once/,
  );
  assert.throws(
    () =>
      scope.define({
        id: "duplicate-write",
        owner: "renderer",
        writes: ["gpu", "gpu"],
      }),
    /once/,
  );
  assert.throws(
    () =>
      scope.define({
        id: "both",
        owner: "renderer",
        reads: ["gpu"],
        writes: ["gpu"],
      }),
    /once/,
  );
  assert.throws(
    () => scope.define({ id: "step", owner: "simulation" }),
    /distinct/,
  );
  assert.throws(
    () => scope.resource("pose", { x: 99 }, "simulation"),
    /distinct/,
  );
  assert.throws(
    () => scope.run("undefined-task", () => 0),
    /no access declaration/,
  );
  assert.equal(scope.snapshot().tasks, before);
  scope.close();
});

test("task declarations copy caller arrays and validate read-write conflicts before group execution", () => {
  const { scope } = fixture(),
    reads: (keyof RuntimeResources)[] = ["bounds"];
  scope.define({ id: "copied", owner: "tools", reads });
  reads.push("pose");
  scope.validateGroup(["inspect-bounds", "other-inspect", "step"]);
  assert.throws(
    () => scope.validateGroup(["step", "inspect-pose"]),
    /conflicting resource access/,
  );
  assert.throws(
    () => scope.validateGroup(["camera", "cull"]),
    /conflicting resource access/,
  );
  scope.safePoint(() =>
    scope.define({ id: "other-encode", owner: "renderer", writes: ["gpu"] }),
  );
  assert.throws(
    () => scope.validateGroup(["encode", "other-encode"]),
    /conflicting resource access/,
  );
  assert.equal(scope.snapshot().runs, 0);
  scope.run("copied", (context) =>
    assert.throws(() => context.read("pose"), /did not declare read/),
  );
  scope.close();
});

test("deterministic planning preserves every dependency and serial execution follows requested order", () => {
  const { scope } = fixture();
  const planned = scope.plan([
    "step",
    "camera",
    "cull",
    "encode",
    "inspect-bounds",
  ]);
  assert.deepEqual(planned, [
    { tasks: ["step", "inspect-bounds"] },
    { tasks: ["camera"] },
    { tasks: ["cull"] },
    { tasks: ["encode"] },
  ]);
  for (const group of planned) scope.validateGroup(group.tasks);
  const sequence: string[] = [];
  const results = scope.runSerial(
    ["step", "camera", "cull", "encode"],
    (id) => {
      sequence.push(id);
      return sequence.length;
    },
  );
  assert.deepEqual(sequence, ["step", "camera", "cull", "encode"]);
  assert.deepEqual(results, [1, 2, 3, 4]);
  assert.throws(() => scope.plan(["step", "step"]), /distinct/);
  const runs = scope.snapshot().runs;
  assert.throws(
    () => scope.runSerial(["step", "missing"], () => 0),
    /no access declaration/,
  );
  assert.equal(
    scope.snapshot().runs,
    runs,
    "Validate the complete sequence before starting it.",
  );
  scope.close();
});

test("nested readers are valid while conflicting nested work is rejected before it starts", () => {
  const { scope } = fixture();
  scope.run("inspect-bounds", (outer) => {
    assert.equal(
      outer.read("bounds").withView((view) => view[0]),
      1,
    );
    scope.run("other-inspect", (inner) =>
      assert.equal(
        inner.read("bounds").withView((view) => view[1]),
        2,
      ),
    );
  });
  let started = false;
  scope.run("step", () => {
    assert.throws(
      () =>
        scope.run("inspect-pose", () => {
          started = true;
        }),
      /conflicting runtime work/,
    );
    assert.throws(
      () =>
        scope.run("step", () => {
          started = true;
        }),
      /conflicting runtime work/,
    );
  });
  assert.equal(started, false);
  assert.equal(scope.snapshot().activeTasks, 0);
  scope.close();
});

test("borrowed views expire when a task ends and replacement invalidates prior generations", () => {
  const { scope, bounds } = fixture();
  let borrow!: ResourceBorrow<Readonly<Float64Array>>,
    escaped!: TaskContext<RuntimeResources>;
  scope.run("inspect-bounds", (context) => {
    escaped = context;
    borrow = context.read("bounds");
    assert.equal(borrow.valid(), true);
    assert.equal(borrow.generation, bounds.generation);
    assert.equal(
      borrow.withView((view) => view[2]),
      3,
    );
  });
  assert.equal(borrow.valid(), false);
  assert.throws(() => borrow.withView((view) => view[0]), /expired/);
  assert.throws(() => escaped.read("bounds"), /ended/);
  assert.throws(
    () => scope.replace("bounds", new Float64Array([9])),
    /safe point/,
  );
  const next = scope.safePoint(() =>
    scope.replace("bounds", new Float64Array([9])),
  );
  assert(next.generation > bounds.generation);
  assert.equal(scope.valid(bounds), false);
  assert.equal(scope.valid(next), true);
  scope.run("inspect-bounds", (context) =>
    assert.equal(
      context.read("bounds").withView((view) => view[0]),
      9,
    ),
  );
  scope.close();
});

test("structural changes are forbidden inside tasks and operational registration requires a safe point", () => {
  const { scope } = fixture();
  let entered = false;
  scope.run("step", () => {
    assert.throws(
      () =>
        scope.safePoint(() => {
          entered = true;
        }),
      /Finish runtime tasks/,
    );
    assert.throws(() => scope.replace("pose", { x: 9 }), /safe point/);
    assert.throws(() => scope.disposeResource("pose"), /safe point/);
    assert.throws(
      () => scope.define({ id: "late", owner: "tools" }),
      /safe point/,
    );
  });
  assert.equal(entered, false);
  assert.throws(
    () => scope.define({ id: "late", owner: "tools" }),
    /safe point/,
  );
  scope.safePoint(() => {
    scope.define({ id: "late", owner: "tools" });
    assert.equal(
      scope.run("late", () => 0),
      0,
    );
    assert.throws(() => scope.safePoint(() => 0), /Finish runtime tasks/);
  });
  assert.equal(
    scope.run("late", () => 9),
    9,
  );
  scope.close();
});

test("a safe point permits a declared activation task while registry changes remain forbidden during its borrow", () => {
  const { scope, pose } = fixture();
  let borrowed!: ResourceBorrow<{ x: number }>;
  scope.run("step", () => {});
  const activated = scope.safePoint(() => {
    const result = scope.run("step", (context) => {
      borrowed = context.write("pose");
      return borrowed.withView((value) => {
        assert.throws(() => scope.replace("pose", { x: 100 }), /safe point/);
        assert.throws(() => scope.disposeResource("pose"), /safe point/);
        assert.throws(
          () => scope.define({ id: "forbidden", owner: "tools" }),
          /safe point/,
        );
        assert.throws(
          () => scope.resource("pose", { x: 100 }, "simulation"),
          /safe point/,
        );
        assert.throws(() => scope.safePoint(() => 0), /Finish runtime tasks/);
        value.x = 42;
        return value.x;
      });
    });
    assert.equal(borrowed.valid(), false);
    // The borrowed task ended; structural registration can advance its generation now.
    scope.replace("pose", { x: result });
    return result;
  });
  assert.equal(activated, 42);
  assert.equal(scope.valid(pose), false);
  assert.equal(
    scope.run("inspect-pose", (context) =>
      context.read("pose").withView((value) => value.x),
    ),
    42,
  );
  assert.equal(scope.snapshot().activeTasks, 0);
  scope.close();
});

test("disposing and re-registering an ID cannot revive an old handle or stale writer declaration", () => {
  const { scope, pose } = fixture();
  scope.run("step", () => {});
  const replacement = scope.safePoint(() => {
    scope.disposeResource("pose");
    return scope.resource("pose", { x: 10 }, "new-owner");
  });
  assert(replacement.generation > pose.generation);
  assert.equal(scope.valid(pose), false);
  assert.equal(scope.valid(replacement), true);
  assert.throws(() => scope.run("step", () => 0), /another runtime owner/);
  scope.safePoint(() =>
    scope.define({ id: "new-step", owner: "new-owner", writes: ["pose"] }),
  );
  scope.run("new-step", (context) =>
    context.write("pose").withView((value) => {
      value.x++;
    }),
  );
  assert.equal(
    scope.run("inspect-pose", (context) =>
      context.read("pose").withView((value) => value.x),
    ),
    11,
  );
  scope.close();
});

test("cancellation invalidates active borrows immediately and leaves subsequent task lifetimes usable", () => {
  const { scope } = fixture();
  let borrow!: ResourceBorrow<{ x: number }>;
  assert.throws(
    () =>
      scope.run("step", (context) => {
        borrow = context.write("pose");
        assert.equal(scope.cancel("unrelated"), 0);
        assert.equal(scope.cancel("step"), 1);
        assert.equal(scope.cancel("step"), 0);
        assert.equal(borrow.valid(), false);
        assert.throws(() => borrow.withView((value) => value.x), /expired/);
      }),
    /canceled/,
  );
  assert.equal(scope.snapshot().activeTasks, 0);
  scope.run("step", (context) =>
    context.write("pose").withView((value) => {
      value.x = 4;
    }),
  );
  assert.equal(scope.snapshot().cancellations, 1);
  scope.close();
});

test("cancellation inside a borrowed callback fails its final lifetime check", () => {
  const { scope } = fixture();
  assert.throws(
    () =>
      scope.run("step", (context) =>
        context.write("pose").withView(() => {
          scope.cancel();
          return 1;
        }),
      ),
    /borrow expired|task was canceled/,
  );
  assert.equal(scope.snapshot().activeTasks, 0);
  scope.close();
});

test("async task and borrow callbacks are rejected without an async lease", async () => {
  const { scope } = fixture();
  let entered = false;
  assert.throws(
    () =>
      scope.run("step", (async () => {
        entered = true;
      }) as never),
    /synchronous callbacks/,
  );
  assert.equal(entered, false);
  scope.run("step", (context) => {
    const borrow = context.write("pose");
    assert.throws(
      () =>
        borrow.withView((async () => {
          entered = true;
        }) as never),
      /synchronous callbacks/,
    );
  });
  assert.equal(entered, false);
  assert.throws(
    () =>
      scope.run("step", (() =>
        Promise.reject(new Error("accidental async task"))) as never),
    /asynchronous work/,
  );
  assert.throws(
    () =>
      scope.run("step", (context) =>
        context
          .write("pose")
          .withView((() =>
            Promise.reject(new Error("accidental async borrow"))) as never),
      ),
    /asynchronous work/,
  );
  assert.throws(
    () =>
      scope.safePoint((async () => {
        entered = true;
      }) as never),
    /synchronous callbacks/,
  );
  await Promise.resolve();
  assert.equal(entered, false);
  assert.equal(scope.snapshot().activeTasks, 0);
  assert.equal(
    scope.run("step", () => 3),
    3,
  );
  scope.close();
});

test("exceptions release task scopes and structural phases without leaking active work", () => {
  const { scope } = fixture();
  assert.throws(
    () =>
      scope.run("step", () => {
        throw new Error("step failure");
      }),
    /step failure/,
  );
  assert.equal(scope.snapshot().activeTasks, 0);
  assert.throws(
    () =>
      scope.safePoint(() => {
        throw new Error("change failure");
      }),
    /change failure/,
  );
  assert.equal(
    scope.run("step", () => 7),
    7,
  );
  scope.close();
});

test("shutdown invalidates handles and live borrows, closes all resources, and rejects later work", () => {
  const { scope, pose } = fixture();
  let borrowed!: ResourceBorrow<{ x: number }>;
  assert.throws(
    () =>
      scope.run("step", (context) => {
        borrowed = context.write("pose");
        scope.close();
        assert.equal(borrowed.valid(), false);
        assert.throws(() => borrowed.withView((value) => value.x), /expired/);
      }),
    /new ownership scope/,
  );
  assert.equal(scope.valid(pose), false);
  assert.deepEqual(
    {
      resources: scope.snapshot().resources,
      tasks: scope.snapshot().tasks,
      activeTasks: scope.snapshot().activeTasks,
      closed: scope.snapshot().closed,
    },
    { resources: 0, tasks: 0, activeTasks: 0, closed: true },
  );
  assert.throws(() => scope.run("step", () => 0), /new ownership scope/);
  assert.throws(() => scope.safePoint(() => 0), /new ownership scope/);
  assert.throws(
    () => scope.resource("pose", { x: 0 }, "simulation"),
    /new ownership scope/,
  );
  scope.close();
  assert.equal(scope.cancel(), 0);
});
