#!/usr/bin/env node

/**
 * Fixes the double dist path issue created by Effect build-utils pack-v2
 * This script moves dist/dist/* to dist/* and removes unnecessary directories
 */

import { existsSync, rmSync, renameSync } from 'fs';
import { join } from 'path';

const distPath = join(process.cwd(), 'dist');
const distDistPath = join(distPath, 'dist');

// Check if dist/dist exists
if (existsSync(distDistPath)) {
  console.log('Fixing double dist path issue...');
  
  // Move contents of dist/dist to dist
  const subdirs = ['esm', 'cjs', 'dts'];
  
  for (const subdir of subdirs) {
    const sourcePath = join(distDistPath, subdir);
    const targetPath = join(distPath, subdir);
    
    if (existsSync(sourcePath)) {
      // Remove target if it exists
      if (existsSync(targetPath)) {
        rmSync(targetPath, { recursive: true, force: true });
      }
      
      // Move source to target
      renameSync(sourcePath, targetPath);
      console.log(`Moved dist/dist/${subdir} to dist/${subdir}`);
    }
  }
  
  // Remove the now-empty dist/dist directory
  rmSync(distDistPath, { recursive: true, force: true });
  console.log('Removed dist/dist directory');
  
  // Also remove dist/src if it exists (not needed for publishing)
  const distSrcPath = join(distPath, 'src');
  if (existsSync(distSrcPath)) {
    rmSync(distSrcPath, { recursive: true, force: true });
    console.log('Removed dist/src directory');
  }
  
  console.log('✅ Fixed dist structure');
} else {
  console.log('No double dist path found, skipping fix');
}