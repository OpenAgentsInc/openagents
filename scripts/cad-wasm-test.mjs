// Runs the browser build of the CAD engine (crates/cad-wasm) under Node and
// checks its tools answer as the Lab's do (`scripts/cad-wasm-test.sh`).
//
// Usage: node scripts/cad-wasm-test.mjs OUT/cad_wasm.js
import { createRequire } from "node:module";
import { statSync } from "node:fs";
import { argv, exit } from "node:process";

const path = argv[2];
const cad = createRequire(import.meta.url)(path);
const fail = (why) => {
  console.error(`cad-wasm-test: ${why}`);
  exit(1);
};

// A plate with rounded corners named by their sketch sides, and a hole.
const plate = {
  kind: "part.v1", id: "plate", material: "Aluminum", features: [
    { id: "outline", op: "sketch", params: { plane: "XY", draw: [
      { op: "sketch.rectangle.center", params: { center: [0, 0], corner: [40, 25] } }] } },
    { id: "body", op: "solid.extrude", params: { sketch: "outline", distance: 6 } },
    { id: "corners", op: "solid.fillet", params: { radius: 5, edges: [
      { between: ["body:side:l1", "body:side:l2"] }, { between: ["body:side:l2", "body:side:l3"] },
      { between: ["body:side:l3", "body:side:l4"] }, { between: ["body:side:l4", "body:side:l1"] }] } },
    { id: "hole", op: "solid.cylinder", params: { diameter: 8, height: 10, base: [0, 0, -2], operation: "cut" } },
  ],
};

const call = (name, args) => JSON.parse(cad.run(name, JSON.stringify(args)));
const started = Date.now();
const checked = call("cad_check", { document: plate });
if (!checked.passed) fail(`cad_check: ${JSON.stringify(checked)}`);
const volume = checked.report.mass.volume_cm3 * 1000;
const expected = 80 * 50 * 6 - 4 * 25 * (1 - Math.PI / 4) * 6 - Math.PI * 16 * 6;
if (Math.abs(volume - expected) / expected > 0.005) fail(`volume ${volume} vs ${expected}`);
console.log(`cad_check: ${volume.toFixed(1)} mm³ (analytic ${expected.toFixed(1)}), ` +
  `${checked.report.mass.mass_kg} kg, ${Date.now() - started} ms`);

const faces = call("cad_faces", { document: plate, part: "plate", after: "body" });
if (!JSON.stringify(faces).includes("body:top")) fail(`cad_faces: ${JSON.stringify(faces)}`);
console.log(`cad_faces: ${faces.solids[0].faces.length} faces, ${faces.solids[0].edges.length} edges`);

const solid = call("cad_solid", { ops: [
  { op: "solid.box", params: { length: 20, width: 20, height: 20, center: [0, 0, 0] } },
  { op: "solid.cylinder", repeat: { var: "i", from: 0, to: 2 },
    params: { radius: 2, height: 30, base: ["-6 + {i} * 6", 0, -15], operation: "cut" } }] });
if (!solid.mass) fail(`cad_solid: ${JSON.stringify(solid)}`);
console.log(`cad_solid: ${solid.mass.volume_cm3} cm³`);

const dxf = "0\nSECTION\n2\nENTITIES\n0\nLINE\n8\n0\n10\n0\n20\n0\n11\n30\n21\n0\n" +
  "0\nLINE\n8\n0\n10\n30\n20\n0\n11\n30\n21\n20\n0\nLINE\n8\n0\n10\n30\n20\n20\n11\n0\n21\n20\n" +
  "0\nLINE\n8\n0\n10\n0\n20\n20\n11\n0\n21\n0\n0\nENDSEC\n0\nEOF\n";
const drawn = call("cad_import_dxf", { dxf });
if (drawn.draw?.[0]?.op !== "sketch.polyline") fail(`cad_import_dxf: ${JSON.stringify(drawn)}`);
console.log(`cad_import_dxf: ${drawn.draw.length} step, closed ${drawn.draw[0].params.closed}`);

const drawn = call("cad_drawing", { document: plate, sections: ["front"] });
if (!drawn.svg?.startsWith("<svg")) fail(`cad_drawing: ${JSON.stringify(drawn).slice(0, 400)}`);
const front = drawn.drawing.views.find((v) => v.view === "front");
if (front.visible < 4 || front.hidden !== 2) fail(`cad_drawing front: ${JSON.stringify(front)}`);
const pdf = cad.drawing_file(JSON.stringify({ document: plate }), "pdf");
if (String.fromCharCode(...pdf.slice(0, 5)) !== "%PDF-") fail("drawing pdf");
console.log(`cad_drawing: ${drawn.drawing.views.length} views, front ${front.visible} visible ` +
  `and ${front.hidden} hidden lines, scale ${drawn.drawing.scale}; PDF ${pdf.length} bytes`);

for (const format of ["glb", "step", "3mf", "stl"]) {
  const bytes = cad.export_file(JSON.stringify(plate), format);
  if (!(bytes?.length > 84)) fail(`export ${format}`);
  console.log(`export ${format}: ${bytes.length} bytes`);
}
console.log(`wasm: ${(statSync(path.replace(/\.js$/, "_bg.wasm")).size / 1e6).toFixed(1)} MB; tools ${cad.tools()}`);
