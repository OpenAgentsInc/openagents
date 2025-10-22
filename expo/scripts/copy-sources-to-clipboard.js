#!/usr/bin/env node
/*
Copies all files (recursively) from these Expo folders to the clipboard,
including each file's repo-relative path:
- expo/app
- expo/components
- expo/constants
- expo/hooks
- expo/lib
- expo/providers

Usage:
  node expo/scripts/copy-sources-to-clipboard.js
*/

const fs = require('fs');
const path = require('path');
const { spawnSync } = require('child_process');

const expoRoot = path.resolve(__dirname, '..');
const repoRoot = path.resolve(expoRoot, '..');
// Expo app sources we want to include
const expoDirs = ['app', 'components', 'constants', 'hooks', 'lib', 'providers', 'types', 'scripts'];
// Rust bridge crate sources we want to include
const bridgeSrcDir = path.join(repoRoot, 'crates', 'codex-bridge', 'src');
const bridgeCargoToml = path.join(repoRoot, 'crates', 'codex-bridge', 'Cargo.toml');

function walkFiles(dir, out) {
  let entries;
  try {
    entries = fs.readdirSync(dir, { withFileTypes: true });
  } catch {
    return; // Skip missing directories
  }
  for (const entry of entries) {
    const full = path.join(dir, entry.name);
    if (entry.isDirectory()) walkFiles(full, out);
    else if (entry.isFile()) out.push(full);
  }
}

const files = [];
for (const r of expoDirs) {
  const dir = path.join(expoRoot, r);
  if (fs.existsSync(dir)) walkFiles(dir, files);
}
// Include Rust crate sources
if (fs.existsSync(bridgeSrcDir)) walkFiles(bridgeSrcDir, files);
if (fs.existsSync(bridgeCargoToml)) files.push(bridgeCargoToml);

files.sort((a, b) => a.localeCompare(b));

const chunks = [];
for (const file of files) {
  // Compute repo-relative path (root is the repository root)
  const repoRel = path.relative(repoRoot, file).replace(/\\/g, '/');
  let content;
  try {
    content = fs.readFileSync(file, 'utf8');
  } catch (e) {
    // Fallback for non-UTF8 files; include as base64 to avoid dropping content
    const buf = fs.readFileSync(file);
    content = `<<<BINARY ${buf.length} bytes (base64)>>>\n${buf.toString('base64')}`;
  }
  chunks.push(`===== FILE: ${repoRel} =====\n${content}`);
}

const output = chunks.join('\n\n');

function which(cmd) {
  const res = spawnSync(process.platform === 'win32' ? 'where' : 'which', [cmd], { stdio: 'pipe' });
  return res.status === 0;
}

function copyToClipboard(text) {
  // macOS
  if (process.platform === 'darwin' && which('pbcopy')) {
    const p = spawnSync('pbcopy', { input: text, encoding: 'utf8' });
    return p.status === 0;
  }
  // Windows
  if (process.platform === 'win32' && which('clip')) {
    const p = spawnSync('clip', { input: text, encoding: 'utf8', shell: true });
    return p.status === 0;
  }
  // Linux/WSL: try wl-copy, xclip, xsel
  if (which('wl-copy')) {
    const p = spawnSync('wl-copy', { input: text, encoding: 'utf8' });
    return p.status === 0;
  }
  if (which('xclip')) {
    const p = spawnSync('xclip', ['-selection', 'clipboard'], { input: text, encoding: 'utf8' });
    return p.status === 0;
  }
  if (which('xsel')) {
    const p = spawnSync('xsel', ['--clipboard', '--input'], { input: text, encoding: 'utf8' });
    return p.status === 0;
  }
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
