import test from "node:test";
import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import {
  CellStream,
  streamPackageIdentity,
  validateStreamPackage,
  type CellStreamPrepared,
  type StreamPackageManifest,
  type StreamResource,
} from "../packages/runtime/cell-stream.ts";

const hash = (bytes: Uint8Array | string) =>
  createHash("sha256").update(bytes).digest("hex");
function sorted(value: unknown): unknown {
  if (Array.isArray(value)) return value.map(sorted);
  if (value && typeof value === "object")
    return Object.fromEntries(
      Object.entries(value)
        .sort(([a], [b]) => (a < b ? -1 : a > b ? 1 : 0))
        .map(([key, child]) => [key, sorted(child)]),
    );
  return value;
}
function seal(manifest: StreamPackageManifest) {
  const { schema, abi, entryCell, cells, files, totals } = manifest;
  manifest.sha256 = hash(
    JSON.stringify(sorted({ schema, abi, entryCell, cells, files, totals })),
  );
  return manifest;
}
function totals(files: StreamResource[]) {
  const unique = [
    ...new Map(files.map((file) => [file.sha256, file])).values(),
  ];
  const sum = (items: StreamResource[], key: "bytes" | "gzipBytes") =>
    items.reduce((n, item) => n + item[key], 0);
  return {
    rawBytes: sum(files, "bytes"),
    gzipBytes: sum(files, "gzipBytes"),
    uniqueRawBytes: sum(unique, "bytes"),
    uniqueGzipBytes: sum(unique, "gzipBytes"),
  };
}
function fixture() {
  const data = new Map<string, Uint8Array<ArrayBuffer>>([
    ["/starter/entry-day.json", new TextEncoder().encode("entry-day")],
    ["/starter/entry-night.json", new TextEncoder().encode("entry-night")],
    ["/starter/shared.bin", new TextEncoder().encode("mesh")],
    ["/private-assets/town-day.json", new TextEncoder().encode("town-day")],
    ["/private-assets/town-night.json", new TextEncoder().encode("town-night")],
    ["/private-assets/collision.bin", new TextEncoder().encode("collision")],
  ]);
  const files = [...data].map(([url, bytes]): StreamResource => ({
    url,
    sha256: hash(bytes),
    bytes: bytes.byteLength,
    gzipBytes: bytes.byteLength + 20,
  }));
  const find = (url: string) => files.find((file) => file.url === url)!;
  const manifest: StreamPackageManifest = seal({
    schema: "aethyrforge.stream-package.v1",
    abi: 1,
    identityScope: "stream-descriptor-v1",
    sha256: "0".repeat(64),
    entryCell: "entry",
    cells: [
      {
        id: "entry",
        quality: "staged-flat-outside-entry",
        bounds: { min: [-4, -4], max: [4, 4] },
        scenes: { day: files[0].url, night: files[1].url },
        resources: [files[0], files[1], find("/starter/shared.bin")],
      },
      {
        id: "town",
        quality: "source",
        bounds: { min: [-20, -20], max: [20, 20] },
        scenes: { day: files[3].url, night: files[4].url },
        resources: [files[3], files[4], find("/starter/shared.bin"), files[5]],
      },
    ],
    files,
    totals: totals(files),
  });
  const requests: string[] = [];
  const fetcher = (async (input: RequestInfo | URL) => {
    const url = String(input);
    requests.push(url);
    const bytes = data.get(url);
    return bytes
      ? new Response(bytes.slice().buffer)
      : new Response(null, { status: 404 });
  }) as typeof fetch;
  return { manifest, data, requests, fetcher };
}
function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((accept) => {
    resolve = accept;
  });
  return { promise, resolve };
}
async function until(check: () => boolean) {
  for (let n = 0; n < 200; n++) {
    if (check()) return;
    await new Promise((resolve) => setTimeout(resolve, 1));
  }
  assert.fail("The bounded asynchronous test did not reach its checkpoint.");
}
function host() {
  const commits: string[] = [],
    releases: string[] = [];
  return {
    commits,
    releases,
    prepare: ({ cell }: { cell: { id: string } }): CellStreamPrepared => ({
      commit: () => {
        commits.push(cell.id);
      },
      dispose: () => {
        releases.push(cell.id);
      },
      decodeMs: 0.25,
    }),
  };
}

