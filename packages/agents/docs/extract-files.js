#!/usr/bin/env node

import fs from 'fs';
import path from 'path';
import { execSync } from 'child_process';
import { fileURLToPath } from 'url';

// Get the directory name correctly in ESM
const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

// Read the README file
const readmePath = path.join(__dirname, 'SOLVER-README.md');
const readmeContent = fs.readFileSync(readmePath, 'utf8');

// Extract file paths using regex
// Look for paths that:
// 1. Start with / (absolute paths)
// 2. Have patterns like file/path.ext
// 3. Are surrounded by backticks
function extractFilePaths(content) {
  const patterns = [
    /`([/\w.-]+\.\w+)`/g,                   // Paths in inline code blocks
    /```typescript\n([\s\S]*?)```/g,        // TypeScript code blocks
    /```json\n([\s\S]*?)```/g,              // JSON code blocks
    /`([/\w.-]+)`(?=\s*-)/g,                // Paths followed by description
    /\*\*`([/\w.-]+\.\w+)`\*\*/g,           // Bold code paths
    /\*\*([/\w.-]+\.\w+)\*\*/g,             // Bold paths
    /\(([/\w.-]+\.\w+)\)/g                  // Paths in parentheses
  ];

  // Additional specific paths mentioned in this repo
  const specificPaths = [
    '/packages/agents/src/common/open-agent.ts',
    '/packages/agents/src/agents/solver/index.ts',
    '/packages/agents/src/agents/solver/types.ts',
    '/packages/agents/src/agents/solver/tools.ts',
    '/packages/agents/src/agents/solver/prompts.ts',
    '/packages/agents/src/common/tools/index.ts',
    '/packages/agents/src/common/types.ts',
    '/packages/agents/package.json',
    '/packages/agents/src/common/providers/index.ts',
    '/packages/agents/src/common/providers/models.ts',
    '/apps/website/app/routes/issues/$id.tsx',
    '/apps/website/app/components/agent/solver-connector.tsx',
    '/apps/website/app/components/agent/solver-controls.tsx'
  ];

  const foundPaths = new Set();
  
  // Extract paths using regex patterns
  for (const pattern of patterns) {
    let match;
    while ((match = pattern.exec(content)) !== null) {
      const path = match[1];
      // Only include if it looks like a file path
      if (path && path.includes('.') && path.includes('/')) {
        foundPaths.add(path);
      }
    }
  }
  
  // Add specific paths
  for (const path of specificPaths) {
    foundPaths.add(path);
  }
  
  // Clean up paths (remove leading slash if present)
  return Array.from(foundPaths).map(p => p.startsWith('/') ? p.substring(1) : p);
}

// Get file paths
const filePaths = extractFilePaths(readmeContent);
console.log(`Found ${filePaths.length} file paths in the README`);

// Create the output string
let output = '';

// Function to get file content or placeholder
function getFileContent(filePath) {
  const fullPath = path.join(process.cwd(), '..', '..', filePath);
  try {
    if (fs.existsSync(fullPath)) {
      return fs.readFileSync(fullPath, 'utf8');
    } else {
      // Look for the file in the codebase
      const basePath = path.join(process.cwd(), '..', '..');
      // Get the relative path (strip leading slash if present)
      const relPath = filePath.startsWith('/') ? filePath.substring(1) : filePath;
      // Use find to locate the file
      try {
        const foundPath = execSync(`find ${basePath} -path "*${relPath}" 2>/dev/null | head -1`, { encoding: 'utf8' }).trim();
        if (foundPath) {
          return fs.readFileSync(foundPath, 'utf8');
        }
      } catch (e) {
        // Ignore find errors
      }
      return `// File does not exist yet: ${filePath}`;
    }
  } catch (error) {
    return `// Error reading file: ${error.message}`;
  }
}

// Add each file path and content to the output
filePaths.forEach(filePath => {
  const content = getFileContent(filePath);
  output += `\n\nFile: ${filePath}\n\`\`\`\n${content}\n\`\`\`\n`;
});

// Copy to clipboard
try {
  // For macOS
  execSync('pbcopy', { input: output });
  console.log('Successfully copied file content to clipboard!');
} catch (error) {
  try {
    // For Windows
    execSync('clip', { input: output });
    console.log('Successfully copied file content to clipboard!');
  } catch (winError) {
    try {
      // For Linux
      execSync('xclip -selection clipboard', { input: output });
      console.log('Successfully copied file content to clipboard!');
    } catch (linuxError) {
      console.error('Failed to copy to clipboard. Try using one of these commands:');
      console.error('- macOS: pbcopy');
      console.error('- Windows: clip');
      console.error('- Linux: xclip -selection clipboard');
      
      // Save to a file instead
      const outputPath = path.join(__dirname, 'extracted-files.txt');
      fs.writeFileSync(outputPath, output);
      console.log(`Output saved to: ${outputPath}`);
    }
  }
}