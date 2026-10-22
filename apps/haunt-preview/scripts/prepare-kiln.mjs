import { cp, mkdir, readFile, writeFile } from "node:fs/promises";
import { homedir } from "node:os";
import { resolve } from "node:path";

const source = resolve(
  process.env.KILN_OUT || `${homedir()}/work/openagents-target-1/kiln-site`,
);
const target = new URL("../public/kiln/", import.meta.url);
try {
  const glue = await readFile(`${source}/pkg/kiln.js`, "utf8");
  if (!glue.includes("start_headless"))
    throw new Error("Missing Kiln browser exports.");
  await mkdir(target, { recursive: true });
  for (const path of ["pkg", "tools", "worker.js"]) {
    await cp(`${source}/${path}`, new URL(path, target), {
      recursive: true,
      force: true,
    });
  }
  await writeFile(
    new URL("config.json", target),
    JSON.stringify({
      relay: "wss://relay.openagents.com",
      site_identity: false,
    }),
  );
} catch (error) {
  console.error(
    `Build Kiln first: KILN_TRANSCRIPT=0 scripts/build-kiln.sh\n${error}`,
  );
  process.exitCode = 1;
}
