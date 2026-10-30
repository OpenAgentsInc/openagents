import { test, expect } from "bun:test";
import { createHash } from "node:crypto";
import { readFileSync } from "node:fs";
const root = new URL("../", import.meta.url);
test("Mammoth's native serving inventory is exactly the pinned original manifest", () => {
  const raw = readFileSync(new URL("fixtures/models/mammoth/manifest.json", root));
  const sha = createHash("sha256").update(raw).digest("hex");
  expect(sha).toBe("4570fecb199493c255d7cad9474941ec43609969b1697a5682be7d9ed4cb9019");
  const m = JSON.parse(raw.toString());
  const rows = m.tensors.map((t: {name:string;shape:number[];shard:number}) =>
    `{${JSON.stringify(t.name)},${t.shape.length},{${[...t.shape,...Array(5-t.shape.length).fill(0)].join(",")}},${t.shard}},`);
  const header = readFileSync(new URL("fixtures/models/mammoth/inventory.h", root), "utf8");
  const actual = header.split("\n").map(s => s.trim()).filter(s => s.startsWith('{"'));
  expect(actual.length).toBe(1184);
  for (let i=0; i<actual.length; i++) expect(actual[i]).toBe(rows[i]);
  expect(rows.length).toBe(1184);
});
