#!/usr/bin/env node

/**
 * Script to recursively copy the contents of all wallet app files to the clipboard
 * with proper formatting for documentation purposes.
 *
 * Usage: node copyAllToClipboard.js
 */

import fs from 'fs';
import path from 'path';
import { execSync } from 'child_process';
import { fileURLToPath } from 'url';

// Get the current directory
const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

// Function to get file extension
const getFileExtension = (filePath) => {
  return path.extname(filePath).slice(1);
};

// Function to determine the language for markdown code blocks
const getLanguage = (ext) => {
  const languageMap = {
    'ts': 'typescript',
    'tsx': 'typescript',
    'js': 'javascript',
    'jsx': 'javascript',
    'mjs': 'javascript',
    'json': 'json',
    'html': 'html',
    'css': 'css',
    'md': 'markdown',
    'yaml': 'yaml',
    'yml': 'yaml',
    'sql': 'sql'
  };
  return languageMap[ext] || '';
};

// Function to copy text to clipboard
const copyToClipboard = (text) => {
  // For macOS
  try {
    execSync('pbcopy', { input: text });
    console.log('Content copied to clipboard!');
  } catch (error) {
    console.error('Failed to copy to clipboard:', error);
  }
};

// Files to explicitly include from root of the wallet app
const rootFilesToInclude = [
  'README.md',
  'package.json',
  'components.json',
  'eslint.config.js',
  'index.html',
  'tsconfig.json',
  'tsconfig.app.json',
  'tsconfig.node.json',
  'vite.config.ts'
];

// Directories to recursively scan within the wallet app
const dirsToScan = [
  'src',
  'public',
  'docs'
];

// Directories to exclude
const dirsToExclude = [
  'node_modules',
  'public'
];

// Function to check if a path should be excluded
const shouldExclude = (filePath) => {
  return dirsToExclude.some(excludedDir =>
    filePath.includes(excludedDir) || filePath === excludedDir
  );
};

// Function to recursively collect all files in a directory
const collectFilesRecursively = (dir, fileList = []) => {
  // Skip if this directory should be excluded
  if (shouldExclude(dir)) {
    return fileList;
  }

  const files = fs.readdirSync(dir);

  files.forEach(file => {
    const filePath = path.join(dir, file);
    const relativePath = path.relative(process.cwd(), filePath);

    // Skip excluded directories, node_modules, .git directories and hidden files
    if (file.startsWith('.') || file === 'node_modules' || shouldExclude(relativePath)) {
      return;
    }

    const stats = fs.statSync(filePath);
    if (stats.isDirectory()) {
      collectFilesRecursively(filePath, fileList);
    } else {
      fileList.push(relativePath);
    }
  });

  return fileList;
};

// Main function to read files and format content
const main = () => {
  let allFiles = [];

  // Add root files
  rootFilesToInclude.forEach(file => {
    if (fs.existsSync(file)) {
      allFiles.push(file);
    } else {
      console.log(`⚠️ Root file not found: ${file}`);
    }
  });

  // Recursively collect files from specified directories
  dirsToScan.forEach(dir => {
    if (fs.existsSync(dir)) {
      collectFilesRecursively(dir, allFiles);
    } else {
      console.log(`⚠️ Directory not found: ${dir}`);
    }
  });

  // Sort files by path for better organization
  allFiles.sort();

  let clipboardContent = '';
  let processedCount = 0;

  for (const filePath of allFiles) {
    try {
      const content = fs.readFileSync(filePath, 'utf8');
      const ext = getFileExtension(filePath);
      const language = getLanguage(ext);

      clipboardContent += `File: ${filePath}\n\n`;
      clipboardContent += '```' + language + '\n';
      clipboardContent += content;
      clipboardContent += '\n```\n\n';

      processedCount++;
      console.log(`✅ Added ${filePath}`);
    } catch (error) {
      console.error(`❌ Error reading ${filePath}:`, error);
    }
  }

  // Copy the formatted content to clipboard
  copyToClipboard(clipboardContent);

  console.log('\nTotal files processed:', processedCount);
  console.log('Total character count:', clipboardContent.length);
};

// Run the main function
main();