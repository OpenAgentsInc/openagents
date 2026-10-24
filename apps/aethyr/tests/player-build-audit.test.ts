import test from "node:test";
import assert from "node:assert/strict";
import { brotliCompressSync, constants, gzipSync } from "node:zlib";
import {
  auditPlayerBuild,
  type PlayerBuildAuditOptions,
  type PlayerBuildRecord,
} from "../tools/audit-player-build.ts";

const bytes = (text: string) => new TextEncoder().encode(text);
function fixture(optional = true): PlayerBuildAuditOptions {
  const records: PlayerBuildRecord[] = [
    {
      fileName: "player.html",
      type: "asset",
      referencedFiles: ["assets/player.js", "artifact.json"],
    },
    {
      fileName: "assets/player.js",
      type: "chunk",
      isEntry: true,
      modules: ["/repo/apps/player/main.ts"],
      imports: ["assets/runtime.js"],
      referencedFiles: ["assets/worker.js", "assets/player.css"],
      dynamicImports: optional ? ["assets/sky.js"] : [],
    },
    {
      fileName: "assets/runtime.js",
      type: "chunk",
      modules: ["/repo/packages/runtime/world.ts"],
      referencedFiles: ["kernel.wasm"],
    },
    {
      fileName: "assets/worker.js",
      type: "asset",
      modules: ["/repo/apps/player/worker.ts"],
      imports: ["./runtime.js"],
    },
    { fileName: "assets/player.css", type: "asset" },
    { fileName: "kernel.wasm", type: "asset" },
    {
      fileName: "artifact.json",
      type: "asset",
      referencedFiles: [
        "starter/world.json",
        "starter/colliders.json",
        "starter/mesh.bin",
      ],
    },
    { fileName: "starter/world.json", type: "asset" },
    { fileName: "starter/colliders.json", type: "asset" },
    { fileName: "starter/mesh.bin", type: "asset" },
    ...(optional
      ? [
          {
            fileName: "assets/sky.js",
            type: "chunk" as const,
            modules: ["/repo/packages/renderer/sky.ts"],
            referencedFiles: ["starter/sky.bin"],
          },
          { fileName: "starter/sky.bin", type: "asset" as const },
        ]
      : []),
  ];
  return {
    roots: ["player.html"],
    records,
    files: new Map(
      records.map((record) => [
        record.fileName,
        bytes(`owned content ${record.fileName} `.repeat(4)),
      ]),
    ),
  };
}

test("emitted player graph follows static, dynamic, HTML, worker, and cooked resource dependencies", () => {
  const options = fixture(),
    report = auditPlayerBuild(options);
  assert.equal(report.ok, true, report.errors.join("\n"));
  assert.equal(report.requests, options.files.size);
  assert.ok(report.reachable.includes("assets/worker.js"));
  assert.ok(report.reachable.includes("assets/sky.js"));
  assert.ok(report.reachable.includes("starter/colliders.json"));
  assert.ok(report.modules.includes("/repo/apps/player/worker.ts"));
  assert.deepEqual(report.unreachable, []);
  assert.deepEqual(report.delivered, report.totals.all);
  assert.equal(report.totals.js.files, 4);
  assert.equal(report.totals.wasm.files, 1);
  assert.equal(report.totals.css.files, 1);
  const measure = (file: Uint8Array) => ({
    raw: file.byteLength,
    gzip: gzipSync(file, { level: 9 }).byteLength,
    br: brotliCompressSync(file, {
      params: { [constants.BROTLI_PARAM_QUALITY]: 6 },
    }).byteLength,
  });
  const expected = [...options.files.values()].map(measure);
  assert.equal(
    report.totals.all.rawBytes,
    expected.reduce((sum, item) => sum + item.raw, 0),
  );
  assert.equal(
    report.totals.all.gzipBytes,
    expected.reduce((sum, item) => sum + item.gzip, 0),
  );
  assert.equal(
    report.totals.all.brotliBytes,
    expected.reduce((sum, item) => sum + item.br, 0),
  );
});

test("removing a feature removes both its code and resources from the actual delivery", () => {
  const full = auditPlayerBuild(fixture()),
    small = auditPlayerBuild(fixture(false));
  assert.equal(full.ok && small.ok, true);
  assert.ok(!small.reachable.includes("assets/sky.js"));
  assert.ok(!small.reachable.includes("starter/sky.bin"));
  assert.ok(!small.modules.some((name) => name.endsWith("/sky.ts")));
  assert.equal(full.requests - small.requests, 2);
  assert.ok(small.totals.js.rawBytes < full.totals.js.rawBytes);
  assert.ok(small.totals.assets.rawBytes < full.totals.assets.rawBytes);
  const leftover = fixture(false);
  (leftover.files as Map<string, Uint8Array>).set(
    "assets/sky.js",
    bytes("unused feature code"),
  );
  const bad = auditPlayerBuild(leftover);
  assert.equal(bad.ok, false);
  assert.ok(
    bad.errors.some((message) => message.includes("Unreachable files")),
  );
  assert.ok(bad.delivered.rawBytes > bad.totals.all.rawBytes);
});

