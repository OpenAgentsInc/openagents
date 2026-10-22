import { cp, mkdir, access, rm } from "node:fs/promises";
import { homedir } from "node:os";
import { resolve } from "node:path";
const source = resolve(
  process.env.HAUNT_MEDIEVAL_OUT ||
    `${homedir()}/.openagents/verse/private/medieval-town/haunt-webgpu`,
);
const target = new URL("../public/medieval-kit/", import.meta.url);
try {
  await access(`${source}/catalog.json`);
} catch {
  await rm(target, { recursive: true, force: true });
  console.warn(
    "Medieval kit unavailable. Run npm run prepare:medieval on a machine with the licensed Everglade kit.",
  );
  process.exit(0);
}
await mkdir(target, { recursive: true });
for (const name of ["catalog.json", "meshes.bin", "atlas.png"])
  await cp(`${source}/${name}`, new URL(name, target));
