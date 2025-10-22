#!/usr/bin/env node
/*
Copies repo sources to the clipboard, including each file's repo‑relative path.

Included:
  - Everything under docs/ (recursive)
  - All crates/* source files (.rs, Cargo.toml, build.rs, README.md); excludes target/
  - Expo app code and configs (app, components, constants, hooks, lib, providers, types, root config files)

Excluded anywhere:
  - node_modules, .git, .expo, dist, build, target, assets, app-example
  - Common binary assets by extension (png,jpg,jpeg,gif,svg,webp,ttf,otf,woff,woff2,mp4,mp3)

Usage:
  node scripts/copy-sources-to-clipboard.js
*/

const fs = require('fs');
const path = require('path');
const { spawnSync } = require('child_process');

const repoRoot = path.resolve(__dirname, '..');
const expoRoot = path.join(repoRoot, 'expo');
const cratesRoot = path.join(repoRoot, 'crates');
const docsRoot = path.join(repoRoot, 'docs');

const DIR_EXCLUDES = new Set(['node_modules', '.git', '.expo', 'dist', 'build', 'target', 'assets', 'app-example']);
const BINARY_EXTS = new Set(['.png', '.jpg', '.jpeg', '.gif', '.svg', '.webp', '.ttf', '.otf', '.woff', '.woff2', '.mp4', '.mp3']);

function shouldSkipDir(name) { return DIR_EXCLUDES.has(name); }
function isBinaryFile(p) { return BINARY_EXTS.has(path.extname(p).toLowerCase()); }

function walkFiltered(dir, out) {
  let entries;
  try { entries = fs.readdirSync(dir, { withFileTypes: true }); } catch { return; }
  for (const entry of entries) {
    const full = path.join(dir, entry.name);
    if (entry.isDirectory()) {
      if (shouldSkipDir(entry.name)) continue;
      walkFiltered(full, out);
    } else if (entry.isFile()) {
      if (isBinaryFile(full)) continue;
      out.push(full);
    }
  }
}

// Gather expo sources (selected folders + root configs)
function gatherExpo(files) {
  const folders = ['app', 'components', 'constants', 'hooks', 'lib', 'providers', 'types'];
  for (const f of folders) walkFiltered(path.join(expoRoot, f), files);
  const rootConfigs = ['package.json', 'tsconfig.app.json', 'tsconfig.json', 'eslint.config.js', 'app.json', 'eas.json', 'expo-env.d.ts', 'README.md'];
  for (const f of rootConfigs) { const p = path.join(expoRoot, f); if (fs.existsSync(p)) files.push(p); }
}

// Gather all crates/* sources (.rs, Cargo.toml, build.rs, README)
function gatherCrates(files) {
  let crates = [];
  try { crates = fs.readdirSync(cratesRoot, { withFileTypes: true }).filter(d => d.isDirectory()).map(d => d.name); } catch {}
  for (const c of crates) {
    const crateDir = path.join(cratesRoot, c);
    walkFiltered(crateDir, files);
    const cargo = path.join(crateDir, 'Cargo.toml');
    if (fs.existsSync(cargo)) files.push(cargo);
    const buildRs = path.join(crateDir, 'build.rs');
    if (fs.existsSync(buildRs)) files.push(buildRs);
    const readme = path.join(crateDir, 'README.md');
    if (fs.existsSync(readme)) files.push(readme);
  }
  // Include workspace Cargo files at root
  ['Cargo.toml', 'Cargo.lock'].forEach(f => { const p = path.join(repoRoot, f); if (fs.existsSync(p)) files.push(p); });
}

function gatherDocs(files) { walkFiltered(docsRoot, files); }

const files = [];
gatherDocs(files);
gatherCrates(files);
gatherExpo(files);

files.sort((a, b) => a.localeCompare(b));

const chunks = [];
for (const file of files) {
  const repoRel = path.relative(repoRoot, file).replace(/\\/g, '/');
  let content;
  try { content = fs.readFileSync(file, 'utf8'); }
  catch (e) { const buf = fs.readFileSync(file); content = `<<<BINARY ${buf.length} bytes (base64)>>>\n${buf.toString('base64')}`; }
  chunks.push(`===== FILE: ${repoRel} =====\n${content}`);
}

const output = chunks.join('\n\n');

function which(cmd) { const res = spawnSync(process.platform === 'win32' ? 'where' : 'which', [cmd], { stdio: 'pipe' }); return res.status === 0; }
function copyToClipboard(text) {
  if (process.platform === 'darwin' && which('pbcopy')) return spawnSync('pbcopy', { input: text, encoding: 'utf8' }).status === 0;
  if (process.platform === 'win32' && which('clip')) return spawnSync('clip', { input: text, encoding: 'utf8', shell: true }).status === 0;
  if (which('wl-copy')) return spawnSync('wl-copy', { input: text, encoding: 'utf8' }).status === 0;
  if (which('xclip')) return spawnSync('xclip', ['-selection', 'clipboard'], { input: text, encoding: 'utf8' }).status === 0;
  if (which('xsel')) return spawnSync('xsel', ['--clipboard', '--input'], { input: text, encoding: 'utf8' }).status === 0;
  return false;
}

const ok = copyToClipboard(output);
if (ok) {
  const bytes = Buffer.byteLength(output, 'utf8');
  console.log(`Copied ${files.length} files to clipboard (${bytes.toLocaleString()} bytes).`);
} else {
  console.log('No clipboard tool found. Printing to stdout so you can copy manually.');
  console.log(output);
}

