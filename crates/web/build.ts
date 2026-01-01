// Build script for Cloudflare Workers deployment
// Copies and prepares files for the dist/ directory

import { mkdir, rm, copyFile, readdir } from "node:fs/promises";
import { join } from "node:path";

const DIST_DIR = "./dist";
const PKG_DIR = "./pkg";

async function clean() {
  try {
    await rm(DIST_DIR, { recursive: true });
  } catch {
    // Directory doesn't exist, that's fine
  }
  await mkdir(DIST_DIR, { recursive: true });
  await mkdir(join(DIST_DIR, "pkg"), { recursive: true });
}

async function copyDir(source: string, dest: string) {
  await mkdir(dest, { recursive: true });
  const entries = await readdir(source, { withFileTypes: true });
  for (const entry of entries) {
    const sourcePath = join(source, entry.name);
    const destPath = join(dest, entry.name);
    if (entry.isDirectory()) {
      await copyDir(sourcePath, destPath);
    } else if (entry.isFile()) {
      await copyFile(sourcePath, destPath);
      console.log(`  Copied: ${destPath.replace(`${DIST_DIR}/`, "")}`);
    }
  }
}

async function copyPkg() {
  await copyDir(PKG_DIR, join(DIST_DIR, "pkg"));
}

async function copyStatic() {
  await copyFile("./index.html", join(DIST_DIR, "index.html"));
  console.log("  Copied: index.html");
  try {
    await copyDir("./static", join(DIST_DIR, "static"));
  } catch {
    // No static assets to copy.
  }
}

async function build() {
  console.log("Building for Cloudflare Workers...\n");

  console.log("1. Cleaning dist/");
  await clean();

  console.log("2. Copying pkg/ files");
  await copyPkg();

  console.log("3. Copying static files");
  await copyStatic();

  console.log("\nBuild complete! Files in dist/:");
  const distFiles = await readdir(DIST_DIR, { recursive: true });
  for (const file of distFiles) {
    console.log(`  ${file}`);
  }
}

build().catch(console.error);
