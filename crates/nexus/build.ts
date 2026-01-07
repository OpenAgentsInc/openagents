/**
 * Build script for Nexus HUD
 *
 * Creates the dist/ directory with:
 * - index.html
 * - pkg/ (WASM client)
 * - static/ (fonts from web crate)
 */

import { rmSync, mkdirSync, cpSync, copyFileSync, existsSync } from "fs";
import { join } from "path";

const __dirname = import.meta.dir;
const dist = join(__dirname, "dist");
const pkg = join(__dirname, "pkg");
const webStatic = join(__dirname, "../web/static");

console.log("Building Nexus HUD dist/...");

// Clean dist/
if (existsSync(dist)) {
    rmSync(dist, { recursive: true });
}
mkdirSync(dist, { recursive: true });

// Copy pkg/ (WASM output)
if (existsSync(pkg)) {
    cpSync(pkg, join(dist, "pkg"), { recursive: true });
    console.log("  Copied pkg/");
} else {
    console.error("  WARNING: pkg/ not found - run build:client first");
}

// Copy index.html
const indexHtml = join(__dirname, "index.html");
if (existsSync(indexHtml)) {
    copyFileSync(indexHtml, join(dist, "index.html"));
    console.log("  Copied index.html");
} else {
    console.error("  WARNING: index.html not found");
}

// Copy static assets (fonts) from web crate
if (existsSync(webStatic)) {
    cpSync(webStatic, join(dist, "static"), { recursive: true });
    console.log("  Copied static/ from web crate");
} else {
    console.log("  No static/ found in web crate, skipping");
}

console.log("Build complete: dist/");