test("forbidden editor, React, Atom, agent, and importer modules fail even through lazy workers", () => {
  for (const module of [
    "/repo/node_modules/react/jsx-runtime.js",
    "/repo/node_modules/react-dom/client.js",
    "/repo/node_modules/.pnpm/react@19/node_modules/react/index.js",
    "/repo/node_modules/@effect/atom-react/dist/index.js",
    "/repo/apps/editor/main.tsx",
    "/repo/packages/editor-services/service.ts",
    "/repo/packages/renderer/mesh-inspection.ts",
    "/repo/packages/renderer/mesh-view.ts",
    "/repo/packages/renderer/material-editor.ts",
    "/repo/packages/runtime/agent-contract.ts",
    "/repo/packages/runtime/agent-oracle.ts?worker",
    "/repo/tools/assets/import.ts",
    "C:\\repo\\packages\\editor-services\\state.ts",
  ]) {
    const options = fixture();
    options.records.find(
      (record) => record.fileName === "assets/worker.js",
    )!.modules = [module];
    const report = auditPlayerBuild(options);
    assert.equal(report.ok, false, module);
    assert.ok(
      report.errors.some((message) =>
        message.includes("Forbidden player source"),
      ),
      module,
    );
  }
  const options = fixture();
  options.forbiddenModules = [/renderer\/sky\.ts/g];
  assert.equal(auditPlayerBuild(options).ok, false);
});

test("missing output bytes, external edges, and absent worker provenance cannot pass", () => {
  const cases: ((options: PlayerBuildAuditOptions) => void)[] = [
    (o) => {
      (o.files as Map<string, Uint8Array>).delete("kernel.wasm");
    },
    (o) => {
      o.records.find((r) => r.fileName === "assets/worker.js")!.modules =
        undefined;
    },
    (o) => {
      o.records.find((r) => r.fileName === "assets/player.js")!.dynamicImports =
        ["https://cdn.test/editor.js"];
    },
    (o) => {
      o.records.find((r) => r.fileName === "assets/player.js")!.imports = [
        "node:fs",
      ];
    },
    (o) => {
      o.records.find((r) => r.fileName === "assets/player.js")!.imports = [
        "../../../outside.js",
      ];
    },
    (o) => {
      o.records = o.records.filter((r) => r.fileName !== "starter/mesh.bin");
    },
    (o) => {
      o.records = [...o.records, o.records[0]];
    },
    (o) => {
      o.roots = ["missing.html"];
    },
    (o) => {
      o.roots = ["../outside.html"];
    },
    (o) => {
      o.roots = [];
    },
  ];
  for (const change of cases) {
    const options = fixture();
    change(options);
    assert.equal(auditPlayerBuild(options).ok, false);
  }
});

test("served source maps and copied but unreachable editor output are detected independently of entrypoint size", () => {
  const options = fixture();
  (options.files as Map<string, Uint8Array>).set(
    "assets/editor.js",
    bytes("copied editor"),
  );
  (options.files as Map<string, Uint8Array>).set(
    "assets/player.js.map",
    bytes("source map"),
  );
  const report = auditPlayerBuild(options);
  assert.equal(report.ok, false);
  assert.ok(report.errors.some((message) => message.includes("Source maps")));
  assert.deepEqual(report.unreachable, [
    "assets/editor.js",
    "assets/player.js.map",
  ]);
  assert.ok(report.delivered.rawBytes > report.totals.all.rawBytes);
});

test("normalized local root references are accepted while ambiguous and encoded filenames are rejected", () => {
  const options = fixture();
  options.records.find((r) => r.fileName === "assets/player.js")!.imports = [
    "/assets/runtime.js",
  ];
  assert.equal(auditPlayerBuild(options).ok, true);
  for (const reference of [
    "//cdn.test/runtime.js",
    "/assets/%72untime.js",
    "/assets/runtime.js?token=value",
    "/assets/runtime.js#code",
  ]) {
    options.records.find((r) => r.fileName === "assets/player.js")!.imports = [
      reference,
    ];
    assert.equal(auditPlayerBuild(options).ok, false, reference);
  }
});