test("cell descriptors validate copied bounds, ABI, scene closure, URL safety, and inventory totals", () => {
  const { manifest } = fixture();
  const valid = validateStreamPackage(manifest);
  assert.deepEqual(valid, manifest);
  valid.cells[0].bounds.min[0] = -100;
  assert.equal(manifest.cells[0].bounds.min[0], -4);
  const changes: ((value: StreamPackageManifest) => void)[] = [
    (m) => {
      (m as any).abi = 2;
    },
    (m) => {
      (m as any).identityScope = "resource-inventory-v1";
    },
    (m) => {
      m.entryCell = "absent";
    },
    (m) => {
      m.cells.push(structuredClone(m.cells[0]));
    },
    (m) => {
      m.files.push(structuredClone(m.files[0]));
    },
    (m) => {
      m.cells[0].resources.push(structuredClone(m.cells[0].resources[0]));
    },
    (m) => {
      m.cells[0].bounds.min[0] = NaN;
    },
    (m) => {
      m.cells[0].bounds.max[0] = m.cells[0].bounds.min[0];
    },
    (m) => {
      m.cells[0].bounds.max[0] = 4.5;
    },
    (m) => {
      m.cells[0].scenes.night = "/starter/not-in-cell.json";
    },
    (m) => {
      m.cells[0].resources[0].bytes++;
    },
    (m) => {
      m.files[0].bytes = 300 * 1024 * 1024;
    },
    (m) => {
      m.files[0].sha256 = "f";
    },
    (m) => {
      m.totals.rawBytes++;
    },
    (m) => {
      m.totals.uniqueGzipBytes++;
    },
  ];
  for (const change of changes) {
    const candidate = structuredClone(manifest);
    change(candidate);
    assert.throws(() => validateStreamPackage(candidate));
  }
  for (const url of [
    "https://other.test/file",
    "//other.test/file",
    "/starter/../secret",
    "/starter/%2e%2e/secret",
    "/starter/./file",
    "/starter//file",
    "/starter/\\file",
    "/starter/file?key=1",
    "/starter/file#fragment",
    "/starter/café.bin",
    "/other/file",
  ]) {
    const candidate = structuredClone(manifest);
    candidate.files[0].url = url;
    assert.throws(() => validateStreamPackage(candidate), url);
  }
  assert.throws(() => validateStreamPackage(manifest, { maxCells: 1 }));
  assert.throws(() => validateStreamPackage(manifest, { maxCells: NaN }));
  assert.throws(() => validateStreamPackage(manifest, { maxResources: -1 }));
  assert.deepEqual(
    validateStreamPackage(manifest, { maxCells: undefined }),
    manifest,
  );
  assert.throws(() => new CellStream({ concurrency: 1000 }));
  assert.throws(() => new CellStream({ maxHeldBytes: -1 }));
});

test("descriptor identity covers bounds, scene mapping, inventory sizes, and every stage", async () => {
  const { manifest, fetcher, requests } = fixture();
  assert.equal(await streamPackageIdentity(manifest), manifest.sha256);
  for (const mutate of [
    (m: StreamPackageManifest) => {
      m.cells[1].bounds.max[0]++;
    },
    (m: StreamPackageManifest) => {
      m.cells[1].scenes.day = m.cells[1].scenes.night;
    },
    (m: StreamPackageManifest) => {
      m.entryCell = "town";
    },
  ]) {
    const stream = new CellStream({ fetch: fetcher }),
      candidate = structuredClone(manifest);
    mutate(candidate);
    stream.configure(candidate);
    const result = await stream.load("entry", host());
    assert.equal(result.ok, false);
    if (!result.ok) assert.match(result.message, /descriptor.*hash/);
    assert.equal(stream.snapshot().activeCell, null);
    stream.dispose();
  }
  assert.equal(
    requests.length,
    0,
    "Descriptor tampering must fail before any asset fetch.",
  );
});

