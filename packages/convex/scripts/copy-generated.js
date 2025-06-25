#!/usr/bin/env node

import fs from 'fs';
import path from 'path';
import { fileURLToPath } from 'url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const rootDir = path.resolve(__dirname, '..');

// Since client.js is at dist/[type]/client.js, and it imports ../convex/_generated/api.js
// We need to put the files at dist/convex/_generated/
const targetDir = path.join(rootDir, 'dist', 'convex', '_generated');

// Create directory if it doesn't exist
fs.mkdirSync(targetDir, { recursive: true });

// Copy generated files
const sourceDir = path.join(rootDir, 'convex', '_generated');
const files = fs.readdirSync(sourceDir);

for (const file of files) {
  const sourcePath = path.join(sourceDir, file);
  const targetPath = path.join(targetDir, file);
  
  fs.copyFileSync(sourcePath, targetPath);
  console.log(`Copied ${file} to dist/convex/_generated`);
}

console.log('✅ Generated files copied to dist/convex/_generated');