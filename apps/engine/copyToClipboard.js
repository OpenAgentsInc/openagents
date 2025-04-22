#!/usr/bin/env node

/**
 * Script to copy the contents of specified files to the clipboard
 * with proper formatting for documentation purposes.
 *
 * Usage: node copyToClipboard.js
 */

import fs from 'fs';
import path from 'path';
import { execSync } from 'child_process';
import { fileURLToPath } from 'url';

// Get the current directory
const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

// Array of files to copy, in order
const filesToCopy = [
  // Documentation files
  'docs/20250422-1300-agent-state-implementation-plan.md',
  'docs/20250422-1256-nextsteps.md',
  'docs/20250422-1255-specsofar.md',
  'docs/20250422-1205-movein.md',
  'docs/agent-state.md',
  'docs/20250422-1000-status.md',
  'docs/20250422-920-setup.md',

  // Source files
  'src/Server.ts',
  'src/Program.ts',
  'src/index.ts',

  // GitHub related source files
  'src/github/GitHubTools.ts',
  'src/github/GitHub.ts',
  'src/github/GitHubTypes.ts',

  // Test files
  'test/Dummy.test.ts',
  'test/github/GitHub.test.ts',
  'test/github/GitHubTools.test.ts'
];

// Function to get file extension
const getFileExtension = (filePath) => {
  return path.extname(filePath).slice(1);
};

// Function to determine the language for markdown code blocks
const getLanguage = (ext) => {
  const languageMap = {
    'ts': 'typescript',
    'js': 'javascript',
    'json': 'json',
    'html': 'html',
    'css': 'css',
    'md': 'markdown'
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

// Main function to read files and format content
const main = () => {
  let clipboardContent = '';

  for (const filePath of filesToCopy) {
    const absolutePath = path.resolve(process.cwd(), filePath);

    try {
      if (fs.existsSync(absolutePath)) {
        const content = fs.readFileSync(absolutePath, 'utf8');
        const ext = getFileExtension(filePath);
        const language = getLanguage(ext);

        clipboardContent += `File: ${filePath}\n\n`;
        clipboardContent += '```' + language + '\n';
        clipboardContent += content;
        clipboardContent += '\n```\n\n';

        console.log(`✅ Added ${filePath}`);
      } else {
        console.log(`⚠️ File not found: ${filePath}`);
      }
    } catch (error) {
      console.error(`❌ Error reading ${filePath}:`, error);
    }
  }

  // Copy the formatted content to clipboard
  copyToClipboard(clipboardContent);

  console.log('\nTotal files processed:', filesToCopy.length);
  console.log('Total character count:', clipboardContent.length);
};

// Run the main function
main();