test("cold loads deduplicate shared resources, warm revisits reuse bytes, and inactive unloading releases them", async () => {
  const f = fixture(),
    h = host(),
    stream = new CellStream({ fetch: f.fetcher });
  stream.configure(f.manifest);
  assert.equal(stream.canEnter([0, 0, 0]), false);
  const first = await stream.load("entry", h);
  assert.deepEqual(first, {
    ok: true,
    cellId: "entry",
    generation: 2,
    cache: "cold",
  });
  assert.equal(stream.canEnter([-4, 500, 4]), true);
  assert.equal(stream.canEnter([4.01, 0, 0]), false);
  assert.equal(stream.canEnter([NaN, 0]), false);
  assert.equal(stream.canEnter([0]), false);
  assert.equal(stream.canEnter(null as never), false);
  const second = await stream.load("town", h);
  assert.equal(second.ok, true);
  assert.equal(stream.canEnter([19, -19]), true);
  assert.equal(
    f.requests.filter((url) => url === "/starter/shared.bin").length,
    1,
  );
  assert.equal(stream.snapshot().cacheHits, 1);
  assert.deepEqual(h.releases, ["entry"]);
  assert.equal(stream.unload("town"), 0, "Never drop active cell inputs.");
  const requests = f.requests.length;
  const revisit = await stream.load("entry", h);
  assert.equal(revisit.ok && revisit.cache, "warm");
  assert.equal(f.requests.length, requests);
  assert.equal(stream.snapshot().requestedBytes, f.manifest.totals.rawBytes);
  assert.equal(stream.snapshot().warmLoads, 1);
  assert.equal(stream.snapshot().coldLoads, 2);
  assert.equal(stream.snapshot().decodeMs, 0.75);
  const cell = stream.cell("entry")!;
  cell.bounds.max[0] = 10_000;
  assert.equal(
    stream.canEnter([100, 0, 0]),
    false,
    "Callers cannot mutate active bounds.",
  );
  const held = stream.snapshot().heldBytes;
  assert(stream.unload("town") > 0);
  assert(stream.snapshot().heldBytes < held);
  assert.equal(stream.snapshot().cachedResources, 3);
  assert.equal(stream.unloadInactive(), 0);
  stream.dispose();
  stream.dispose();
  assert.equal(stream.snapshot().heldBytes, 0);
  assert.equal(stream.canEnter([0, 0, 0]), false);
  assert.deepEqual(h.releases, ["entry", "town", "entry"]);
});

test("missing, corrupt, truncated, oversized, unreadable, and redirected resources keep the old cell", async () => {
  for (const failure of [
    "missing",
    "corrupt",
    "short",
    "long",
    "unreadable",
    "redirect",
  ] as const) {
    const f = fixture(),
      h = host();
    let broken = false;
    const stream = new CellStream({
      fetch: (async (input, init) => {
        if (!broken || String(input) !== "/private-assets/town-day.json")
          return f.fetcher(input, init);
        if (failure === "missing") return new Response(null, { status: 404 });
        if (failure === "unreadable") return new Response(null);
        if (failure === "redirect")
          return new Proxy(new Response("town-day"), {
            get(target, property) {
              return property === "redirected"
                ? true
                : Reflect.get(target, property, target);
            },
          });
        return new Response(
          failure === "corrupt"
            ? "TOWN-DAY"
            : failure === "short"
              ? "x"
              : "town-day-too-large",
        );
      }) as typeof fetch,
    });
    stream.configure(f.manifest);
    assert.equal((await stream.load("entry", h)).ok, true);
    const active = stream.snapshot();
    broken = true;
    const result = await stream.load("town", h);
    assert.equal(result.ok, false, failure);
    if (!result.ok) assert.equal(result.reason, "failed", failure);
    assert.equal(stream.snapshot().activeCell, "entry", failure);
    assert.deepEqual(stream.snapshot().bounds, active.bounds);
    assert.equal(stream.snapshot().inFlightBytes, 0);
    assert.deepEqual(h.commits, ["entry"]);
    assert.equal(stream.canEnter([10, 0, 0]), false);
    stream.dispose();
  }
});

