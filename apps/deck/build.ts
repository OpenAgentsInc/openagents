import { createHash } from "node:crypto";
import { cp, mkdir, readFile, readdir, rm, writeFile } from "node:fs/promises";
import { join } from "node:path";

const DIST_DIR = "./dist";
const PKG_DIR = "./pkg";
const STATIC_DIR = "./static";

async function clean() {
  await rm(DIST_DIR, { recursive: true, force: true });
  await mkdir(DIST_DIR, { recursive: true });
}

async function copyPkg() {
  await cp(PKG_DIR, join(DIST_DIR, "pkg"), { recursive: true });
}

async function copyStatic() {
  const entries = await readdir(STATIC_DIR, { withFileTypes: true });
  for (const entry of entries) {
    if (entry.name === "pkg") {
      continue;
    }

    const source = join(STATIC_DIR, entry.name);
    const destination = join(DIST_DIR, entry.name);

    if (entry.isDirectory()) {
      await cp(source, destination, { recursive: true });
    } else {
      await cp(source, destination);
    }
  }
}

function digestHex(input: Uint8Array | string) {
  return createHash("sha256").update(input).digest("hex").slice(0, 12);
}

async function stampVersionedAssetUrls() {
  const jsPath = join(DIST_DIR, "pkg", "deck.js");
  const wasmPath = join(DIST_DIR, "pkg", "deck_bg.wasm");
  const indexPath = join(DIST_DIR, "index.html");

  const [jsSource, wasmSource, indexSource] = await Promise.all([
    readFile(jsPath, "utf8"),
    readFile(wasmPath),
    readFile(indexPath, "utf8"),
  ]);

  const wasmVersion = digestHex(wasmSource);
  const jsVersion = digestHex(`${jsSource}\n${wasmVersion}`);

  const stampedJs = jsSource
    .replaceAll("'deck_bg.wasm'", `'deck_bg.wasm?v=${wasmVersion}'`)
    .replaceAll('"deck_bg.wasm"', `"deck_bg.wasm?v=${wasmVersion}"`);
  const stampedIndex = indexSource.replace(
    './pkg/deck.js',
    `./pkg/deck.js?v=${jsVersion}`,
  );

  await Promise.all([
    writeFile(jsPath, stampedJs),
    writeFile(indexPath, stampedIndex),
  ]);
}

async function main() {
  console.log("Building apps/deck dist/ for Cloudflare Workers...");
  await clean();
  await copyPkg();
  await copyStatic();
  await stampVersionedAssetUrls();

  console.log("Built dist/ with:");
  const entries = await readdir(DIST_DIR, { recursive: true });
  for (const entry of entries) {
    console.log(`  ${entry}`);
  }
}

main().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
