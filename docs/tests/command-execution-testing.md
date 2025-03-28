# Testing Command Execution Functionality

This guide explains how to test the command execution functionality in the OpenAgents applications, particularly focusing on ensuring it works in Electron but gracefully fails in web environments.

## Problem Context

We encountered an issue where browser environments were trying to import Node.js-specific modules like `child_process`, causing errors:

```
Uncaught Error: Module "child_process" has been externalized for browser compatibility. Cannot access "child_process.spawn" in client code.
```

This happens because:
1. The `commandExecutor.ts` file uses Node.js modules
2. These modules don't exist in browser environments
3. When bundled for the web, Vite externalized them but the code still tried to access them

## Solution Applied

We implemented a fix with proper environment detection:
1. Conditionally import Node.js modules only in Node.js environments
2. Check for environment type before attempting to use Node.js APIs
3. Provide graceful fallbacks for browser environments

## Manual Testing Instructions

### Option 1: Using the SimpleCommandTest Component

We've created a standalone test component that directly tests the command execution functionality:

1. **First, integrate the component into your app:**

   Copy the file `/docs/examples/SimpleCommandTest.tsx` to your application and import it:

   ```tsx
   import { SimpleCommandTest } from './path/to/SimpleCommandTest';
   
   // Then use it in your component:
   return <SimpleCommandTest />;
   ```

2. **Test in Electron:**
   - The component should show "Running in: Node.js"
   - "Child Process available: Yes" should be displayed
   - Commands like `echo "Hello World"` should work
   - Results should display in the output area

3. **Test in Web Browser:**
   - The component should show "Running in: Browser"
   - "Child Process available: No" should be displayed
   - Attempting to run commands should show an error message stating "Command execution is only available in the Electron app"
   - The app should not crash or show errors in the console about externalized modules

### Option 2: Programmatic Testing

You can also test the functionality programmatically:

```typescript
import { safeExecuteCommand } from '@openagents/core/src/utils/commandExecutor';

// Test function
async function testCommandExecution() {
  console.log('Testing command execution...');
  
  try {
    const result = await safeExecuteCommand('echo "Hello World"');
    console.log('Command result:', result);
    
    if ('error' in result) {
      console.log('Command execution failed with error:', result.error);
    } else {
      console.log('Command executed successfully!');
      console.log('Stdout:', result.stdout);
      console.log('Stderr:', result.stderr);
      console.log('Exit code:', result.exitCode);
    }
  } catch (error) {
    console.error('Exception during command execution:', error);
  }
}

// Call the test function
testCommandExecution();
```

## Expected Results

### In Electron:
- Commands should execute successfully
- Results should include stdout, stderr, and exit code
- No errors should appear in the console

### In Web Browser:
- Commands should fail gracefully with a clear error message
- No errors about "externalized modules" should appear in the console
- The application should continue to function normally

## Troubleshooting

If you encounter issues:

1. **Browser still showing module errors:**
   - Check that the `commandExecutor.ts` file is being properly transpiled
   - Verify Vite/Webpack configuration for proper handling of Node.js modules
   - Try adding the module to external dependencies in your bundler config

2. **Commands not working in Electron:**
   - Check if your Electron configuration blocks child_process
   - Verify that bash/shell is available in the environment
   - Check if the command is being blocked by the dangerous command filter

3. **Environment detection not working:**
   - Log values from environment checks to see what's being detected
   - Check for polyfills that might be interfering with environment detection
   - Try different methods of detecting the environment

## Security Note

Remember that command execution is powerful and potentially dangerous. The implementation includes safety measures:
- Dangerous command patterns are blocked
- Commands timeout after 30 seconds
- Proper error handling prevents hung processes

For production use, consider additional safety measures like:
- Command whitelisting
- User confirmation for certain commands
- Restricted execution environment