test("latest request wins even when an older asynchronous preparation ignores cancellation", async () => {
  const f = fixture(),
    h = host(),
    stream = new CellStream({ fetch: f.fetcher });
  stream.configure(f.manifest);
  await stream.load("entry", h);
  const stage = deferred<CellStreamPrepared>();
  let started = false,
    discarded = 0,
    badCommits = 0;
  const old = stream.load("town", {
    prepare: () => {
      started = true;
      return stage.promise;
    },
  });
  await until(() => started);
  assert.equal(
    stream.canEnter([10, 0, 0]),
    false,
    "Fetching/preparing is not movement admission.",
  );
  const latest = await stream.load("entry", h);
  assert.equal(latest.ok, true);
  stage.resolve({
    commit: () => {
      badCommits++;
    },
    dispose: () => {
      discarded++;
    },
  });
  const result = await old;
  assert.equal(result.ok, false);
  if (!result.ok) assert.equal(result.reason, "stale");
  assert.equal(badCommits, 0);
  assert.equal(discarded, 1);
  assert.equal(stream.snapshot().activeCell, "entry");
  assert.equal(stream.snapshot().pendingLoads, 0);
  stream.dispose();
});

test("abort, cancel, and disposal discard prepared candidates and release their reservations", async () => {
  for (const mode of ["signal", "cancel", "dispose"] as const) {
    const f = fixture(),
      h = host(),
      stream = new CellStream({ fetch: f.fetcher });
    stream.configure(f.manifest);
    await stream.load("entry", h);
    const stage = deferred<CellStreamPrepared>(),
      controller = new AbortController();
    let started = false,
      discarded = 0,
      committed = 0;
    const load = stream.load("town", {
      signal: controller.signal,
      prepare: () => {
        started = true;
        return stage.promise;
      },
    });
    await until(() => started);
    if (mode === "signal") controller.abort();
    else stream[mode]();
    stage.resolve({
      commit: () => {
        committed++;
      },
      dispose: () => {
        discarded++;
      },
    });
    const result = await load;
    assert.equal(result.ok, false);
    if (!result.ok) assert.equal(result.reason, "canceled");
    assert.equal(committed, 0);
    assert.equal(discarded, 1);
    assert.equal(
      stream.snapshot().activeCell,
      mode === "dispose" ? null : "entry",
    );
    assert.equal(stream.snapshot().inFlightBytes, 0);
    stream.dispose();
    assert.equal(stream.snapshot().heldBytes, 0);
    await assert.rejects(stream.load("entry", h), /new stream/);
  }
});

test("cancellation during fetch does not adopt late bytes or stop the active scene", async () => {
  const f = fixture(),
    h = host(),
    responses: ReturnType<typeof deferred<Response>>[] = [];
  let holding = false;
  const stream = new CellStream({
    fetch: (async (input, init) => {
      if (!holding || String(input) === "/starter/shared.bin")
        return f.fetcher(input, init);
      const response = deferred<Response>();
      responses.push(response);
      return response.promise;
    }) as typeof fetch,
  });
  stream.configure(f.manifest);
  await stream.load("entry", h);
  holding = true;
  const load = stream.load("town", h);
  await until(() => responses.length === 3);
  stream.cancel();
  for (const response of responses) response.resolve(new Response("late"));
  const result = await load;
  assert.equal(result.ok, false);
  if (!result.ok) assert.equal(result.reason, "canceled");
  assert.equal(stream.snapshot().activeCell, "entry");
  assert.equal(
    stream.snapshot().cacheBytes,
    f.manifest.cells[0].resources.reduce((n, file) => n + file.bytes, 0),
  );
  assert.equal(stream.snapshot().inFlightBytes, 0);
  stream.dispose();
});

test("preparation or synchronous activation failures retain collision admission and dispose the candidate", async () => {
  for (const mode of ["prepare", "commit"] as const) {
    const f = fixture(),
      h = host(),
      stream = new CellStream({ fetch: f.fetcher });
    stream.configure(f.manifest);
    await stream.load("entry", h);
    let released = 0;
    const result = await stream.load("town", {
      prepare: () => {
        if (mode === "prepare") throw new Error("Decode failed");
        return {
          commit: () => {
            throw new Error("Atomic host refused candidate");
          },
          dispose: () => {
            released++;
          },
        };
      },
    });
    assert.equal(result.ok, false);
    assert.equal(stream.snapshot().activeCell, "entry");
    assert.equal(stream.canEnter([10, 0, 0]), false);
    assert.equal(released, mode === "commit" ? 1 : 0);
    assert.deepEqual(h.releases, []);
    stream.unloadInactive();
    stream.dispose();
  }
});

