import { spawnSync } from "node:child_process";

const lanes = [
  ["current-source", "1bbdb32789e1f79932df44941236ea3658f6e965"],
  ["bundled-0.144.1", "44918ea10c0f99151c6710411b4322c2f5c96bea"],
] as const;

for (const [lane, ref] of lanes) {
  const result = spawnSync(process.execPath, [new URL("generate.ts", import.meta.url).pathname], {
    stdio: "inherit",
    env: { ...process.env, CODEX_PROTOCOL_LANE: lane, CODEX_PROTOCOL_REF: ref },
  });
  if (result.status !== 0) process.exit(result.status ?? 1);
  const wire = spawnSync(process.execPath, [new URL("generate-wire.ts", import.meta.url).pathname], {
    stdio: "inherit",
    env: { ...process.env, CODEX_PROTOCOL_LANE: lane },
  });
  if (wire.status !== 0) process.exit(wire.status ?? 1);
}

const fixtures = spawnSync(process.execPath, [new URL("generate-notification-fixtures.ts", import.meta.url).pathname], {
  stdio: "inherit",
  env: process.env,
});
if (fixtures.status !== 0) process.exit(fixtures.status ?? 1);