test("a changed hash at the same URL fetches new bytes; the working old generation remains until commit", async () => {
  const f = fixture(),
    h = host(),
    stream = new CellStream({ fetch: f.fetcher });
  stream.configure(f.manifest);
  await stream.load("entry", h);
  const next = structuredClone(f.manifest),
    url = "/starter/shared.bin";
  const bytes = new TextEncoder().encode("new mesh");
  f.data.set(url, bytes);
  for (const file of [
    ...next.files,
    ...next.cells.flatMap((cell) => cell.resources),
  ])
    if (file.url === url) {
      file.bytes = bytes.byteLength;
      file.gzipBytes = file.bytes + 20;
      file.sha256 = hash(bytes);
    }
  next.totals = totals(next.files);
  seal(next);
  stream.configure(next);
  assert.equal(stream.snapshot().activeCell, "entry");
  let observed = "";
  const result = await stream.load("entry", {
    prepare: (input) => {
      observed = new TextDecoder().decode(input.resources.get(url));
      return h.prepare(input);
    },
  });
  assert.equal(result.ok, true);
  assert.equal(observed, "new mesh");
  assert.equal(f.requests.filter((item) => item === url).length, 2);
  assert(
    stream.unloadInactive() > 0,
    "Old content at a reused URL can be released after activation.",
  );
  stream.dispose();
});

test("cache integrity checks catch accidental host mutation instead of activating corrupted content", async () => {
  const f = fixture(),
    h = host(),
    stream = new CellStream({ fetch: f.fetcher });
  stream.configure(f.manifest);
  let borrowed: Uint8Array | undefined;
  await stream.load("entry", {
    prepare: (input) => {
      borrowed = input.resources.get("/starter/shared.bin");
      return h.prepare(input);
    },
  });
  borrowed![0] ^= 1;
  const result = await stream.load("town", h);
  assert.equal(result.ok, false);
  if (!result.ok) assert.match(result.message, /cached resource/);
  assert.equal(stream.snapshot().activeCell, "entry");
  stream.dispose();
});

test("memory reservations include both active and candidate work, and fetch concurrency stays bounded", async () => {
  const f = fixture(),
    h = host();
  const entryBytes = f.manifest.cells[0].resources.reduce(
    (n, file) => n + file.bytes,
    0,
  );
  const stream = new CellStream({
    fetch: f.fetcher,
    maxHeldBytes: entryBytes + 1,
  });
  stream.configure(f.manifest);
  assert.equal((await stream.load("entry", h)).ok, true);
  assert.equal((await stream.load("town", h)).ok, false);
  assert.equal(stream.snapshot().activeCell, "entry");
  assert(stream.snapshot().highWaterBytes <= entryBytes + 1);
  assert.equal(stream.snapshot().inFlightBytes, 0);
  stream.dispose();
  let active = 0,
    maximum = 0;
  const limited = new CellStream({
    concurrency: 2,
    fetch: (async (input, init) => {
      active++;
      maximum = Math.max(maximum, active);
      try {
        await new Promise((resolve) => setTimeout(resolve, 4));
        return await f.fetcher(input, init);
      } finally {
        active--;
      }
    }) as typeof fetch,
  });
  limited.configure(f.manifest);
  assert.equal((await limited.load("town", host())).ok, true);
  assert.equal(maximum, 2);
  assert.equal(active, 0);
  limited.dispose();
});

test("invalid reconfiguration leaves the working descriptor, generation, cache, and bounds alone", async () => {
  const f = fixture(),
    h = host(),
    stream = new CellStream({ fetch: f.fetcher });
  stream.configure(f.manifest);
  await stream.load("entry", h);
  const before = stream.snapshot(),
    invalid = structuredClone(f.manifest);
  invalid.cells[0].bounds.max[0] = Infinity;
  assert.throws(() => stream.configure(invalid));
  assert.deepEqual(stream.snapshot(), before);
  f.manifest.cells[0].bounds.max[0] = 1_000_000;
  assert.equal(stream.canEnter([100, 0, 0]), false);
  stream.dispose();
});
